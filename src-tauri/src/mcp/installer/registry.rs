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

use super::types::{AgentTarget, ConfigFormat};

/// Supported operating system categories for configuration resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsKind {
    Linux,
    MacOs,
    Windows,
}

impl OsKind {
    /// Detects the host operating system at runtime.
    pub fn current() -> Self {
        #[cfg(target_os = "macos")]
        {
            OsKind::MacOs
        }
        #[cfg(target_os = "windows")]
        {
            OsKind::Windows
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            OsKind::Linux
        }
    }
}

/// Resolved base system directories consumed by the agent registry.
#[derive(Debug, Clone)]
pub struct PlatformPaths {
    pub os: OsKind,
    pub home: PathBuf,
    pub xdg_config: PathBuf,
    pub appdata: PathBuf,
    pub macos_app_support: PathBuf,
}

impl PlatformPaths {
    /// Resolves platform base directories from current environment variables.
    pub fn from_env() -> Self {
        let os = OsKind::current();
        let home = get_home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let xdg_config = get_xdg_config_dir(&home);
        let appdata = get_appdata_dir(&home);
        let macos_app_support = home.join("Library").join("Application Support");

        Self {
            os,
            home,
            xdg_config,
            appdata,
            macos_app_support,
        }
    }
}

/// Helper to get user home directory across platforms.
pub fn get_home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("USERPROFILE")
                .ok()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
        })
}

/// Helper to get XDG config home or fallback to ~/.config.
pub fn get_xdg_config_dir(home: &Path) -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
}

/// Helper to get Windows %APPDATA% or fallback to user profile roaming directory.
pub fn get_appdata_dir(home: &Path) -> PathBuf {
    std::env::var("APPDATA")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("AppData").join("Roaming"))
}

/// Resolves path and candidate app directories for an agent ID under specified platform paths.
pub fn resolve_agent_paths(
    agent_id: &str,
    paths: &PlatformPaths,
) -> Option<(PathBuf, Vec<PathBuf>)> {
    match agent_id {
        "antigravity" => {
            let config_path = paths.home.join(".gemini").join("config").join("mcp_config.json");
            let app_dirs = vec![paths.home.join(".gemini")];
            Some((config_path, app_dirs))
        }
        "claude" => match paths.os {
            OsKind::MacOs => {
                let base = paths.macos_app_support.join("Claude");
                let config_path = base.join("claude_desktop_config.json");
                Some((config_path, vec![base]))
            }
            OsKind::Windows => {
                let base = paths.appdata.join("Claude");
                let config_path = base.join("claude_desktop_config.json");
                Some((config_path, vec![base]))
            }
            OsKind::Linux => {
                let primary_base = paths.xdg_config.join("Claude");
                let fallback_base = paths.xdg_config.join("claude");
                let primary_config = primary_base.join("claude_desktop_config.json");
                let fallback_config = fallback_base.join("claude_desktop_config.json");

                let config_path = if fallback_config.exists()
                    || (fallback_base.exists() && !primary_config.exists() && !primary_base.exists())
                {
                    fallback_config
                } else {
                    primary_config
                };

                Some((config_path, vec![primary_base, fallback_base]))
            }
        },
        "cursor" => {
            let config_path = paths.home.join(".cursor").join("mcp.json");
            let app_dirs = vec![paths.home.join(".cursor")];
            Some((config_path, app_dirs))
        }
        "windsurf" => {
            let config_path = paths
                .home
                .join(".codeium")
                .join("windsurf")
                .join("mcp_config.json");
            let app_dirs = vec![
                paths.home.join(".codeium").join("windsurf"),
                paths.home.join(".codeium"),
            ];
            Some((config_path, app_dirs))
        }
        "cline" => {
            let base = match paths.os {
                OsKind::MacOs => paths.macos_app_support.clone(),
                OsKind::Windows => paths.appdata.clone(),
                OsKind::Linux => paths.xdg_config.clone(),
            };
            let storage = base
                .join("Code")
                .join("User")
                .join("globalStorage")
                .join("saoudrizwan.claude-dev");
            let config_path = storage.join("settings").join("cline_mcp_settings.json");
            Some((config_path, vec![storage]))
        }
        "roo-code" => {
            let base = match paths.os {
                OsKind::MacOs => paths.macos_app_support.clone(),
                OsKind::Windows => paths.appdata.clone(),
                OsKind::Linux => paths.xdg_config.clone(),
            };
            let storage = base
                .join("Code")
                .join("User")
                .join("globalStorage")
                .join("rooveterinaryinc.roo-cline");
            let config_path = storage.join("settings").join("cline_mcp_settings.json");
            Some((config_path, vec![storage]))
        }
        "zed" => {
            let (config_path, app_dir) = match paths.os {
                OsKind::Windows => {
                    let base = paths.appdata.join("Zed");
                    (base.join("settings.json"), base)
                }
                OsKind::MacOs | OsKind::Linux => {
                    let base = paths.xdg_config.join("zed");
                    (base.join("settings.json"), base)
                }
            };
            Some((config_path, vec![app_dir]))
        }
        "codex" => {
            let config_path = paths.home.join(".codex").join("config.toml");
            let app_dirs = vec![paths.home.join(".codex")];
            Some((config_path, app_dirs))
        }
        _ => None,
    }
}

