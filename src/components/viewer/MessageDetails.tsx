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

import React, { useState, useRef } from 'react';
import {
  Info,
  X,
  Maximize2,
  Minimize2,
  ChevronRight,
  ChevronDown,
  ArrowDown,
  Copy,
  Check,
  Clock,
  Activity,
} from 'lucide-react';
import { Badge } from '../ui/badge';
import { ResizeHandle } from '../ui/resize-handle';
import { useResizable } from '../../hooks/useResizable';
import { PayloadViewer } from './PayloadViewer';
import {
  formatByteSize,
  formatFullDateTime,
  normalizeEncoding,
} from '../../lib/formatters';
import type { EncodingType, MessageItem, PutKind, ReplySample } from '../../types/zenoh';

/**
 * Unified data model representing any inspected message or RPC reply.
 */
export interface UnifiedMessageData {
  keyExpr: string;
  payload?: Uint8Array | number[] | null;
  encoding?: EncodingType | string;
  timestamp: number;

  // Pub/Sub fields (optional)
  direction?: 'incoming' | 'outgoing';
  kind?: PutKind | string;
  sessionId?: string;
  senderZid?: string | null;
  profileName?: string;
  senderTitle?: string;
  senderSubtitle?: string;

  // Query Reply fields (optional)
  latencyMs?: number;
  replierId?: string | null;
  isError?: boolean;
  errorMessage?: string | null;

  // Custom metadata rows
  extraDetails?: Array<{ label: string; value: React.ReactNode }>;

  // Protobuf metadata (optional)
  protoTypeName?: string | null;
}

/**
 * Adapter converting a Pub/Sub MessageItem into UnifiedMessageData.
 */
export function fromMessageItem(
  msg: MessageItem,
  options?: {
    senderTitle?: string;
    senderSubtitle?: string;
    profileName?: string;
  }
): UnifiedMessageData {
  const defaultZid = msg.direction === 'outgoing' ? msg.senderZid : (msg.sourceId || msg.senderZid);
  const defaultTitle =
    options?.senderTitle ||
    (msg.direction === 'outgoing'
      ? 'Local Client'
      : (msg.sourceId || msg.senderZid || 'Unknown / Anonymous'));

  return {
    keyExpr: msg.keyExpr,
    payload: msg.payload,
    encoding: msg.encoding,
    timestamp: msg.timestamp,
    direction: msg.direction,
    kind: msg.kind,
    sessionId: msg.sessionId,
    senderZid: defaultZid,
    profileName: options?.profileName,
    senderTitle: defaultTitle,
    senderSubtitle: options?.senderSubtitle,
    protoTypeName: msg.protoTypeName,
  };
}

/**
 * Adapter converting a Query ReplySample into UnifiedMessageData.
 */
export function fromReplySample(reply: ReplySample): UnifiedMessageData {
  return {
    keyExpr: reply.key_expr,
    payload: reply.payload,
    encoding: reply.encoding,
    timestamp: reply.timestamp,
    latencyMs: reply.latency_ms,
    replierId: reply.replier_id,
    isError: reply.is_err,
    errorMessage: reply.error_message,
    direction: 'incoming',
  };
}

export interface MessageDetailsProps {
  data: UnifiedMessageData | null;
  title?: string;
  onClose?: () => void;
  initialWidth?: number;
  minWidth?: number;
  storageKey?: string;
  showResizeHandle?: boolean;
  className?: string;
}

