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

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');
const isWindows = process.platform === 'win32';
const binScript = path.resolve(rootDir, 'bin/zenohx.js');
const debugBin = path.resolve(
  rootDir,
  `src-tauri/target/debug/zenohx-mcp${isWindows ? '.exe' : ''}`
);

const SUPPORTED_AGENT_IDS = [
  'antigravity',
  'claude',
  'cursor',
  'windsurf',
  'cline',
  'roo-code',
  'zed',
  'codex',
  'hermes',
  'openclaw',
];

/**
 * Asserts that a string consists strictly of ASCII characters (codes 0-127).
 * Ensures zero emojis or non-ASCII characters appear in terminal outputs.
 */
function assertStrictAscii(str, contextName) {
  for (let i = 0; i < str.length; i++) {
    const code = str.charCodeAt(i);
    if (code > 127) {
      const char = str[i];
      assert.fail(
        `Non-ASCII character '${char}' (code 0x${code.toString(16)}) detected in ${contextName}`
      );
    }
  }
}

/**
 * Executes a zenohx-mcp CLI command via bin/zenohx.js with specified environment.
 */
function runMcpCli(args, customEnv = {}) {
  const env = {
    ...process.env,
    ...customEnv,
  };

  if (!env.ZENOHX_MCP_BIN && fs.existsSync(debugBin)) {
    env.ZENOHX_MCP_BIN = debugBin;
  }

  const stdout = execFileSync(
    process.execPath,
    [binScript, 'mcp', ...args],
    {
      cwd: rootDir,
      env,
      encoding: 'utf-8',
      stdio: ['pipe', 'pipe', 'pipe'],
    }
  );

  return stdout;
}

/**
 * Test 1: Verify `list-agents` CLI output format, headers, IDs, and zero emojis.
 */
function testListAgents() {
  console.log('[TEST 1] Verifying zenohx-mcp list-agents output...');

  const stdout = runMcpCli(['list-agents']);

  // 1. Verify headers
  assert.ok(stdout.includes('AGENT ID'), 'Output must contain "AGENT ID" header');
  assert.ok(stdout.includes('AGENT NAME'), 'Output must contain "AGENT NAME" header');
  assert.ok(stdout.includes('STATUS'), 'Output must contain "STATUS" header');
  assert.ok(stdout.includes('CONFIG PATH'), 'Output must contain "CONFIG PATH" header');

  // 2. Verify all 8 supported client IDs
  for (const agentId of SUPPORTED_AGENT_IDS) {
    assert.ok(
      stdout.includes(agentId),
      `Output must contain supported agent ID "${agentId}"`
    );
  }

  // 3. Strictly zero emojis (all ASCII)
  assertStrictAscii(stdout, 'list-agents output');

  console.log('[PASS] Test 1: list-agents headers, agent IDs, and strict ASCII verified.');
}

/**
 * Test 2: Verify isolated agent installation, backup creation, and safe uninstallation.
 */
