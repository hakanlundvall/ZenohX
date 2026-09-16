# Automated ZenohX MCP Agent Installation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Provide an automated, safe 1-click and CLI installation feature to detect and configure the ZenohX MCP server across major AI clients (Antigravity, Claude Desktop, Cursor, Windsurf, VS Code Cline/Roo Code, Zed, Codex) without emojis.

**Architecture:** A shared native Rust engine in `src-tauri/src/mcp/installer/` that:
1. Resolves platform-specific configuration paths across Linux, macOS, and Windows.
2. Performs atomic updates and creates `.bak` backups before modifying JSON/TOML configuration files, safely merging `"zenohx"` into `mcpServers` (or `context_servers` / `[mcp_servers]`).
3. Exposes CLI subcommands `zenohx-mcp install`, `zenohx-mcp uninstall`, and `zenohx-mcp list-agents`.
4. Exposes Tauri commands for the desktop GUI, integrated into a dedicated "AI & Agents" tab in Settings.

**Tech Stack:** Rust (Tauri 2, Serde JSON, TOML 0.8), TypeScript / React 18, Tailwind CSS, Lucide React (no emojis).

**Spec:** [`docs/superpowers/specs/2026-09-16-auto-install-mcp-to-agents-design.md`](file:///home/khanhdew/Documents/Keemos/zenohx/docs/superpowers/specs/2026-09-16-auto-install-mcp-to-agents-design.md)

## Global Constraints
- Target clients: Antigravity, Claude Desktop, Cursor, Windsurf, VS Code (Cline & Roo Code), Zed, Codex.
- Safe operations: Always create `.bak` before modifying; use atomic writes (`.tmp` -> `rename`); preserve all third-party servers.
- UI & Output style: Strictly **NO EMOJIS**. Use Lucide React SVG icons in the UI and plain text / ASCII brackets (`[OK]`, `[ERROR]`, `[SKIP]`) in CLI outputs.
- Backward compatibility: Zero regressions on existing MCP stdio JSON-RPC server and GUI workspaces.

---

### Task 1: Agent Registry & Cross-Platform Path Resolution

**Files:**
- Create: `src-tauri/src/mcp/installer/mod.rs`
- Create: `src-tauri/src/mcp/installer/types.rs`
- Create: `src-tauri/src/mcp/installer/registry.rs`
- Test: `src-tauri/src/mcp/installer/tests.rs`

**Interfaces:**
- Consumes: Standard OS environment paths (`HOME`, `XDG_CONFIG_HOME`, `APPDATA`, `USERPROFILE`).
- Produces:
  - `pub enum ConfigFormat { JsonMcpServers, JsonContextServers, TomlMcpServers }`
  - `pub struct AgentTarget { pub id: String, pub name: String, pub detected: bool, pub installed: bool, pub config_path: PathBuf, pub format: ConfigFormat }`
  - `pub fn get_all_agents() -> Vec<AgentTarget>`
  - `pub fn resolve_binary_command() -> (String, Vec<String>)`

- [x] **Step 1: Write failing tests for agent detection and path resolution**

```rust
// src-tauri/src/mcp/installer/tests.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_supported_agents_registered() {
        let agents = registry::get_all_agents();
        assert_eq!(agents.len(), 8);
        let ids: Vec<&str> = agents.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"antigravity"));
        assert!(ids.contains(&"claude"));
        assert!(ids.contains(&"cursor"));
        assert!(ids.contains(&"windsurf"));
        assert!(ids.contains(&"cline"));
        assert!(ids.contains(&"roo-code"));
        assert!(ids.contains(&"zed"));
        assert!(ids.contains(&"codex"));
    }

    #[test]
    fn test_agent_config_formats() {
        let agents = registry::get_all_agents();
        let zed = agents.iter().find(|a| a.id == "zed").expect("zed agent");
        assert_eq!(zed.format, types::ConfigFormat::JsonContextServers);

        let codex = agents.iter().find(|a| a.id == "codex").expect("codex agent");
        assert_eq!(codex.format, types::ConfigFormat::TomlMcpServers);

        let claude = agents.iter().find(|a| a.id == "claude").expect("claude agent");
        assert_eq!(claude.format, types::ConfigFormat::JsonMcpServers);
    }
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml mcp::installer::tests`
Expected: FAIL with module `installer` not found.

- [x] **Step 3: Implement Agent Registry and Path Resolution**

In `src-tauri/src/mcp/installer/types.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigFormat {
    JsonMcpServers,
    JsonContextServers,
    TomlMcpServers,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTarget {
    pub id: String,
    pub name: String,
    pub detected: bool,
    pub installed: bool,
    pub config_path: PathBuf,
    pub format: ConfigFormat,
}
```

In `src-tauri/src/mcp/installer/registry.rs`:
Implement path resolution across platforms for each agent and binary resolution logic.

In `src-tauri/src/mcp/installer/mod.rs`:
Expose `types`, `registry`, and tests. Re-export in `src-tauri/src/mcp/mod.rs`.

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml mcp::installer::tests`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src-tauri/src/mcp/installer
git commit -m "feat(installer): add agent registry and cross-platform path resolution"
```

---

### Task 2: Safe Configuration Mutator Engine (JSON & TOML with Backups)

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `toml = "0.8"`)
- Create: `src-tauri/src/mcp/installer/mutator.rs`
- Test: `src-tauri/src/mcp/installer/mutator_tests.rs`