/// Strips JavaScript/JSONC comments (single-line // and block /* ... */) from JSON text.
pub fn strip_json_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' {
            in_string = !in_string;
            out.push(c);
        } else if !in_string && c == '/' && chars.peek() == Some(&'/') {
            chars.next(); // skip second '/'
            for ch in chars.by_ref() {
                if ch == '\n' {
                    out.push('\n');
                    break;
                }
            }
        } else if !in_string && c == '/' && chars.peek() == Some(&'*') {
            chars.next(); // skip '*'
            while let Some(ch) = chars.next() {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next(); // skip '/'
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Checks whether ZenohX MCP server is configured inside the configuration file.
pub fn is_agent_installed(config_path: &Path, format: ConfigFormat) -> bool {
    if !config_path.exists() {
        return false;
    }
    let content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    match format {
        ConfigFormat::JsonMcpServers => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                val.get("mcpServers")
                    .and_then(|s| s.get("zenohx"))
                    .is_some()
            } else {
                let stripped = strip_json_comments(&content);
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stripped) {
                    val.get("mcpServers")
                        .and_then(|s| s.get("zenohx"))
                        .is_some()
                } else {
                    content.contains("\"zenohx\"") && content.contains("\"mcpServers\"")
                }
            }
        }
        ConfigFormat::JsonContextServers => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                val.get("context_servers")
                    .and_then(|s| s.get("zenohx"))
                    .is_some()
            } else {
                let stripped = strip_json_comments(&content);
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stripped) {
                    val.get("context_servers")
                        .and_then(|s| s.get("zenohx"))
                        .is_some()
                } else {
                    content.contains("\"zenohx\"") && content.contains("\"context_servers\"")
                }
            }
        }
        ConfigFormat::TomlMcpServers => {
            content.contains("[mcp_servers.zenohx]")
                || content.contains("[mcp_servers.\"zenohx\"]")
                || (content.contains("[mcp_servers]")
                    && content.lines().any(|l| {
                        let t = l.trim();
                        t.starts_with("zenohx") || t.starts_with("\"zenohx\"")
                    }))
        }
    }
}

/// Builds an AgentTarget from specifications, path resolution, and disk state.
pub fn evaluate_agent(
    id: &str,
    name: &str,
    format: ConfigFormat,
    paths: &PlatformPaths,
) -> Option<AgentTarget> {
    let (config_path, app_dirs) = resolve_agent_paths(id, paths)?;

    let config_exists = config_path.exists();
    let app_dir_exists = app_dirs.iter().any(|d| d.exists());
    let detected = config_exists || app_dir_exists;

    let installed = is_agent_installed(&config_path, format);

    Some(AgentTarget {
        id: id.to_string(),
        name: name.to_string(),
        detected,
        installed,
        config_path,
        format,
    })
}

/// Static registry of all 8 supported agent targets.
const SUPPORTED_AGENTS: [(&str, &str, ConfigFormat); 8] = [
    ("antigravity", "Antigravity CLI", ConfigFormat::JsonMcpServers),
    ("claude", "Claude Desktop", ConfigFormat::JsonMcpServers),
    ("cursor", "Cursor IDE", ConfigFormat::JsonMcpServers),
    ("windsurf", "Windsurf", ConfigFormat::JsonMcpServers),
    ("cline", "VS Code (Cline)", ConfigFormat::JsonMcpServers),
    ("roo-code", "VS Code (Roo Code)", ConfigFormat::JsonMcpServers),
    ("zed", "Zed Editor", ConfigFormat::JsonContextServers),
    ("codex", "Codex CLI", ConfigFormat::TomlMcpServers),
];

/// Returns detection and installation status for all supported agents on the host.
pub fn get_all_agents() -> Vec<AgentTarget> {
    let paths = PlatformPaths::from_env();
    get_all_agents_with_paths(&paths)
}

/// Returns detection and installation status using custom platform paths.
pub fn get_all_agents_with_paths(paths: &PlatformPaths) -> Vec<AgentTarget> {
    SUPPORTED_AGENTS
        .iter()
        .filter_map(|(id, name, format)| evaluate_agent(id, name, *format, paths))
        .collect()
}

/// Finds a specific agent by ID on the host system.
pub fn get_agent_by_id(id: &str) -> Option<AgentTarget> {
    get_all_agents().into_iter().find(|a| a.id == id)
}

/// Resolves the binary command and arguments to register for ZenohX MCP.
///
/// If ~/.zenohx/bin/zenohx-mcp or an executable in the running binary directory exists,
/// returns `(binary_path, vec![])`.
/// Otherwise falls back to npm npx invocation: `("npx", vec!["-y", "zenohx", "mcp"])`.
pub fn resolve_binary_command() -> (String, Vec<String>) {
    // 1. Check ~/.zenohx/bin/zenohx-mcp
    if let Some(home) = get_home_dir() {
        let bin_dir = home.join(".zenohx").join("bin");
        let candidates = [
            bin_dir.join("zenohx-mcp"),
            bin_dir.join("zenohx-mcp.exe"),
        ];
        for candidate in &candidates {
            if candidate.is_file() {
                return (candidate.to_string_lossy().to_string(), Vec::new());
            }
        }
    }

    // 2. Check current running binary and its parent directory
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(file_name) = current_exe.file_name().and_then(|f| f.to_str()) {
            if file_name == "zenohx-mcp" || file_name == "zenohx-mcp.exe" {
                return (current_exe.to_string_lossy().to_string(), Vec::new());
            }
        }
        if let Some(parent) = current_exe.parent() {
            let candidates = [
                parent.join("zenohx-mcp"),
                parent.join("zenohx-mcp.exe"),
            ];
            for candidate in &candidates {
                if candidate.is_file() {
                    return (candidate.to_string_lossy().to_string(), Vec::new());
                }
            }
        }
    }

    // 3. Fallback to npx
    (
        "npx".to_string(),
        vec![
            "-y".to_string(),
            "zenohx".to_string(),
            "mcp".to_string(),
        ],
    )
}
