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

// Set up mock window and Tauri internals
let mockInvokeHandler: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> = async () => undefined;

// @ts-expect-error Mocking global window
globalThis.window = globalThis;
// @ts-expect-error Mocking tauri internals
globalThis.window.__TAURI_INTERNALS__ = {
  invoke: async (cmd: string, args?: Record<string, unknown>) => {
    return mockInvokeHandler(cmd, args);
  },
  transformCallback: (cb: unknown) => cb,
};

import {
  getMcpAgents,
  getMcpConfigJson,
  installMcpAgent,
  uninstallMcpAgent,
  installAllDetectedMcpAgents,
} from '../src/lib/tauri';
import type { AgentTarget } from '../src/types/zenoh';

describe('AI & Agents MCP IPC and Tab State Transitions', () => {
  const mockAgentsList: AgentTarget[] = [
    {
      id: 'antigravity',
      name: 'Antigravity CLI',
      detected: true,
      installed: false,
      config_path: '/home/user/.gemini/config/mcp_config.json',
      format: 'JsonMcpServers',
    },
    {
      id: 'claude',
      name: 'Claude Desktop',
      detected: false,
      installed: false,
      config_path: '/home/user/.config/Claude/claude_desktop_config.json',
      format: 'JsonMcpServers',
    },
    {
      id: 'cursor',
      name: 'Cursor IDE',
      detected: true,
      installed: true,
      config_path: '/home/user/.cursor/mcp.json',
      format: 'JsonMcpServers',
    },
    {
      id: 'zed',
      name: 'Zed Editor',
      detected: true,
      installed: false,
      config_path: '/home/user/.config/zed/settings.json',
      format: 'JsonContextServers',
    },
  ];

  beforeEach(() => {
    mockInvokeHandler = async () => undefined;
  });

  test('getMcpAgents invokes get_mcp_agents and returns agent list', async () => {
    let invokedCmd = '';
    mockInvokeHandler = async (cmd) => {
      invokedCmd = cmd;
      if (cmd === 'get_mcp_agents') {
        return [...mockAgentsList];
      }
      return undefined;
    };

    const agents = await getMcpAgents();
    assert.equal(invokedCmd, 'get_mcp_agents');
    assert.equal(agents.length, 4);
    assert.equal(agents[0].id, 'antigravity');
    assert.equal(agents[0].detected, true);
    assert.equal(agents[0].installed, false);
  });

  test('installMcpAgent invokes install_mcp_agent with agentId and updates state', async () => {
    let invokedCmd = '';
    let invokedArgs: Record<string, unknown> | undefined;

    mockInvokeHandler = async (cmd, args) => {
      invokedCmd = cmd;
      invokedArgs = args;
      if (cmd === 'install_mcp_agent') {
        const id = (args?.agentId || args?.agent_id) as string;
        const target = mockAgentsList.find((a) => a.id === id);
        if (!target) throw new Error('Unknown agent ID');
        return {
          ...target,
          installed: true,
        };
      }
      return undefined;
    };

    const updated = await installMcpAgent('antigravity');
    assert.equal(invokedCmd, 'install_mcp_agent');
    assert.equal(invokedArgs?.agentId, 'antigravity');
    assert.equal(updated.id, 'antigravity');
    assert.equal(updated.installed, true);
  });

  test('uninstallMcpAgent invokes uninstall_mcp_agent with agentId and updates state', async () => {
    let invokedCmd = '';
    let invokedArgs: Record<string, unknown> | undefined;

    mockInvokeHandler = async (cmd, args) => {
      invokedCmd = cmd;
      invokedArgs = args;
      if (cmd === 'uninstall_mcp_agent') {
        const id = (args?.agentId || args?.agent_id) as string;
        const target = mockAgentsList.find((a) => a.id === id);
        if (!target) throw new Error('Unknown agent ID');
        return {
          ...target,
          installed: false,
        };
      }
      return undefined;
    };

    const updated = await uninstallMcpAgent('cursor');
    assert.equal(invokedCmd, 'uninstall_mcp_agent');
    assert.equal(invokedArgs?.agentId, 'cursor');
    assert.equal(updated.id, 'cursor');
    assert.equal(updated.installed, false);
  });

  test('installAllDetectedMcpAgents invokes install_all_detected_mcp_agents', async () => {
    let invokedCmd = '';

    mockInvokeHandler = async (cmd) => {
      invokedCmd = cmd;
      if (cmd === 'install_all_detected_mcp_agents') {
        return mockAgentsList
          .filter((a) => a.detected && !a.installed)
          .map((a) => ({ ...a, installed: true }));
      }
      return undefined;
    };

    const results = await installAllDetectedMcpAgents();
    assert.equal(invokedCmd, 'install_all_detected_mcp_agents');
    assert.equal(results.length, 2);
    assert.ok(results.every((a) => a.installed));
    assert.deepEqual(
      results.map((a) => a.id).sort(),
      ['antigravity', 'zed']
    );
  });

  test('handles IPC rejection gracefully when command errors', async () => {
    mockInvokeHandler = async (cmd) => {
      if (cmd === 'install_mcp_agent') {
        throw new Error('Permission denied writing to config file');
      }
      return undefined;
    };

    await assert.rejects(
      async () => {
        await installMcpAgent('antigravity');
      },
      {
        message: /Permission denied/,
      }
    );
  });

  test('summary counts calculate accurately for agents list', () => {
    const total = mockAgentsList.length;
    const detected = mockAgentsList.filter((a) => a.detected).length;
    const installed = mockAgentsList.filter((a) => a.installed).length;
    const canInstallAll = mockAgentsList.some((a) => a.detected && !a.installed);

    assert.equal(total, 4);
    assert.equal(detected, 3);
    assert.equal(installed, 1);
    assert.equal(canInstallAll, true);
  });

  test('canInstallAll is false when all detected agents are already installed', () => {
    const list: AgentTarget[] = [
      {
        id: 'antigravity',
        name: 'Antigravity CLI',
        detected: true,
        installed: true,
        config_path: '/path/to/config.json',
        format: 'JsonMcpServers',
      },
      {
        id: 'claude',
        name: 'Claude Desktop',
        detected: false,
        installed: false,
        config_path: '/path/to/claude.json',
        format: 'JsonMcpServers',
      },
    ];

    const canInstallAll = list.some((a) => a.detected && !a.installed);
    assert.equal(canInstallAll, false);
  });

  test('handles uninstall command failure gracefully', async () => {
    mockInvokeHandler = async (cmd) => {
      if (cmd === 'uninstall_mcp_agent') {
        throw new Error('Failed to parse JSON file');
      }
      return undefined;
    };

    await assert.rejects(
      async () => {
        await uninstallMcpAgent('cursor');
      },
      {
        message: /Failed to parse JSON file/,
      }
    );
  });

  test('handles installAllDetectedMcpAgents failure gracefully', async () => {
    mockInvokeHandler = async (cmd) => {
      if (cmd === 'install_all_detected_mcp_agents') {
        throw new Error('Disk full');
      }
      return undefined;
    };

    await assert.rejects(
      async () => {
        await installAllDetectedMcpAgents();
      },
      {
        message: /Disk full/,
      }
    );
  });

  test('getMcpConfigJson invokes get_mcp_config_json and returns formatted JSON snippet', async () => {
    let invokedCmd = '';
    mockInvokeHandler = async (cmd) => {
      invokedCmd = cmd;
      if (cmd === 'get_mcp_config_json') {
        return JSON.stringify({
          mcpServers: {
            zenohx: {
              command: '/test/path/to/zenohx-mcp',
              args: [],
            },
          },
        });
      }
      return undefined;
    };

    const jsonStr = await getMcpConfigJson();
    assert.equal(invokedCmd, 'get_mcp_config_json');
    const parsed = JSON.parse(jsonStr);
    assert.ok(parsed.mcpServers.zenohx);
    assert.equal(parsed.mcpServers.zenohx.command, '/test/path/to/zenohx-mcp');
  });

  test('AgentsTab filters undetected agents and displays only detected ones', () => {
    // mockAgentsList: antigravity (detected=true), claude (detected=false), cursor (detected=true), zed (detected=true)
    const detectedOnly = mockAgentsList.filter((a) => a.detected || a.installed);
    assert.equal(detectedOnly.length, 3);
    assert.ok(!detectedOnly.some((a) => a.id === 'claude'));
    assert.ok(detectedOnly.some((a) => a.id === 'antigravity'));
    assert.ok(detectedOnly.some((a) => a.id === 'cursor'));
    assert.ok(detectedOnly.some((a) => a.id === 'zed'));
  });

  test('AgentsTab search filters detected agents by name or ID case-insensitively', () => {
    const detectedOnly = mockAgentsList.filter((a) => a.detected || a.installed);

    // Search "cursor"
    const searchCursor = detectedOnly.filter(
      (a) =>
        a.name.toLowerCase().includes('cursor') ||
        a.id.toLowerCase().includes('cursor')
    );
    assert.equal(searchCursor.length, 1);
    assert.equal(searchCursor[0].id, 'cursor');

    // Search "cli" -> matches "Antigravity CLI"
    const searchCli = detectedOnly.filter(
      (a) =>
        a.name.toLowerCase().includes('cli') ||
        a.id.toLowerCase().includes('cli')
    );
    assert.equal(searchCli.length, 1);
    assert.equal(searchCli[0].id, 'antigravity');

    // Search "nonexistent" -> matches 0
    const searchNone = detectedOnly.filter(
      (a) =>
        a.name.toLowerCase().includes('nonexistent') ||
        a.id.toLowerCase().includes('nonexistent')
    );
    assert.equal(searchNone.length, 0);
  });
});