**Interfaces:**
- Consumes: `AgentTarget`, `ConfigFormat`, binary path from Task 1.
- Produces:
  - `pub fn install_agent(target: &AgentTarget, binary_cmd: &str, binary_args: &[String]) -> Result<AgentTarget, String>`
  - `pub fn uninstall_agent(target: &AgentTarget) -> Result<AgentTarget, String>`

- [x] **Step 1: Add `toml = "0.8"` to Cargo.toml and write failing tests for JSON & TOML mutation**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutate_json_mcp_servers_creates_backup_and_merges() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("mcp_config.json");
        std::fs::write(&config_path, r#"{"mcpServers":{"other":{"command":"other"}}}"#).unwrap();

        let target = AgentTarget {
            id: "claude".to_string(),
            name: "Claude".to_string(),
            detected: true,
            installed: false,
            config_path: config_path.clone(),
            format: ConfigFormat::JsonMcpServers,
        };

        let updated = install_agent(&target, "/path/to/zenohx-mcp", &[]).expect("install ok");
        assert!(updated.installed);
        assert!(config_path.with_extension("json.bak").exists());

        let content = std::fs::read_to_string(&config_path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(val["mcpServers"]["zenohx"].is_object());
        assert!(val["mcpServers"]["other"].is_object());
    }

    #[test]
    fn test_mutate_zed_context_servers() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("settings.json");
        std::fs::write(&config_path, r#"{"theme":"dark"}"#).unwrap();

        let target = AgentTarget {
            id: "zed".to_string(),
            name: "Zed".to_string(),
            detected: true,
            installed: false,
            config_path: config_path.clone(),
            format: ConfigFormat::JsonContextServers,
        };

        let updated = install_agent(&target, "/path/to/zenohx-mcp", &[]).expect("install ok");
        assert!(updated.installed);

        let content = std::fs::read_to_string(&config_path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(val["theme"], "dark");
        assert!(val["context_servers"]["zenohx"].is_object());
    }
}
```

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml mcp::installer::mutator_tests`
Expected: FAIL with missing `install_agent`.

- [x] **Step 3: Implement Safe Mutator with Backups and Atomic Writes**

Implement `src-tauri/src/mcp/installer/mutator.rs` supporting:
- `.bak` backup creation.
- Atomic file write via `.tmp` and `std::fs::rename`.
- Merging and uninstallation for `JsonMcpServers`, `JsonContextServers`, and `TomlMcpServers`.

- [x] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml mcp::installer::mutator_tests`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/mcp/installer/mutator.rs
git commit -m "feat(installer): implement safe JSON and TOML configuration mutator"
```

---

### Task 3: CLI Subcommand Integration (`zenohx-mcp install / uninstall / list`)

**Files:**
- Create: `src-tauri/src/mcp/installer/cli.rs`
- Modify: `src-tauri/src/bin/zenohx-mcp.rs`
- Modify: `bin/zenohx.js` (npm wrapper forwarding)

**Interfaces:**
- Consumes: Command line arguments from `std::env::args()`.
- Produces:
  - `pub fn handle_cli_args(args: &[String]) -> Result<bool, Box<dyn std::error::Error>>`
  - Formatted ASCII table for `list-agents`
  - Automated installation output for `install [--all | <agent_id>]` and `uninstall`

- [x] **Step 1: Write test for CLI argument parsing**

In `src-tauri/src/mcp/installer/cli.rs`: test flag matching (`list`, `install`, `uninstall`).

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml mcp::installer::cli`
Expected: FAIL.

- [x] **Step 3: Implement CLI handlers in `cli.rs` and wire into `zenohx-mcp.rs`**

```rust
// In zenohx-mcp.rs:
let args: Vec<String> = std::env::args().collect();
if args.len() > 1 {
    let handled = zenohx_lib::mcp::installer::cli::handle_cli_args(&args).await?;
    if handled {
        return Ok(());
    }
}
// Default to running stdio server
run_mcp_stdio_server().await?;
```

Ensure output is clean ASCII with `[OK]`, `[SKIP]`, `[ERROR]`, and zero emojis.

- [x] **Step 4: Verify building and running CLI subcommands**

Run: `cargo run --manifest-path src-tauri/Cargo.toml --bin zenohx-mcp -- list-agents`
Expected: Outputs formatted table of agents without errors or emojis.

- [x] **Step 5: Commit**

```bash
git add src-tauri/src/bin/zenohx-mcp.rs src-tauri/src/mcp/installer/cli.rs bin/zenohx.js
git commit -m "feat(cli): add install, uninstall, and list-agents CLI commands to zenohx-mcp"
```

---

### Task 4: Tauri Backend Commands Integration

**Files:**
- Create: `src-tauri/src/commands/mcp_commands.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `mcp::installer` functions from Tasks 1 & 2.
- Produces:
  - Tauri command `get_mcp_agents() -> Result<Vec<AgentTarget>, String>`
  - Tauri command `install_mcp_agent(agent_id: String) -> Result<AgentTarget, String>`
  - Tauri command `uninstall_mcp_agent(agent_id: String) -> Result<AgentTarget, String>`
  - Tauri command `install_all_detected_mcp_agents() -> Result<Vec<AgentTarget>, String>`

- [x] **Step 1: Write failing test for Tauri command handlers**

- [x] **Step 2: Implement Tauri commands and register in `generate_handler!`**

In `src-tauri/src/commands/mcp_commands.rs`:
```rust
use tauri::command;
use crate::mcp::installer::{get_all_agents, install_agent_by_id, uninstall_agent_by_id, AgentTarget};

#[command]
pub async fn get_mcp_agents() -> Result<Vec<AgentTarget>, String> {
    Ok(get_all_agents())
}

#[command]
pub async fn install_mcp_agent(agent_id: String) -> Result<AgentTarget, String> {
    install_agent_by_id(&agent_id)
}

#[command]
pub async fn uninstall_mcp_agent(agent_id: String) -> Result<AgentTarget, String> {
    uninstall_agent_by_id(&agent_id)
}

#[command]
pub async fn install_all_detected_mcp_agents() -> Result<Vec<AgentTarget>, String> {
    let mut results = Vec::new();
    for agent in get_all_agents() {
        if agent.detected && !agent.installed {
            if let Ok(res) = install_agent_by_id(&agent.id) {
                results.push(res);
            }
        }
    }
    Ok(results)
}
```

Register in `src-tauri/src/lib.rs`.

- [x] **Step 3: Run `cargo test` and `cargo check`**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [x] **Step 4: Commit**

```bash
git add src-tauri/src/commands/mcp_commands.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(tauri): add Tauri commands for MCP agent detection and installation"
```

---

### Task 5: Frontend React "AI & Agents" Settings Tab

**Files:**
- Create: `src/components/settings/tabs/AgentsTab.tsx`
- Modify: `src/components/settings/SettingsWorkspace.tsx`
- Modify: `src/types/zenoh.ts`
- Modify: `src/lib/tauri.ts`

**Interfaces:**
- Consumes: Tauri commands `get_mcp_agents`, `install_mcp_agent`, `uninstall_mcp_agent`, `install_all_detected_mcp_agents`.
- Produces:
  - Settings tab `"agents"` with `Bot` Lucide SVG icon.
  - Interactive agent list with copy button for config paths and 1-click Install / Uninstall.
  - Strictly NO EMOJIS.

- [x] **Step 1: Write test for AgentsTab rendering and status updates**

In `tests/agents-tab.test.ts`: verify state transitions (Detected -> Installed).

- [x] **Step 2: Run test to verify it fails**

Run: `npm run test:unit`
Expected: FAIL.

- [x] **Step 3: Implement `AgentsTab.tsx` and integrate into `SettingsWorkspace.tsx`**

Implement `src/components/settings/tabs/AgentsTab.tsx` using Tailwind, Radix UI badges, and Lucide React icons (`Bot`, `CheckCircle2`, `AlertCircle`, `Copy`, `Download`, `Trash2`, `RefreshCw`).
Ensure zero emojis are present in JSX or strings.

Mount in `src/components/settings/SettingsWorkspace.tsx`:
Add `agents` to `TabType`:
```typescript
type TabType = 'preferences' | 'network' | 'agents' | 'protobuf' | 'updates' | 'history' | 'shortcuts';
```
Add tab button and render `<AgentsTab />`.

- [x] **Step 4: Verify TypeScript types and unit tests pass**

Run: `npm run test:types && npm run test:unit`
Expected: PASS with 0 errors.

- [x] **Step 5: Commit**

```bash
git add src/components/settings/tabs/AgentsTab.tsx src/components/settings/SettingsWorkspace.tsx src/types/zenoh.ts src/lib/tauri.ts tests/
git commit -m "feat(ui): add AI & Agents tab in ZenohX settings"
```

---

### Task 6: Full E2E & Integration Verification

**Files:**
- Create: `tests/mcp_installer_e2e_test.mjs`
- Modify: `package.json`

**Interfaces:**
- Consumes: `zenohx-mcp list-agents` and `zenohx-mcp install` CLI.
- Produces: Automated verification testing CLI outputs and file mutation on isolated test configs.

- [x] **Step 1: Implement `tests/mcp_installer_e2e_test.mjs`**

Verify:
- `zenohx-mcp list-agents` outputs clean table without emojis.
- `zenohx-mcp install` correctly mutates a temporary test config.

- [x] **Step 2: Run full test suite**

Run: `npm run test:all && node tests/mcp_installer_e2e_test.mjs`
Expected: All suites pass.

- [x] **Step 3: Commit**

```bash
git add tests/mcp_installer_e2e_test.mjs package.json
git commit -m "test(installer): add automated test for agent installation CLI"
```
