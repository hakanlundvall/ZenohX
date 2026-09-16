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

use std::io::Write;
use super::types::AgentTarget;

/// Checks if an argument string is a known CLI subcommand or top-level help flag.
fn is_known_subcommand_or_flag(arg: &str) -> bool {
    matches!(
        arg,
        "list-agents"
            | "list"
            | "install"
            | "uninstall"
            | "help"
            | "--help"
            | "-h"
    )
}

/// Formats the list of supported agents as an aligned ASCII table.
/// Strictly no emojis are used.
pub fn format_agents_table(agents: &[AgentTarget]) -> String {
    let id_header = "AGENT ID";
    let name_header = "AGENT NAME";
    let status_header = "STATUS";
    let path_header = "CONFIG PATH";

    let mut id_width = id_header.len();
    let mut name_width = name_header.len();
    let mut status_width = status_header.len();

    let mut rows = Vec::with_capacity(agents.len());
    for a in agents {
        let status = if a.installed {
            "Installed"
        } else if a.detected {
            "Detected"
        } else {
            "Not Detected"
        };
        let path_str = a.config_path.display().to_string();
        id_width = id_width.max(a.id.len());
        name_width = name_width.max(a.name.len());
        status_width = status_width.max(status.len());
        rows.push((&a.id, &a.name, status, path_str));
    }

    let mut out = String::new();
    out.push_str(&format!(
        "{:<iw$}  {:<nw$}  {:<sw$}  {}\n",
        id_header,
        name_header,
        status_header,
        path_header,
        iw = id_width,
        nw = name_width,
        sw = status_width
    ));

    let sep_len = id_width + 2 + name_width + 2 + status_width + 2 + path_header.len();
    out.push_str(&format!("{}\n", "-".repeat(sep_len)));

    for (id, name, status, path) in rows {
        out.push_str(&format!(
            "{:<iw$}  {:<nw$}  {:<sw$}  {}\n",
            id,
            name,
            status,
            path,
            iw = id_width,
            nw = name_width,
            sw = status_width
        ));
    }

    out
}

/// Prints clean CLI usage instructions. Strictly no emojis.
fn print_help<W: Write>(writer: &mut W) -> Result<(), std::io::Error> {
    writeln!(writer, "ZenohX MCP Server & Agent Configuration Manager")?;
    writeln!(writer)?;
    writeln!(writer, "USAGE:")?;
    writeln!(writer, "    zenohx-mcp [SUBCOMMAND]")?;
    writeln!(writer)?;
    writeln!(writer, "SUBCOMMANDS:")?;
    writeln!(
        writer,
        "    list-agents, list     List all supported AI agents and installation status"
    )?;
    writeln!(
        writer,
        "    install [AGENT_ID]    Install ZenohX MCP to an agent (or --all detected agents)"
    )?;
    writeln!(
        writer,
        "    uninstall [AGENT_ID]  Uninstall ZenohX MCP from an agent (or --all installed agents)"
    )?;
    writeln!(writer, "    help, --help, -h      Print help information")?;
    writeln!(writer)?;
    writeln!(
        writer,
        "If no subcommand is provided, zenohx-mcp starts the stdio JSON-RPC MCP server."
    )?;
    Ok(())
}

/// Handles CLI arguments writing output to the specified writer.
/// Returns Ok(true) if a CLI command was handled, Ok(false) if no command was given.
pub fn handle_cli_args_with_writer<W: Write>(
    args: &[String],
    writer: &mut W,
) -> Result<bool, Box<dyn std::error::Error>> {
    handle_cli_args_internal(args, writer, None)
}

/// Handles CLI arguments with custom platform paths for testing isolation.
pub fn handle_cli_args_with_writer_and_paths<W: Write>(
    args: &[String],
    writer: &mut W,
    paths: Option<&super::registry::PlatformPaths>,
) -> Result<bool, Box<dyn std::error::Error>> {
    handle_cli_args_internal(args, writer, paths)
}

