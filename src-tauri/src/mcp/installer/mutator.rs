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

use std::path::{Path, PathBuf};
use toml_edit::{Array, DocumentMut, Item, Table, Value};

use super::types::{AgentTarget, ConfigFormat};

/// Computes the backup path for a configuration file (<config_path>.bak).
pub fn get_backup_path(path: &Path) -> PathBuf {
    match path.file_name() {
        Some(name) => {
            let mut bak = name.to_os_string();
            bak.push(".bak");
            path.with_file_name(bak)
        }
        None => path.with_extension("bak"),
    }
}

/// Computes the temporary path for atomic writes (<config_path>.tmp).
pub fn get_temp_path(path: &Path) -> PathBuf {
    match path.file_name() {
        Some(name) => {
            let mut tmp = name.to_os_string();
            tmp.push(".tmp");
            path.with_file_name(tmp)
        }
        None => path.with_extension("tmp"),
    }
}

/// Performs safe atomic file write:
/// 1. Ensures parent directory exists.
/// 2. If the file exists, creates a backup copy at <config_path>.bak.
/// 3. Writes content to <config_path>.tmp and atomically renames it to <config_path>.
pub fn safe_write_config(path: &Path, content: &str) -> Result<(), String> {
    // 1. Ensure parent directories exist
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "Failed to create directory {}: {}",
                    parent.display(),
                    e
                )
            })?;
        }
    }

    // 2. Backup existing configuration file
    if path.exists() {
        let backup_path = get_backup_path(path);
        std::fs::copy(path, &backup_path).map_err(|e| {
            format!(
                "Failed to create backup copy at {}: {}",
                backup_path.display(),
                e
            )
        })?;
    }

    // 3. Write atomically via temporary file and rename
    let temp_path = get_temp_path(path);
    std::fs::write(&temp_path, content).map_err(|e| {
        format!(
            "Failed to write temporary configuration file {}: {}",
            temp_path.display(),
            e
        )
    })?;

    if let Err(e) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!(
            "Failed to rename temporary file to {}: {}",
            path.display(),
            e
        ));
    }

    Ok(())
}