function testIsolatedAgentLifecycle() {
  console.log('[TEST 2] Verifying isolated agent install, backup, and uninstall...');

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'zenohx-e2e-installer-'));
  console.log(`[INFO] Isolated test directory: ${tmpDir}`);

  try {
    const isolatedEnv = {
      HOME: tmpDir,
      USERPROFILE: tmpDir,
      XDG_CONFIG_HOME: path.join(tmpDir, '.config'),
      APPDATA: path.join(tmpDir, 'AppData', 'Roaming'),
      ZENOHX_MCP_BIN: fs.existsSync(debugBin) ? debugBin : (process.env.ZENOHX_MCP_BIN || ''),
    };

    // -------------------------------------------------------------------------
    // Subtest 2a: Single target installation (Cursor) with pre-existing config
    // -------------------------------------------------------------------------
    console.log('[INFO] Subtest 2a: Testing single agent (Cursor) lifecycle...');

    const cursorDir = path.join(tmpDir, '.cursor');
    fs.mkdirSync(cursorDir, { recursive: true });
    const cursorConfigPath = path.join(cursorDir, 'mcp.json');
    const cursorBackupPath = path.join(cursorDir, 'mcp.json.bak');

    const initialCursorConfig = {
      mcpServers: {
        'existing-cursor-tool': {
          command: 'echo',
          args: ['hello-cursor'],
        },
      },
    };
    fs.writeFileSync(cursorConfigPath, JSON.stringify(initialCursorConfig, null, 2), 'utf-8');

    assert.ok(
      !fs.existsSync(cursorBackupPath),
      'Backup file must not exist before install'
    );

    // Run install cursor
    const installCursorOut = runMcpCli(['install', 'cursor'], isolatedEnv);
    assertStrictAscii(installCursorOut, 'install cursor output');
    assert.ok(
      installCursorOut.includes('[OK] Installed to Cursor IDE'),
      'Output must confirm Cursor installation'
    );

    // Verify backup file was created and retains original content
    assert.ok(
      fs.existsSync(cursorBackupPath),
      'Backup file mcp.json.bak must exist after install'
    );
    const cursorBackupContent = JSON.parse(fs.readFileSync(cursorBackupPath, 'utf-8'));
    assert.ok(
      cursorBackupContent.mcpServers['existing-cursor-tool'],
      'Backup must retain pre-existing tool'
    );
    assert.strictEqual(
      cursorBackupContent.mcpServers['zenohx'],
      undefined,
      'Backup must not contain zenohx entry'
    );

    // Verify updated configuration has zenohx and retains existing tool
    const postInstallCursorConfig = JSON.parse(fs.readFileSync(cursorConfigPath, 'utf-8'));
    assert.ok(
      postInstallCursorConfig.mcpServers['zenohx'],
      'Configuration must contain zenohx entry'
    );
    assert.ok(
      postInstallCursorConfig.mcpServers['existing-cursor-tool'],
      'Configuration must preserve pre-existing tool'
    );

    // Run uninstall cursor
    const uninstallCursorOut = runMcpCli(['uninstall', 'cursor'], isolatedEnv);
    assertStrictAscii(uninstallCursorOut, 'uninstall cursor output');
    assert.ok(
      uninstallCursorOut.includes('[OK] Uninstalled from Cursor IDE'),
      'Output must confirm Cursor uninstallation'
    );

    // Verify zenohx entry was removed and pre-existing tool remains
    const postUninstallCursorConfig = JSON.parse(fs.readFileSync(cursorConfigPath, 'utf-8'));
    assert.strictEqual(
      postUninstallCursorConfig.mcpServers['zenohx'],
      undefined,
      'Configuration must no longer contain zenohx entry'
    );
    assert.ok(
      postUninstallCursorConfig.mcpServers['existing-cursor-tool'],
      'Configuration must still preserve pre-existing tool after uninstall'
    );

    console.log('[PASS] Subtest 2a: Single agent install, backup, and uninstall verified.');

    // -------------------------------------------------------------------------
    // Subtest 2b: Multi-target installation (--all) across JSON and TOML agents
    // -------------------------------------------------------------------------
    console.log('[INFO] Subtest 2b: Testing multi-agent (--all) lifecycle...');

    // Setup Antigravity config (JSON mcpServers)
    const geminiDir = path.join(tmpDir, '.gemini', 'config');
    fs.mkdirSync(geminiDir, { recursive: true });
    const geminiConfigPath = path.join(geminiDir, 'mcp_config.json');
    const geminiBackupPath = path.join(geminiDir, 'mcp_config.json.bak');
    fs.writeFileSync(
      geminiConfigPath,
      JSON.stringify({ mcpServers: { 'anti-server': { command: 'node' } } }, null, 2),
      'utf-8'
    );

    // Setup Zed config (JSON context_servers)
    const zedDir = isWindows
      ? path.join(tmpDir, 'AppData', 'Roaming', 'Zed')
      : path.join(tmpDir, '.config', 'zed');
    fs.mkdirSync(zedDir, { recursive: true });
    const zedConfigPath = path.join(zedDir, 'settings.json');
    const zedBackupPath = path.join(zedDir, 'settings.json.bak');
    fs.writeFileSync(
      zedConfigPath,
      JSON.stringify({ context_servers: { 'zed-server': { command: 'zed-mcp' } } }, null, 2),
      'utf-8'
    );

    // Setup Codex config (TOML [mcp_servers])
    const codexDir = path.join(tmpDir, '.codex');
    fs.mkdirSync(codexDir, { recursive: true });
    const codexConfigPath = path.join(codexDir, 'config.toml');
    const codexBackupPath = path.join(codexDir, 'config.toml.bak');
    const initialCodexToml = '[mcp_servers.existing_tool]\ncommand = "codex-tool"\n';
    fs.writeFileSync(codexConfigPath, initialCodexToml, 'utf-8');

    // Run install --all
    const installAllOut = runMcpCli(['install', '--all'], isolatedEnv);
    assertStrictAscii(installAllOut, 'install --all output');
    assert.ok(installAllOut.includes('[OK] Installed to Antigravity CLI'));
    assert.ok(installAllOut.includes('[OK] Installed to Zed Editor'));
    assert.ok(installAllOut.includes('[OK] Installed to Codex CLI'));

    // Verify backups created for all three
    assert.ok(fs.existsSync(geminiBackupPath), 'Antigravity backup must exist');
    assert.ok(fs.existsSync(zedBackupPath), 'Zed backup must exist');
    assert.ok(fs.existsSync(codexBackupPath), 'Codex backup must exist');

    // Verify configs mutated with zenohx
    const geminiJson = JSON.parse(fs.readFileSync(geminiConfigPath, 'utf-8'));
    assert.ok(geminiJson.mcpServers['zenohx'], 'Antigravity must contain zenohx');
    assert.ok(geminiJson.mcpServers['anti-server'], 'Antigravity must retain anti-server');

    const zedJson = JSON.parse(fs.readFileSync(zedConfigPath, 'utf-8'));
    assert.ok(zedJson.context_servers['zenohx'], 'Zed must contain zenohx');
    assert.ok(zedJson.context_servers['zed-server'], 'Zed must retain zed-server');

    const codexToml = fs.readFileSync(codexConfigPath, 'utf-8');
    assert.ok(codexToml.includes('zenohx'), 'Codex TOML must contain zenohx');
    assert.ok(codexToml.includes('existing_tool'), 'Codex TOML must retain existing_tool');

    // Run uninstall --all
    const uninstallAllOut = runMcpCli(['uninstall', '--all'], isolatedEnv);
    assertStrictAscii(uninstallAllOut, 'uninstall --all output');
    assert.ok(uninstallAllOut.includes('[OK] Uninstalled from Antigravity CLI'));
    assert.ok(uninstallAllOut.includes('[OK] Uninstalled from Zed Editor'));
    assert.ok(uninstallAllOut.includes('[OK] Uninstalled from Codex CLI'));

    // Verify zenohx removed while existing configs preserved
    const postGeminiJson = JSON.parse(fs.readFileSync(geminiConfigPath, 'utf-8'));
    assert.strictEqual(postGeminiJson.mcpServers['zenohx'], undefined);
    assert.ok(postGeminiJson.mcpServers['anti-server']);

    const postZedJson = JSON.parse(fs.readFileSync(zedConfigPath, 'utf-8'));
    assert.strictEqual(postZedJson.context_servers['zenohx'], undefined);
    assert.ok(postZedJson.context_servers['zed-server']);

    const postCodexToml = fs.readFileSync(codexConfigPath, 'utf-8');
    assert.ok(!postCodexToml.includes('zenohx'), 'Codex TOML must not contain zenohx');
    assert.ok(postCodexToml.includes('existing_tool'), 'Codex TOML must retain existing_tool');

    console.log('[PASS] Subtest 2b: Multi-agent install (--all) and uninstall verified.');
    console.log('[PASS] Test 2: Isolated agent lifecycle passed successfully.');
  } finally {
    // Clean up temporary test directory
    try {
      fs.rmSync(tmpDir, { recursive: true, force: true });
      console.log(`[INFO] Cleaned up temporary directory: ${tmpDir}`);
    } catch (err) {
      console.warn(`[WARN] Failed to clean up ${tmpDir}: ${err.message}`);
    }
  }
}

function main() {
  console.log('=== Starting ZenohX MCP Installer E2E Integration Test Suite ===');
  try {
    testListAgents();
    testIsolatedAgentLifecycle();
    console.log('=== ZenohX MCP Installer E2E Integration Test Suite PASSED ===');
    process.exit(0);
  } catch (err) {
    console.error(`[FAIL] E2E Test failed: ${err.message}`);
    if (err.stack) {
      console.error(err.stack);
    }
    process.exit(1);
  }
}

main();
