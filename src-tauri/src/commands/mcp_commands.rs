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

use crate::mcp::installer::{
    get_all_agents, install_agent_by_id, uninstall_agent_by_id, AgentTarget,
};

/// Retrieves the status and configuration paths of all supported MCP agent targets.
#[tauri::command]
pub async fn get_mcp_agents() -> Result<Vec<AgentTarget>, String> {
    Ok(get_all_agents())
}

/// Installs the ZenohX MCP server configuration into the specified agent.
#[tauri::command]
pub async fn install_mcp_agent(agent_id: String) -> Result<AgentTarget, String> {
    install_agent_by_id(&agent_id)
}

/// Uninstalls the ZenohX MCP server configuration from the specified agent.
#[tauri::command]
pub async fn uninstall_mcp_agent(agent_id: String) -> Result<AgentTarget, String> {
    uninstall_agent_by_id(&agent_id)
}

/// Installs the ZenohX MCP server configuration into all detected agents that are not yet installed.
#[tauri::command]
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

/// Returns the standard JSON MCP configuration snippet for manual configuration.
#[tauri::command]
pub async fn get_mcp_config_json() -> Result<String, String> {
    let (cmd, args) = crate::mcp::installer::resolve_binary_command();
    let snippet = serde_json::json!({
        "mcpServers": {
            "zenohx": {
                "command": cmd,
                "args": args
            }
        }
    });
    serde_json::to_string_pretty(&snippet).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use parking_lot::Mutex;
    use std::fs;
    use std::path::PathBuf;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        _lock: parking_lot::MutexGuard<'static, ()>,
        temp_dir: PathBuf,
        orig_home: Option<String>,
        orig_xdg: Option<String>,
        orig_appdata: Option<String>,
        orig_userprofile: Option<String>,
    }

    impl EnvGuard {
        fn new(prefix: &str) -> Self {
            let lock = ENV_LOCK.lock();
            let temp_dir = std::env::temp_dir()
                .join(format!("zenohx-cmd-test-{}-{}", prefix, uuid::Uuid::new_v4()));
            fs::create_dir_all(&temp_dir).unwrap();

            let orig_home = std::env::var("HOME").ok();
            let orig_xdg = std::env::var("XDG_CONFIG_HOME").ok();
            let orig_appdata = std::env::var("APPDATA").ok();
            let orig_userprofile = std::env::var("USERPROFILE").ok();

            std::env::set_var("HOME", &temp_dir);
            std::env::set_var("USERPROFILE", &temp_dir);
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.join(".config"));
            std::env::set_var("APPDATA", temp_dir.join("AppData").join("Roaming"));

            Self {
                _lock: lock,
                temp_dir,
                orig_home,
                orig_xdg,
                orig_appdata,
                orig_userprofile,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(ref orig) = self.orig_home {
                std::env::set_var("HOME", orig);
            } else {
                std::env::remove_var("HOME");
            }

            if let Some(ref orig) = self.orig_xdg {
                std::env::set_var("XDG_CONFIG_HOME", orig);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }

            if let Some(ref orig) = self.orig_appdata {
                std::env::set_var("APPDATA", orig);
            } else {
                std::env::remove_var("APPDATA");
            }

            if let Some(ref orig) = self.orig_userprofile {
                std::env::set_var("USERPROFILE", orig);
            } else {
                std::env::remove_var("USERPROFILE");
            }

            let _ = fs::remove_dir_all(&self.temp_dir);
        }
    }

    #[tokio::test]
    async fn test_get_mcp_agents() {
        let agents = get_mcp_agents().await.expect("should return agents");
        assert_eq!(agents.len(), 10);
        let ids: Vec<&str> = agents.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"antigravity"));
        assert!(ids.contains(&"claude"));
        assert!(ids.contains(&"cursor"));
        assert!(ids.contains(&"windsurf"));
        assert!(ids.contains(&"cline"));
        assert!(ids.contains(&"roo-code"));
        assert!(ids.contains(&"zed"));
        assert!(ids.contains(&"codex"));
        assert!(ids.contains(&"hermes"));
        assert!(ids.contains(&"openclaw"));
    }

    #[tokio::test]
    async fn test_install_mcp_agent_unknown_id() {
        let res = install_mcp_agent("unknown-agent-id".to_string()).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Unknown agent ID"));
    }

    #[tokio::test]
    async fn test_uninstall_mcp_agent_unknown_id() {
        let res = uninstall_mcp_agent("unknown-agent-id".to_string()).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Unknown agent ID"));
    }

    #[tokio::test]
    async fn test_install_all_detected_mcp_agents_empty() {
        let _guard = EnvGuard::new("empty");
        let res = install_all_detected_mcp_agents().await;
        assert!(res.is_ok());
        let installed = res.unwrap();
        assert!(installed.is_empty());
    }

    #[tokio::test]
    async fn test_install_and_uninstall_detected_agent_lifecycle() {
        let guard = EnvGuard::new("lifecycle");
        let zed_dir = if cfg!(target_os = "windows") {
            guard.temp_dir.join("AppData").join("Roaming").join("Zed")
        } else {
            guard.temp_dir.join(".config").join("zed")
        };
        fs::create_dir_all(&zed_dir).unwrap();
        let zed_config = zed_dir.join("settings.json");
        fs::write(&zed_config, "{}").unwrap();

        let agents = get_mcp_agents().await.expect("get agents ok");
        let zed = agents.iter().find(|a| a.id == "zed").expect("zed found");
        assert!(zed.detected);
        assert!(!zed.installed);

        let installed = install_all_detected_mcp_agents()
            .await
            .expect("install all ok");
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].id, "zed");
        assert!(installed[0].installed);

        let content = fs::read_to_string(&zed_config).unwrap();
        assert!(content.contains("context_servers"));
        assert!(content.contains("zenohx"));

        let uninstalled = uninstall_mcp_agent("zed".to_string())
            .await
            .expect("uninstall ok");
        assert_eq!(uninstalled.id, "zed");
        assert!(!uninstalled.installed);

        let content_after = fs::read_to_string(&zed_config).unwrap();
        assert!(!content_after.contains("zenohx"));
    }

    #[tokio::test]
    async fn test_get_mcp_config_json() {
        let snippet = get_mcp_config_json().await.expect("get_mcp_config_json ok");
        let parsed: serde_json::Value = serde_json::from_str(&snippet).expect("valid json");
        assert!(parsed["mcpServers"]["zenohx"].is_object());
        assert!(parsed["mcpServers"]["zenohx"]["command"].is_string());
        assert!(parsed["mcpServers"]["zenohx"]["args"].is_array());
    }
}