/// Strips trailing commas from JSON text when not within a string literal.
pub fn strip_trailing_commas(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut escaped = false;
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
        } else if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
        } else if c == ',' {
            // Look ahead past whitespace to check if next character closes container
            let mut j = i + 1;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            if j < len && (chars[j] == '}' || chars[j] == ']') {
                // Trailing comma before closing brace/bracket: omit
                i += 1;
            } else {
                out.push(c);
                i += 1;
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// Parses JSON text into a Value, tolerating comments and trailing commas.
fn parse_json_value(raw: &str) -> Result<serde_json::Value, String> {
    if raw.trim().is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(raw) {
        return Ok(val);
    }
    let no_comments = super::registry::strip_json_comments(raw);
    let no_trailing = strip_trailing_commas(&no_comments);
    serde_json::from_str::<serde_json::Value>(&no_trailing).map_err(|e| {
        format!("Failed to parse JSON configuration: {}", e)
    })
}

/// Mutates a JSON configuration file by merging a zenohx entry under the specified key.
fn mutate_json(
    path: &Path,
    key: &str,
    binary_cmd: &str,
    binary_args: &[String],
) -> Result<(), String> {
    let raw = if path.exists() {
        std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?
    } else {
        String::new()
    };

    let mut root = parse_json_value(&raw)?;
    let root_map = root.as_object_mut().ok_or_else(|| {
        format!("Root value in {} must be a JSON object", path.display())
    })?;

    let servers_val = root_map
        .entry(key.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

    let servers = if servers_val.is_object() {
        servers_val.as_object_mut().unwrap()
    } else {
        *servers_val = serde_json::Value::Object(serde_json::Map::new());
        servers_val.as_object_mut().unwrap()
    };

    let mut zenohx_entry = serde_json::Map::new();
    zenohx_entry.insert(
        "command".to_string(),
        serde_json::Value::String(binary_cmd.to_string()),
    );
    zenohx_entry.insert(
        "args".to_string(),
        serde_json::Value::Array(
            binary_args
                .iter()
                .map(|a| serde_json::Value::String(a.clone()))
                .collect(),
        ),
    );

    servers.insert("zenohx".to_string(), serde_json::Value::Object(zenohx_entry));

    let serialized = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize JSON: {}", e))?;

    safe_write_config(path, &serialized)
}

/// Removes the zenohx entry from a JSON configuration file under the specified key.
fn unmutate_json(path: &Path, key: &str) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let mut root = parse_json_value(&raw)?;
    let root_map = match root.as_object_mut() {
        Some(m) => m,
        None => return Ok(()),
    };

    let removed = if let Some(servers_val) = root_map.get_mut(key) {
        if let Some(servers) = servers_val.as_object_mut() {
            servers.remove("zenohx").is_some()
        } else {
            false
        }
    } else {
        false
    };

    if !removed {
        return Ok(());
    }

    let serialized = serde_json::to_string_pretty(&root)
        .map_err(|e| format!("Failed to serialize JSON: {}", e))?;

    safe_write_config(path, &serialized)
}

/// Mutates a TOML configuration file by merging [mcp_servers.zenohx] while preserving existing comments and formatting.
fn mutate_toml(
    path: &Path,
    binary_cmd: &str,
    binary_args: &[String],
) -> Result<(), String> {
    let raw = if path.exists() {
        std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?
    } else {
        String::new()
    };

    let mut doc: DocumentMut = if raw.trim().is_empty() {
        DocumentMut::new()
    } else {
        raw.parse::<DocumentMut>()
            .map_err(|e| format!("Failed to parse TOML in {}: {}", path.display(), e))?
    };

    if !doc.contains_table("mcp_servers") {
        doc["mcp_servers"] = Item::Table(Table::new());
    }

    let mut zenohx_table = Table::new();
    zenohx_table.insert("command", toml_edit::value(binary_cmd));
    let mut args_array = Array::new();
    for arg in binary_args {
        args_array.push(arg.as_str());
    }
    zenohx_table.insert("args", Item::Value(Value::Array(args_array)));

    doc["mcp_servers"]["zenohx"] = Item::Table(zenohx_table);

    let serialized = doc.to_string();
    safe_write_config(path, &serialized)
}

/// Removes the zenohx entry from a TOML configuration file under [mcp_servers] while preserving comments.
fn unmutate_toml(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    if raw.trim().is_empty() {
        return Ok(());
    }

    let mut doc = raw
        .parse::<DocumentMut>()
        .map_err(|e| format!("Failed to parse TOML in {}: {}", path.display(), e))?;

    let removed = if let Some(mcp_servers) = doc.get_mut("mcp_servers").and_then(|i| i.as_table_like_mut()) {
        mcp_servers.remove("zenohx").is_some()
    } else {
        false
    };

    if !removed {
        return Ok(());
    }

    let serialized = doc.to_string();
    safe_write_config(path, &serialized)
}

/// Mutates a YAML configuration file by merging mcp_servers.zenohx while preserving comments and structure.
fn mutate_yaml(
    path: &Path,
    binary_cmd: &str,
    binary_args: &[String],
) -> Result<(), String> {
    let raw = if path.exists() {
        std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?
    } else {
        String::new()
    };

    let args_str = serde_json::to_string(binary_args).unwrap_or_else(|_| "[]".to_string());

    if raw.trim().is_empty() {
        let serialized = format!(
            "mcp_servers:\n  zenohx:\n    command: \"{}\"\n    args: {}\n",
            binary_cmd, args_str
        );
        return safe_write_config(path, &serialized);
    }

    let mut lines: Vec<String> = raw.lines().map(|s| s.to_string()).collect();

    // 1. Find line with `mcp_servers:`
    let mut mcp_idx = None;
    let mut mcp_indent = 0;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("mcp_servers:") {
            mcp_idx = Some(i);
            mcp_indent = line.len() - trimmed.len();
            break;
        }
    }

    match mcp_idx {
        None => {
            // No mcp_servers: block found. Append at the end.
            if !lines.is_empty() && !lines.last().unwrap().trim().is_empty() {
                lines.push(String::new());
            }
            lines.push("mcp_servers:".to_string());
            lines.push("  zenohx:".to_string());
            lines.push(format!("    command: \"{}\"", binary_cmd));
            lines.push(format!("    args: {}", args_str));
        }
        Some(idx) => {
            // Handle cases where mcp_servers: was written like `mcp_servers: {}`
            let line_trimmed = lines[idx].trim();
            if line_trimmed == "mcp_servers: {}" || line_trimmed == "mcp_servers: null" {
                lines[idx] = format!("{}mcp_servers:", " ".repeat(mcp_indent));
            }

            // Detect child indentation
            let mut child_indent = mcp_indent + 2;
            let mut zx_idx = None;
            let mut zx_end_idx = None;

            let mut i = idx + 1;
            while i < lines.len() {
                let cur = &lines[i];
                let cur_trimmed = cur.trim_start();
                if cur_trimmed.is_empty() || cur_trimmed.starts_with('#') {
                    i += 1;
                    continue;
                }
                let cur_indent = cur.len() - cur_trimmed.len();
                if cur_indent <= mcp_indent {
                    break;
                }
                // First valid child determines child_indent
                if child_indent == mcp_indent + 2 {
                    child_indent = cur_indent;
                }
                if cur_trimmed.starts_with("zenohx:") && cur_indent == child_indent {
                    zx_idx = Some(i);
                    let mut j = i + 1;
                    while j < lines.len() {
                        let sub = &lines[j];
                        let sub_trimmed = sub.trim_start();
                        if sub_trimmed.is_empty() {
                            j += 1;
                            continue;
                        }
                        let sub_indent = sub.len() - sub_trimmed.len();
                        if sub_indent <= child_indent {
                            break;
                        }
                        j += 1;
                    }
                    zx_end_idx = Some(j);
                    break;
                }
                i += 1;
            }

            let indent_str = " ".repeat(child_indent);
            let inner_indent_str = " ".repeat(child_indent + 2);
            let new_entry = vec![
                format!("{}zenohx:", indent_str),
                format!("{}command: \"{}\"", inner_indent_str, binary_cmd),
                format!("{}args: {}", inner_indent_str, args_str),
            ];

            if let (Some(start), Some(end)) = (zx_idx, zx_end_idx) {
                // Replace existing zenohx block
                lines.splice(start..end, new_entry);
            } else {
                // Insert new zenohx entry right after mcp_servers:
                lines.splice(idx + 1..idx + 1, new_entry);
            }
        }
    }

    let mut serialized = lines.join("\n");
    if !serialized.ends_with('\n') {
        serialized.push('\n');
    }
    safe_write_config(path, &serialized)
}

/// Removes the zenohx entry from a YAML configuration file under mcp_servers while preserving comments.
fn unmutate_yaml(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }

    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    if raw.trim().is_empty() {
        return Ok(());
    }

    let mut lines: Vec<String> = raw.lines().map(|s| s.to_string()).collect();

    let mut mcp_idx = None;
    let mut mcp_indent = 0;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("mcp_servers:") {
            mcp_idx = Some(i);
            mcp_indent = line.len() - trimmed.len();
            break;
        }
    }

    let mcp_idx = match mcp_idx {
        Some(idx) => idx,
        None => return Ok(()),
    };

    let mut zx_idx = None;
    let mut zx_end_idx = None;
    let mut i = mcp_idx + 1;

    while i < lines.len() {
        let cur = &lines[i];
        let cur_trimmed = cur.trim_start();
        if cur_trimmed.is_empty() || cur_trimmed.starts_with('#') {
            i += 1;
            continue;
        }
        let cur_indent = cur.len() - cur_trimmed.len();
        if cur_indent <= mcp_indent {
            break;
        }
        if cur_trimmed.starts_with("zenohx:") {
            zx_idx = Some(i);
            let mut j = i + 1;
            while j < lines.len() {
                let sub = &lines[j];
                let sub_trimmed = sub.trim_start();
                if sub_trimmed.is_empty() {
                    j += 1;
                    continue;
                }
                let sub_indent = sub.len() - sub_trimmed.len();
                if sub_indent <= cur_indent {
                    break;
                }
                j += 1;
            }
            zx_end_idx = Some(j);
            break;
        }
        i += 1;
    }

    if let (Some(start), Some(end)) = (zx_idx, zx_end_idx) {
        lines.drain(start..end);
        let mut serialized = lines.join("\n");
        if !serialized.ends_with('\n') {
            serialized.push('\n');
        }
        safe_write_config(path, &serialized)?;
    }

    Ok(())
}

