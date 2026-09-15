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

import React, { useState, useMemo, useEffect } from 'react';
import {
  Layers,
  AlertCircle,
  Settings2,
  MoreVertical,
  Play,
  Power,
  Copy,
  Check,
  CopyPlus,
  Trash2,
  ExternalLink,
} from 'lucide-react';
import { useConnectionStore } from '../../stores/connectionStore';
import { useMessageStore } from '../../stores/messageStore';
import { openProfileInNewWindow } from '../../lib/tauri';
import { SubscriptionList } from './SubscriptionList';
import { MessageList } from './MessageList';
import { PublishBar } from './PublishBar';
import { MessageDetails, fromMessageItem } from '../viewer/MessageDetails';
import { Button } from '../ui/button';
import { Badge } from '../ui/badge';
import { ResizeHandle } from '../ui/resize-handle';
import { useResizable } from '../../hooks/useResizable';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '../ui/dropdown-menu';
import { ProfileModal } from '../connections/ProfileModal';

interface PubSubWorkspaceProps {
  className?: string;
}

export const PubSubWorkspace: React.FC<PubSubWorkspaceProps> = ({ className = '' }) => {
  // Connection store state
  const selectedProfileId = useConnectionStore((s) => s.selectedProfileId);
  const profiles = useConnectionStore((s) => s.profiles);
  const activeSessions = useConnectionStore((s) => s.activeSessions);
  const sessionToProfile = useConnectionStore((s) => s.sessionToProfile);
  const connect = useConnectionStore((s) => s.connect);
  const disconnect = useConnectionStore((s) => s.disconnect);
  const saveProfile = useConnectionStore((s) => s.saveProfile);
  const deleteProfile = useConnectionStore((s) => s.deleteProfile);
  const selectProfile = useConnectionStore((s) => s.selectProfile);

  // Active session and profile details
  const profile = useMemo(
    () => profiles.find((p) => p.id === selectedProfileId) || null,
    [profiles, selectedProfileId]
  );
  const session = selectedProfileId ? activeSessions[selectedProfileId] : null;
  const isConnected = Boolean(session);
  const sessionId = session?.id;

  // Message store state
  const selectedMessage = useMessageStore((s) => s.selectedMessage);
  const selectMessage = useMessageStore((s) => s.selectMessage);
  const subscriptions = useMessageStore((s) => s.subscriptions);
  const loadHistory = useMessageStore((s) => s.loadHistory);
  const loadSubscriptions = useMessageStore((s) => s.loadSubscriptions);
  const clearMessages = useMessageStore((s) => s.clearMessages);

  // Dynamic sender / origin details for the selected message
  const selectedMessageSender = useMemo(() => {
    if (!selectedMessage) return null;

    const msgProfileId =
      selectedMessage.profileId ||
      (selectedMessage.sessionId ? sessionToProfile[selectedMessage.sessionId] : undefined) ||
      selectedProfileId;
    const msgProfile = profiles.find((p) => p.id === msgProfileId) || null;

    if (selectedMessage.direction === 'outgoing') {
      const title = msgProfile?.name ? `${msgProfile.name} (Local Client)` : 'Local Client (ZenohX)';
      return {
        title,
        subtitle: 'Sent by this client',
        profileName: msgProfile?.name,
        zid: selectedMessage.senderZid || (msgProfileId && activeSessions[msgProfileId]?.zid),
        isOutgoing: true,
      };
    } else {
      const fullZid = selectedMessage.sourceId || selectedMessage.senderZid || 'Unknown / Anonymous';
      return {
        title: fullZid,
        subtitle: 'Received from network',
        profileName: msgProfile?.name,
        zid: selectedMessage.sourceId || selectedMessage.senderZid,
        isOutgoing: false,
      };
    }
  }, [selectedMessage, profiles, sessionToProfile, selectedProfileId, activeSessions]);

  // Auto load message history and subscription presets from SQLite when profile/session changes
  useEffect(() => {
    if (selectedProfileId) {
      loadHistory(selectedProfileId);
      loadSubscriptions(selectedProfileId, sessionId);
    }
  }, [selectedProfileId, sessionId, loadHistory, loadSubscriptions]);

  // Panel layout and modal toggles
  const [showSubscriptionPanel, setShowSubscriptionPanel] = useState<boolean>(true);
  const [isEditModalOpen, setIsEditModalOpen] = useState<boolean>(false);
  const [copiedZid, setCopiedZid] = useState<boolean>(false);

  // Unified message item for inspector
  const unifiedSelectedMessage = useMemo(() => {
    if (!selectedMessage) return null;
    return fromMessageItem(selectedMessage, {
      senderTitle: selectedMessageSender?.title,
      senderSubtitle: selectedMessageSender?.subtitle,
      profileName: selectedMessageSender?.profileName,
    });
  }, [selectedMessage, selectedMessageSender]);

  const handleCopyZid = () => {
    if (session?.zid) {
      navigator.clipboard.writeText(session.zid);
      setCopiedZid(true);
      setTimeout(() => setCopiedZid(false), 2000);
    }
  };

  const handleToggleConnect = async () => {
    if (!selectedProfileId) return;
    try {
      if (isConnected) {
        await disconnect(selectedProfileId);
      } else {
        await connect(selectedProfileId);
      }
    } catch {
      // Handled by connection store
    }
  };

  const handleDuplicateProfile = async () => {
    if (!profile) return;
    const now = Math.floor(Date.now() / 1000);
    const duplicated = {
      ...profile,
      id: crypto.randomUUID ? crypto.randomUUID() : `profile-${Date.now()}`,
      name: `${profile.name} (Copy)`,
      created_at: now,
      updated_at: now,
    };
    await saveProfile(duplicated);
    selectProfile(duplicated.id);
  };

  const handleDeleteProfile = async () => {
    if (!profile) return;
    if (window.confirm(`Are you sure you want to delete profile "${profile.name}"?`)) {
      await deleteProfile(profile.id);
    }
  };

  const handleClearHistory = async () => {
    if (selectedProfileId) {
      await clearMessages(sessionId || undefined, selectedProfileId);
    }
  };

  // Resizable Subscriptions Left Panel
  const {
    size: subPanelWidth,
    isDragging: isSubDragging,
    startDragging: startSubDragging,
    resetToDefault: resetSubWidth,
  } = useResizable({
    initialSize: 280,
    minSize: 200,
    maxSize: 450,
    storageKey: 'zenohx_pubsub_sub_width',
  });

  // Stats for the workspace
  const sessionSubs = useMemo(() => {
    if (selectedProfileId) {
      return subscriptions.filter((s) => s.profileId === selectedProfileId);
    }
    if (sessionId) {
      return subscriptions.filter((s) => s.sessionId === sessionId);
    }
    return subscriptions;
  }, [subscriptions, selectedProfileId, sessionId]);

  return (
    <div className={`flex flex-col h-full w-full bg-background text-foreground overflow-hidden ${className}`}>
      {/* Workspace Top Header Bar */}
      <header className="flex flex-wrap items-center justify-between gap-2 border-b bg-card px-4 py-2 select-none shrink-0">
        {/* Left: Connection name (no icon) + Mode badge + Session ZID */}
        <div className="flex items-center gap-2 min-w-0">
          <h2
            className="text-xs font-semibold text-foreground truncate max-w-[220px]"
            title={profile ? profile.name : 'Pub / Sub Workspace'}
          >
            {profile ? profile.name : 'Pub / Sub Workspace'}
          </h2>

          {profile && (
            <Badge variant="secondary" className="text-[10px] uppercase font-mono px-1.5 py-0 shrink-0">
              {profile.mode}
            </Badge>
          )}

          {/* Connection Status Indicator & ZID */}
          {isConnected ? (
            <div className="flex items-center gap-1.5 shrink-0">
              <span className="inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
              <span
                className="text-[11px] font-mono text-muted-foreground bg-muted px-1.5 py-0.5 rounded cursor-pointer hover:text-foreground transition-colors"
                title={session?.zid ? `Click to copy ZID: ${session.zid}` : 'Connected'}
                onClick={handleCopyZid}
              >
                ZID: {session?.zid ? `${session.zid.slice(0, 8)}…` : 'Connected'}
              </span>
            </div>
          ) : (
            <div className="flex items-center gap-1.5 shrink-0">
              <span className="inline-flex rounded-full h-2 w-2 bg-muted-foreground/40"></span>
              <span className="text-[11px] text-muted-foreground">Disconnected</span>
            </div>
          )}
        </div>

        {/* Right: Topics toggle, Connection Edit, 3-dot dropdown menu */}
        <div className="flex items-center gap-1.5">
          {/* Subscriptions / Topics Panel Toggle Button */}
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => setShowSubscriptionPanel(!showSubscriptionPanel)}
            className={`h-7 px-2 text-xs gap-1.5 transition-colors ${
              showSubscriptionPanel
                ? 'bg-accent text-accent-foreground border-accent-foreground/20'
                : 'text-muted-foreground hover:text-foreground'
            }`}
            title={
              showSubscriptionPanel
                ? 'Hide Topics panel'
                : 'Show Topics panel'
            }
          >
            <Layers className="w-3.5 h-3.5" />
            <span>Topics ({sessionSubs.length})</span>
          </Button>

          {/* Connection Edit Button */}
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={!profile}
            onClick={() => setIsEditModalOpen(true)}
            className="h-7 px-2 text-xs gap-1.5 text-muted-foreground hover:text-foreground"
            title="Edit Connection Profile"
          >
            <Settings2 className="w-3.5 h-3.5" />
            <span>Connection Edit</span>
          </Button>

          {/* 3-dot Actions Dropdown Menu */}
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
                title="More Actions"
              >
                <MoreVertical className="w-3.5 h-3.5" />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-48">
              {profile && (
                <>
                  <DropdownMenuItem onClick={handleToggleConnect}>
                    {isConnected ? (
                      <>
                        <Power className="w-3.5 h-3.5 mr-2 text-rose-500" />
                        <span>Disconnect</span>
                      </>
                    ) : (
                      <>
                        <Play className="w-3.5 h-3.5 mr-2 text-emerald-500" />
                        <span>Connect</span>
                      </>
                    )}
                  </DropdownMenuItem>

                  {session?.zid && (
                    <DropdownMenuItem onClick={handleCopyZid}>
                      {copiedZid ? (
                        <>
                          <Check className="w-3.5 h-3.5 mr-2 text-emerald-500" />
                          <span>Copied ZID!</span>
                        </>
                      ) : (
                        <>
                          <Copy className="w-3.5 h-3.5 mr-2" />
                          <span>Copy ZID</span>
                        </>
                      )}
                    </DropdownMenuItem>
                  )}

                  <DropdownMenuItem onClick={() => profile && openProfileInNewWindow(profile)}>
                    <ExternalLink className="w-3.5 h-3.5 mr-2" />
                    <span>Open in New Window</span>
                  </DropdownMenuItem>

                  <DropdownMenuItem onClick={() => setIsEditModalOpen(true)}>
                    <Settings2 className="w-3.5 h-3.5 mr-2" />
                    <span>Edit Profile</span>
                  </DropdownMenuItem>

                  <DropdownMenuItem onClick={handleDuplicateProfile}>
                    <CopyPlus className="w-3.5 h-3.5 mr-2" />
                    <span>Duplicate Profile</span>
                  </DropdownMenuItem>

                  <DropdownMenuSeparator />

                  <DropdownMenuItem onClick={handleClearHistory} className="text-muted-foreground hover:text-destructive">
                    <Trash2 className="w-3.5 h-3.5 mr-2" />
                    <span>Clear Messages</span>
                  </DropdownMenuItem>

                  <DropdownMenuSeparator />

                  <DropdownMenuItem onClick={handleDeleteProfile} className="text-destructive focus:text-destructive">
                    <Trash2 className="w-3.5 h-3.5 mr-2" />
                    <span>Delete Profile</span>
                  </DropdownMenuItem>
                </>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </header>

      {/* Disconnected Notice Banner if selected profile is not connected */}
      {!isConnected && (
        <div className="flex items-center justify-between border-b bg-muted/50 px-4 py-1.5 text-xs text-muted-foreground">
          <div className="flex items-center gap-2">
            <AlertCircle className="w-3.5 h-3.5 shrink-0" />
            <span>
              Session is offline. Connect to Zenoh network to receive real-time streams and publish samples.
            </span>
          </div>
          {profile && (
            <Button
              size="sm"
              variant="outline"
              onClick={() => connect(profile.id)}
              className="h-6 px-2 text-xs"
            >
              Connect
            </Button>
          )}
        </div>
      )}

      {/* Main Workspace Stage Body (Split View) */}
      <div className="flex-1 flex min-h-0 relative overflow-hidden">
        {/* Left: Subscriptions Side Panel */}
        {showSubscriptionPanel && (
          <>
            <div
              style={{ width: `${subPanelWidth}px` }}
              className="shrink-0 h-full border-r border-border overflow-hidden"
            >
              <SubscriptionList
                sessionId={sessionId}
                profileId={profile?.id}
                className="h-full"
              />
            </div>
            <ResizeHandle
              isDragging={isSubDragging}
              onMouseDown={startSubDragging}
              onReset={resetSubWidth}
            />
          </>
        )}

        {/* Center & Right: Message Stream Feed + Inspector Panel */}
        <div className="flex-1 flex flex-col min-w-0 h-full overflow-hidden">
          {/* Upper split: Messages list + Details Inspector */}
          <div className="flex-1 flex min-h-0 relative overflow-hidden">
            {/* Center: Message Feed */}
            <div className="flex-1 flex flex-col min-w-0 h-full">
              <MessageList
                sessionId={sessionId}
                profileId={profile?.id}
                onSelectMessage={(msg) => selectMessage(msg)}
                className="h-full"
              />
            </div>

            {/* Right: Message Details Inspector */}
            {unifiedSelectedMessage && (
              <MessageDetails
                data={unifiedSelectedMessage}
                title="Message Details"
                onClose={() => selectMessage(null)}
                storageKey="zenohx_pubsub_inspector_width"
              />
            )}
          </div>

          {/* Bottom Publisher Toolbar */}
          <PublishBar
            sessionId={sessionId}
            profileId={profile?.id}
            className="shrink-0"
          />
        </div>
      </div>

      {/* Profile Modal for Connection Edit */}
      {profile && (
        <ProfileModal
          isOpen={isEditModalOpen}
          onClose={() => setIsEditModalOpen(false)}
          profile={profile}
          onSaved={(saved) => {
            saveProfile(saved);
            setIsEditModalOpen(false);
          }}
        />
      )}
    </div>
  );
};

export default PubSubWorkspace;
