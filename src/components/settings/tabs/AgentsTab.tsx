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

import React, { useState, useEffect, useCallback, useMemo } from 'react';
import {
  Bot,
  RefreshCw,
  Download,
  Trash2,
  Copy,
  Check,
  CheckCircle2,
  AlertCircle,
  Loader2,
  Search,
  X,
} from 'lucide-react';
import { Button } from '../../ui/button';
import { Badge } from '../../ui/badge';
import { SimpleTooltip } from '../../ui/tooltip';
import {
  getMcpAgents,
  getMcpConfigJson,
  installMcpAgent,
  uninstallMcpAgent,
  installAllDetectedMcpAgents,
} from '../../../lib/tauri';
import type { AgentTarget } from '../../../types/zenoh';

export interface AgentsTabProps {
  className?: string;
}

export const AgentsTab: React.FC<AgentsTabProps> = ({ className = '' }) => {
  const [agents, setAgents] = useState<AgentTarget[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [operatingAgentId, setOperatingAgentId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [isJsonCopied, setIsJsonCopied] = useState<boolean>(false);
  const [searchQuery, setSearchQuery] = useState<string>('');

  const loadAgents = useCallback(async () => {
    setIsLoading(true);
    setErrorMessage(null);
    try {
      const list = await getMcpAgents();
      setAgents(list);
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    loadAgents();
  }, [loadAgents]);

  const totalCount = agents.length;
  // Display only detected agents (no display the undetected)
  const detectedAgents = useMemo(
    () => agents.filter((a) => a.detected || a.installed),
    [agents]
  );
  const detectedCount = detectedAgents.length;
  const installedCount = useMemo(() => agents.filter((a) => a.installed).length, [agents]);
  const canInstallAll = useMemo(
    () => detectedAgents.some((a) => !a.installed),
    [detectedAgents]
  );

  const filteredAgents = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return detectedAgents;
    return detectedAgents.filter(
      (a) =>
        a.name.toLowerCase().includes(q) ||
        a.id.toLowerCase().includes(q) ||
        a.config_path.toLowerCase().includes(q)
    );
  }, [detectedAgents, searchQuery]);

  const handleInstall = async (agentId: string) => {
    setOperatingAgentId(agentId);
    setErrorMessage(null);
    try {
      const updated = await installMcpAgent(agentId);
      setAgents((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setOperatingAgentId(null);
    }
  };

  const handleUninstall = async (agentId: string) => {
    setOperatingAgentId(agentId);
    setErrorMessage(null);
    try {
      const updated = await uninstallMcpAgent(agentId);
      setAgents((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setOperatingAgentId(null);
    }
  };

  const handleInstallAll = async () => {
    setOperatingAgentId('all');
    setErrorMessage(null);
    try {
      await installAllDetectedMcpAgents();
      await loadAgents();
    } catch (err) {
      setErrorMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setOperatingAgentId(null);
    }
  };

  const handleCopyJson = async () => {
    try {
      const snippet = await getMcpConfigJson();
      if (typeof navigator !== 'undefined' && navigator.clipboard) {
        await navigator.clipboard.writeText(snippet);
        setIsJsonCopied(true);
        setTimeout(() => {
          setIsJsonCopied(false);
        }, 2000);
      }
    } catch (err) {
      console.error('Failed to copy MCP config JSON:', err);
    }
  };

  const handleCopyPath = async (agentId: string, path: string) => {
    try {
      if (typeof navigator !== 'undefined' && navigator.clipboard) {
        await navigator.clipboard.writeText(path);
        setCopiedId(agentId);
        setTimeout(() => {
          setCopiedId((curr) => (curr === agentId ? null : curr));
        }, 2000);
      }
    } catch (err) {
      console.error('Failed to copy path:', err);
    }
  };

  const formatConfigType = (format: AgentTarget['format']): string => {
    switch (format) {
      case 'JsonMcpServers':
        return 'JSON (mcpServers)';
      case 'JsonContextServers':
        return 'JSON (context_servers)';
      case 'TomlMcpServers':
        return 'TOML ([mcp_servers])';
      default:
        return format;
    }
  };

  return (
    <div className={`max-w-4xl mx-auto p-6 space-y-6 ${className}`}>
      {/* Header section */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h3 className="text-sm font-semibold text-foreground flex items-center gap-2">
            <Bot className="w-4 h-4 text-primary" />
            AI & Agents MCP Integration
          </h3>
          <p className="text-xs text-muted-foreground mt-0.5">
            Configure ZenohX MCP server across supported AI coding assistants and CLI agents.
          </p>
        </div>

        {/* Global Action Buttons */}
        <div className="flex items-center gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={loadAgents}
            disabled={isLoading || Boolean(operatingAgentId)}
            className="h-8 text-xs gap-1.5"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? 'animate-spin' : ''}`} />
            <span>Refresh</span>
          </Button>

          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={handleCopyJson}
            disabled={isLoading}
            className="h-8 text-xs gap-1.5"
            title="Copy standard mcpServers configuration JSON for manual setup"
          >
            {isJsonCopied ? (
              <Check className="w-3.5 h-3.5 text-emerald-500" />
            ) : (
              <Copy className="w-3.5 h-3.5" />
            )}
            <span>{isJsonCopied ? 'Copied JSON' : 'Copy as JSON'}</span>
          </Button>

          <Button
            type="button"
            variant="default"
            size="sm"
            onClick={handleInstallAll}
            disabled={isLoading || !canInstallAll || Boolean(operatingAgentId)}
            className="h-8 text-xs gap-1.5"
          >
            {operatingAgentId === 'all' ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Download className="w-3.5 h-3.5" />
            )}
            <span>Install All Detected</span>
          </Button>
        </div>
      </div>

      {/* Status Summary Banner */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <div className="rounded-lg border bg-card p-3 shadow-xs">
          <div className="text-xs text-muted-foreground font-medium">Supported Agents</div>
          <div className="text-xl font-bold text-foreground mt-1">
            {isLoading ? '-' : totalCount}
          </div>
        </div>
        <div className="rounded-lg border bg-card p-3 shadow-xs">
          <div className="text-xs text-muted-foreground font-medium">Detected on System</div>
          <div className="text-xl font-bold text-blue-600 dark:text-blue-400 mt-1">
            {isLoading ? '-' : detectedCount}
          </div>
        </div>
        <div className="rounded-lg border bg-card p-3 shadow-xs">
          <div className="text-xs text-muted-foreground font-medium">ZenohX MCP Installed</div>
          <div className="text-xl font-bold text-emerald-600 dark:text-emerald-400 mt-1">
            {isLoading ? '-' : installedCount}
          </div>
        </div>
      </div>

      {/* Error Message Display */}
      {errorMessage && (
        <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-xs text-destructive flex items-start gap-2">
          <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
          <div className="flex-1 break-words">{errorMessage}</div>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => setErrorMessage(null)}
            className="h-5 px-1.5 text-xs text-destructive hover:bg-destructive/20"
          >
            Dismiss
          </Button>
        </div>
      )}

      {/* Agents List Card */}
      <div className="rounded-xl border bg-card shadow-xs overflow-hidden">
        <div className="p-3.5 sm:p-4 border-b bg-muted/20 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="text-xs font-semibold text-foreground uppercase tracking-wider">
              Supported AI Targets
            </span>
            <Badge variant="secondary" className="text-[10px] px-1.5 py-0 font-normal">
              {installedCount} of {detectedCount} installed
            </Badge>
          </div>

          {/* Search Input on the right */}
          <div className="relative w-full sm:w-56">
            <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search detected agents..."
              className="w-full h-7 pl-8 pr-7 text-xs rounded-md border bg-background text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-primary transition-all"
            />
            {searchQuery && (
              <button
                type="button"
                onClick={() => setSearchQuery('')}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground p-0.5"
                title="Clear search"
              >
                <X className="w-3 h-3" />
              </button>
            )}
          </div>
        </div>

        {isLoading ? (
          <div className="p-8 flex flex-col items-center justify-center gap-2 text-muted-foreground">
            <Loader2 className="w-6 h-6 animate-spin text-primary" />
            <span className="text-xs">Scanning AI agent configurations...</span>
          </div>
        ) : detectedAgents.length === 0 ? (
          <div className="p-8 text-center text-xs text-muted-foreground">
            No supported AI agents currently detected on this system.
          </div>
        ) : filteredAgents.length === 0 ? (
          <div className="p-8 text-center text-xs text-muted-foreground">
            No detected agents matching &quot;{searchQuery}&quot;.
          </div>
        ) : (
          <div className="divide-y">
            {filteredAgents.map((agent) => {
              const isOperating = operatingAgentId === agent.id;
              const isCopied = copiedId === agent.id;

              return (
                <div
                  key={agent.id}
                  className="p-4 flex flex-col gap-3 hover:bg-muted/10 transition-colors"
                >
                  {/* Top Row: Info, Badges, and Action Button */}
                  <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
                    <div className="flex items-center gap-2 flex-wrap">
                      <span className="text-xs font-semibold text-foreground">
                        {agent.name}
                      </span>
                      <Badge variant="outline" className="text-[10px] font-mono px-1.5 py-0">
                        {agent.id}
                      </Badge>
                      <Badge variant="secondary" className="text-[10px] px-1.5 py-0">
                        {formatConfigType(agent.format)}
                      </Badge>

                      {/* Status Badges */}
                      {agent.installed ? (
                        <span className="inline-flex items-center gap-1 rounded-md border border-emerald-500/20 bg-emerald-500/10 px-2 py-0.5 text-xs font-medium text-emerald-600 dark:text-emerald-400">
                          <CheckCircle2 className="w-3 h-3" />
                          Installed
                        </span>
                      ) : agent.detected ? (
                        <span className="inline-flex items-center rounded-md border border-blue-500/20 bg-blue-500/10 px-2 py-0.5 text-xs font-medium text-blue-600 dark:text-blue-400">
                          Detected
                        </span>
                      ) : (
                        <span className="inline-flex items-center rounded-md border border-muted bg-muted/60 px-2 py-0.5 text-xs font-medium text-muted-foreground">
                          Not Detected
                        </span>
                      )}
                    </div>

                    {/* Action Button */}
                    <div className="flex items-center gap-2 self-start sm:self-auto">
                      {agent.installed ? (
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => handleUninstall(agent.id)}
                          disabled={Boolean(operatingAgentId)}
                          className="h-7 px-2.5 text-xs text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30 gap-1"
                        >
                          {isOperating ? (
                            <Loader2 className="w-3 h-3 animate-spin" />
                          ) : (
                            <Trash2 className="w-3 h-3" />
                          )}
                          <span>Uninstall</span>
                        </Button>
                      ) : (
                        <Button
                          type="button"
                          variant="default"
                          size="sm"
                          onClick={() => handleInstall(agent.id)}
                          disabled={Boolean(operatingAgentId)}
                          className="h-7 px-2.5 text-xs gap-1"
                        >
                          {isOperating ? (
                            <Loader2 className="w-3 h-3 animate-spin" />
                          ) : (
                            <Download className="w-3 h-3" />
                          )}
                          <span>Install</span>
                        </Button>
                      )}
                    </div>
                  </div>

                  {/* Config Path Row */}
                  <div className="flex items-center gap-2">
                    <span className="text-[11px] text-muted-foreground shrink-0 select-none">
                      Config:
                    </span>
                    <div
                      className="font-mono text-[11px] bg-muted/50 border rounded px-2 py-1 truncate flex-1 text-muted-foreground select-all"
                      title={agent.config_path}
                    >
                      {agent.config_path}
                    </div>
                    <SimpleTooltip content={isCopied ? 'Copied' : 'Copy Path'}>
                      <Button
                        type="button"
                        variant="ghost"
                        size="iconSm"
                        onClick={() => handleCopyPath(agent.id, agent.config_path)}
                        className="h-7 w-7 shrink-0 text-muted-foreground hover:text-foreground"
                      >
                        {isCopied ? (
                          <Check className="w-3.5 h-3.5 text-emerald-500" />
                        ) : (
                          <Copy className="w-3.5 h-3.5" />
                        )}
                      </Button>
                    </SimpleTooltip>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Informational Footer */}
      <div className="rounded-lg border bg-muted/20 p-3 text-xs text-muted-foreground flex items-start gap-2.5">
        <AlertCircle className="w-4 h-4 shrink-0 mt-0.5 text-muted-foreground" />
        <span>
          Note: After installing or uninstalling an MCP configuration, please restart or reload the
          corresponding AI assistant for the changes to take effect.
        </span>
      </div>
    </div>
  );
};

export default AgentsTab;
