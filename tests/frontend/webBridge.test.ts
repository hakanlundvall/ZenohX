// Copyright 2026 ZenohX Contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { test, describe, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

/** Minimal in-memory WebSocket standing in for the ZenohX server. */
class FakeSocket {
  static OPEN = 1;
  static instances: FakeSocket[] = [];
  readyState = 0;
  sent: Array<{ id: number; cmd: string; args: Record<string, unknown> }> = [];
  onopen: (() => void) | null = null;
  onmessage: ((ev: { data: string }) => void) | null = null;
  onclose: ((ev: { code: number }) => void) | null = null;

  constructor(public url: string) {
    FakeSocket.instances.push(this);
  }
  send(data: string) {
    this.sent.push(JSON.parse(data));
  }
  close() {}
  open() {
    this.readyState = FakeSocket.OPEN;
    this.onopen?.();
  }
  serverSends(msg: unknown) {
    this.onmessage?.({ data: JSON.stringify(msg) });
  }
  serverCloses(code: number) {
    this.readyState = 3;
    this.onclose?.({ code });
  }
}

// @ts-expect-error test global
globalThis.window = globalThis;
// @ts-expect-error test global
globalThis.WebSocket = FakeSocket;

import { WebBridge, bridgeUrl } from '../../src/lib/webBridge';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

function install(bridge: WebBridge) {
  const w = globalThis as unknown as Record<string, unknown>;
  w.__TAURI_INTERNALS__ = bridge.internals();
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = bridge.eventPluginInternals();
}

async function connected(url = 'ws://host/ws?token=t') {
  const bridge = new WebBridge(url);
  install(bridge);
  const ready = bridge.connect();
  const socket = FakeSocket.instances.at(-1)!;
  socket.open();
  await ready;
  return { bridge, socket };
}

describe('web bridge', () => {
  beforeEach(() => {
    FakeSocket.instances = [];
  });

  test('bridgeUrl uses the page host, or ?ws= for the dev server', () => {
    const loc = (href: string) => new URL(href) as unknown as Location;
    assert.equal(bridgeUrl(loc('http://127.0.0.1:7880/'), 'abc'), 'ws://127.0.0.1:7880/ws?token=abc');
    assert.equal(bridgeUrl(loc('https://example.org/'), null), 'wss://example.org/ws');
    assert.equal(
      bridgeUrl(loc('http://localhost:1420/?ws=127.0.0.1:7880'), 'a b'),
      'ws://127.0.0.1:7880/ws?token=a%20b'
    );
  });

  test('invoke round-trips through the socket, including errors', async () => {
    const { socket } = await connected();

    const ok = invoke('get_all_sessions', { x: 1 });
    const failing = invoke('connect_session', { config: {} });
    assert.deepEqual(
      socket.sent.map((m) => [m.cmd, m.args]),
      [
        ['get_all_sessions', { x: 1 }],
        ['connect_session', { config: {} }],
      ]
    );

    socket.serverSends({ type: 'response', id: socket.sent[1].id, ok: false, error: 'boom' });
    socket.serverSends({ type: 'response', id: socket.sent[0].id, ok: true, result: [{ id: 's1' }] });
    assert.deepEqual(await ok, [{ id: 's1' }]);
    await assert.rejects(failing, (e) => e === 'boom');
  });

  test('requests made before the socket opens are queued', async () => {
    const bridge = new WebBridge('ws://host/ws');
    install(bridge);
    const ready = bridge.connect();
    const socket = FakeSocket.instances.at(-1)!;
    const pending = invoke('load_profiles');
    assert.equal(socket.sent.length, 0);
    socket.open();
    await ready;
    assert.equal(socket.sent[0].cmd, 'load_profiles');
    socket.serverSends({ type: 'response', id: socket.sent[0].id, ok: true, result: [] });
    assert.deepEqual(await pending, []);
  });

  test('backend events reach listen() handlers until unlistened', async () => {
    const { socket } = await connected();
    const received: unknown[] = [];
    const unlisten = await listen('zenohx://samples-batched', (e) => received.push(e.payload));
    const other: unknown[] = [];
    await listen('zenohx://query', (e) => other.push(e.payload));

    socket.serverSends({ type: 'event', event: 'zenohx://samples-batched', payload: [{ key_expr: 'a' }] });
    assert.deepEqual(received, [[{ key_expr: 'a' }]]);
    assert.deepEqual(other, []);

    await unlisten();
    socket.serverSends({ type: 'event', event: 'zenohx://samples-batched', payload: [] });
    assert.equal(received.length, 1);
    // listen/unlisten are handled in the browser, never sent to the server
    assert.equal(socket.sent.length, 0);
  });

  test('desktop-only plugin calls are answered locally', async () => {
    const { socket } = await connected();
    assert.equal(await invoke('plugin:updater|check', {}), null);
    await assert.rejects(invoke('plugin:webview|create_webview_window', { options: {} }));
    assert.equal(socket.sent.length, 0);
    // app version comes from the backend
    void invoke('plugin:app|version');
    assert.equal(socket.sent[0].cmd, 'plugin:app|version');
  });

  test('a rejected token stops reconnecting and fails pending calls', async () => {
    const statuses: string[] = [];
    const bridge = new WebBridge('ws://host/ws?token=bad', (s) => statuses.push(s));
    install(bridge);
    const ready = bridge.connect();
    const socket = FakeSocket.instances.at(-1)!;
    const pending = invoke('load_profiles');
    socket.serverCloses(4401);
    await assert.rejects(ready, /token/);
    await assert.rejects(pending);
    assert.deepEqual(statuses, ['unauthorized']);
  });

  test('a lost connection fails in-flight calls and reconnects', async () => {
    const statuses: string[] = [];
    const bridge = new WebBridge('ws://host/ws', (s) => statuses.push(s));
    install(bridge);
    const ready = bridge.connect();
    FakeSocket.instances.at(-1)!.open();
    await ready;
    const pending = invoke('load_profiles');
    FakeSocket.instances.at(-1)!.serverCloses(1006);
    await assert.rejects(pending, /lost/);
    assert.deepEqual(statuses, ['connected', 'disconnected']);
    await new Promise((r) => setTimeout(r, 2100));
    assert.equal(FakeSocket.instances.length, 2);
    bridge.close();
  });
});
