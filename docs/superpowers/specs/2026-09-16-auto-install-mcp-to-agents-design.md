# Design Specification: Automated ZenohX MCP Installation to AI Agents

**Date:** 2026-09-16  
**Status:** Proposed  
**Scope:** Architectural (Agent Registry, Safe Config Mutator, CLI Installer Subcommands, and GUI Settings Integration)

---

## 1. Overview & Problem Statement

ZenohX provides a Model Context Protocol (MCP) server (`zenohx-mcp`) enabling AI assistants to scout networks, manage sessions, publish/subscribe, execute queries, and control the desktop GUI.

However, manually locating configuration files across different operating systems (Linux, macOS, Windows) and editing complex JSON/TOML configurations for different AI clients (Antigravity, Claude Desktop, Cursor, Windsurf, VS Code Cline/Roo Code, Zed, Codex) is error-prone. Users can accidentally corrupt syntax or overwrite other existing MCP servers.

This feature provides an automated, non-destructive installation and uninstallation system for AI clients:
1. **CLI Commands**: `zenohx-mcp install`, `zenohx-mcp uninstall`, and `zenohx-mcp list-agents`.
2. **Desktop GUI Interface**: A dedicated "AI & Agents" tab in ZenohX Settings offering 1-click detection, installation, and uninstallation.
3. **Safe File Operations**: Automatically creates `.bak` backups before modification, writes files atomically, preserves all third-party server configurations, and avoids using emojis in UI or terminal outputs.

---

## 2. Agent Registry & Platform Paths Matrix

### 2.1 Supported AI Clients

| Agent ID | Client Name | Configuration Format | Configuration Paths |
| :--- | :--- | :--- | :--- |
| `antigravity` | Antigravity CLI | JSON (`mcpServers`) | **Linux/macOS:** `$HOME/.gemini/config/mcp_config.json`<br>**Windows:** `%USERPROFILE%\.gemini\config\mcp_config.json` |
| `claude` | Claude Desktop | JSON (`mcpServers`) | **macOS:** `$HOME/Library/Application Support/Claude/claude_desktop_config.json`<br>**Linux:** `$HOME/.config/Claude/claude_desktop_config.json`<br>**Windows:** `%APPDATA%\Claude\claude_desktop_config.json` |
| `cursor` | Cursor IDE | JSON (`mcpServers`) | **Linux/macOS:** `$HOME/.cursor/mcp.json`<br>**Windows:** `%USERPROFILE%\.cursor\mcp.json` |
| `windsurf` | Windsurf | JSON (`mcpServers`) | **Linux/macOS:** `$HOME/.codeium/windsurf/mcp_config.json`<br>**Windows:** `%USERPROFILE%\.codeium\windsurf\mcp_config.json` |
| `cline` | VS Code (Cline) | JSON (`mcpServers`) | **Linux:** `$HOME/.config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json`<br>**macOS:** `$HOME/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json`<br>**Windows:** `%APPDATA%\Code\User\globalStorage\saoudrizwan.claude-dev\settings\cline_mcp_settings.json` |
| `roo-code` | VS Code (Roo Code) | JSON (`mcpServers`) | **Linux:** `$HOME/.config/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json`<br>**macOS:** `$HOME/Library/Application Support/Code/User/globalStorage/rooveterinaryinc.roo-cline/settings/cline_mcp_settings.json`<br>**Windows:** `%APPDATA%\Code\User\globalStorage\rooveterinaryinc.roo-cline\settings\cline_mcp_settings.json` |
| `zed` | Zed Editor | JSON (`context_servers`) | **Linux/macOS:** `$HOME/.config/zed/settings.json`<br>**Windows:** `%APPDATA%\Zed\settings.json` |
| `codex` | Codex CLI | TOML (`[mcp_servers]`) | **Linux/macOS:** `$HOME/.codex/config.toml`<br>**Windows:** `%USERPROFILE%\.codex\config.toml` |

### 2.2 Detection Logic
An agent target is marked as:
* **Detected**: If its configuration file exists on disk, OR if its parent application data directory exists (e.g. `~/.cursor` exists, indicating the software is installed).
* **Installed**: If the configuration file exists and contains a valid `"zenohx"` server entry under its respective schema key.
* **Not Detected**: If neither the configuration file nor its parent application directory is found.

---

## 3. Configuration Mutator Specification (`src-tauri/src/mcp/installer/`)

