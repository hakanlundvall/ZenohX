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

use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::AppState;
use super::get_socket_path;
use super::types::{IpcRequest, IpcResponse};

/// Dispatches raw IPC requests received over the Unix socket or named pipe.
pub async fn handle_request_raw(
    req: IpcRequest,
    state: Option<&AppState>,
    app_handle: Option<&AppHandle>,
) -> IpcResponse {
    match req {
        IpcRequest::Ping => IpcResponse::ok("live_gui", serde_json::json!({ "status": "pong" })),
        IpcRequest::GetState => {
            let active_tab = "pubsub";
            let connected = match state {
                Some(s) => !s.session_manager.get_all_sessions().await.is_empty(),
                None => false,
            };
            IpcResponse::ok(
                "live_gui",
                serde_json::json!({
                    "connected": connected,
                    "active_tab": active_tab,
                }),
            )
        }
        IpcRequest::ExecuteTool { tool, args } => match tool.as_str() {
            "zenohx_gui_switch_workspace" => {
                let workspace = args
                    .get("workspace")
                    .and_then(|w| w.as_str())
                    .unwrap_or("pubsub");
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "zenohx://gui-switch-tab",
                        serde_json::json!({ "workspace": workspace }),
                    );
                    let _ = app.emit(
                        "zenohx://mcp-action",
                        serde_json::json!({
                            "action": "Switch Workspace",
                            "details": format!("Switched to tab '{}'", workspace)
                        }),
                    );
                }
                IpcResponse::ok("live_gui", serde_json::json!({ "switched_to": workspace }))
            }
            _ => {
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "zenohx://mcp-action",
                        serde_json::json!({
                            "action": tool.clone(),
                            "details": format!("AI executed {}", tool)
                        }),
                    );
                }
                if let Some(_s) = state {
                    // Note: Task 4 will implement crate::mcp::tools::execute_tool_on_state
                    IpcResponse::err(
                        "live_gui",
                        format!("Tool '{}' execution not supported in live GUI yet", tool),
                    )
                } else {
                    IpcResponse::err("live_gui", "AppState not available")
                }
            }
        },
    }
}

/// Binds a Unix domain socket at `socket_path` with strict 0600 file permissions.
#[cfg(unix)]
pub fn bind_unix_listener(socket_path: &std::path::Path) -> std::io::Result<tokio::net::UnixListener> {
    use std::os::unix::fs::PermissionsExt;

    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if socket_path.exists() {
        let _ = std::fs::remove_file(socket_path);
    }

    let listener = tokio::net::UnixListener::bind(socket_path)?;
    let _ = std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600));
    Ok(listener)
}

/// Handles a single incoming Unix stream connection, parsing newline-delimited requests.
#[cfg(unix)]
pub async fn handle_connection(
    stream: tokio::net::UnixStream,
    state: Option<Arc<AppState>>,
    app_handle: Option<AppHandle>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();
    while let Ok(bytes) = buf_reader.read_line(&mut line).await {
        if bytes == 0 {
            break;
        }
        let resp = match serde_json::from_str::<IpcRequest>(&line) {
            Ok(req) => handle_request_raw(req, state.as_deref(), app_handle.as_ref()).await,
            Err(e) => IpcResponse::err("live_gui", format!("Invalid JSON request: {}", e)),
        };
        if let Ok(mut out) = serde_json::to_string(&resp) {
            out.push('\n');
            let _ = writer.write_all(out.as_bytes()).await;
            let _ = writer.flush().await;
        }
        line.clear();
    }
}

