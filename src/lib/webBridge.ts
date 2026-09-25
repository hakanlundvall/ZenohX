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

/**
 * Web access bridge.
 *
 * When ZenohX runs with `--web`, the UI is served to regular browsers. There is
 * no Tauri webview there, so this module installs a `window.__TAURI_INTERNALS__`
 * that carries `invoke` calls and backend events over a WebSocket to the
 * ZenohX process (see src-tauri/src/web). Everything built on @tauri-apps/api
 * keeps working unchanged.
 */

type Callback = (payload: unknown) => void;

interface PendingRequest {
  resolve: (value: unknown) => void;
  reject: (reason: unknown) => void;
}

type ServerMessage =
  | { type: 'response'; id: number; ok: boolean; result?: unknown; error?: string }
  | { type: 'event'; event: string; payload: unknown };

const TOKEN_STORAGE_KEY = 'zenohx-web-token';
const RECONNECT_DELAY_MS = 2000;
/** WebSocket close code the server uses for a missing or wrong token. */
const CLOSE_UNAUTHORIZED = 4401;

declare global {
  interface Window {
    /** Set by the ZenohX web server in the index.html it serves. */
    __ZENOHX_WEB__?: boolean;
  }
}

/** True when the page was served by the ZenohX web server (or points at one via `?ws=`). */
export function isWebBridgeMode(): boolean {
  if (typeof window === 'undefined') return false;
  if ('__TAURI_INTERNALS__' in window) return false;
  return Boolean(window.__ZENOHX_WEB__) || new URLSearchParams(window.location.search).has('ws');
}

/** Reads the access token from the URL (then removes it from the address bar) or storage. */
export function takeAccessToken(): string | null {
  const url = new URL(window.location.href);
  const fromUrl = url.searchParams.get('token');
  if (fromUrl) {
    try {
      localStorage.setItem(TOKEN_STORAGE_KEY, fromUrl);
    } catch {
      // Storage unavailable; the token still works for this page load
    }
    url.searchParams.delete('token');
    window.history.replaceState(window.history.state, '', url.toString());
    return fromUrl;
  }
  try {
    return localStorage.getItem(TOKEN_STORAGE_KEY);
  } catch {
    return null;
  }
}

/** WebSocket endpoint: `?ws=host:port` when given (dev server), else the page's own host. */
export function bridgeUrl(location: Location, token: string | null): string {
  const params = new URLSearchParams(location.search);
  const host = params.get('ws') || location.host;
  const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
  return `${scheme}://${host}/ws${token ? `?token=${encodeURIComponent(token)}` : ''}`;
}

export class WebBridge {
  private socket: WebSocket | null = null;
  private nextRequestId = 1;
  private nextCallbackId = 1;
  private nextEventId = 1;
  private readonly pending = new Map<number, PendingRequest>();
  private readonly callbacks = new Map<number, Callback>();
  /** event name -> (eventId -> callbackId) */
  private readonly listeners = new Map<string, Map<number, number>>();
  private readonly queue: string[] = [];
  private closedForGood = false;

  constructor(
    private readonly url: string,
    private readonly onStatus: (status: 'connected' | 'disconnected' | 'unauthorized') => void = () => {}
  ) {}

  /** Opens the connection; resolves once connected (or rejects if the token is refused). */
  connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      let opened = false;
      const socket = new WebSocket(this.url);
      this.socket = socket;