fn handle_cli_args_internal<W: Write>(
    args: &[String],
    writer: &mut W,
    paths: Option<&super::registry::PlatformPaths>,
) -> Result<bool, Box<dyn std::error::Error>> {
    if args.is_empty() {
        return Ok(false);
    }

    let sub_args: &[String] = if is_known_subcommand_or_flag(&args[0]) {
        &args[0..]
    } else if args.len() > 1 {
        &args[1..]
    } else {
        return Ok(false);
    };

    let (binary_cmd, binary_args) = super::registry::resolve_binary_command();
    let get_agents = || match paths {
        Some(p) => super::registry::get_all_agents_with_paths(p),
        None => super::registry::get_all_agents(),
    };

    let cmd = sub_args[0].as_str();
    match cmd {
        "help" | "--help" | "-h" => {
            print_help(writer)?;
            Ok(true)
        }
        "list-agents" | "list" => {
            let agents = get_agents();
            let table = format_agents_table(&agents);
            write!(writer, "{}", table)?;
            Ok(true)
        }
        "install" => {
            let target_arg = sub_args.get(1).map(|s| s.as_str());
            match target_arg {
                Some("--help") | Some("-h") => {
                    writeln!(writer, "Usage: zenohx-mcp install [agent_id | --all]")?;
                }
                Some(id) if id != "--all" => {
                    let agents = get_agents();
                    match agents.into_iter().find(|a| a.id == id) {
                        Some(target) => {
                            match super::mutator::install_agent(&target, &binary_cmd, &binary_args) {
                                Ok(updated) => {
                                    writeln!(
                                        writer,
                                        "[OK] Installed to {}: {}",
                                        updated.name,
                                        updated.config_path.display()
                                    )?;
                                }
                                Err(e) => {
                                    writeln!(
                                        writer,
                                        "[ERROR] Failed to install {}: {}",
                                        target.name,
                                        e
                                    )?;
                                }
                            }
                        }
                        None => {
                            writeln!(writer, "[ERROR] Unknown agent ID: {}", id)?;
                        }
                    }
                }
                _ => {
                    let agents = get_agents();
                    for agent in &agents {
                        if agent.installed {
                            writeln!(writer, "[SKIP] Already installed: {}", agent.name)?;
                        } else if !agent.detected {
                            writeln!(writer, "[SKIP] Not detected: {}", agent.name)?;
                        } else {
                            match super::mutator::install_agent(agent, &binary_cmd, &binary_args) {
                                Ok(updated) => {
                                    writeln!(
                                        writer,
                                        "[OK] Installed to {}: {}",
                                        updated.name,
                                        updated.config_path.display()
                                    )?;
                                }
                                Err(e) => {
                                    writeln!(
                                        writer,
                                        "[ERROR] Failed to install {}: {}",
                                        agent.name,
                                        e
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
            Ok(true)
        }
        "uninstall" => {
            let target_arg = sub_args.get(1).map(|s| s.as_str());
            match target_arg {
                Some("--help") | Some("-h") => {
                    writeln!(writer, "Usage: zenohx-mcp uninstall [agent_id | --all]")?;
                }
                Some("--all") => {
                    let agents = get_agents();
                    for agent in &agents {
                        if agent.installed {
                            match super::mutator::uninstall_agent(agent) {
                                Ok(updated) => {
                                    writeln!(
                                        writer,
                                        "[OK] Uninstalled from {}: {}",
                                        updated.name,
                                        updated.config_path.display()
                                    )?;
                                }
                                Err(e) => {
                                    writeln!(
                                        writer,
                                        "[ERROR] Failed to uninstall {}: {}",
                                        agent.name,
                                        e
                                    )?;
                                }
                            }
                        } else {
                            writeln!(writer, "[SKIP] Not installed: {}", agent.name)?;
                        }
                    }
                }
                Some(id) => {
                    let agents = get_agents();
                    match agents.into_iter().find(|a| a.id == id) {
                        Some(target) => {
                            match super::mutator::uninstall_agent(&target) {
                                Ok(updated) => {
                                    writeln!(
                                        writer,
                                        "[OK] Uninstalled from {}: {}",
                                        updated.name,
                                        updated.config_path.display()
                                    )?;
                                }
                                Err(e) => {
                                    writeln!(
                                        writer,
                                        "[ERROR] Failed to uninstall {}: {}",
                                        target.name,
                                        e
                                    )?;
                                }
                            }
                        }
                        None => {
                            writeln!(writer, "[ERROR] Unknown agent ID: {}", id)?;
                        }
                    }
                }
                None => {
                    writeln!(
                        writer,
                        "[ERROR] Please specify an agent ID or --all to uninstall."
                    )?;
                    writeln!(writer, "Usage: zenohx-mcp uninstall [agent_id | --all]")?;
                }
            }
            Ok(true)
        }
        unknown => {
            writeln!(writer, "[ERROR] Unknown subcommand: {}", unknown)?;
            writeln!(writer, "Run 'zenohx-mcp --help' for usage.")?;
            Ok(true)
        }
    }
}

/// Handles CLI arguments for zenohx-mcp.
/// Returns Ok(true) if a CLI command was handled, Ok(false) if no command was given.
pub fn handle_cli_args(args: &[String]) -> Result<bool, Box<dyn std::error::Error>> {
    let mut stdout = std::io::stdout();
    handle_cli_args_with_writer(args, &mut stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::installer::types::{AgentTarget, ConfigFormat};
    use std::path::PathBuf;

    #[test]
    fn test_cli_no_subcommand() {
        let mut buf = Vec::new();
        // Empty args
        let res = handle_cli_args_with_writer(&[], &mut buf).unwrap();
        assert!(!res);

        // Only binary name
        let res = handle_cli_args_with_writer(&["zenohx-mcp".to_string()], &mut buf).unwrap();
        assert!(!res);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_cli_help_flags() {
        for flag in &["help", "--help", "-h"] {
            let mut buf = Vec::new();
            let res = handle_cli_args_with_writer(&[flag.to_string()], &mut buf).unwrap();
            assert!(res);
            let out = String::from_utf8_lossy(&buf).to_string();
            assert!(out.contains("USAGE:"));
            assert!(out.contains("SUBCOMMANDS:"));
            assert!(out.contains("list-agents"));
            assert!(out.contains("install"));
            assert!(out.contains("uninstall"));
            // Strictly no emojis: all characters must be ASCII
            assert!(out.chars().all(|c| c.is_ascii()));
        }
    }

    #[test]
    fn test_format_agents_table_structure_and_no_emojis() {
        let targets = vec![
            AgentTarget {
                id: "antigravity".to_string(),
                name: "Antigravity CLI".to_string(),
                detected: true,
                installed: true,
                config_path: PathBuf::from("/home/user/.gemini/config/mcp_config.json"),
                format: ConfigFormat::JsonMcpServers,
            },
            AgentTarget {
                id: "claude".to_string(),
                name: "Claude Desktop".to_string(),
                detected: true,
                installed: false,
                config_path: PathBuf::from("/home/user/.config/Claude/claude_desktop_config.json"),
                format: ConfigFormat::JsonMcpServers,
            },
            AgentTarget {
                id: "windsurf".to_string(),
                name: "Windsurf".to_string(),
                detected: false,
                installed: false,
                config_path: PathBuf::from("/home/user/.codeium/windsurf/mcp_config.json"),
                format: ConfigFormat::JsonMcpServers,
            },
        ];

        let table = format_agents_table(&targets);
        assert!(table.contains("AGENT ID"));
        assert!(table.contains("AGENT NAME"));
        assert!(table.contains("STATUS"));
        assert!(table.contains("CONFIG PATH"));
        assert!(table.contains("Installed"));
        assert!(table.contains("Detected"));
        assert!(table.contains("Not Detected"));
        assert!(table.contains("antigravity"));
        assert!(table.contains("claude"));
        assert!(table.contains("windsurf"));
        // Strictly no emojis
        assert!(table.chars().all(|c| c.is_ascii()));
    }

    #[test]
    fn test_cli_list_agents() {
        for cmd in &["list-agents", "list"] {
            let mut buf = Vec::new();
            let res = handle_cli_args_with_writer(
                &["zenohx-mcp".to_string(), cmd.to_string()],
                &mut buf,
            )
            .unwrap();
            assert!(res);
            let out = String::from_utf8_lossy(&buf).to_string();
            assert!(out.contains("AGENT ID"));
            assert!(out.contains("AGENT NAME"));
            assert!(out.contains("STATUS"));
            assert!(out.contains("CONFIG PATH"));
            assert!(out.chars().all(|c| c.is_ascii()));
        }
    }

    #[test]
    fn test_cli_install_unknown_agent() {
        let mut buf = Vec::new();
        let res = handle_cli_args_with_writer(
            &[
                "zenohx-mcp".to_string(),
                "install".to_string(),
                "unknown_agent_xyz".to_string(),
            ],
            &mut buf,
        )
        .unwrap();
        assert!(res);
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("[ERROR]"));
        assert!(out.contains("unknown_agent_xyz"));
        assert!(out.chars().all(|c| c.is_ascii()));
    }

    #[test]
    fn test_cli_uninstall_no_args() {
        let mut buf = Vec::new();
        let res = handle_cli_args_with_writer(
            &["zenohx-mcp".to_string(), "uninstall".to_string()],
            &mut buf,
        )
        .unwrap();
        assert!(res);
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("[ERROR]"));
        assert!(out.contains("--all"));
        assert!(out.chars().all(|c| c.is_ascii()));
    }

    #[test]
    fn test_cli_unknown_subcommand() {
        let mut buf = Vec::new();
        let res = handle_cli_args_with_writer(
            &["zenohx-mcp".to_string(), "unknown-subcmd".to_string()],
            &mut buf,
        )
        .unwrap();
        assert!(res);
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("[ERROR]"));
        assert!(out.contains("unknown-subcmd"));
        assert!(out.chars().all(|c| c.is_ascii()));
    }

    #[test]
    fn test_cli_subcommand_help_flag() {
        let mut buf = Vec::new();
        let res = handle_cli_args_with_writer(
            &["zenohx-mcp".to_string(), "install".to_string(), "--help".to_string()],
            &mut buf,
        )
        .unwrap();
        assert!(res);
        let out = String::from_utf8_lossy(&buf).to_string();
        assert!(out.contains("Usage: zenohx-mcp install"));
        assert!(out.chars().all(|c| c.is_ascii()));

        buf.clear();
        let res = handle_cli_args_with_writer(
            &["zenohx-mcp".to_string(), "uninstall".to_string(), "-h".to_string()],
            &mut buf,
        )
        .unwrap();
        assert!(res);
        let out = String::from_utf8_lossy(&buf).to_string();
        assert!(out.contains("Usage: zenohx-mcp uninstall"));
        assert!(out.chars().all(|c| c.is_ascii()));
    }

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("zenohx-cli-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &PathBuf {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_cli_install_all_and_uninstall_all() {
        let temp_dir = TempDir::new();
        let paths = crate::mcp::installer::registry::PlatformPaths {
            os: crate::mcp::installer::registry::OsKind::Linux,
            home: temp_dir.path().clone(),
            xdg_config: temp_dir.path().join(".config"),
            appdata: temp_dir.path().join("AppData"),
            macos_app_support: temp_dir.path().join("Library/Application Support"),
        };

        // 1. In empty temp dir, all agents should be [SKIP] Not detected
        let mut buf = Vec::new();
        let res = handle_cli_args_with_writer_and_paths(
            &["zenohx-mcp".to_string(), "install".to_string(), "--all".to_string()],
            &mut buf,
            Some(&paths),
        )
        .unwrap();
        assert!(res);
        let out = String::from_utf8_lossy(&buf).to_string();
        assert!(out.contains("[SKIP] Not detected:"));
        assert!(out.chars().all(|c| c.is_ascii()));

        // 2. Create a mock app directory to simulate a detected agent (Cursor)
        let cursor_dir = temp_dir.path().join(".cursor");
        std::fs::create_dir_all(&cursor_dir).unwrap();

        buf.clear();
        let res = handle_cli_args_with_writer_and_paths(
            &["zenohx-mcp".to_string(), "install".to_string(), "--all".to_string()],
            &mut buf,
            Some(&paths),
        )
        .unwrap();
        assert!(res);
        let out = String::from_utf8_lossy(&buf).to_string();
        assert!(out.contains("[OK] Installed to Cursor IDE:"));
        assert!(out.chars().all(|c| c.is_ascii()));

        // 3. Running install again should show [SKIP] Already installed for Cursor
        buf.clear();
        let res = handle_cli_args_with_writer_and_paths(
            &["zenohx-mcp".to_string(), "install".to_string(), "--all".to_string()],
            &mut buf,
            Some(&paths),
        )
        .unwrap();
        assert!(res);
        let out = String::from_utf8_lossy(&buf).to_string();
        assert!(out.contains("[SKIP] Already installed: Cursor IDE"));

        // 4. Uninstall --all should uninstall Cursor
        buf.clear();
        let res = handle_cli_args_with_writer_and_paths(
            &["zenohx-mcp".to_string(), "uninstall".to_string(), "--all".to_string()],
            &mut buf,
            Some(&paths),
        )
        .unwrap();
        assert!(res);
        let out = String::from_utf8_lossy(&buf).to_string();
        assert!(out.contains("[OK] Uninstalled from Cursor IDE:"));
        assert!(out.chars().all(|c| c.is_ascii()));
    }
}
