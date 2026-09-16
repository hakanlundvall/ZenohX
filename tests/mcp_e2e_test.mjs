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

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');

const EXPECTED_TOOLS = [
  'zenoh_scout',
  'zenoh_connect_session',
  'zenoh_disconnect_session',
  'zenoh_get_sessions',
  'zenoh_get_profiles',
  'zenoh_create_profile',
  'zenoh_edit_profile',
  'zenoh_publish',
  'zenoh_subscribe',
  'zenoh_unsubscribe',
  'zenoh_get_messages',
  'zenoh_query',
  'zenoh_declare_queryable',
  'zenoh_inspect_topology',
  'zenohx_gui_switch_workspace',
  'zenohx_gui_get_state',
];

const EXPECTED_RESOURCES = [
  'zenohx://sessions',
  'zenohx://profiles',
  'zenohx://messages/recent',
  'zenohx://topology',
];

async function runMcpE2ETest() {
  console.log('--- Starting ZenohX MCP stdio E2E Integration Test ---');

  const debugBin = path.resolve(rootDir, 'src-tauri/target/debug/zenohx-mcp');
  const targetBin = process.env.ZENOHX_MCP_BIN || (!process.env.FORCE_CARGO_RUN && fs.existsSync(debugBin) ? debugBin : null);

  const [cmd, args] = targetBin
    ? [targetBin, []]
    : ['cargo', ['run', '--manifest-path', path.join(rootDir, 'src-tauri/Cargo.toml'), '--bin', 'zenohx-mcp']];

  console.log(`Spawning MCP process: ${cmd} ${args.join(' ')}`);

  const proc = spawn(cmd, args, {
    cwd: rootDir,
    stdio: ['pipe', 'pipe', 'inherit'],
  });

  const state = {
    receivedInit: false,
    receivedToolsList: false,
    receivedResourcesList: false,
    receivedScoutCall: false,
    receivedGetSessionsCall: false,
  };

  function send(msg) {
    const jsonStr = JSON.stringify(msg);
    proc.stdin.write(jsonStr + '\n');
  }

  const testTimeout = setTimeout(() => {
    console.error('Test timed out after 20 seconds!');
    proc.kill('SIGKILL');
    process.exit(1);
  }, 20000);

  let stdoutBuffer = '';

  const completionPromise = new Promise((resolve, reject) => {
    proc.on('error', (err) => {
      clearTimeout(testTimeout);
      reject(new Error(`Process spawn error: ${err.message}`));
    });

    proc.on('close', (code, signal) => {
      clearTimeout(testTimeout);
      console.log(`MCP process closed (exit code: ${code}, signal: ${signal})`);
      if (code !== 0 && code !== null) {
        reject(new Error(`Process exited with non-zero code ${code}`));
      } else {
        resolve();
      }
    });

    proc.stdout.on('data', (chunk) => {
      stdoutBuffer += chunk.toString();
      const lines = stdoutBuffer.split('\n');
      stdoutBuffer = lines.pop() ?? '';

      for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed) continue;

        try {
          const resp = JSON.parse(trimmed);
          handleResponse(resp);
        } catch (e) {
          clearTimeout(testTimeout);
          proc.kill();
          reject(new Error(`Failed to parse MCP stdout line as JSON: ${e.message}\nRaw line: ${trimmed}`));
          return;
        }
      }
    });

    function handleResponse(resp) {
      assert.equal(resp.jsonrpc, '2.0', `Expected JSON-RPC 2.0, got ${resp.jsonrpc}`);

      if (resp.error) {
        clearTimeout(testTimeout);
        proc.kill();
        reject(new Error(`MCP returned error response for id ${resp.id}: [${resp.error.code}] ${resp.error.message}`));
        return;
      }

      // 1. Handshake response
      if (resp.id === 1) {
        console.log('✔ Step 1: Received valid "initialize" response');
        assert.equal(resp.result?.serverInfo?.name, 'zenohx-mcp');
        assert.ok(typeof resp.result?.serverInfo?.version === 'string');
        assert.equal(resp.result?.protocolVersion, '2024-11-05');
        assert.ok(resp.result?.capabilities?.tools !== undefined);
        assert.ok(resp.result?.capabilities?.resources !== undefined);
        state.receivedInit = true;

        // Send notifications/initialized notification
        send({ jsonrpc: '2.0', method: 'notifications/initialized' });

        // Next: request tools/list
        send({ jsonrpc: '2.0', id: 2, method: 'tools/list' });
      }
      // 2. tools/list response
      else if (resp.id === 2) {
        console.log('✔ Step 2: Received valid "tools/list" response');
        assert.ok(Array.isArray(resp.result?.tools), 'tools must be an array');
        const tools = resp.result.tools;
        assert.equal(tools.length, EXPECTED_TOOLS.length, `Expected ${EXPECTED_TOOLS.length} tools, got ${tools.length}`);

        const toolNames = tools.map((t) => t.name);
        for (const expected of EXPECTED_TOOLS) {
          assert.ok(toolNames.includes(expected), `Expected tool '${expected}' not found in tools/list`);
        }

        for (const tool of tools) {
          assert.ok(typeof tool.name === 'string' && tool.name.length > 0, 'Tool name must be non-empty string');
          assert.ok(typeof tool.description === 'string', `Tool ${tool.name} missing description`);
          assert.ok(typeof tool.inputSchema === 'object' && tool.inputSchema !== null, `Tool ${tool.name} missing inputSchema`);
        }
        state.receivedToolsList = true;

        // Next: request resources/list
        send({ jsonrpc: '2.0', id: 3, method: 'resources/list' });
      }
      // 3. resources/list response
      else if (resp.id === 3) {
        console.log('✔ Step 3: Received valid "resources/list" response');
        assert.ok(Array.isArray(resp.result?.resources), 'resources must be an array');
        const resources = resp.result.resources;
        assert.equal(resources.length, 4, `Expected 4 resources, got ${resources.length}`);

        const resourceUris = resources.map((r) => r.uri);
        for (const expected of EXPECTED_RESOURCES) {
          assert.ok(resourceUris.includes(expected), `Expected resource '${expected}' not found in resources/list`);
        }

        for (const res of resources) {
          assert.ok(typeof res.uri === 'string', 'Resource missing uri');
          assert.ok(typeof res.name === 'string', 'Resource missing name');
          assert.ok(typeof res.description === 'string', 'Resource missing description');
          assert.equal(res.mimeType, 'application/json', 'Resource mimeType should be application/json');
        }
        state.receivedResourcesList = true;

        // Next: call zenoh_scout
        send({
          jsonrpc: '2.0',
          id: 4,
          method: 'tools/call',
          params: {
            name: 'zenoh_scout',
            arguments: { timeout_ms: 200 },
          },
        });
      }
      // 4. tools/call zenoh_scout response
      else if (resp.id === 4) {
        console.log('✔ Step 4: Received valid "zenoh_scout" call response');
        assert.equal(resp.result?.isError, false, 'zenoh_scout returned error');
        assert.ok(Array.isArray(resp.result?.content), 'content must be an array');
        assert.equal(resp.result.content[0]?.type, 'text');
        const text = resp.result.content[0]?.text;
        assert.ok(typeof text === 'string', 'content text must be string');
        assert.ok(text.includes('[Mode:'), 'Expected execution mode header in output');

        // Extract and verify scout JSON array payload
        const lines = text.split('\n');
        const jsonContent = lines.slice(1).join('\n').trim();
        const nodes = JSON.parse(jsonContent);
        assert.ok(Array.isArray(nodes), 'Parsed scout result must be an array');
        state.receivedScoutCall = true;

        // Next: call zenoh_get_sessions
        send({
          jsonrpc: '2.0',
          id: 5,
          method: 'tools/call',
          params: {
            name: 'zenoh_get_sessions',
            arguments: {},
          },
        });
      }
      // 5. tools/call zenoh_get_sessions response
      else if (resp.id === 5) {
        console.log('✔ Step 5: Received valid "zenoh_get_sessions" call response');
        assert.equal(resp.result?.isError, false, 'zenoh_get_sessions returned error');
        assert.ok(Array.isArray(resp.result?.content), 'content must be an array');
        assert.equal(resp.result.content[0]?.type, 'text');
        const text = resp.result.content[0]?.text;
        assert.ok(typeof text === 'string', 'content text must be string');
        assert.ok(text.includes('[Mode:'), 'Expected execution mode header in output');

        // Extract and verify session JSON list payload
        const lines = text.split('\n');
        const jsonContent = lines.slice(1).join('\n').trim();
        const sessions = JSON.parse(jsonContent);
        assert.ok(Array.isArray(sessions), 'Parsed sessions must be an array');
        state.receivedGetSessionsCall = true;

        console.log('All MCP verification assertions passed! Initiating graceful shutdown...');
        // Graceful shutdown by closing stdin
        proc.stdin.end();

        // Safety timeout to ensure process terminates after stdin closes
        setTimeout(() => {
          if (!proc.killed) {
            proc.kill();
          }
        }, 2000).unref();
      }
    }
  });

  // Kick off handshake
  send({
    jsonrpc: '2.0',
    id: 1,
    method: 'initialize',
    params: {
      protocolVersion: '2024-11-05',
      capabilities: {},
      clientInfo: { name: 'zenohx-e2e-test', version: '1.0.0' },
    },
  });

  await completionPromise;

  assert.ok(state.receivedInit, 'Failed to verify initialize');
  assert.ok(state.receivedToolsList, 'Failed to verify tools/list');
  assert.ok(state.receivedResourcesList, 'Failed to verify resources/list');
  assert.ok(state.receivedScoutCall, 'Failed to verify zenoh_scout call');
  assert.ok(state.receivedGetSessionsCall, 'Failed to verify zenoh_get_sessions call');

  console.log('=== ZenohX MCP stdio E2E Integration Test PASSED ===');
}

runMcpE2ETest().catch((err) => {
  console.error('MCP E2E test failed:', err);
  process.exit(1);
});