/// Starts the background IPC server in the Tauri runtime.
pub fn start_ipc_server(app_handle: AppHandle, state: Arc<AppState>) {
    #[cfg(unix)]
    tauri::async_runtime::spawn(async move {
        let socket_path = get_socket_path();
        let listener = match bind_unix_listener(&socket_path) {
            Ok(l) => l,
            Err(e) => {
                eprintln!(
                    "[ZenohX IPC] Failed to bind Unix socket at {:?}: {}",
                    socket_path, e
                );
                return;
            }
        };

        eprintln!("[ZenohX IPC] Server listening on {:?}", socket_path);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let state_clone = state.clone();
                    let app_clone = app_handle.clone();
                    tokio::spawn(async move {
                        handle_connection(stream, Some(state_clone), Some(app_clone)).await;
                    });
                }
                Err(e) => {
                    eprintln!("[ZenohX IPC] Accept error: {}", e);
                    break;
                }
            }
        }
    });

    #[cfg(not(unix))]
    {
        let _ = (app_handle, state);
        eprintln!("[ZenohX IPC] Server on non-unix platform not implemented yet");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::types::IpcRequest;
    use serde_json::json;

    fn create_test_state() -> AppState {
        let session_manager = crate::zenoh::SessionManager::new();
        let db = crate::db::Database::new_in_memory().expect("in-memory db");
        let mdns_manager = crate::mdns::MdnsManager::new("zenohx-test", 7447);
        AppState {
            session_manager,
            db,
            mdns_manager,
        }
    }

    #[tokio::test]
    async fn test_ping_handler() {
        let req = IpcRequest::Ping;
        let resp = handle_request_raw(req, None, None).await;
        assert!(resp.success);
        assert_eq!(resp.mode, "live_gui");
        assert_eq!(resp.data["status"], "pong");
        assert!(resp.error.is_none());
    }

    #[tokio::test]
    async fn test_get_state_handler_none_state() {
        let req = IpcRequest::GetState;
        let resp = handle_request_raw(req, None, None).await;
        assert!(resp.success);
        assert_eq!(resp.mode, "live_gui");
        assert_eq!(resp.data["connected"], false);
        assert_eq!(resp.data["active_tab"], "pubsub");
    }

    #[tokio::test]
    async fn test_get_state_handler_with_state() {
        let state = create_test_state();
        let req = IpcRequest::GetState;
        let resp = handle_request_raw(req, Some(&state), None).await;
        assert!(resp.success);
        assert_eq!(resp.mode, "live_gui");
        assert_eq!(resp.data["connected"], false);
        assert_eq!(resp.data["active_tab"], "pubsub");
    }

    #[tokio::test]
    async fn test_switch_workspace_handler() {
        let req = IpcRequest::ExecuteTool {
            tool: "zenohx_gui_switch_workspace".to_string(),
            args: json!({ "workspace": "query" }),
        };
        let resp = handle_request_raw(req, None, None).await;
        assert!(resp.success);
        assert_eq!(resp.mode, "live_gui");
        assert_eq!(resp.data["switched_to"], "query");
    }

    #[tokio::test]
    async fn test_switch_workspace_default() {
        let req = IpcRequest::ExecuteTool {
            tool: "zenohx_gui_switch_workspace".to_string(),
            args: json!({}),
        };
        let resp = handle_request_raw(req, None, None).await;
        assert!(resp.success);
        assert_eq!(resp.mode, "live_gui");
        assert_eq!(resp.data["switched_to"], "pubsub");
    }

    #[tokio::test]
    async fn test_execute_tool_without_state() {
        let req = IpcRequest::ExecuteTool {
            tool: "zenoh_scout".to_string(),
            args: json!({}),
        };
        let resp = handle_request_raw(req, None, None).await;
        assert!(!resp.success);
        assert_eq!(resp.mode, "live_gui");
        assert_eq!(resp.error, Some("AppState not available".to_string()));
    }

    #[tokio::test]
    async fn test_execute_tool_unsupported_with_state() {
        let state = create_test_state();
        let req = IpcRequest::ExecuteTool {
            tool: "zenoh_scout".to_string(),
            args: json!({}),
        };
        let resp = handle_request_raw(req, Some(&state), None).await;
        assert!(!resp.success);
        assert_eq!(resp.mode, "live_gui");
        assert!(resp
            .error
            .unwrap()
            .contains("Tool 'zenoh_scout' execution not supported in live GUI yet"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_bind_unix_listener_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = std::env::temp_dir();
        let socket_path = temp_dir.join(format!("zenohx-test-perm-{}.sock", uuid::Uuid::new_v4()));

        let _listener = bind_unix_listener(&socket_path).expect("bind socket");
        assert!(socket_path.exists());

        let metadata = std::fs::metadata(&socket_path).expect("socket metadata");
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "Socket permissions must be 0600, found {:o}",
            mode
        );

        let _ = std::fs::remove_file(&socket_path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_server_client_roundtrip() {
        use crate::ipc::IpcClient;

        let temp_dir = std::env::temp_dir();
        let socket_path = temp_dir.join(format!("zenohx-test-rt-{}.sock", uuid::Uuid::new_v4()));

        let listener = bind_unix_listener(&socket_path).expect("bind listener");
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let server_task = tokio::spawn(async move {
            tokio::select! {
                _ = async {
                    while let Ok((stream, _)) = listener.accept().await {
                        tokio::spawn(async move {
                            handle_connection(stream, None, None).await;
                        });
                    }
                } => {},
                _ = &mut shutdown_rx => {},
            }
        });

        let mut client = IpcClient::connect_to(&socket_path).await.expect("client connect");

        // 1. Test Ping
        let ping_resp = client.call(&IpcRequest::Ping).await.expect("ping call");
        assert!(ping_resp.success);
        assert_eq!(ping_resp.data["status"], "pong");

        // 2. Test GetState
        let state_resp = client.call(&IpcRequest::GetState).await.expect("get_state call");
        assert!(state_resp.success);
        assert_eq!(state_resp.data["active_tab"], "pubsub");

        // 3. Test ExecuteTool
        let tool_resp = client
            .call(&IpcRequest::ExecuteTool {
                tool: "zenohx_gui_switch_workspace".to_string(),
                args: json!({ "workspace": "admin" }),
            })
            .await
            .expect("tool call");
        assert!(tool_resp.success);
        assert_eq!(tool_resp.data["switched_to"], "admin");

        let _ = shutdown_tx.send(());
        let _ = server_task.await;
        let _ = std::fs::remove_file(&socket_path);
    }
}