/// Installs the ZenohX MCP server into the target agent's configuration.
pub fn install_agent(
    target: &AgentTarget,
    binary_cmd: &str,
    binary_args: &[String],
) -> Result<AgentTarget, String> {
    match target.format {
        ConfigFormat::JsonMcpServers => {
            mutate_json(&target.config_path, "mcpServers", binary_cmd, binary_args)?;
        }
        ConfigFormat::JsonContextServers => {
            mutate_json(&target.config_path, "context_servers", binary_cmd, binary_args)?;
        }
        ConfigFormat::TomlMcpServers => {
            mutate_toml(&target.config_path, binary_cmd, binary_args)?;
        }
        ConfigFormat::YamlMcpServers => {
            mutate_yaml(&target.config_path, binary_cmd, binary_args)?;
        }
    }

    if target.id == "antigravity" {
        if let Some(home) = super::registry::get_home_dir() {
            sync_antigravity_mcp_directory(&home);
        }
    }

    let mut updated = target.clone();
    updated.detected = true;
    updated.installed = true;
    Ok(updated)
}

pub const ZENOHX_MCP_INSTRUCTIONS: &str = r#"# ZenohX MCP Server Guidelines

ZenohX is a visual desktop application and debugging workbench for Zenoh networks (routers, peers, and clients).

