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
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use crate::AppState;
use super::get_socket_path;
use super::types::{IpcRequest, IpcResponse};

/// Maximum allowed frame size for IPC stream requests (16 MB).
pub const MAX_FRAME_SIZE: u64 = 16 * 1024 * 1024; // 16 MB

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
        IpcRequest::ExecuteTool { tool, args } => {
            if let Some(app) = app_handle {
                if tool != "zenohx_gui_switch_workspace" && tool != "zenoh_publish" {
                    let _ = app.emit(
                        "zenohx://mcp-action",
                        serde_json::json!({
                            "action": tool.clone(),
                            "details": format!("AI executed {}", tool)
                        }),
                    );
                }
            }
            if tool == "zenohx_gui_switch_workspace" {
                let default_state = crate::mcp::tools::get_headless_state().await;
                crate::mcp::tools::execute_tool_on_state_with_mode(&tool, args, default_state, app_handle, "live_gui").await
            } else if let Some(s) = state {
                crate::mcp::tools::execute_tool_on_state_with_mode(&tool, args, s, app_handle, "live_gui").await
            } else {
                IpcResponse::err("live_gui", "AppState not available")
            }
        }
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
    loop {
        line.clear();
        let bytes = match (&mut buf_reader).take(MAX_FRAME_SIZE + 1).read_line(&mut line).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                eprintln!("[ZenohX IPC] Read error: {}", e);
                break;
            }
        };

        if bytes > MAX_FRAME_SIZE as usize {
            let resp = IpcResponse::err(
                "live_gui",
                format!(
                    "Request frame exceeded maximum allowed size of {} bytes",
                    MAX_FRAME_SIZE
                ),
            );
            if let Ok(mut out) = serde_json::to_string(&resp) {
                out.push('\n');
                let _ = writer.write_all(out.as_bytes()).await;
                let _ = writer.flush().await;
            }
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
        if line.capacity() > 64 * 1024 {
            line = String::new();
        }
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
            tool: "non_existent_tool".to_string(),
            args: json!({}),
        };
        let resp = handle_request_raw(req, Some(&state), None).await;
        assert!(!resp.success);
        assert_eq!(resp.mode, "live_gui");
        assert!(resp
            .error
            .unwrap()
            .contains("Unknown tool: non_existent_tool"));
    }

    #[tokio::test]
    async fn test_execute_tool_supported_with_state() {
        let state = create_test_state();
        let req = IpcRequest::ExecuteTool {
            tool: "zenoh_get_sessions".to_string(),
            args: json!({}),
        };
        let resp = handle_request_raw(req, Some(&state), None).await;
        assert!(resp.success);
        assert_eq!(resp.data.as_array().unwrap().len(), 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_bind_unix_listener_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let id = uuid::Uuid::new_v4().simple().to_string();
        let socket_path = std::path::PathBuf::from(format!("/tmp/zx-p-{}.sock", &id[..8]));

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

        let id = uuid::Uuid::new_v4().simple().to_string();
        let socket_path = std::path::PathBuf::from(format!("/tmp/zx-rt-{}.sock", &id[..8]));

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

    #[cfg(unix)]
    #[tokio::test]
    async fn test_handle_connection_rejects_oversized_frame() {
        let (client_stream, server_stream) =
            tokio::net::UnixStream::pair().expect("unix stream pair");

        let server_handle = tokio::spawn(async move {
            handle_connection(server_stream, None, None).await;
        });

        let (mut client_reader, mut client_writer) = client_stream.into_split();

        // Stream 17 chunks of 1 MB without newline to exceed MAX_FRAME_SIZE (16 MB)
        let write_handle = tokio::spawn(async move {
            let chunk = vec![b'a'; 1024 * 1024];
            for _ in 0..17 {
                if client_writer.write_all(&chunk).await.is_err() {
                    break;
                }
            }
            let _ = client_writer.write_all(b"\n").await;
            let _ = client_writer.flush().await;
        });

        let mut buf_reader = BufReader::new(&mut client_reader);
        let mut response_line = String::new();
        buf_reader
            .read_line(&mut response_line)
            .await
            .expect("read response");

        let resp: IpcResponse =
            serde_json::from_str(&response_line).expect("parse error response");
        assert!(!resp.success);
        assert_eq!(resp.mode, "live_gui");
        assert!(
            resp.error
                .expect("error message")
                .contains("Request frame exceeded maximum allowed size"),
            "Expected frame size limit error"
        );

        let _ = write_handle.await;
        let _ = server_handle.await;
    }
}

