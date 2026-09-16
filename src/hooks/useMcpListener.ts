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
import { useMessageStore } from '../stores/messageStore';
import { useQueryStore } from '../stores/queryStore';
import { useConnectionStore } from '../stores/connectionStore';

export type WorkspaceTab = 'pubsub' | 'query' | 'traffic' | 'topology' | 'settings';

export interface McpActionPayload {
  action: string;
  details: string;
}

export interface GuiSwitchTabPayload {
  workspace: WorkspaceTab;
}

export interface SubscriptionAddedPayload {
  id: string;
  session_id: string;
  profile_id: string;
  key_expr: string;
  encoding?: string;
  active?: boolean;
}

export interface SubscriptionRemovedPayload {
  id: string;
}

export interface QueryableAddedPayload {
  id: string;
  session_id: string;
  profile_id?: string;
  key_expr: string;
  auto_reply?: boolean;
  reply_mode?: 'payload' | 'script';
  reply_payload?: string;
  script_code?: string;
  reply_encoding?: string;
}

export interface ProfileUpdatedPayload {
  profile: {
    id: string;
    name: string;
    mode: string;
    connect_locators: string[];
    listen_locators: string[];
    scout_multicast: boolean;
  };
  session_restarted?: boolean;
  session_id?: string;
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
 * - Listens for `zenohx://subscription-added` and `zenohx://subscription-removed` to sync subscriptions.
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
    let unlistenSubAdded: (() => void) | undefined;
    let unlistenSubRemoved: (() => void) | undefined;
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

    listen<SubscriptionAddedPayload>('zenohx://subscription-added', (event) => {
      if (event.payload?.id && event.payload?.key_expr) {
        useMessageStore.getState().addExternalSubscription({
          id: event.payload.id,
          sessionId: event.payload.session_id,
          profileId: event.payload.profile_id,
          keyExpr: event.payload.key_expr,
          encoding: event.payload.encoding,
          active: event.payload.active,
        });
      }
    })
      .then((un) => {
        if (!isMounted) {
          un();
        } else {
          unlistenSubAdded = un;
        }
      })
      .catch((err) => {
        console.warn('Failed to listen to zenohx://subscription-added:', err);
      });

    listen<SubscriptionRemovedPayload>('zenohx://subscription-removed', (event) => {
      if (event.payload?.id) {
        useMessageStore.getState().removeExternalSubscription(event.payload.id);
      }
    })
      .then((un) => {
        if (!isMounted) {
          un();
        } else {
          unlistenSubRemoved = un;
        }
      })
      .catch((err) => {
        console.warn('Failed to listen to zenohx://subscription-removed:', err);
      });

    let unlistenQueryableAdded: (() => void) | undefined;
    listen<QueryableAddedPayload>('zenohx://queryable-added', (event) => {
      if (event.payload?.id && event.payload?.key_expr) {
        useQueryStore.getState().addExternalQueryable({
          id: event.payload.id,
          sessionId: event.payload.session_id,
          profileId: event.payload.profile_id,
          keyExpr: event.payload.key_expr,
          autoReply: event.payload.auto_reply ?? true,
          replyMode: event.payload.reply_mode,
          replyPayload: event.payload.reply_payload,
          scriptCode: event.payload.script_code,
          replyEncoding: event.payload.reply_encoding,
        });
      }
    })
      .then((un) => {
        if (!isMounted) {
          un();
        } else {
          unlistenQueryableAdded = un;
        }
      })
      .catch((err) => {
        console.warn('Failed to listen to zenohx://queryable-added:', err);
      });

    let unlistenProfileUpdated: (() => void) | undefined;
    listen<ProfileUpdatedPayload>('zenohx://profile-updated', (event) => {
      if (event.payload?.profile) {
        useConnectionStore.getState().loadProfiles().catch((err) => {
          console.warn('Failed to reload profiles on zenohx://profile-updated:', err);
        });
        if (event.payload.session_restarted) {
          useConnectionStore.getState().refreshSessions().catch((err) => {
            console.warn('Failed to refresh sessions on zenohx://profile-updated:', err);
          });
        }
      }
    })
      .then((un) => {
        if (!isMounted) {
          un();
        } else {
          unlistenProfileUpdated = un;
        }
      })
      .catch((err) => {
        console.warn('Failed to listen to zenohx://profile-updated:', err);
      });

    return () => {
      isMounted = false;
      if (actionTimer) {
        clearTimeout(actionTimer);
      }
      unlistenTab?.();
      unlistenAction?.();
      unlistenSubAdded?.();
      unlistenSubRemoved?.();
      unlistenQueryableAdded?.();
      unlistenProfileUpdated?.();
    };
  }, [onSwitchTab]);

  return { lastAction };
}