## Best Practices & Interaction Guidelines

### 1. Interacting with Nodes in the ZenohX Application
* **Do NOT create ad-hoc sessions blindly**: The user usually runs the ZenohX GUI with one or more nodes/sessions already active or configured.
* **Inspect Active Nodes First**: Always call `zenoh_get_sessions` to retrieve running nodes and active sessions. Inspect the node's `id` (session UUID), `zid` (Zenoh ID), and `mode` (router, peer, client).
* **Inspect Saved Profiles**: Call `zenoh_get_profiles` to list the user's saved connection profiles (e.g. Local Router, Edge Client).
* **Starting a Configured Node**: If you need to start a session for a configured profile, call `zenoh_connect_session` with `profile_id: "<profile-id>"`. Do not create ad-hoc generic sessions unless explicitly asked.

### 2. Publishing and Subscribing
* **Target the Active Session**: Always pass the `session_id` obtained from `zenoh_get_sessions` into `zenoh_publish`, `zenoh_subscribe`, and `zenoh_query`.
* **Verifying Delivery**: Use `zenoh_get_messages` with `key_expr` or `limit` to confirm that messages published or received are logged in the app's history.

### 3. Network Scouting vs Local Inspection
* `zenoh_get_sessions` / `zenoh_get_profiles`: Inspects the nodes and sessions inside the ZenohX application.
* `zenoh_scout`: Scans the external physical network via UDP multicast to discover external Zenoh daemons/routers on the LAN. Do NOT use `zenoh_scout` when the user asks to see or interact with nodes inside ZenohX.
"#;

/// Synchronizes the instructions.md and lazy tool schemas into Antigravity CLI MCP directory.
pub fn sync_antigravity_mcp_directory(home_path: &Path) {
    let mcp_dir = home_path
        .join(".gemini")
        .join("antigravity-cli")
        .join("mcp")
        .join("zenohx");
    let _ = std::fs::create_dir_all(&mcp_dir);
    let _ = std::fs::write(mcp_dir.join("instructions.md"), ZENOHX_MCP_INSTRUCTIONS);

    // Sync lazy JSON tool definitions
    let tools = crate::mcp::tools::get_tool_definitions();
    for tool in tools {
        if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
            let schema_file = mcp_dir.join(format!("{}.json", name));
            let input_schema = tool
                .get("inputSchema")
                .cloned()
                .unwrap_or(serde_json::json!({ "type": "object", "properties": {} }));
            let schema_obj = serde_json::json!({
                "name": name,
                "description": tool.get("description").and_then(|d| d.as_str()).unwrap_or(""),
                "parameters": input_schema,
            });
            if let Ok(serialized) = serde_json::to_string(&schema_obj) {
                let _ = std::fs::write(schema_file, serialized);
            }
        }
    }
}

/// Uninstalls the ZenohX MCP server from the target agent's configuration.
pub fn uninstall_agent(target: &AgentTarget) -> Result<AgentTarget, String> {
    match target.format {
        ConfigFormat::JsonMcpServers => {
            unmutate_json(&target.config_path, "mcpServers")?;
        }
        ConfigFormat::JsonContextServers => {
            unmutate_json(&target.config_path, "context_servers")?;
        }
        ConfigFormat::TomlMcpServers => {
            unmutate_toml(&target.config_path)?;
        }
        ConfigFormat::YamlMcpServers => {
            unmutate_yaml(&target.config_path)?;
        }
    }

    let mut updated = target.clone();
    updated.detected = target.detected || target.config_path.exists();
    updated.installed = false;
    Ok(updated)
}

/// Installs ZenohX MCP to an agent by its ID using resolved binary command.
pub fn install_agent_by_id(agent_id: &str) -> Result<AgentTarget, String> {
    let target = super::registry::get_agent_by_id(agent_id)
        .ok_or_else(|| format!("Unknown agent ID: {}", agent_id))?;
    let (binary_cmd, binary_args) = super::registry::resolve_binary_command();
    install_agent(&target, &binary_cmd, &binary_args)
}

/// Uninstalls ZenohX MCP from an agent by its ID.
pub fn uninstall_agent_by_id(agent_id: &str) -> Result<AgentTarget, String> {
    let target = super::registry::get_agent_by_id(agent_id)
        .ok_or_else(|| format!("Unknown agent ID: {}", agent_id))?;
    uninstall_agent(&target)
}
