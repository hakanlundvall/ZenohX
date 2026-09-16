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

import { test, describe, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import React from 'react';

// Tauri Event Mock Setup
interface RegisteredListener {
  id: number;
  handler: (event: { payload: unknown; event: string }) => void;
}

const listeners = new Map<string, RegisteredListener[]>();
let nextListenerId = 1;
let delayPromiseResolve = false;
let pendingResolvers: Array<() => void> = [];

// @ts-expect-error Mocking global window
globalThis.window = globalThis;

// @ts-expect-error Mocking tauri internals
globalThis.window.__TAURI_INTERNALS__ = {
  invoke: async (cmd: string, args?: Record<string, unknown>) => {
    if (cmd === 'plugin:event|listen') {
      const event = args?.event as string;
      const handler = args?.handler as (event: { payload: unknown; event: string }) => void;
      const id = nextListenerId++;

      if (delayPromiseResolve) {
        await new Promise<void>((resolve) => {
          pendingResolvers.push(resolve);
        });
      }

      if (!listeners.has(event)) {
        listeners.set(event, []);
      }
      listeners.get(event)!.push({ id, handler });
      return id;
    }
    if (cmd === 'plugin:event|unlisten') {
      const event = args?.event as string;
      const eventId = args?.eventId as number;
      if (listeners.has(event)) {
        const arr = listeners.get(event)!.filter((l) => l.id !== eventId);
        listeners.set(event, arr);
      }
      return undefined;
    }
    return undefined;
  },
  transformCallback: (cb: unknown) => cb,
};

// @ts-expect-error Mocking tauri event plugin internals
globalThis.window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
  unregisterListener: (event: string, eventId: number) => {
    if (listeners.has(event)) {
      const arr = listeners.get(event)!.filter((l) => l.id !== eventId);
      listeners.set(event, arr);
    }
  },
};

function emitTauriEvent(event: string, payload: unknown) {
  const handlers = listeners.get(event) || [];
  for (const item of handlers) {
    item.handler({ event, payload });
  }
}

// Minimalist React hook test harness for Node.js
interface HookHarness<T, P> {
  result: { current: T };
  rerender: (newProps?: P) => void;
  unmount: () => void;
}

function renderHook<T, P>(hook: (props: P) => T, initialProps: P): HookHarness<T, P> {
  let currentProps = initialProps;
  const stateMap = new Map<number, unknown>();
  let stateIndex = 0;
  let cleanups: (() => void)[] = [];
  let isMounted = true;
  const result = { current: undefined as unknown as T };

  function run() {
    if (!isMounted) return;
    stateIndex = 0;

    // @ts-expect-error React internals access for unit testing
    const oldDispatcher = React.__SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED?.ReactCurrentDispatcher?.current;

    // @ts-expect-error React internals access for unit testing
    React.__SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED.ReactCurrentDispatcher.current = {
      useState: (initial: unknown) => {
        const id = stateIndex++;
        if (!stateMap.has(id)) {
          stateMap.set(id, typeof initial === 'function' ? (initial as () => unknown)() : initial);
        }
        const setState = (action: unknown) => {
          const oldVal = stateMap.get(id);
          const newVal = typeof action === 'function' ? (action as (prev: unknown) => unknown)(oldVal) : action;
          if (newVal !== oldVal) {
            stateMap.set(id, newVal);
            run();
          }
        };
        return [stateMap.get(id), setState];
      },
      useEffect: (effect: () => void | (() => void), deps?: unknown[]) => {
        const id = stateIndex++;
        const prevDeps = stateMap.get(id) as unknown[] | undefined;
        const depsChanged = !prevDeps || !deps || deps.some((d, i) => d !== prevDeps[i]);
        if (depsChanged) {
          stateMap.set(id, deps);
          const cleanup = effect();
          if (typeof cleanup === 'function') {
            cleanups.push(cleanup);
          }
        }
      },
    };

    try {
      result.current = hook(currentProps);
    } finally {
      // @ts-expect-error React internals access for unit testing
      React.__SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED.ReactCurrentDispatcher.current = oldDispatcher;
    }
  }

  run();

  return {
    result,
    rerender: (newProps?: P) => {
      if (newProps !== undefined) currentProps = newProps;
      run();
    },
    unmount: () => {
      isMounted = false;
      for (const cleanup of cleanups) {
        cleanup();
      }
      cleanups = [];
    },
  };
}

// Import useMcpListener
import { useMcpListener, WorkspaceTab, McpActionPayload } from '../src/hooks/useMcpListener';
import { useMessageStore } from '../src/stores/messageStore';
import { useQueryStore } from '../src/stores/queryStore';
import { useConnectionStore } from '../src/stores/connectionStore';

