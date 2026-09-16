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

import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';

export type WorkspaceTab = 'pubsub' | 'query' | 'traffic' | 'topology' | 'settings';

export interface McpActionPayload {
  action: string;
  details: string;
}

export interface GuiSwitchTabPayload {
  workspace: WorkspaceTab;
}

export type UseMcpListenerOptions =
  | ((tab: WorkspaceTab) => void)
  | {
      setActiveTab?: (tab: WorkspaceTab) => void;
      onSwitchTab?: (tab: WorkspaceTab) => void;
    };

/**
 * React hook that subscribes to Tauri events from the ZenohX MCP server and GUI controller.
 *
 * - Listens for `zenohx://gui-switch-tab` to switch active workspace tabs in real time.
 * - Listens for `zenohx://mcp-action` to display real-time toast notifications for AI actions.
 */
export function useMcpListener(optionsOrCallback: UseMcpListenerOptions) {
  const [lastAction, setLastAction] = useState<McpActionPayload | null>(null);

  const onSwitchTab =
    typeof optionsOrCallback === 'function'
      ? optionsOrCallback
      : optionsOrCallback.setActiveTab ?? optionsOrCallback.onSwitchTab;

  useEffect(() => {
    let isMounted = true;
    let unlistenTab: (() => void) | undefined;
    let unlistenAction: (() => void) | undefined;
    let actionTimer: ReturnType<typeof setTimeout> | undefined;

    listen<GuiSwitchTabPayload>('zenohx://gui-switch-tab', (event) => {
      if (event.payload?.workspace && onSwitchTab) {
        onSwitchTab(event.payload.workspace);
      }
    })
      .then((un) => {
        if (!isMounted) {
          un();
        } else {
          unlistenTab = un;
        }
      })
      .catch((err) => {
        console.warn('Failed to listen to zenohx://gui-switch-tab:', err);
      });

    listen<McpActionPayload>('zenohx://mcp-action', (event) => {
      if (event.payload) {
        setLastAction(event.payload);

        if (actionTimer) {
          clearTimeout(actionTimer);
        }

        actionTimer = setTimeout(() => {
          setLastAction((curr) => (curr === event.payload ? null : curr));
        }, 4000);
      }
    })
      .then((un) => {
        if (!isMounted) {
          un();
        } else {
          unlistenAction = un;
        }
      })
      .catch((err) => {
        console.warn('Failed to listen to zenohx://mcp-action:', err);
      });

    return () => {
      isMounted = false;
      if (actionTimer) {
        clearTimeout(actionTimer);
      }
      unlistenTab?.();
      unlistenAction?.();
    };
  }, [onSwitchTab]);

  return { lastAction };
}
