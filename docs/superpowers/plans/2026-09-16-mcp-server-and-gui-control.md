# ZenohX Model Context Protocol (MCP) Server & Live GUI Control Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a native Rust Model Context Protocol (MCP) server for ZenohX that enables AI agents to control the live desktop app (pub/sub, queries, sessions, workspace navigation) via local IPC, while gracefully falling back to headless background execution when the desktop app is closed.

**Architecture:** A dual-mode system consisting of:
1. An async local IPC server in the Tauri backend (`UnixListener` / Windows Named Pipe) dispatching GUI & Zenoh commands and emitting UI synchronization events.
2. A standalone native Rust CLI (`zenohx-mcp`) speaking MCP JSON-RPC 2.0 over `stdio`. It connects to the GUI IPC socket if active, or initializes a headless `SessionManager` & SQLite `Database` if closed.
3. A React hook (`useMcpListener`) in the frontend reacting to workspace switch events and displaying AI activity toasts.

**Tech Stack:** Rust (Tauri 2, Tokio, Zenoh 1.10, Rusqlite, Serde JSON), TypeScript / React 18, Zustand.

**Spec:** [`docs/superpowers/specs/2026-09-16-mcp-server-and-gui-control-design.md`](file:///home/khanhdew/Documents/Keemos/zenohx/docs/superpowers/specs/2026-09-16-mcp-server-and-gui-control-design.md)

## Global Constraints
- Target binary: `zenohx-mcp` (in `src-tauri/src/bin/zenohx-mcp.rs`).
- Stdio isolation: MCP communicates over `stdin`/`stdout`. Any debugging or logging in `zenohx-mcp` MUST go to `stderr`.
- Socket path: `$XDG_RUNTIME_DIR/zenohx.sock` (fallback `~/.zenohx/zenohx.sock`) on Unix with `0600` permissions; `\\.\pipe\zenohx-ipc` on Windows.
- Backward compatibility: Existing Tauri commands, database schemas, and React UI components must remain 100% functional.
- Zero extra runtime requirements: Pure Rust binary, no Node.js runtime needed to run the MCP server.

---

### Task 1: IPC Subsystem Core & Framing

**Files:**
- Create: `src-tauri/src/ipc/mod.rs`
- Create: `src-tauri/src/ipc/types.rs`
- Create: `src-tauri/src/ipc/client.rs`
- Test: `src-tauri/src/ipc/tests.rs`

**Interfaces:**
- Consumes: None (pure types & socket transport).
- Produces:
  - `pub enum IpcRequest { Ping, ExecuteTool { tool: String, args: serde_json::Value }, GetState }`
  - `pub struct IpcResponse { pub success: bool, pub mode: String, pub data: serde_json::Value, pub error: Option<String> }`
  - `pub struct IpcClient`: `pub async fn connect() -> Result<Self, String>`, `pub async fn call(&mut self, req: &IpcRequest) -> Result<IpcResponse, String>`
  - `pub fn get_socket_path() -> std::path::PathBuf`

- [ ] **Step 1: Write failing tests for IPC types, framing, and socket path resolution**

```rust
// src-tauri/src/ipc/tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::types::{IpcRequest, IpcResponse};
    use serde_json::json;

    #[test]
    fn test_ipc_request_serialization_roundtrip() {
        let req = IpcRequest::ExecuteTool {
            tool: "zenoh_scout".to_string(),
            args: json!({ "timeout_ms": 500 }),
        };
        let serialized = serde_json::to_string(&req).expect("serialize");
        let deserialized: IpcRequest = serde_json::from_str(&serialized).expect("deserialize");
        match deserialized {
            IpcRequest::ExecuteTool { tool, args } => {
                assert_eq!(tool, "zenoh_scout");
                assert_eq!(args["timeout_ms"], 500);
            }
            _ => panic!("unexpected request type"),
        }
    }

    #[test]
    fn test_ipc_response_serialization_roundtrip() {
        let resp = IpcResponse {
            success: true,
            mode: "live_gui".to_string(),
            data: json!({ "count": 2 }),
            error: None,
        };
        let serialized = serde_json::to_string(&resp).expect("serialize");
        let deserialized: IpcResponse = serde_json::from_str(&serialized).expect("deserialize");
        assert!(deserialized.success);
        assert_eq!(deserialized.mode, "live_gui");
        assert_eq!(deserialized.data["count"], 2);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ipc::tests`
Expected: FAIL with module `ipc` not found.

- [ ] **Step 3: Implement IPC types, socket path resolution, and client**

Create `src-tauri/src/ipc/types.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum IpcRequest {
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "execute_tool")]
    ExecuteTool {
        tool: String,
        args: serde_json::Value,
    },
    #[serde(rename = "get_state")]
    GetState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub success: bool,
    pub mode: String,
    pub data: serde_json::Value,
    pub error: Option<String>,
}

impl IpcResponse {
    pub fn ok(mode: &str, data: serde_json::Value) -> Self {
        Self {
            success: true,
            mode: mode.to_string(),
            data,
            error: None,
        }
    }

    pub fn err(mode: &str, err: impl Into<String>) -> Self {
        Self {
            success: false,
            mode: mode.to_string(),
            data: serde_json::Value::Null,
            error: Some(err.into()),
        }
    }
}
```

Create `src-tauri/src/ipc/mod.rs`:
```rust
pub mod client;
pub mod server;
pub mod types;

#[cfg(test)]
mod tests;

pub use client::IpcClient;
pub use types::{IpcRequest, IpcResponse};

use std::path::PathBuf;

pub fn get_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        if !runtime_dir.trim().is_empty() {
            return PathBuf::from(runtime_dir).join("zenohx.sock");
        }
    }

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".zenohx").join("zenohx.sock")
}
```

Create `src-tauri/src/ipc/client.rs` using `tokio::net::UnixStream` on unix with framing:
```rust
use super::types::{IpcRequest, IpcResponse};
use super::get_socket_path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct IpcClient {
    #[cfg(unix)]
    stream: tokio::net::UnixStream,
}

impl IpcClient {
    pub async fn connect() -> Result<Self, String> {
        let path = get_socket_path();
        #[cfg(unix)]
        {
            let stream = tokio::net::UnixStream::connect(&path)
                .await
                .map_err(|e| format!("Failed to connect to IPC socket at {:?}: {}", path, e))?;
            Ok(Self { stream })
        }
        #[cfg(not(unix))]
        {
            Err("IPC on non-unix not implemented yet".to_string())
        }
    }

    pub async fn call(&mut self, req: &IpcRequest) -> Result<IpcResponse, String> {
        #[cfg(unix)]
        {
            let mut line = serde_json::to_string(req).map_err(|e| e.to_string())?;
            line.push('\n');

            let (reader, mut writer) = self.stream.split();
            writer.write_all(line.as_bytes()).await.map_err(|e| e.to_string())?;
            writer.flush().await.map_err(|e| e.to_string())?;

            let mut buf_reader = BufReader::new(reader);
            let mut response_line = String::new();
            buf_reader
                .read_line(&mut response_line)
                .await
                .map_err(|e| e.to_string())?;

            if response_line.trim().is_empty() {
                return Err("Empty response from ZenohX IPC server".to_string());
            }

            serde_json::from_str(&response_line).map_err(|e| format!("Failed to parse response: {}", e))
        }
        #[cfg(not(unix))]
        {
            Err("IPC on non-unix not implemented yet".to_string())
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ipc::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ipc
git commit -m "feat(ipc): add IPC types, socket path resolution, and client framing"
```

---

### Task 2: IPC Server in Tauri Backend

**Files:**
- Create: `src-tauri/src/ipc/server.rs`
- Modify: `src-tauri/src/lib.rs:25-87`
- Test: `src-tauri/src/ipc/server.rs` (in-tree mock test)

**Interfaces:**
- Consumes: `AppState`, `tauri::AppHandle`, `IpcRequest`, `IpcResponse`
- Produces:
  - `pub fn start_ipc_server(app_handle: tauri::AppHandle, state: std::sync::Arc<AppState>)`
  - Handles incoming socket connections, invokes tools via `AppState`, emits `zenohx://gui-switch-tab` and `zenohx://mcp-action`.

- [ ] **Step 1: Write failing test for IPC server request handling**

```rust
// In src-tauri/src/ipc/server.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::types::IpcRequest;

    #[tokio::test]
    async fn test_ping_handler() {
        let req = IpcRequest::Ping;
        let resp = handle_request_raw(req, None, None).await;
        assert!(resp.success);
        assert_eq!(resp.mode, "live_gui");
        assert_eq!(resp.data["status"], "pong");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ipc::server::tests`
Expected: FAIL with `handle_request_raw` unresolved.

- [ ] **Step 3: Implement IPC Server and Request Router**

In `src-tauri/src/ipc/server.rs`:
```rust
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tauri::{AppHandle, Emitter};
use crate::AppState;
use super::types::{IpcRequest, IpcResponse};
use super::get_socket_path;

pub async fn handle_request_raw(
    req: IpcRequest,
    state: Option<&AppState>,
    app_handle: Option<&AppHandle>,
) -> IpcResponse {
    match req {
        IpcRequest::Ping => IpcResponse::ok("live_gui", serde_json::json!({ "status": "pong" })),
        IpcRequest::GetState => {
            let active_tab = "pubsub";
            IpcResponse::ok("live_gui", serde_json::json!({
                "connected": state.map(|s| !s.session_manager.get_all_sessions().is_empty()).unwrap_or(false),
                "active_tab": active_tab,
            }))
        }
        IpcRequest::ExecuteTool { tool, args } => {
            match tool.as_str() {
                "zenohx_gui_switch_workspace" => {
                    let workspace = args.get("workspace").and_then(|w| w.as_str()).unwrap_or("pubsub");
                    if let Some(app) = app_handle {
                        let _ = app.emit("zenohx://gui-switch-tab", serde_json::json!({ "workspace": workspace }));
                        let _ = app.emit("zenohx://mcp-action", serde_json::json!({
                            "action": "Switch Workspace",
                            "details": format!("Switched to tab '{}'", workspace)
                        }));
                    }
                    IpcResponse::ok("live_gui", serde_json::json!({ "switched_to": workspace }))
                }
                _ => {
                    if let Some(app) = app_handle {
                        let _ = app.emit("zenohx://mcp-action", serde_json::json!({
                            "action": tool.clone(),
                            "details": format!("AI executed {}", tool)
                        }));
                    }
                    // Dispatch to tool executor
                    if let Some(s) = state {
                        crate::mcp::tools::execute_tool_on_state(&tool, args, s, app_handle).await
                    } else {
                        IpcResponse::err("live_gui", "AppState not available")
                    }
                }
            }
        }
    }
}

pub fn start_ipc_server(app_handle: AppHandle, state: Arc<AppState>) {
    #[cfg(unix)]
    tauri::async_runtime::spawn(async move {
        let socket_path = get_socket_path();
        if let Some(parent) = socket_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }

        let listener = match tokio::net::UnixListener::bind(&socket_path) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[ZenohX IPC] Failed to bind Unix socket at {:?}: {}", socket_path, e);
                return;
            }
        };

        // Set 0600 permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600));
        }

        eprintln!("[ZenohX IPC] Server listening on {:?}", socket_path);

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let state_clone = state.clone();
                    let app_clone = app_handle.clone();
                    tokio::spawn(async move {
                        let (reader, mut writer) = stream.into_split();
                        let mut buf_reader = BufReader::new(reader);
                        let mut line = String::new();
                        while let Ok(bytes) = buf_reader.read_line(&mut line).await {
                            if bytes == 0 { break; }
                            if let Ok(req) = serde_json::from_str::<IpcRequest>(&line) {
                                let resp = handle_request_raw(req, Some(&state_clone), Some(&app_clone)).await;
                                if let Ok(mut out) = serde_json::to_string(&resp) {
                                    out.push('\n');
                                    let _ = writer.write_all(out.as_bytes()).await;
                                    let _ = writer.flush().await;
                                }
                            }
                            line.clear();
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[ZenohX IPC] Accept error: {}", e);
                    break;
                }
            }
        }
    });
}
```

In `src-tauri/src/lib.rs`:
Start the IPC server in `.setup()`:
```rust
let app_state_arc = Arc::new(AppState {
    session_manager: sm_clone.clone(),
    db,
    mdns_manager,
});
crate::ipc::server::start_ipc_server(app.handle().clone(), app_state_arc.clone());
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ipc::server::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ipc src-tauri/src/lib.rs
git commit -m "feat(ipc): implement Tauri IPC server and request dispatcher"
```

---

### Task 3: MCP Protocol Engine & JSON-RPC 2.0 Handler

**Files:**
- Create: `src-tauri/src/mcp/mod.rs`
- Create: `src-tauri/src/mcp/protocol.rs`
- Test: `src-tauri/src/mcp/protocol_tests.rs`

**Interfaces:**
- Consumes: JSON-RPC 2.0 requests from `stdin`.
- Produces:
  - Protocol models: `JsonRpcRequest`, `JsonRpcResponse`, `McpTool`, `McpResource`.
  - Stdio event loop function: `pub async fn run_mcp_stdio_server() -> Result<(), Box<dyn std::error::Error>>`

- [ ] **Step 1: Write failing tests for JSON-RPC MCP initialize and tools/list parsing**

```rust
// src-tauri/src/mcp/protocol_tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::{handle_jsonrpc_message, JsonRpcRequest};
    use serde_json::json;

    #[tokio::test]
    async fn test_mcp_initialize() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: Some(json!({
                "protocolVersion": "2024-11-05",
                "clientInfo": { "name": "test-client", "version": "1.0" }
            })),
        };
        let resp = handle_jsonrpc_message(req).await;
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Some(json!(1)));
        assert!(resp.error.is_none());
        let result = resp.result.expect("result present");
        assert_eq!(result["serverInfo"]["name"], "zenohx-mcp");
    }

    #[tokio::test]
    async fn test_mcp_tools_list() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(2)),
            method: "tools/list".to_string(),
            params: None,
        };
        let resp = handle_jsonrpc_message(req).await;
        let result = resp.result.expect("result present");
        let tools = result["tools"].as_array().expect("tools array");
        assert!(tools.iter().any(|t| t["name"] == "zenoh_scout"));
        assert!(tools.iter().any(|t| t["name"] == "zenoh_publish"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml mcp::protocol_tests`
Expected: FAIL with module `mcp` unresolved.

- [ ] **Step 3: Implement JSON-RPC 2.0 MCP Protocol Engine**

In `src-tauri/src/mcp/protocol.rs`:
```rust
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub async fn handle_jsonrpc_message(req: JsonRpcRequest) -> JsonRpcResponse {
    let id = req.id.clone();
    match req.method.as_str() {
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "listChanged": false }
                },
                "serverInfo": {
                    "name": "zenohx-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            error: None,
        },
        "notifications/initialized" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({})),
            error: None,
        },
        "ping" => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({})),
            error: None,
        },
        "tools/list" => {
            let tools = super::tools::get_tool_definitions();
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({ "tools": tools })),
                error: None,
            }
        },
        "resources/list" => {
            let resources = super::tools::get_resource_definitions();
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({ "resources": resources })),
                error: None,
            }
        },
        "tools/call" => {
            let params = req.params.unwrap_or(json!({}));
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

            let tool_output = super::tools::dispatch_mcp_tool(tool_name, arguments).await;
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "content": [{
                        "type": "text",
                        "text": tool_output.text
                    }],
                    "isError": tool_output.is_error
                })),
                error: None,
            }
        },
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method '{}' not found", req.method),
                data: None,
            }),
        },
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml mcp::protocol_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/mcp
git commit -m "feat(mcp): implement MCP JSON-RPC 2.0 protocol engine"
```

---

### Task 4: MCP Tools & Dispatcher (Scout, Pub/Sub, Query, GUI)

**Files:**
- Create: `src-tauri/src/mcp/tools.rs`
- Modify: `src-tauri/src/mcp/mod.rs`
- Test: `src-tauri/src/mcp/tools.rs` (unit tests for tool execution & schemas)

**Interfaces:**
- Consumes: `IpcClient` (for GUI mode), `SessionManager`, `Database` (for Headless mode).
- Produces:
  - `pub fn get_tool_definitions() -> Vec<serde_json::Value>`
  - `pub fn get_resource_definitions() -> Vec<serde_json::Value>`
  - `pub async fn dispatch_mcp_tool(name: &str, args: serde_json::Value) -> McpToolResult`
  - `pub async fn execute_tool_on_state(tool: &str, args: serde_json::Value, state: &AppState, app_handle: Option<&AppHandle>) -> IpcResponse`

- [ ] **Step 1: Write failing test for tool registry and parameter validation**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions_validity() {
        let tools = get_tool_definitions();
        assert!(tools.len() >= 10);
        for tool in tools {
            assert!(tool.get("name").is_some());
            assert!(tool.get("description").is_some());
            assert!(tool.get("inputSchema").is_some());
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml mcp::tools::tests`
Expected: FAIL with `get_tool_definitions` not found.

- [ ] **Step 3: Implement Tool Definitions and Dual-Mode Execution Logic**

In `src-tauri/src/mcp/tools.rs`:
Define schemas for all 13 tools:
- `zenoh_scout`, `zenoh_connect_session`, `zenoh_disconnect_session`, `zenoh_get_sessions`
- `zenoh_publish`, `zenoh_subscribe`, `zenoh_unsubscribe`, `zenoh_get_messages`
- `zenoh_query`, `zenoh_declare_queryable`, `zenoh_inspect_topology`
- `zenohx_gui_switch_workspace`, `zenohx_gui_get_state`

Implement `dispatch_mcp_tool(name, args)`:
1. Attempt to connect to local IPC socket via `IpcClient::connect().await`.
2. If connection succeeds:
   - Call `client.call(&IpcRequest::ExecuteTool { tool: name.to_string(), args })`.
   - Format response with `[Mode: Live GUI]`.
3. If connection fails:
   - If tool is a GUI tool (`zenohx_gui_switch_workspace`): return error `"Desktop GUI is not running; cannot switch workspace"`.
   - For Zenoh tools: lazily obtain Headless `AppState` (instantiate `SessionManager` & load `zenohx.db`).
   - Call `execute_tool_on_state(name, args, &headless_state, None)`.
   - Format response with `[Mode: Headless (GUI not running)]`.

Implement `execute_tool_on_state`:
- `zenoh_scout`: calls `state.session_manager.scout(None, timeout)`
- `zenoh_connect_session`: calls `state.session_manager.connect(config)`
- `zenoh_disconnect_session`: calls `state.session_manager.disconnect(session_id)`
- `zenoh_get_sessions`: calls `state.session_manager.get_all_sessions()`
- `zenoh_publish`: calls `state.session_manager.publish_with_options(...)` and saves to DB.
- `zenoh_get_messages`: queries messages from `state.db`.
- `zenoh_query`: calls `state.session_manager.query(...)`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml mcp::tools::tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/mcp/tools.rs src-tauri/src/mcp/mod.rs
git commit -m "feat(mcp): implement tool definitions and dual-mode dispatch logic"
```

---

### Task 5: Standalone CLI Binary (`zenohx-mcp`)

**Files:**
- Create: `src-tauri/src/bin/zenohx-mcp.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `package.json`
- Test: `src-tauri/src/bin/zenohx-mcp.rs` (CLI invocation test)

**Interfaces:**
- Consumes: `zenohx_lib::mcp::protocol::handle_jsonrpc_message`
- Produces: Standalone binary `zenohx-mcp` executing stdio loop.

- [ ] **Step 1: Configure Cargo.toml for `zenohx-mcp` binary**

Modify `src-tauri/Cargo.toml`:
```toml
[[bin]]
name = "zenohx-mcp"
path = "src/bin/zenohx-mcp.rs"
```

- [ ] **Step 2: Implement `src/bin/zenohx-mcp.rs`**

```rust
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use zenohx_lib::mcp::protocol::{handle_jsonrpc_message, JsonRpcRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("[zenohx-mcp] Starting ZenohX MCP server on stdio (PID: {})...", std::process::id());

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    while let Ok(bytes_read) = reader.read_line(&mut line).await {
        if bytes_read == 0 {
            break; // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        match serde_json::from_str::<JsonRpcRequest>(trimmed) {
            Ok(req) => {
                let response = handle_jsonrpc_message(req).await;
                let mut out_str = serde_json::to_string(&response)?;
                out_str.push('\n');
                stdout.write_all(out_str.as_bytes()).await?;
                stdout.flush().await?;
            }
            Err(err) => {
                eprintln!("[zenohx-mcp] Invalid JSON-RPC request: {}", err);
            }
        }

        line.clear();
    }

    eprintln!("[zenohx-mcp] Stdio stream closed, exiting.");
    Ok(())
}
```

- [ ] **Step 3: Add `mcp` script to `package.json`**

In `package.json`:
```json
"scripts": {
  ...
  "mcp": "cargo run --manifest-path src-tauri/Cargo.toml --bin zenohx-mcp"
}
```

- [ ] **Step 4: Test building and running `zenohx-mcp`**

Run: `cargo check --manifest-path src-tauri/Cargo.toml --bin zenohx-mcp`
Expected: Build succeeds with 0 errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/bin/zenohx-mcp.rs package.json
git commit -m "feat(cli): add standalone zenohx-mcp binary and npm script"
```

---

### Task 6: Frontend React Synchronization

**Files:**
- Create: `src/hooks/useMcpListener.ts`
- Modify: `src/App.tsx`
- Test: `tests/mcp-listener.test.ts` (or unit test with mocked Tauri event emitter)

**Interfaces:**
- Consumes: Tauri events `zenohx://gui-switch-tab` and `zenohx://mcp-action`.
- Produces:
  - React hook `useMcpListener({ setActiveTab })`.
  - Toast / notification pill displaying active AI operations in real time.

- [ ] **Step 1: Write test for `useMcpListener` state reaction**

Create unit test verifying event payload handling.

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test:unit`
Expected: FAIL because hook is not yet created.

- [ ] **Step 3: Implement `useMcpListener` and UI toast indicator in `src/App.tsx`**

Create `src/hooks/useMcpListener.ts`:
```typescript
import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';

export type WorkspaceTab = 'pubsub' | 'query' | 'traffic' | 'topology' | 'settings';

interface McpActionPayload {
  action: string;
  details: string;
}

interface GuiSwitchTabPayload {
  workspace: WorkspaceTab;
}

export function useMcpListener(onSwitchTab: (tab: WorkspaceTab) => void) {
  const [lastAction, setLastAction] = useState<McpActionPayload | null>(null);

  useEffect(() => {
    let unlistenTab: (() => void) | undefined;
    let unlistenAction: (() => void) | undefined;

    listen<GuiSwitchTabPayload>('zenohx://gui-switch-tab', (event) => {
      if (event.payload?.workspace) {
        onSwitchTab(event.payload.workspace);
      }
    }).then((un) => {
      unlistenTab = un;
    });

    listen<McpActionPayload>('zenohx://mcp-action', (event) => {
      setLastAction(event.payload);
      const timer = setTimeout(() => {
        setLastAction((curr) => (curr === event.payload ? null : curr));
      }, 4000);
      return () => clearTimeout(timer);
    }).then((un) => {
      unlistenAction = un;
    });

    return () => {
      unlistenTab?.();
      unlistenAction?.();
    };
  }, [onSwitchTab]);

  return { lastAction };
}
```

In `src/App.tsx`:
Mount `const { lastAction } = useMcpListener(setActiveTab);` and render an unobtrusive banner/toast when `lastAction` is active:
```tsx
{lastAction && (
  <div className="fixed bottom-6 right-6 z-50 flex items-center gap-2 rounded-lg bg-primary/95 px-4 py-2.5 text-xs text-primary-foreground shadow-lg backdrop-blur animate-in fade-in slide-in-from-bottom-2">
    <Sparkles className="h-4 w-4 animate-pulse text-amber-300" />
    <span className="font-semibold">AI Assistant:</span>
    <span>{lastAction.details}</span>
  </div>
)}
```

- [ ] **Step 4: Run type check and unit tests to verify they pass**

Run: `npm run test:types && npm run test:unit`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/hooks/useMcpListener.ts src/App.tsx tests/
git commit -m "feat(ui): add useMcpListener hook and AI action notification toast"
```

---

### Task 7: Full E2E Verification & Integration Testing

**Files:**
- Create: `tests/mcp_e2e_test.mjs`
- Modify: `package.json`

**Interfaces:**
- Consumes: `zenohx-mcp` binary via stdio.
- Produces: Automated verification script sending JSON-RPC requests (`initialize`, `tools/list`, `tools/call`) to the compiled binary and asserting response contents.

- [ ] **Step 1: Write E2E test script `tests/mcp_e2e_test.mjs`**

```javascript
import { spawn } from 'child_process';
import path from 'path';

async function runMcpE2ETest() {
  console.log('Running ZenohX MCP stdio E2E test...');
  const proc = spawn('cargo', ['run', '--manifest-path', 'src-tauri/Cargo.toml', '--bin', 'zenohx-mcp'], {
    stdio: ['pipe', 'pipe', 'inherit'],
  });

  function send(msg) {
    proc.stdin.write(JSON.stringify(msg) + '\n');
  }

  let receivedInit = false;
  let receivedTools = false;

  proc.stdout.on('data', (chunk) => {
    const lines = chunk.toString().split('\n').filter(Boolean);
    for (const line of lines) {
      const resp = JSON.parse(line);
      if (resp.id === 1 && resp.result?.serverInfo?.name === 'zenohx-mcp') {
        receivedInit = true;
        send({ jsonrpc: '2.0', id: 2, method: 'tools/list' });
      } else if (resp.id === 2 && Array.isArray(resp.result?.tools)) {
        receivedTools = true;
        proc.kill();
      }
    }
  });

  send({ jsonrpc: '2.0', id: 1, method: 'initialize' });

  await new Promise((resolve) => proc.on('close', resolve));
  if (!receivedInit || !receivedTools) {
    throw new Error(`MCP test failed: init=${receivedInit}, tools=${receivedTools}`);
  }
  console.log('MCP E2E test passed successfully!');
}

runMcpE2ETest().catch((err) => {
  console.error(err);
  process.exit(1);
});
```

- [ ] **Step 2: Run all test suites**

Run: `npm run test:all && node tests/mcp_e2e_test.mjs`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/mcp_e2e_test.mjs package.json
git commit -m "test(mcp): add automated E2E test for zenohx-mcp stdio protocol"
```
