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

use super::*;
use std::fs;
use std::path::PathBuf;

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("zenohx-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

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

    let antigravity = agents
        .iter()
        .find(|a| a.id == "antigravity")
        .expect("antigravity agent");
    assert_eq!(antigravity.format, types::ConfigFormat::JsonMcpServers);

    let cursor = agents.iter().find(|a| a.id == "cursor").expect("cursor agent");
    assert_eq!(cursor.format, types::ConfigFormat::JsonMcpServers);

    let windsurf = agents
        .iter()
        .find(|a| a.id == "windsurf")
        .expect("windsurf agent");
    assert_eq!(windsurf.format, types::ConfigFormat::JsonMcpServers);

    let cline = agents.iter().find(|a| a.id == "cline").expect("cline agent");
    assert_eq!(cline.format, types::ConfigFormat::JsonMcpServers);

    let roo = agents
        .iter()
        .find(|a| a.id == "roo-code")
        .expect("roo-code agent");
    assert_eq!(roo.format, types::ConfigFormat::JsonMcpServers);
}

#[test]
fn test_get_agent_by_id() {
    let claude = registry::get_agent_by_id("claude");
    assert!(claude.is_some());
    assert_eq!(claude.unwrap().name, "Claude Desktop");

    let zed = registry::get_agent_by_id("zed");
    assert!(zed.is_some());
    assert_eq!(zed.unwrap().name, "Zed Editor");

    let codex = registry::get_agent_by_id("codex");
    assert!(codex.is_some());
    assert_eq!(codex.unwrap().name, "Codex CLI");

    let non_existent = registry::get_agent_by_id("non-existent-agent");
    assert!(non_existent.is_none());
}

#[test]
fn test_resolve_binary_command_fallback() {
    let (cmd, args) = registry::resolve_binary_command();
    assert!(!cmd.is_empty());
    if cmd == "npx" {
        assert_eq!(args, vec!["-y", "zenohx", "mcp"]);
    } else {
        assert!(args.is_empty());
    }
}

#[test]
fn test_resolve_binary_command_with_zenohx_bin() {
    let temp_dir = TempDir::new();
    let bin_dir = temp_dir.path().join(".zenohx").join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let fake_bin = bin_dir.join("zenohx-mcp");
    fs::write(&fake_bin, b"binary content").unwrap();

    let original_home = std::env::var("HOME").ok();
    std::env::set_var("HOME", temp_dir.path());

    let (cmd, args) = registry::resolve_binary_command();

    if let Some(orig) = original_home {
        std::env::set_var("HOME", orig);
    } else {
        std::env::remove_var("HOME");
    }

    assert_eq!(cmd, fake_bin.to_string_lossy().to_string());
    assert!(args.is_empty());
}

#[test]
fn test_cross_platform_path_resolution_windows() {
    let home = PathBuf::from("/mock/windows/user");
    let appdata = PathBuf::from("/mock/windows/appdata");
    let paths = registry::PlatformPaths {
        os: registry::OsKind::Windows,
        home: home.clone(),
        xdg_config: home.join(".config"),
        appdata: appdata.clone(),
        macos_app_support: home.join("Library").join("Application Support"),
    };

    let agents = registry::get_all_agents_with_paths(&paths);
    let find_path = |id: &str| -> PathBuf {
        agents
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.config_path.clone())
            .unwrap()
    };

    assert_eq!(
        find_path("antigravity"),
        home.join(".gemini").join("config").join("mcp_config.json")
    );
    assert_eq!(
        find_path("claude"),
        appdata.join("Claude").join("claude_desktop_config.json")
    );
    assert_eq!(
        find_path("cursor"),
        home.join(".cursor").join("mcp.json")
    );
    assert_eq!(
        find_path("windsurf"),
        home.join(".codeium").join("windsurf").join("mcp_config.json")
    );
    assert_eq!(
        find_path("cline"),
        appdata.join("Code").join("User").join("globalStorage").join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json")
    );
    assert_eq!(
        find_path("roo-code"),
        appdata.join("Code").join("User").join("globalStorage").join("rooveterinaryinc.roo-cline").join("settings").join("cline_mcp_settings.json")
    );
    assert_eq!(
        find_path("zed"),
        appdata.join("Zed").join("settings.json")
    );
    assert_eq!(
        find_path("codex"),
        home.join(".codex").join("config.toml")
    );
}

#[test]
fn test_cross_platform_path_resolution_macos() {
    let home = PathBuf::from("/Users/testuser");
    let xdg = home.join(".config");
    let macos_support = home.join("Library").join("Application Support");
    let paths = registry::PlatformPaths {
        os: registry::OsKind::MacOs,
        home: home.clone(),
        xdg_config: xdg.clone(),
        appdata: home.join("AppData").join("Roaming"),
        macos_app_support: macos_support.clone(),
    };

    let agents = registry::get_all_agents_with_paths(&paths);
    let find_path = |id: &str| -> PathBuf {
        agents
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.config_path.clone())
            .unwrap()
    };

    assert_eq!(
        find_path("antigravity"),
        home.join(".gemini").join("config").join("mcp_config.json")
    );
    assert_eq!(
        find_path("claude"),
        macos_support.join("Claude").join("claude_desktop_config.json")
    );
    assert_eq!(
        find_path("cursor"),
        home.join(".cursor").join("mcp.json")
    );
    assert_eq!(
        find_path("windsurf"),
        home.join(".codeium").join("windsurf").join("mcp_config.json")
    );
    assert_eq!(
        find_path("cline"),
        macos_support.join("Code").join("User").join("globalStorage").join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json")
    );
    assert_eq!(
        find_path("roo-code"),
        macos_support.join("Code").join("User").join("globalStorage").join("rooveterinaryinc.roo-cline").join("settings").join("cline_mcp_settings.json")
    );
    assert_eq!(
        find_path("zed"),
        xdg.join("zed").join("settings.json")
    );
    assert_eq!(
        find_path("codex"),
        home.join(".codex").join("config.toml")
    );
}