      socket.onopen = () => {
        opened = true;
        this.onStatus('connected');
        for (const msg of this.queue.splice(0)) socket.send(msg);
        resolve();
      };
      socket.onmessage = (ev) => this.handleMessage(String(ev.data));
      socket.onclose = (ev) => {
        this.socket = null;
        this.failPending('Connection to ZenohX lost');
        if (ev.code === CLOSE_UNAUTHORIZED) {
          this.closedForGood = true;
          this.onStatus('unauthorized');
          if (!opened) reject(new Error('Access token rejected'));
          return;
        }
        this.onStatus('disconnected');
        if (!opened) reject(new Error('Could not connect to ZenohX'));
        if (!this.closedForGood) {
          setTimeout(() => this.connect().catch(() => {}), RECONNECT_DELAY_MS);
        }
      };
    });
  }

  close(): void {
    this.closedForGood = true;
    this.socket?.close();
  }

  /** Implementation of `window.__TAURI_INTERNALS__`. */
  internals() {
    return {
      invoke: (cmd: string, args: Record<string, unknown> = {}) => this.invoke(cmd, args),
      transformCallback: (callback?: Callback, once = false) => {
        const id = this.nextCallbackId++;
        this.callbacks.set(id, (payload) => {
          if (once) this.callbacks.delete(id);
          callback?.(payload);
        });
        return id;
      },
      unregisterCallback: (id: number) => this.callbacks.delete(id),
      convertFileSrc: (filePath: string) => filePath,
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { windowLabel: 'main', label: 'main' },
      },
    };
  }

  /** Implementation of `window.__TAURI_EVENT_PLUGIN_INTERNALS__`. */
  eventPluginInternals() {
    return {
      unregisterListener: (event: string, eventId: number) => {
        this.listeners.get(event)?.delete(eventId);
      },
    };
  }

  invoke(cmd: string, args: Record<string, unknown>): Promise<unknown> {
    const local = this.handleLocally(cmd, args);
    if (local !== undefined) return local;

    const id = this.nextRequestId++;
    const message = JSON.stringify({ id, cmd, args });
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      if (this.socket?.readyState === WebSocket.OPEN) {
        this.socket.send(message);
      } else {
        this.queue.push(message);
      }
    });
  }

  /** Commands answered in the browser; `undefined` means "send to the backend". */
  private handleLocally(cmd: string, args: Record<string, unknown>): Promise<unknown> | undefined {
    switch (cmd) {
      case 'plugin:event|listen': {
        const event = String(args.event);
        const eventId = this.nextEventId++;
        if (!this.listeners.has(event)) this.listeners.set(event, new Map());
        this.listeners.get(event)!.set(eventId, Number(args.handler));
        return Promise.resolve(eventId);
      }
      case 'plugin:event|unlisten':
        this.listeners.get(String(args.event))?.delete(Number(args.eventId));
        return Promise.resolve();
      case 'plugin:event|emit':
      case 'plugin:event|emit_to':
        this.dispatchEvent(String(args.event), args.payload);
        return Promise.resolve();
      case 'plugin:shell|open':
        window.open(String(args.path), '_blank', 'noopener');
        return Promise.resolve();
      case 'plugin:updater|check':
        // The browser is not an installation; updates belong to the desktop app.
        return Promise.resolve(null);
      case 'plugin:process|restart':
        window.location.reload();
        return Promise.resolve();
      case 'plugin:webview|create_webview_window':
        // Rejecting makes openProfileInNewWindow fall back to a new browser tab.
        return Promise.reject(new Error('New windows open as browser tabs in web mode'));
      default:
        return undefined;
    }
  }

  private handleMessage(raw: string): void {
    let msg: ServerMessage;
    try {
      msg = JSON.parse(raw);
    } catch {
      return;
    }
    if (msg.type === 'response') {
      const pending = this.pending.get(msg.id);
      if (!pending) return;
      this.pending.delete(msg.id);
      if (msg.ok) pending.resolve(msg.result);
      else pending.reject(msg.error ?? 'Unknown error');
    } else if (msg.type === 'event') {
      this.dispatchEvent(msg.event, msg.payload);
    }
  }

  private dispatchEvent(event: string, payload: unknown): void {
    const listeners = this.listeners.get(event);
    if (!listeners) return;
    for (const [eventId, callbackId] of [...listeners]) {
      this.callbacks.get(callbackId)?.({ event, id: eventId, payload });
    }
  }

  private failPending(reason: string): void {
    for (const { reject } of this.pending.values()) reject(reason);
    this.pending.clear();
  }
}

function showBanner(text: string | null): void {
  const id = 'zenohx-web-banner';
  let banner = document.getElementById(id);
  if (!text) {
    banner?.remove();
    return;
  }
  if (!banner) {
    banner = document.createElement('div');
    banner.id = id;
    banner.style.cssText =
      'position:fixed;top:0;left:0;right:0;z-index:99999;padding:6px 12px;font:12px system-ui,sans-serif;' +
      'text-align:center;background:#b91c1c;color:#fff';
    document.body.appendChild(banner);
  }
  banner.textContent = text;
}

/**
 * Installs the bridge when the page is served by the ZenohX web server.
 * Must run before any @tauri-apps/api call.
 */
export async function installWebBridgeIfNeeded(): Promise<void> {
  if (!isWebBridgeMode()) {
    if (typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window)) {
      // Plain browser on the dev server without a backend link.
      showBanner(
        'Not connected to ZenohX. Start it with --web (or ZENOHX_WEB=1) and open the address it prints.'
      );
    }
    return;
  }

  const token = takeAccessToken();
  const bridge = new WebBridge(bridgeUrl(window.location, token), (status) => {
    if (status === 'connected') showBanner(null);
    else if (status === 'disconnected') showBanner('Connection to ZenohX lost, reconnecting…');
    else showBanner('Access denied: open the web address (including ?token=…) printed by ZenohX.');
  });
  const w = window as unknown as Record<string, unknown>;
  w.__TAURI_INTERNALS__ = bridge.internals();
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = bridge.eventPluginInternals();

  try {
    await bridge.connect();
  } catch (err) {
    console.error('[web] ', err);
  }
}
