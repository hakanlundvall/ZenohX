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

#[cfg(test)]
mod tests {
    use crate::mcp::installer::mutator::*;
    use crate::mcp::installer::types::{AgentTarget, ConfigFormat};
    use std::fs;
    use std::path::PathBuf;

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("zenohx-mutator-test-{}", uuid::Uuid::new_v4()));
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
    fn test_mutate_json_mcp_servers_creates_backup_and_merges() {
        let temp_dir = TempDir::new();
        let config_path = temp_dir.path().join("mcp_config.json");
        fs::write(&config_path, r#"{"mcpServers":{"other":{"command":"other"}}}"#).unwrap();

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

        let content = fs::read_to_string(&config_path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(val["mcpServers"]["zenohx"].is_object());
        assert_eq!(val["mcpServers"]["zenohx"]["command"], "/path/to/zenohx-mcp");
        assert!(val["mcpServers"]["other"].is_object());

        // Backup contains original content without zenohx
        let bak_content = fs::read_to_string(config_path.with_extension("json.bak")).unwrap();
        let bak_val: serde_json::Value = serde_json::from_str(&bak_content).unwrap();
        assert!(bak_val["mcpServers"]["zenohx"].is_null());
        assert!(bak_val["mcpServers"]["other"].is_object());
    }

    #[test]
    fn test_mutate_zed_context_servers() {
        let temp_dir = TempDir::new();
        let config_path = temp_dir.path().join("settings.json");
        fs::write(&config_path, r#"{"theme":"dark"}"#).unwrap();

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
        assert!(config_path.with_extension("json.bak").exists());

        let content = fs::read_to_string(&config_path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(val["theme"], "dark");
        assert!(val["context_servers"]["zenohx"].is_object());
        assert_eq!(val["context_servers"]["zenohx"]["command"], "/path/to/zenohx-mcp");
    }

    #[test]
    fn test_mutate_codex_toml() {
        let temp_dir = TempDir::new();
        let config_path = temp_dir.path().join("config.toml");
        fs::write(
            &config_path,
            r#"
[general]
model = "gpt-4"
"#,
        )
        .unwrap();

        let target = AgentTarget {
            id: "codex".to_string(),
            name: "Codex CLI".to_string(),
            detected: true,
            installed: false,
            config_path: config_path.clone(),
            format: ConfigFormat::TomlMcpServers,
        };

        let args = vec!["--flag".to_string(), "val".to_string()];
        let updated = install_agent(&target, "/path/to/zenohx-mcp", &args).expect("install ok");
        assert!(updated.installed);
        assert!(config_path.with_extension("toml.bak").exists());

        let content = fs::read_to_string(&config_path).unwrap();
        let val: toml::Value = toml::from_str(&content).unwrap();
        assert_eq!(val["general"]["model"].as_str().unwrap(), "gpt-4");
        assert_eq!(
            val["mcp_servers"]["zenohx"]["command"].as_str().unwrap(),
            "/path/to/zenohx-mcp"
        );
        let parsed_args = val["mcp_servers"]["zenohx"]["args"].as_array().unwrap();
        assert_eq!(parsed_args.len(), 2);
        assert_eq!(parsed_args[0].as_str().unwrap(), "--flag");
        assert_eq!(parsed_args[1].as_str().unwrap(), "val");
    }

    #[test]
    fn test_mutate_creates_parent_dir_when_not_exists() {
        let temp_dir = TempDir::new();
        let deep_path = temp_dir.path().join("nested").join("sub").join("mcp.json");

        let target = AgentTarget {
            id: "cursor".to_string(),
            name: "Cursor IDE".to_string(),
            detected: false,
            installed: false,
            config_path: deep_path.clone(),
            format: ConfigFormat::JsonMcpServers,
        };

        let updated = install_agent(&target, "zenohx-mcp", &[]).expect("install ok");
        assert!(updated.installed);
        assert!(updated.detected);
        assert!(deep_path.exists());
        // Since original file didn't exist, no .bak should exist
        assert!(!deep_path.with_extension("json.bak").exists());

        let content = fs::read_to_string(&deep_path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(val["mcpServers"]["zenohx"]["command"], "zenohx-mcp");
    }

    #[test]
    fn test_mutate_json_with_comments_and_trailing_commas() {
        let temp_dir = TempDir::new();
        let config_path = temp_dir.path().join("mcp_config.json");
        fs::write(
            &config_path,
            r#"{
            // Existing custom comment
            "mcpServers": {
                "other": {
                    "command": "other-cmd",
                },
            },
        }"#,
        )
        .unwrap();

        let target = AgentTarget {
            id: "antigravity".to_string(),
            name: "Antigravity CLI".to_string(),
            detected: true,
            installed: false,
            config_path: config_path.clone(),
            format: ConfigFormat::JsonMcpServers,
        };

        let updated = install_agent(&target, "/opt/zenohx/zenohx-mcp", &[]).expect("install ok");
        assert!(updated.installed);
        assert!(config_path.with_extension("json.bak").exists());

        let content = fs::read_to_string(&config_path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).expect("valid json");
        assert_eq!(
            val["mcpServers"]["zenohx"]["command"],
            "/opt/zenohx/zenohx-mcp"
        );
        assert_eq!(val["mcpServers"]["other"]["command"], "other-cmd");
    }

    #[test]
    fn test_uninstall_json_mcp_servers() {
        let temp_dir = TempDir::new();
        let config_path = temp_dir.path().join("mcp_config.json");
        fs::write(
            &config_path,
            r#"{
            "mcpServers": {
                "zenohx": { "command": "zenohx-mcp", "args": [] },
                "other": { "command": "other-agent", "args": [] }
            }
        }"#,
        )
        .unwrap();

        let target = AgentTarget {
            id: "claude".to_string(),
            name: "Claude Desktop".to_string(),
            detected: true,
            installed: true,
            config_path: config_path.clone(),
            format: ConfigFormat::JsonMcpServers,
        };

        let updated = uninstall_agent(&target).expect("uninstall ok");
        assert!(!updated.installed);
        assert!(config_path.with_extension("json.bak").exists());

        let content = fs::read_to_string(&config_path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(val["mcpServers"]["zenohx"].is_null());
        assert!(val["mcpServers"]["other"].is_object());

        // Now uninstall when zenohx is only server - preserves empty container
        let updated2 = uninstall_agent(&updated).expect("uninstall again ok");
        assert!(!updated2.installed);
    }

    #[test]
    fn test_uninstall_zed_context_servers() {
        let temp_dir = TempDir::new();
        let config_path = temp_dir.path().join("settings.json");
        fs::write(
            &config_path,
            r#"{
            "theme": "solarized",
            "context_servers": {
                "zenohx": { "command": "zenohx-mcp", "args": [] }
            }
        }"#,
        )
        .unwrap();

        let target = AgentTarget {
            id: "zed".to_string(),
            name: "Zed Editor".to_string(),
            detected: true,
            installed: true,
            config_path: config_path.clone(),
            format: ConfigFormat::JsonContextServers,
        };

        let updated = uninstall_agent(&target).expect("uninstall ok");
        assert!(!updated.installed);
        assert!(config_path.with_extension("json.bak").exists());

        let content = fs::read_to_string(&config_path).unwrap();
        let val: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(val["theme"], "solarized");
        assert!(val["context_servers"]["zenohx"].is_null());
        assert!(val["context_servers"].is_object());
    }

    #[test]
    fn test_uninstall_codex_toml() {
        let temp_dir = TempDir::new();
        let config_path = temp_dir.path().join("config.toml");
        fs::write(
            &config_path,
            r#"
[general]
model = "gpt-4"

[mcp_servers.zenohx]
command = "zenohx-mcp"
args = []
"#,
        )
        .unwrap();

        let target = AgentTarget {
            id: "codex".to_string(),
            name: "Codex CLI".to_string(),
            detected: true,
            installed: true,
            config_path: config_path.clone(),
            format: ConfigFormat::TomlMcpServers,
        };

        let updated = uninstall_agent(&target).expect("uninstall ok");
        assert!(!updated.installed);
        assert!(config_path.with_extension("toml.bak").exists());

        let content = fs::read_to_string(&config_path).unwrap();
        let val: toml::Value = toml::from_str(&content).unwrap();
        assert_eq!(val["general"]["model"].as_str().unwrap(), "gpt-4");
        assert!(val.get("mcp_servers").and_then(|s| s.get("zenohx")).is_none());
    }

    #[test]
    fn test_uninstall_non_existent_file() {
        let temp_dir = TempDir::new();
        let config_path = temp_dir.path().join("does_not_exist.json");

        let target = AgentTarget {
            id: "claude".to_string(),
            name: "Claude Desktop".to_string(),
            detected: false,
            installed: false,
            config_path: config_path.clone(),
            format: ConfigFormat::JsonMcpServers,
        };

        let updated = uninstall_agent(&target).expect("uninstall ok on missing file");
        assert!(!updated.installed);
    }

    #[test]
    fn test_install_and_uninstall_agent_by_id_unknown() {
        let err = install_agent_by_id("non-existent-agent");
        assert!(err.is_err());

        let err2 = uninstall_agent_by_id("non-existent-agent");
        assert!(err2.is_err());
    }
}