### 3.1 Executable Resolution
* The installer determines the stable command path to register:
  1. If `~/.zenohx/bin/zenohx-mcp` exists (or when installing from a compiled binary, copies/symlinks the current executable to `~/.zenohx/bin/zenohx-mcp` with `0755` permissions), register:
     ```json
     {
       "command": "/home/user/.zenohx/bin/zenohx-mcp",
       "args": []
     }
     ```
  2. If running in an npm environment without local binary, register:
     ```json
     {
       "command": "npx",
       "args": ["-y", "zenohx", "mcp"]
     }
     ```

### 3.2 Safe File Operations
1. **Directory Creation**: Ensures parent directories exist using `std::fs::create_dir_all`.
2. **Automatic Backup**: If the target file already exists, copies it to `<config_path>.bak`.
3. **Atomic Write**: Formats the new configuration string and writes it to `<config_path>.tmp`. Calls `std::fs::rename` over the target file to guarantee atomic updates without partial file writes.
4. **Preservation of Existing Settings**:
   * For JSON files: reads into `serde_json::Value`. Retains all keys, comments (if JSONC), and existing servers under `mcpServers` or `context_servers`.
   * For TOML files: parses into `toml::Value::Table`. Retains all existing tables, inserting or updating only the `[mcp_servers.zenohx]` section.
5. **Uninstall Cleanliness**:
   * Removes only the `"zenohx"` key. If the parent container (`mcpServers` or `context_servers`) becomes empty, preserves the empty container to keep the client config valid.

---

## 4. CLI Subcommand Specification (`zenohx-mcp`)

When CLI arguments are provided to `zenohx-mcp`:

### 4.1 Command Grammar
* `zenohx-mcp list-agents` (or `zenohx-mcp list`):
  * Scans all supported clients and outputs an aligned ASCII table with columns: `Agent ID`, `Agent Name`, `Status`, and `Config Path`. No emojis.
* `zenohx-mcp install [agent_id]`:
  * If `agent_id` is supplied: installs to the specified agent.
  * If `--all` or no argument is supplied: installs to all detected agents on the machine.
  * Prints clear status messages: `[OK] Installed to Claude Desktop: /path/...` or `[SKIP] Not detected: Windsurf`.
* `zenohx-mcp uninstall [agent_id]`:
  * Removes ZenohX from the specified agent (or all installed agents with `--all`).
* Running without arguments defaults to starting the stdio MCP JSON-RPC server.

---

## 5. GUI Integration & Tauri Commands

### 5.1 Tauri Backend Commands (`src-tauri/src/commands/mcp_commands.rs`)
* `get_mcp_agents() -> Result<Vec<AgentTarget>, String>`:
  * Returns detection and installation status for all supported agents.
* `install_mcp_agent(agent_id: String) -> Result<AgentTarget, String>`:
  * Installs ZenohX MCP to the selected agent and returns the updated status.
* `uninstall_mcp_agent(agent_id: String) -> Result<AgentTarget, String>`:
  * Removes ZenohX MCP from the selected agent and returns the updated status.
* `install_all_detected_mcp_agents() -> Result<Vec<AgentTarget>, String>`:
  * Installs to every detected agent in one operation.

### 5.2 Frontend React Interface (`src/components/settings/tabs/AgentsTab.tsx`)
* Added to `SettingsWorkspace.tsx` as a new tab: `"AI & Agents"`.
* Uses Lucide React SVG icons (e.g. `Bot`, `CheckCircle2`, `AlertCircle`, `Copy`, `Download`, `Trash2`). Zero emojis.
* Top action bar:
  * "Install to All Detected" button.
  * "Refresh Status" button.
* Agent Cards:
  * Displays agent name, status badge (`Installed`, `Detected`, `Not Detected`), config path with a copy button, and action button:
    * `Install` (primary style).
    * `Uninstall` (secondary/outline style).
* Informational footer:
  * Plain text note explaining that users should restart or reload their AI assistant after installation.

---

## 6. Verification Plan

1. **Rust Unit Tests**:
   * Test cross-platform path resolution for each supported agent.
   * Test JSON safe mutator (inserting into empty file, merging with existing servers, taking backup).
   * Test Zed `context_servers` format and Codex TOML format.
   * Test uninstaller removing `"zenohx"` while preserving other servers.
2. **CLI Integration Test**:
   * Execute `zenohx-mcp list-agents` and verify clean table output without panic.
   * Execute `zenohx-mcp install --agent <id>` on a temporary test config directory and verify valid JSON written.
3. **Frontend Tests**:
   * TypeScript typecheck (`npm run test:types`).
   * Unit tests for `AgentsTab` actions and state changes (`npm run test:unit`).
4. **Full Test Suite Verification**:
   * Run `npm run test:all`.