#[test]
fn test_cross_platform_path_resolution_linux() {
    let home = PathBuf::from("/home/testuser");
    let xdg = home.join(".config");
    let paths = registry::PlatformPaths {
        os: registry::OsKind::Linux,
        home: home.clone(),
        xdg_config: xdg.clone(),
        appdata: home.join("AppData").join("Roaming"),
        macos_app_support: home.join("Library").join("Application Support"),
    };

    let agents = registry::get_all_agents_with_paths(&paths);
    let find_path = |id: &str| -> PathBuf {
        agents
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.config_path.clone())
            .unwrap()
    };

    assert_eq!(
        find_path("antigravity"),
        home.join(".gemini").join("config").join("mcp_config.json")
    );
    assert_eq!(
        find_path("claude"),
        xdg.join("Claude").join("claude_desktop_config.json")
    );
    assert_eq!(
        find_path("cursor"),
        home.join(".cursor").join("mcp.json")
    );
    assert_eq!(
        find_path("windsurf"),
        home.join(".codeium").join("windsurf").join("mcp_config.json")
    );
    assert_eq!(
        find_path("cline"),
        xdg.join("Code").join("User").join("globalStorage").join("saoudrizwan.claude-dev").join("settings").join("cline_mcp_settings.json")
    );
    assert_eq!(
        find_path("roo-code"),
        xdg.join("Code").join("User").join("globalStorage").join("rooveterinaryinc.roo-cline").join("settings").join("cline_mcp_settings.json")
    );
    assert_eq!(
        find_path("zed"),
        xdg.join("zed").join("settings.json")
    );
    assert_eq!(
        find_path("codex"),
        home.join(".codex").join("config.toml")
    );
}

#[test]
fn test_is_agent_installed_json_and_comments() {
    let temp_dir = TempDir::new();
    let path = temp_dir.path().join("config.json");

    // Non-existent file
    assert!(!registry::is_agent_installed(&path, types::ConfigFormat::JsonMcpServers));

    // File without zenohx
    fs::write(&path, r#"{"mcpServers": {"other": {"command": "test"}}}"#).unwrap();
    assert!(!registry::is_agent_installed(&path, types::ConfigFormat::JsonMcpServers));

    // File with zenohx
    fs::write(
        &path,
        r#"{"mcpServers": {"zenohx": {"command": "zenohx-mcp", "args": []}}}"#,
    )
    .unwrap();
    assert!(registry::is_agent_installed(&path, types::ConfigFormat::JsonMcpServers));

    // File with JSON comments
    fs::write(
        &path,
        r#"{
        // Comment before mcpServers
        "mcpServers": {
            /* block comment */
            "zenohx": {
                "command": "zenohx-mcp"
            }
        }
    }"#,
    )
    .unwrap();
    assert!(registry::is_agent_installed(&path, types::ConfigFormat::JsonMcpServers));
}

#[test]
fn test_is_agent_installed_zed() {
    let temp_dir = TempDir::new();
    let path = temp_dir.path().join("settings.json");

    fs::write(
        &path,
        r#"{"context_servers": {"zenohx": {"command": "zenohx-mcp"}}}"#,
    )
    .unwrap();
    assert!(registry::is_agent_installed(&path, types::ConfigFormat::JsonContextServers));

    fs::write(&path, r#"{"context_servers": {}}"#).unwrap();
    assert!(!registry::is_agent_installed(&path, types::ConfigFormat::JsonContextServers));
}

#[test]
fn test_is_agent_installed_codex_toml() {
    let temp_dir = TempDir::new();
    let path = temp_dir.path().join("config.toml");

    fs::write(
        &path,
        r#"
[mcp_servers.zenohx]
command = "zenohx-mcp"
args = []
"#,
    )
    .unwrap();
    assert!(registry::is_agent_installed(&path, types::ConfigFormat::TomlMcpServers));

    fs::write(
        &path,
        r#"
[mcp_servers]
other = { command = "test" }
"#,
    )
    .unwrap();
    assert!(!registry::is_agent_installed(&path, types::ConfigFormat::TomlMcpServers));
}

#[test]
fn test_evaluate_agent_detected_when_app_dir_exists() {
    let temp_dir = TempDir::new();
    let home = temp_dir.path().to_path_buf();

    // Create ~/.cursor directory without mcp.json
    fs::create_dir_all(home.join(".cursor")).unwrap();

    let paths = registry::PlatformPaths {
        os: registry::OsKind::Linux,
        home: home.clone(),
        xdg_config: home.join(".config"),
        appdata: home.join("AppData").join("Roaming"),
        macos_app_support: home.join("Library").join("Application Support"),
    };

    let target = registry::evaluate_agent(
        "cursor",
        "Cursor IDE",
        types::ConfigFormat::JsonMcpServers,
        &paths,
    )
    .expect("cursor target");

    assert!(target.detected);
    assert!(!target.installed);
    assert_eq!(target.config_path, home.join(".cursor").join("mcp.json"));
}