describe('useMcpListener Hook', () => {
  beforeEach(() => {
    listeners.clear();
    nextListenerId = 1;
    delayPromiseResolve = false;
    pendingResolvers = [];
  });

  afterEach(() => {
    listeners.clear();
  });

  test('initializes with null lastAction and registers listeners', async () => {
    let switchedTab: WorkspaceTab | null = null;
    const harness = renderHook((cb) => useMcpListener(cb), (tab: WorkspaceTab) => {
      switchedTab = tab;
    });

    assert.equal(harness.result.current.lastAction, null);

    // Yield microtasks to allow listen promises to resolve
    await new Promise((resolve) => setTimeout(resolve, 10));

    assert.equal(listeners.get('zenohx://gui-switch-tab')?.length, 1);
    assert.equal(listeners.get('zenohx://mcp-action')?.length, 1);

    harness.unmount();
  });

  test('triggers tab switch callback when zenohx://gui-switch-tab event fires', async () => {
    let switchedTab: WorkspaceTab | null = null;
    const harness = renderHook((cb) => useMcpListener(cb), (tab: WorkspaceTab) => {
      switchedTab = tab;
    });

    await new Promise((resolve) => setTimeout(resolve, 10));

    emitTauriEvent('zenohx://gui-switch-tab', { workspace: 'query' });
    assert.equal(switchedTab, 'query');

    emitTauriEvent('zenohx://gui-switch-tab', { workspace: 'traffic' });
    assert.equal(switchedTab, 'traffic');

    emitTauriEvent('zenohx://gui-switch-tab', { workspace: 'topology' });
    assert.equal(switchedTab, 'topology');

    harness.unmount();
  });

  test('supports options object parameter { setActiveTab }', async () => {
    let switchedTab: WorkspaceTab | null = null;
    const harness = renderHook(
      (opts) => useMcpListener(opts),
      { setActiveTab: (tab: WorkspaceTab) => { switchedTab = tab; } }
    );

    await new Promise((resolve) => setTimeout(resolve, 10));

    emitTauriEvent('zenohx://gui-switch-tab', { workspace: 'settings' });
    assert.equal(switchedTab, 'settings');

    harness.unmount();
  });

  test('ignores invalid or empty workspace tab payloads', async () => {
    let callCount = 0;
    const harness = renderHook((cb) => useMcpListener(cb), () => {
      callCount++;
    });

    await new Promise((resolve) => setTimeout(resolve, 10));

    emitTauriEvent('zenohx://gui-switch-tab', {});
    emitTauriEvent('zenohx://gui-switch-tab', null);
    emitTauriEvent('zenohx://gui-switch-tab', { workspace: null });

    assert.equal(callCount, 0);

    harness.unmount();
  });

  test('updates lastAction on zenohx://mcp-action', async () => {
    const harness = renderHook((cb) => useMcpListener(cb), () => {});

    await new Promise((resolve) => setTimeout(resolve, 10));

    const payload: McpActionPayload = {
      action: 'Publish',
      details: "Published to 'demo/test'",
    };

    emitTauriEvent('zenohx://mcp-action', payload);
    assert.deepEqual(harness.result.current.lastAction, payload);

    harness.unmount();
  });

  test('handles consecutive actions and immediately updates lastAction', async () => {
    const harness = renderHook((cb) => useMcpListener(cb), () => {});

    await new Promise((resolve) => setTimeout(resolve, 10));

    const action1: McpActionPayload = {
      action: 'Publish',
      details: "Published to 'demo/test'",
    };
    emitTauriEvent('zenohx://mcp-action', action1);
    assert.deepEqual(harness.result.current.lastAction, action1);

    const action2: McpActionPayload = {
      action: 'Switch Workspace',
      details: "Switched to tab 'topology'",
    };
    emitTauriEvent('zenohx://mcp-action', action2);
    assert.deepEqual(harness.result.current.lastAction, action2);

    harness.unmount();
  });

  test('unregisters listeners on unmount', async () => {
    const harness = renderHook((cb) => useMcpListener(cb), () => {});

    await new Promise((resolve) => setTimeout(resolve, 10));

    assert.equal(listeners.get('zenohx://gui-switch-tab')?.length, 1);
    assert.equal(listeners.get('zenohx://mcp-action')?.length, 1);

    harness.unmount();

    // Yield microtasks for unlisten async invocations
    await new Promise((resolve) => setTimeout(resolve, 10));

    assert.equal(listeners.get('zenohx://gui-switch-tab')?.length, 0);
    assert.equal(listeners.get('zenohx://mcp-action')?.length, 0);
  });

  test('cleans up pending listeners if unmounted before listen promises resolve', async () => {
    delayPromiseResolve = true;
    const harness = renderHook((cb) => useMcpListener(cb), () => {});

    // Unmount before promises resolve
    harness.unmount();

    // Resolve the delayed promises
    for (const resolve of pendingResolvers) {
      resolve();
    }
    await new Promise((resolve) => setTimeout(resolve, 20));

    // Listeners should be unlistened immediately upon promise completion
    assert.equal(listeners.get('zenohx://gui-switch-tab')?.length || 0, 0);
    assert.equal(listeners.get('zenohx://mcp-action')?.length || 0, 0);
  });

  test('registers and unregisters zenohx://subscription-added and removed listeners', async () => {
    const harness = renderHook((cb) => useMcpListener(cb), () => {});

    await new Promise((resolve) => setTimeout(resolve, 10));

    assert.equal(listeners.get('zenohx://subscription-added')?.length, 1);
    assert.equal(listeners.get('zenohx://subscription-removed')?.length, 1);

    harness.unmount();
    await new Promise((resolve) => setTimeout(resolve, 10));

    assert.equal(listeners.get('zenohx://subscription-added')?.length, 0);
    assert.equal(listeners.get('zenohx://subscription-removed')?.length, 0);
  });

  test('adds subscription to useMessageStore on zenohx://subscription-added and removes on zenohx://subscription-removed', async () => {
    const harness = renderHook((cb) => useMcpListener(cb), () => {});

    await new Promise((resolve) => setTimeout(resolve, 10));

    emitTauriEvent('zenohx://subscription-added', {
      id: 'sub-test-123',
      session_id: 'session-456',
      profile_id: 'profile-789',
      key_expr: 'demo/mcp/added',
      encoding: 'json',
      active: true,
    });

    const currentSubs = useMessageStore.getState().subscriptions;
    const addedSub = currentSubs.find((s) => s.id === 'sub-test-123');
    assert.ok(addedSub, 'Subscription should be added to messageStore');
    assert.equal(addedSub.keyExpr, 'demo/mcp/added');
    assert.equal(addedSub.profileId, 'profile-789');

    emitTauriEvent('zenohx://subscription-removed', { id: 'sub-test-123' });
    const subsAfter = useMessageStore.getState().subscriptions;
    assert.ok(!subsAfter.some((s) => s.id === 'sub-test-123'), 'Subscription should be removed from messageStore');

    harness.unmount();
  });

  test('adds queryable to useQueryStore on zenohx://queryable-added', async () => {
    const harness = renderHook((cb) => useMcpListener(cb), () => {});

    await new Promise((resolve) => setTimeout(resolve, 10));

    emitTauriEvent('zenohx://queryable-added', {
      id: 'queryable-test-456',
      session_id: 'session-789',
      profile_id: 'profile-abc',
      key_expr: 'demo/mcp/script',
      auto_reply: true,
      reply_mode: 'script',
      script_code: 'return { status: "ok" };',
      reply_encoding: 'json',
    });

    const currentQueryables = useQueryStore.getState().activeQueryables;
    const addedQueryable = currentQueryables.find((q) => q.id === 'queryable-test-456');
    assert.ok(addedQueryable, 'Queryable should be added to queryStore');
    assert.equal(addedQueryable.keyExpr, 'demo/mcp/script');
    assert.equal(addedQueryable.replyMode, 'script');
    assert.equal(addedQueryable.scriptCode, 'return { status: "ok" };');

    harness.unmount();
  });

  test('reloads profiles on zenohx://profile-updated', async () => {
    let loadProfilesCalled = false;
    const origLoadProfiles = useConnectionStore.getState().loadProfiles;
    useConnectionStore.setState({
      loadProfiles: async () => {
        loadProfilesCalled = true;
      },
    });

    const harness = renderHook((cb) => useMcpListener(cb), () => {});
    await new Promise((resolve) => setTimeout(resolve, 10));

    emitTauriEvent('zenohx://profile-updated', {
      profile: {
        id: 'profile-test-123',
        name: 'Updated Node',
        mode: 'peer',
        connect_locators: [],
        listen_locators: [],
        scout_multicast: true,
      },
      session_restarted: false,
    });

    assert.equal(loadProfilesCalled, true, 'loadProfiles should be called on zenohx://profile-updated');

    harness.unmount();
    useConnectionStore.setState({ loadProfiles: origLoadProfiles });
  });
});
