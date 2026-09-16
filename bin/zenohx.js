#!/usr/bin/env node

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
 * ZenohX NPM Launcher CLI
 * Enables running `npx zenohx` to download and launch the native ZenohX desktop application.
 */

import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import https from 'node:https';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const pkg = JSON.parse(fs.readFileSync(path.resolve(__dirname, '../package.json'), 'utf-8'));
const VERSION = pkg.version || '0.1.1';
const REPO = 'khanhdew/ZenohX';

function getBinaryInfo() {
  const platform = os.platform();
  const arch = os.arch();

  if (platform === 'linux') {
    return {
      name: 'zenohx',
      assetName: 'zenohx_amd64.AppImage',
      isAppImage: true,
    };
  } else if (platform === 'darwin') {
    return {
      name: 'ZenohX.app',
      assetName: arch === 'arm64' ? 'ZenohX_aarch64.app.tar.gz' : 'ZenohX_x64.app.tar.gz',
      isMacTar: true,
    };
  } else if (platform === 'win32') {
    return {
      name: 'ZenohX.exe',
      assetName: 'ZenohX_x64_en-US.msi.zip',
      isWinZip: true,
    };
  } else {
    throw new Error(`Unsupported OS platform: ${platform}`);
  }
}

function getCacheDir() {
  const home = os.homedir();
  const cacheDir = path.join(home, '.zenohx', `v${VERSION}`);
  if (!fs.existsSync(cacheDir)) {
    fs.mkdirSync(cacheDir, { recursive: true });
  }
  return cacheDir;
}

function downloadFile(url, destPath) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(destPath);
    https
      .get(url, { headers: { 'User-Agent': 'zenohx-npm-launcher' } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          file.close();
          fs.unlinkSync(destPath);
          return downloadFile(res.headers.location, destPath).then(resolve).catch(reject);
        }

        if (res.statusCode !== 200) {
          file.close();
          fs.unlinkSync(destPath);
          return reject(new Error(`Failed to download binary: HTTP ${res.statusCode} from ${url}`));
        }

        res.pipe(file);
        file.on('finish', () => {
          file.close(resolve);
        });
      })
      .on('error', (err) => {
        file.close();
        if (fs.existsSync(destPath)) fs.unlinkSync(destPath);
        reject(err);
      });
  });
}

function findMcpBinary() {
  const isWin = os.platform() === 'win32';
  const binName = isWin ? 'zenohx-mcp.exe' : 'zenohx-mcp';

  if (process.env.ZENOHX_MCP_BIN && fs.existsSync(process.env.ZENOHX_MCP_BIN)) {
    return process.env.ZENOHX_MCP_BIN;
  }

  const homeBin = path.join(os.homedir(), '.zenohx', 'bin', binName);
  if (fs.existsSync(homeBin)) {
    return homeBin;
  }

  const releaseBin = path.resolve(__dirname, '../src-tauri/target/release', binName);
  if (fs.existsSync(releaseBin)) {
    return releaseBin;
  }

  const debugBin = path.resolve(__dirname, '../src-tauri/target/debug', binName);
  if (fs.existsSync(debugBin)) {
    return debugBin;
  }

  return null;
}

function handleMcpCommand() {
  const mcpArgs = process.argv.slice(3);
  const mcpBin = findMcpBinary();

  if (mcpBin) {
    const child = spawn(mcpBin, mcpArgs, { stdio: 'inherit' });
    child.on('exit', (code) => {
      process.exit(code !== null ? code : 0);
    });
    child.on('error', (err) => {
      console.error(`[ERROR] Failed to run zenohx-mcp: ${err.message}`);
      process.exit(1);
    });
    return;
  }

  const cargoManifest = path.resolve(__dirname, '../src-tauri/Cargo.toml');
  if (fs.existsSync(cargoManifest)) {
    const cargoArgs = [
      'run',
      '--manifest-path',
      cargoManifest,
      '--bin',
      'zenohx-mcp',
      '--',
      ...mcpArgs,
    ];
    const child = spawn('cargo', cargoArgs, { stdio: 'inherit' });
    child.on('exit', (code) => {
      process.exit(code !== null ? code : 0);
    });
    child.on('error', (err) => {
      console.error(`[ERROR] Failed to run zenohx-mcp via cargo: ${err.message}`);
      process.exit(1);
    });
    return;
  }

  console.error('[ERROR] zenohx-mcp binary not found.');
  console.error('Please ensure ZenohX is installed at ~/.zenohx/bin/zenohx-mcp or build from source.');
  process.exit(1);
}

async function main() {
  if (process.argv[2] === 'mcp') {
    return handleMcpCommand();
  }

  try {
    const info = getBinaryInfo();
    const cacheDir = getCacheDir();
    const targetPath = path.join(cacheDir, info.assetName);

    console.log(`\x1b[36m[ZenohX]\x1b[0m Launching ZenohX v${VERSION} for ${os.platform()}-${os.arch()}...`);

    if (!fs.existsSync(targetPath)) {
      const downloadUrl = `https://github.com/${REPO}/releases/download/v${VERSION}/${info.assetName}`;
      console.log(`\x1b[33m[ZenohX]\x1b[0m Downloading binary from GitHub Releases...`);
      console.log(`         ${downloadUrl}`);
      await downloadFile(downloadUrl, targetPath);

      if (info.isAppImage) {
        fs.chmodSync(targetPath, 0o755);
      }
      console.log(`\x1b[32m[ZenohX]\x1b[0m Download complete.`);
    }

    // Launch binary
    let child;
    if (info.isAppImage) {
      child = spawn(targetPath, process.argv.slice(2), {
        stdio: 'inherit',
        detached: true,
      });
    } else if (os.platform() === 'darwin') {
      child = spawn('open', ['-a', targetPath, '--args', ...process.argv.slice(2)], {
        stdio: 'inherit',
        detached: true,
      });
    } else {
      child = spawn(targetPath, process.argv.slice(2), {
        stdio: 'inherit',
        detached: true,
      });
    }

    child.unref();
    process.exit(0);
  } catch (err) {
    console.error(`\x1b[31m[ZenohX Error]\x1b[0m ${err.message}`);
    console.error(`Download directly from: https://github.com/${REPO}/releases/latest`);
    process.exit(1);
  }
}

main();