export const MessageDetails: React.FC<MessageDetailsProps> = ({
  data,
  title = 'Message Details',
  onClose,
  initialWidth = 380,
  minWidth = 280,
  storageKey = 'zenohx_message_inspector_width',
  showResizeHandle = true,
  className = '',
}) => {
  const [inspectorView, setInspectorView] = useState<'all' | 'payload'>('all');
  const [inspectorExpanded, setInspectorExpanded] = useState<boolean>(false);
  const [isMetaCollapsed, setIsMetaCollapsed] = useState<boolean>(false);
  const [copiedKey, setCopiedKey] = useState<boolean>(false);
  const [copiedSenderZid, setCopiedSenderZid] = useState<boolean>(false);
  const [copiedReplierId, setCopiedReplierId] = useState<boolean>(false);

  const handleCopySenderZid = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedSenderZid(true);
    setTimeout(() => setCopiedSenderZid(false), 2000);
  };

  const handleCopyReplierId = (text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedReplierId(true);
    setTimeout(() => setCopiedReplierId(false), 2000);
  };

  const payloadSectionRef = useRef<HTMLDivElement>(null);

  // Resizable Right Panel
  const {
    size: inspectorWidth,
    isDragging: isInspectorDragging,
    startDragging: startInspectorDragging,
    resetToDefault: resetInspectorWidth,
  } = useResizable({
    initialSize: initialWidth,
    minSize: minWidth,
    maxSize: 700,
    reverse: true,
    storageKey,
  });

  const scrollToPayload = () => {
    if (payloadSectionRef.current) {
      payloadSectionRef.current.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
  };

  if (!data) {
    return null;
  }

  const payloadLength = data.payload?.length || 0;
  const normalizedEnc = normalizeEncoding(data.encoding, data.payload, data.keyExpr);

  return (
    <>
      {showResizeHandle && (
        <ResizeHandle
          isDragging={isInspectorDragging}
          onMouseDown={startInspectorDragging}
          onReset={resetInspectorWidth}
          className="hidden md:flex"
        />
      )}

      <div
        style={{
          width: `${inspectorExpanded ? Math.max(580, inspectorWidth) : inspectorWidth}px`,
        }}
        className={`border-l border-border bg-card flex flex-col shrink-0 h-full overflow-hidden max-md:fixed max-md:inset-0 max-md:z-50 max-md:w-full max-w-full md:max-w-[75vw] min-w-[280px] ${className}`}
      >
        {/* Header */}
        <div className="flex items-center justify-between p-2.5 border-b bg-muted/20 shrink-0">
          <div className="flex items-center gap-1.5 min-w-0 flex-1">
            {data.latencyMs !== undefined ? (
              <Activity className="w-3.5 h-3.5 text-muted-foreground shrink-0" />
            ) : (
              <Info className="w-3.5 h-3.5 text-muted-foreground shrink-0" />
            )}
            <span className="font-semibold text-xs text-foreground truncate">
              {title}
            </span>
          </div>

          <div className="flex items-center gap-1">
            {/* View Switcher: All vs Payload */}
            <div className="flex items-center rounded border bg-muted/50 p-0.5 text-[10px] mr-1">
              <button
                type="button"
                onClick={() => setInspectorView('all')}
                className={`px-1.5 py-0.5 rounded transition-colors ${
                  inspectorView === 'all'
                    ? 'bg-background text-foreground font-semibold shadow-xs'
                    : 'text-muted-foreground hover:text-foreground'
                }`}
                title="Show overview and payload"
              >
                All
              </button>
              <button
                type="button"
                onClick={() => setInspectorView('payload')}
                className={`px-1.5 py-0.5 rounded transition-colors ${
                  inspectorView === 'payload'
                    ? 'bg-background text-foreground font-semibold shadow-xs'
                    : 'text-muted-foreground hover:text-foreground'
                }`}
                title="Show payload only"
              >
                Payload
              </button>
            </div>

            {/* Expand / Narrow Width Button */}
            <button
              type="button"
              onClick={() => setInspectorExpanded(!inspectorExpanded)}
              className="hidden md:inline-flex p-1 rounded text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
              title={inspectorExpanded ? 'Narrow inspector' : 'Widen inspector'}
            >
              {inspectorExpanded ? (
                <Minimize2 className="w-3.5 h-3.5" />
              ) : (
                <Maximize2 className="w-3.5 h-3.5" />
              )}
            </button>

            {/* Close Inspector */}
            {onClose && (
              <button
                type="button"
                onClick={onClose}
                className="p-1 rounded text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                title="Close inspector"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            )}
          </div>
        </div>

        {/* Scrollable Inspector Body */}
        <div className="flex-1 min-h-0 overflow-y-auto flex flex-col">
          {/* Metadata Overview */}
          {inspectorView === 'all' && (
            <div className="p-3 border-b bg-muted/10 space-y-2.5 text-xs shrink-0">
              {/* Section Header with Collapse & Scroll to Payload */}
              <div className="flex items-center justify-between pb-1 border-b border-border/40">
                <button
                  type="button"
                  onClick={() => setIsMetaCollapsed(!isMetaCollapsed)}
                  className="flex items-center gap-1 text-[10px] font-semibold text-muted-foreground uppercase hover:text-foreground transition-colors"
                  title={isMetaCollapsed ? 'Expand metadata' : 'Collapse metadata'}
                >
                  {isMetaCollapsed ? (
                    <ChevronRight className="w-3 h-3" />
                  ) : (
                    <ChevronDown className="w-3 h-3" />
                  )}
                  <span>Metadata Overview</span>
                </button>

                <button
                  type="button"
                  onClick={scrollToPayload}
                  className="text-[10px] text-muted-foreground hover:text-foreground flex items-center gap-0.5 transition-colors"
                  title="Scroll down to payload"
                >
                  <span>Scroll to Payload</span>
                  <ArrowDown className="w-2.5 h-2.5" />
                </button>
              </div>

              {isMetaCollapsed ? (
                <div className="flex items-center gap-2 text-xs font-mono pt-0.5 text-muted-foreground truncate">
                  <span className="font-semibold text-foreground truncate">{data.keyExpr}</span>
                  {data.latencyMs !== undefined && (
                    <>
                      <span>•</span>
                      <span className="text-foreground font-semibold">{data.latencyMs} ms</span>
                    </>
                  )}
                  {data.kind === 'delete' && (
                    <>
                      <span>•</span>
                      <Badge
                        variant="destructive"
                        className="text-[9px] font-mono font-semibold uppercase px-1 py-0"
                      >
                        DELETE
                      </Badge>
                    </>
                  )}
                  <span>•</span>
                  <span>{formatByteSize(payloadLength)}</span>
                </div>
              ) : (
                <>
                  {/* Key Expression */}
                  <div>
                    <div className="text-[10px] uppercase font-semibold text-muted-foreground mb-0.5 flex items-center justify-between">
                      <span>Key Expression</span>
                      <button
                        type="button"
                        onClick={() => {
                          navigator.clipboard.writeText(data.keyExpr);
                          setCopiedKey(true);
                          setTimeout(() => setCopiedKey(false), 2000);
                        }}
                        className="text-[10px] text-muted-foreground hover:text-foreground flex items-center gap-1"
                        title="Copy key expression"
                      >
                        {copiedKey ? (
                          <>
                            <Check className="w-2.5 h-2.5 text-emerald-500" />
                            <span className="text-emerald-500">Copied</span>
                          </>
                        ) : (
                          <>
                            <Copy className="w-2.5 h-2.5" />
                            <span>Copy</span>
                          </>
                        )}
                      </button>
                    </div>
                    <div className="font-mono text-xs font-medium text-foreground break-all bg-muted/40 p-1.5 rounded border">
                      {data.keyExpr}
                    </div>
                  </div>

                  {/* Sender / Origin Information (Pub/Sub) */}
                  {(data.senderTitle || data.senderZid || data.sessionId || data.profileName) && (
                    <div>
                      <div className="text-[10px] uppercase font-semibold text-muted-foreground mb-0.5 flex items-center justify-between">
                        <span>Sender / Origin</span>
                        {data.senderSubtitle && (
                          <span className="text-[9px] text-muted-foreground font-normal">
                            {data.senderSubtitle}
                          </span>
                        )}
                      </div>
                      <div className="p-2 rounded bg-muted/40 border space-y-1.5 font-mono text-[11px]">
                        {data.senderTitle && (
                          <div className="flex items-center justify-between gap-1.5 font-medium text-foreground">
                            <span className="font-semibold break-all select-all">
                              {data.senderTitle}
                            </span>
                            {(data.senderZid || (data.senderTitle && data.senderTitle !== 'Unknown / Anonymous')) && (
                              <button
                                type="button"
                                onClick={() => handleCopySenderZid(data.senderZid || data.senderTitle!)}
                                title="Copy Sender ZID"
                                className="p-1 rounded hover:bg-muted text-muted-foreground hover:text-foreground transition-colors shrink-0"
                              >
                                {copiedSenderZid ? (
                                  <Check className="w-3.5 h-3.5 text-emerald-500" />
                                ) : (
                                  <Copy className="w-3.5 h-3.5" />
                                )}
                              </button>
                            )}
                          </div>
                        )}

                        {/* Secondary metadata: Session ZID (if distinct from sender title), Session ID, Profile */}
                        {(data.sessionId ||
                          data.profileName ||
                          (data.senderZid && data.senderZid !== data.senderTitle)) && (
                          <div className="text-[10px] text-muted-foreground flex flex-wrap gap-x-3 gap-y-1 pt-1 border-t border-border/50">
                            {data.senderZid && data.senderZid !== data.senderTitle && (
                              <div className="flex items-center gap-1">
                                <span className="text-muted-foreground">
                                  {data.direction === 'outgoing' ? 'Session ZID:' : 'Publisher ZID:'}
                                </span>
                                <span className="font-semibold text-foreground break-all">
                                  {data.senderZid}
                                </span>
                              </div>
                            )}
                            {data.sessionId && (
                              <div className="flex items-center gap-1">
                                <span className="text-muted-foreground">Session:</span>
                                <span className="font-semibold text-foreground">
                                  {data.sessionId.slice(0, 8)}
                                </span>
                              </div>
                            )}
                            {data.profileName && (
                              <div className="flex items-center gap-1">
                                <span className="text-muted-foreground">Profile:</span>
                                <span className="font-semibold text-foreground">
                                  {data.profileName}
                                </span>
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    </div>
                  )}

                  {/* Grid: Latency, Status/Origin, Encoding, Payload Size, Timestamp */}
                  <div className="grid grid-cols-2 gap-2 pt-0.5">
                    {/* Latency (for RPC Reply) */}
                    {data.latencyMs !== undefined && (
                      <div>
                        <div className="text-[10px] uppercase font-semibold text-muted-foreground mb-0.5">
                          Latency
                        </div>
                        <div className="flex items-center gap-1 font-mono text-xs font-medium text-foreground">
                          <Clock className="w-3 h-3 text-muted-foreground" />
                          <span>{data.latencyMs} ms</span>
                        </div>
                      </div>
                    )}

                    {/* Replier Node (for RPC Reply) */}
                    {data.replierId !== undefined && (
                      <div className="col-span-2">
                        <div className="text-[10px] uppercase font-semibold text-muted-foreground mb-0.5">
                          Replier Node
                        </div>
                        <div className="flex items-center justify-between gap-1.5 font-mono text-xs text-foreground bg-muted/30 p-1.5 rounded border">
                          <span className="break-all select-all">
                            {data.replierId ? `ZID: ${data.replierId}` : 'Anonymous'}
                          </span>
                          {data.replierId && (
                            <button
                              type="button"
                              onClick={() => handleCopyReplierId(data.replierId!)}
                              title="Copy Replier ZID"
                              className="p-1 rounded hover:bg-muted text-muted-foreground hover:text-foreground transition-colors shrink-0"
                            >
                              {copiedReplierId ? (
                                <Check className="w-3.5 h-3.5 text-emerald-500" />
                              ) : (
                                <Copy className="w-3.5 h-3.5" />
                              )}
                            </button>
                          )}
                        </div>
                      </div>
                    )}

                    {/* Status (for RPC Reply) */}
                    {data.isError !== undefined && (
                      <div>
                        <div className="text-[10px] uppercase font-semibold text-muted-foreground mb-0.5">
                          Status
                        </div>
                        <div>
                          {data.isError ? (
                            <Badge variant="destructive" className="text-[10px] px-1.5 py-0 uppercase">
                              Error
                            </Badge>
                          ) : (
                            <Badge variant="secondary" className="text-[10px] px-1.5 py-0 uppercase">
                              OK
                            </Badge>
                          )}
                        </div>
                      </div>
                    )}

                    {/* Operation Kind (when delete) */}
                    {data.kind === 'delete' && (
                      <div>
                        <div className="text-[10px] uppercase font-semibold text-muted-foreground mb-0.5">
                          Operation
                        </div>
                        <div>
                          <Badge variant="destructive" className="text-[10px] px-1.5 py-0 uppercase font-mono">
                            DELETE
                          </Badge>
                        </div>
                      </div>
                    )}

                    {/* Encoding */}
                    <div>
                      <div className="text-[10px] uppercase font-semibold text-muted-foreground mb-0.5">
                        Encoding
                      </div>
                      <div className="font-mono text-xs uppercase font-medium text-foreground">
                        {normalizedEnc}
                      </div>
                    </div>

                    {/* Payload Size */}
                    <div>
                      <div className="text-[10px] uppercase font-semibold text-muted-foreground mb-0.5">
                        Payload Size
                      </div>
                      <div className="font-mono text-xs font-medium text-foreground">
                        {formatByteSize(payloadLength)}
                      </div>
                    </div>

                    {/* Timestamp */}
                    <div className="col-span-2">
                      <div className="text-[10px] uppercase font-semibold text-muted-foreground mb-0.5">
                        Timestamp
                      </div>
                      <div className="font-mono text-[11px] text-foreground flex items-center gap-1">
                        <Clock className="w-3 h-3 text-muted-foreground shrink-0" />
                        <span className="truncate" title={formatFullDateTime(data.timestamp)}>
                          {formatFullDateTime(data.timestamp)}
                        </span>
                      </div>
                    </div>

                    {/* Extra details if provided */}
                    {data.extraDetails?.map((extra, idx) => (
                      <div key={idx}>
                        <div className="text-[10px] uppercase font-semibold text-muted-foreground mb-0.5">
                          {extra.label}
                        </div>
                        <div className="font-mono text-xs text-foreground">{extra.value}</div>
                      </div>
                    ))}
                  </div>

                  {/* Query error message banner if reply had error */}
                  {data.isError && data.errorMessage && (
                    <div className="rounded bg-destructive/10 border border-destructive/20 p-2 text-xs text-destructive">
                      <span className="font-semibold">Error: </span>
                      {data.errorMessage}
                    </div>
                  )}
                </>
              )}
            </div>
          )}

          {/* Payload Viewer Area */}
          <div
            ref={payloadSectionRef}
            className={`p-3 flex flex-col space-y-1.5 ${
              inspectorView === 'payload'
                ? 'flex-1 min-h-0 h-full overflow-hidden'
                : 'flex-1 min-h-[420px] shrink-0'
            }`}
          >
            <div className="text-[10px] uppercase font-semibold text-muted-foreground mb-1.5 shrink-0 flex items-center justify-between">
              <span>Payload Content</span>
              {inspectorView === 'all' && (
                <span className="text-[10px] font-mono text-muted-foreground font-normal">
                  {formatByteSize(payloadLength)}
                </span>
              )}
            </div>
            <PayloadViewer
              payload={data.payload}
              encoding={normalizedEnc}
              keyExpr={data.keyExpr}
              protoTypeName={data.protoTypeName}
              showMetrics={true}
              maxHeight="100%"
              className="flex-1 min-h-0"
            />
          </div>
        </div>
      </div>
    </>
  );
};

export default MessageDetails;
