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

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::ipc::client::IpcClient;
use crate::ipc::types::{IpcRequest, IpcResponse};
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    pub text: String,
    pub is_error: bool,
}

static HEADLESS_STATE: tokio::sync::OnceCell<AppState> = tokio::sync::OnceCell::const_new();

/// Resolves standard SQLite database path across OS platforms.
/// Prioritizes Tauri v2 application data directory (`com.zenohx.app`) so headless mode
/// shares the live GUI SQLite database.
pub fn resolve_db_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            let p0 = PathBuf::from(&app_data).join("com.zenohx.app").join("zenohx.db");
            if p0.exists() {
                return p0;
            }
            let p = PathBuf::from(&app_data).join("zenohx").join("zenohx.db");
            if p.exists() {
                return p;
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let p0 = PathBuf::from(&home)
                .join("Library/Application Support/com.zenohx.app/zenohx.db");
            if p0.exists() {
                return p0;
            }
            let p = PathBuf::from(&home)
                .join("Library/Application Support/zenohx/zenohx.db");
            if p.exists() {
                return p;
            }
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
            let p0 = PathBuf::from(&data_home).join("com.zenohx.app").join("zenohx.db");
            if p0.exists() {
                return p0;
            }
            let p = PathBuf::from(&data_home).join("zenohx").join("zenohx.db");
            if p.exists() {
                return p;
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let p0 = PathBuf::from(&home).join(".local/share/com.zenohx.app/zenohx.db");
            if p0.exists() {
                return p0;
            }
            let p1 = PathBuf::from(&home).join(".local/share/zenohx/zenohx.db");
            if p1.exists() {
                return p1;
            }
            let p2 = PathBuf::from(&home).join(".zenohx/zenohx.db");
            if p2.exists() {
                return p2;
            }
            let p3 = PathBuf::from(&home).join("zenohx.db");
            if p3.exists() {
                return p3;
            }
        }
    }

    let local = PathBuf::from("zenohx.db");
    if local.exists() {
        return local;
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("APPDATA") {
            let default_dir = PathBuf::from(app_data).join("com.zenohx.app");
            let _ = std::fs::create_dir_all(&default_dir);
            return default_dir.join("zenohx.db");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let default_dir = PathBuf::from(home).join("Library/Application Support/com.zenohx.app");
            let _ = std::fs::create_dir_all(&default_dir);
            return default_dir.join("zenohx.db");
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
            let default_dir = PathBuf::from(data_home).join("com.zenohx.app");
            let _ = std::fs::create_dir_all(&default_dir);
            return default_dir.join("zenohx.db");
        }
        if let Ok(home) = std::env::var("HOME") {
            let default_dir = PathBuf::from(home).join(".local/share/com.zenohx.app");
            let _ = std::fs::create_dir_all(&default_dir);
            return default_dir.join("zenohx.db");
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let default_dir = PathBuf::from(home).join(".zenohx");
        let _ = std::fs::create_dir_all(&default_dir);
        default_dir.join("zenohx.db")
    } else {
        PathBuf::from("zenohx.db")
    }
}

/// Lazily initializes and returns the shared headless AppState.
pub async fn get_headless_state() -> &'static AppState {
    HEADLESS_STATE
        .get_or_init(|| async {
            let session_manager = crate::zenoh::SessionManager::new();
            let db_path = resolve_db_path();
            let db = crate::db::Database::new(&db_path)
                .or_else(|_err| -> rusqlite::Result<crate::db::Database> {
                    let mem = crate::db::Database::new_in_memory()?;
                    mem.init_tables()?;
                    Ok(mem)
                })
                .expect("failed to initialize SQLite database");
            let mdns_manager = crate::mdns::MdnsManager::new("zenohx-headless", 7447);
            AppState {
                session_manager,
                db,
                mdns_manager,
            }
        })
        .await
}

/// Returns definitions and JSON Schemas for all 13 supported MCP tools.
pub fn get_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        json!({
            "name": "zenoh_scout",
            "description": "Scans the local physical network via multicast for external Zenoh routers, peers, and locators. NOTE: To inspect or interact with nodes inside the ZenohX app, use 'zenoh_get_sessions' or 'zenoh_get_profiles' instead.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Scouting timeout in milliseconds (default: 1000)",
                        "default": 1000
                    }
                }
            }
        }),
        json!({
            "name": "zenoh_connect_session",
            "description": "Starts/opens a Zenoh session. To start an existing node configured in the app, pass 'profile_id' (retrieve IDs via 'zenoh_get_profiles'). Only pass mode/locators without profile_id if an ad-hoc session is explicitly requested.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "profile_id": {
                        "type": "string",
                        "description": "Optional saved profile ID to load connection configuration from"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["peer", "client", "router"],
                        "description": "Zenoh session mode (default: client)"
                    },
                    "connect_locators": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of endpoint locators to connect to"
                    },
                    "listen_locators": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of endpoint locators to listen on"
                    }
                }
            }
        }),
        json!({
            "name": "zenoh_disconnect_session",
            "description": "Disconnects an active Zenoh session, or all active sessions if session_id is omitted.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "UUID of the session to disconnect. If omitted, disconnects all sessions."
                    }
                }
            }
        }),
        json!({
            "name": "zenoh_get_sessions",
            "description": "Returns active Zenoh nodes/sessions currently running in the ZenohX app. ALWAYS call this first to discover active node sessions before subscribing, publishing, or creating new sessions.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "zenoh_get_profiles",
            "description": "Lists all saved connection profiles (nodes) configured in the ZenohX app (e.g. Local Router, Edge Client). Use this to see configured nodes and retrieve their profile IDs.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
        json!({
            "name": "zenoh_create_profile",
            "description": "Creates a new connection profile (node) in ZenohX, saves it to SQLite so it appears in the GUI, and optionally connects it immediately.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Display name of the new node profile (e.g. 'R3', 'Edge Sensor')"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["peer", "client", "router"],
                        "description": "Zenoh operation mode"
                    },
                    "connect_locators": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of connect locators (e.g. ['tcp/127.0.0.1:7448'])"
                    },
                    "listen_locators": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of listen locators (e.g. ['tcp/0.0.0.0:7449'])"
                    },
                    "scout_multicast": {
                        "type": "boolean",
                        "description": "Enable or disable multicast scouting (default: true)",
                        "default": true
                    },
                    "connect_now": {
                        "type": "boolean",
                        "description": "If true, immediately opens an active Zenoh session for this new profile after saving",
                        "default": false
                    }
                },
                "required": ["name", "mode"]
            }
        }),
        json!({
            "name": "zenoh_edit_profile",
            "description": "Edits an existing connection profile (node) in ZenohX. You can update its name, connect locators, listen locators, or multicast scouting. NOTE: The Zenoh operation mode (peer/client/router) CANNOT be changed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "profile_id": {
                        "type": "string",
                        "description": "ID of the profile to edit"
                    },
                    "name": {
                        "type": "string",
                        "description": "New display name for the profile"
                    },
                    "connect_locators": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "New list of connect locators (e.g. ['tcp/192.168.1.50:7447'])"
                    },
                    "listen_locators": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "New list of listen locators (e.g. ['tcp/0.0.0.0:7447'])"
                    },
                    "scout_multicast": {
                        "type": "boolean",
                        "description": "Enable or disable multicast scouting"
                    },
                    "restart_session": {
                        "type": "boolean",
                        "description": "If true and the profile currently has an active running session, disconnect and restart the session with the updated profile configuration"
                    }
                },
                "required": ["profile_id"]
            }
        }),
        json!({
            "name": "zenoh_publish",
            "description": "Publishes a data sample to the specified Zenoh key expression. Specify 'session_id' to publish through an existing running node session (from zenoh_get_sessions).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key_expr": {
                        "type": "string",
                        "description": "Zenoh key expression to publish to"
                    },
                    "payload": {
                        "type": "string",
                        "description": "Payload data (string or serialized JSON)"
                    },
                    "encoding": {
                        "type": "string",
                        "description": "MIME type / encoding (default: text/plain)",
                        "default": "text/plain"
                    },
                    "priority": {
                        "type": "string",
                        "enum": ["real_time", "interactive_high", "interactive_low", "data_high", "data", "data_low", "background"],
                        "description": "Zenoh priority QoS"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Optional session UUID to use for publishing"
                    }
                },
                "required": ["key_expr", "payload"]
            }
        }),
        json!({
            "name": "zenoh_subscribe",
            "description": "Declares a subscriber on a key expression to capture incoming samples. Specify 'session_id' to attach the subscriber to a specific running node session (from zenoh_get_sessions).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key_expr": {
                        "type": "string",
                        "description": "Zenoh key expression to subscribe to"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Optional session UUID to subscribe on"
                    }
                },
                "required": ["key_expr"]
            }
        }),
        json!({
            "name": "zenoh_unsubscribe",
            "description": "Cancels an active Zenoh subscription by subscription_id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "subscription_id": {
                        "type": "string",
                        "description": "Subscription UUID to unsubscribe"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Optional session UUID"
                    }
                },
                "required": ["subscription_id"]
            }
        }),
        json!({
            "name": "zenoh_get_messages",
            "description": "Reads captured message buffers or SQLite message history.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key_expr": {
                        "type": "string",
                        "description": "Optional key expression filter"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of messages to return (default: 50)",
                        "default": 50
                    },
                    "profile_id": {
                        "type": "string",
                        "description": "Optional profile ID filter"
                    }
                }
            }
        }),
        json!({
            "name": "zenoh_query",
            "description": "Issues a Zenoh GET query and collects all replies.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key_expr": {
                        "type": "string",
                        "description": "Zenoh key expression or selector to query"
                    },
                    "target": {
                        "type": "string",
                        "enum": ["all", "best_matching", "complete"],
                        "description": "Query target routing policy (default: all)",
                        "default": "all"
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Query timeout in milliseconds (default: 5000)",
                        "default": 5000
                    },
                    "payload": {
                        "type": "string",
                        "description": "Optional query predicate payload"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Optional session UUID to use"
                    }
                },
                "required": ["key_expr"]
            }
        }),
        json!({
            "name": "zenoh_declare_queryable",
            "description": "Registers a queryable endpoint that automatically returns a predefined response or executes dynamic JavaScript script responses.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key_expr": {
                        "type": "string",
                        "description": "Zenoh key expression for the queryable"
                    },
                    "reply_payload": {
                        "type": "string",
                        "description": "Static payload string returned in replies (optional if script_code is provided)"
                    },
                    "script_code": {
                        "type": "string",
                        "description": "JavaScript code to dynamically compute query replies. Receives 'query' with { keyExpr, params, payload, timestamp }. Return a JSON object/string or explicit { payload, encoding, keyExpr }."
                    },
                    "encoding": {
                        "type": "string",
                        "description": "Reply encoding (default: text/plain)",
                        "default": "text/plain"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Optional session UUID to register on"
                    }
                },
                "required": ["key_expr"]
            }
        }),
        json!({
            "name": "zenoh_inspect_topology",
            "description": "Queries admin space (@/admin/**) to discover routers, peers, and links.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Optional session UUID"
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum recursion depth (default: 3)",
                        "default": 3
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Timeout in milliseconds (default: 2000)",
                        "default": 2000
                    }
                }
            }
        }),
        json!({
            "name": "zenohx_gui_switch_workspace",
            "description": "Switches the active workspace tab in the ZenohX desktop interface.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workspace": {
                        "type": "string",
                        "enum": ["pubsub", "query", "traffic", "topology", "settings"],
                        "description": "Target workspace tab"
                    }
                },
                "required": ["workspace"]
            }
        }),
        json!({
            "name": "zenohx_gui_get_state",
            "description": "Retrieves GUI state: current tab, active profile, and connection status.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }),
    ]
}

/// Returns definitions for exposed MCP resources.
pub fn get_resource_definitions() -> Vec<serde_json::Value> {
    vec![
        json!({
            "uri": "zenohx://sessions",
            "name": "Active Zenoh Sessions",
            "description": "JSON summary of active Zenoh sessions and status",
            "mimeType": "application/json"
        }),
        json!({
            "uri": "zenohx://profiles",
            "name": "Connection Profiles",
            "description": "List of saved connection profiles from SQLite",
            "mimeType": "application/json"
        }),
        json!({
            "uri": "zenohx://messages/recent",
            "name": "Recent Messages",
            "description": "Snapshot of recently published and received messages",
            "mimeType": "application/json"
        }),
        json!({
            "uri": "zenohx://topology",
            "name": "Discovered Topology",
            "description": "Discovered topology nodes, routers, and connectivity graph",
            "mimeType": "application/json"
        }),
    ]
}

/// Helper to get explicit session ID from arguments or select first active session.
async fn resolve_session_id(args: &serde_json::Value, state: &AppState) -> Result<Uuid, String> {
    if let Some(sid_str) = args.get("session_id").and_then(|v| v.as_str()) {
        Uuid::parse_str(sid_str).map_err(|e| format!("Invalid session_id UUID: {}", e))
    } else {
        let sessions = state.session_manager.get_all_sessions().await;
        if let Some(first) = sessions.first() {
            Ok(first.id)
        } else {
            Err("No active Zenoh session. Please connect a session first using zenoh_connect_session.".to_string())
        }
    }
}

/// Executes a tool on `AppState` directly.
/// If `app_handle` is Some, execution runs in Live GUI mode (emitting Tauri events).
/// If `app_handle` is None, execution runs in Headless mode.
pub async fn execute_tool_on_state(
    tool: &str,
    args: serde_json::Value,
    state: &AppState,
    app_handle: Option<&AppHandle>,
) -> IpcResponse {
    let mode = if app_handle.is_some() {
        "live_gui"
    } else {
        "headless"
    };
    execute_tool_on_state_with_mode(tool, args, state, app_handle, mode).await
}

/// Executes a tool on `AppState` directly with an explicit execution mode.
pub async fn execute_tool_on_state_with_mode(
    tool: &str,
    args: serde_json::Value,
    state: &AppState,
    app_handle: Option<&AppHandle>,
    mode: &str,
) -> IpcResponse {

    match tool {
        "zenoh_scout" => {
            let timeout_ms = args
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(1000);
            match state.session_manager.scout_locators(timeout_ms).await {
                Ok(nodes) => IpcResponse::ok(mode, json!(nodes)),
                Err(e) => IpcResponse::err(mode, format!("Scout failed: {}", e)),
            }
        }

        "zenoh_connect_session" => {
            let profile_id = args.get("profile_id").and_then(|v| v.as_str());
            let config = if let Some(pid) = profile_id {
                match state.db.get_profile_by_id(pid) {
                    Ok(Some(p)) => {
                        let user_auth = p
                            .user_auth
                            .as_ref()
                            .and_then(|v| serde_json::from_value(v.clone()).ok());
                        let tls_config = p
                            .tls_config
                            .as_ref()
                            .and_then(|v| serde_json::from_value(v.clone()).ok());
                        crate::zenoh::types::SessionConfig {
                            profile_id: Some(p.id),
                            mode: p.mode,
                            connect_locators: p.connect_locators,
                            listen_locators: p.listen_locators,
                            scout_multicast: p.scout_multicast,
                            scout_gossip: true,
                            reconnect_retry: None,
                            user_auth,
                            tls_config,
                            custom_config: p.custom_config,
                        }
                    }
                    Ok(None) => {
                        return IpcResponse::err(
                            mode,
                            format!("Profile '{}' not found in database", pid),
                        )
                    }
                    Err(e) => {
                        return IpcResponse::err(
                            mode,
                            format!("Database error fetching profile: {}", e),
                        )
                    }
                }
            } else {
                let mode_str = args
                    .get("mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("client")
                    .to_string();
                let connect_locators: Vec<String> = args
                    .get("connect_locators")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let listen_locators: Vec<String> = args
                    .get("listen_locators")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                crate::zenoh::types::SessionConfig {
                    profile_id: None,
                    mode: mode_str,
                    connect_locators,
                    listen_locators,
                    scout_multicast: true,
                    scout_gossip: true,
                    reconnect_retry: None,
                    user_auth: None,
                    tls_config: None,
                    custom_config: None,
                }
            };

            match state.session_manager.connect(config).await {
                Ok(session_id) => {
                    match state.session_manager.get_session_info(&session_id).await {
                        Ok(info) => IpcResponse::ok(mode, json!(info)),
                        Err(_) => IpcResponse::ok(mode, json!({ "session_id": session_id.to_string() })),
                    }
                }
                Err(e) => IpcResponse::err(mode, format!("Failed to connect session: {}", e)),
            }
        }

        "zenoh_disconnect_session" => {
            let session_id_str = args.get("session_id").and_then(|v| v.as_str());
            if let Some(sid_str) = session_id_str {
                match Uuid::parse_str(sid_str) {
                    Ok(sid) => match state.session_manager.disconnect(&sid).await {
                        Ok(_) => IpcResponse::ok(mode, json!({ "disconnected": sid_str })),
                        Err(e) => IpcResponse::err(mode, format!("Failed to disconnect session: {}", e)),
                    },
                    Err(e) => IpcResponse::err(mode, format!("Invalid session_id UUID: {}", e)),
                }
            } else {
                let sessions = state.session_manager.get_all_sessions().await;
                let count = sessions.len();
                for s in sessions {
                    let _ = state.session_manager.disconnect(&s.id).await;
                }
                IpcResponse::ok(mode, json!({ "disconnected_all": true, "count": count }))
            }
        }

        "zenoh_get_sessions" => {
            let sessions = state.session_manager.get_all_sessions().await;
            IpcResponse::ok(mode, json!(sessions))
        }

        "zenoh_get_profiles" => {
            match state.db.get_profiles() {
                Ok(profiles) => IpcResponse::ok(mode, json!(profiles)),
                Err(e) => IpcResponse::err(mode, format!("Failed to load profiles: {}", e)),
            }
        }

        "zenoh_create_profile" => {
            let name = match args.get("name").and_then(|v| v.as_str()) {
                Some(n) if !n.trim().is_empty() => n.trim().to_string(),
                _ => return IpcResponse::err(mode, "Parameter 'name' is required and cannot be empty"),
            };
            let mode_str = match args.get("mode").and_then(|v| v.as_str()) {
                Some(m) if !m.trim().is_empty() => m.trim().to_lowercase(),
                _ => return IpcResponse::err(mode, "Parameter 'mode' is required ('peer', 'client', or 'router')"),
            };
            let connect_locators: Vec<String> = args
                .get("connect_locators")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let listen_locators: Vec<String> = args
                .get("listen_locators")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let scout_multicast = args
                .get("scout_multicast")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let connect_now = args
                .get("connect_now")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let now = chrono::Utc::now().timestamp_millis();
            let profile_id = Uuid::new_v4().to_string();

            let profile = crate::db::models::ConnectionProfile {
                id: profile_id,
                name: name.clone(),
                mode: mode_str.clone(),
                connect_locators: connect_locators.clone(),
                listen_locators: listen_locators.clone(),
                scout_multicast,
                user_auth: None,
                tls_config: None,
                custom_config: None,
                created_at: now,
                updated_at: now,
            };

            if let Err(e) = profile.validate() {
                return IpcResponse::err(mode, format!("Validation error for profile: {}", e));
            }

            if let Err(e) = state.db.save_profile(&profile) {
                return IpcResponse::err(mode, format!("Failed to save new profile: {}", e));
            }

            let mut session_started = false;
            let mut new_session_id = None;

            if connect_now {
                let config = crate::zenoh::types::SessionConfig {
                    profile_id: Some(profile.id.clone()),
                    mode: profile.mode.clone(),
                    connect_locators: profile.connect_locators.clone(),
                    listen_locators: profile.listen_locators.clone(),
                    scout_multicast: profile.scout_multicast,
                    scout_gossip: true,
                    reconnect_retry: None,
                    user_auth: None,
                    tls_config: None,
                    custom_config: None,
                };
                match state.session_manager.connect(config).await {
                    Ok(sid) => {
                        session_started = true;
                        new_session_id = Some(sid.to_string());
                    }
                    Err(e) => {
                        return IpcResponse::err(
                            mode,
                            format!("Profile '{}' created, but failed to connect session: {}", name, e),
                        );
                    }
                }
            }

            if let Some(app) = app_handle {
                let _ = app.emit(
                    "zenohx://mcp-action",
                    json!({
                        "action": "Create Profile",
                        "details": format!("Created profile '{}' ({})", profile.name, profile.mode)
                    }),
                );
                let _ = app.emit(
                    "zenohx://profile-updated",
                    json!({
                        "profile": profile,
                        "session_restarted": false,
                        "session_id": new_session_id
                    }),
                );
            }

            IpcResponse::ok(
                mode,
                json!({
                    "profile": profile,
                    "session_started": session_started,
                    "session_id": new_session_id
                }),
            )
        }

        "zenoh_edit_profile" => {
            // Strict enforcement: mode cannot be changed
            if args.get("mode").is_some() {
                return IpcResponse::err(
                    mode,
                    "Changing the Zenoh operation mode (peer/client/router) of an existing profile is not allowed. Mode is immutable.",
                );
            }

            let profile_id = match args.get("profile_id").and_then(|v| v.as_str()) {
                Some(id) if !id.trim().is_empty() => id.trim(),
                _ => return IpcResponse::err(mode, "Parameter 'profile_id' is required"),
            };

            let mut profile = match state.db.get_profile_by_id(profile_id) {
                Ok(Some(p)) => p,
                Ok(None) => {
                    return IpcResponse::err(
                        mode,
                        format!("Profile '{}' not found in database", profile_id),
                    )
                }
                Err(e) => {
                    return IpcResponse::err(
                        mode,
                        format!("Database error fetching profile: {}", e),
                    )
                }
            };

            if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
                if name.trim().is_empty() {
                    return IpcResponse::err(mode, "Profile name cannot be empty");
                }
                profile.name = name.trim().to_string();
            }

            if let Some(connect_locs) = args.get("connect_locators") {
                match serde_json::from_value::<Vec<String>>(connect_locs.clone()) {
                    Ok(locs) => {
                        if let Some(serde_json::Value::Object(ref mut custom_map)) = profile.custom_config {
                            if let Some(serde_json::Value::Object(ref mut conn_map)) = custom_map.get_mut("connect") {
                                conn_map.insert("endpoints".to_string(), serde_json::json!(locs));
                            }
                        }
                        profile.connect_locators = locs;
                    }
                    Err(e) => return IpcResponse::err(mode, format!("Invalid connect_locators array: {}", e)),
                }
            }

            if let Some(listen_locs) = args.get("listen_locators") {
                match serde_json::from_value::<Vec<String>>(listen_locs.clone()) {
                    Ok(locs) => {
                        if let Some(serde_json::Value::Object(ref mut custom_map)) = profile.custom_config {
                            if let Some(serde_json::Value::Object(ref mut listen_map)) = custom_map.get_mut("listen") {
                                listen_map.insert("endpoints".to_string(), serde_json::json!(locs));
                            }
                        }
                        profile.listen_locators = locs;
                    }
                    Err(e) => return IpcResponse::err(mode, format!("Invalid listen_locators array: {}", e)),
                }
            }

            if let Some(scout) = args.get("scout_multicast").and_then(|v| v.as_bool()) {
                profile.scout_multicast = scout;
            }

            if let Err(e) = profile.validate() {
                return IpcResponse::err(mode, format!("Validation error for profile: {}", e));
            }

            profile.updated_at = chrono::Utc::now().timestamp_millis();

            if let Err(e) = state.db.save_profile(&profile) {
                return IpcResponse::err(mode, format!("Failed to save updated profile: {}", e));
            }

            let restart_session = args
                .get("restart_session")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let mut session_restarted = false;
            let mut new_session_id = None;

            if restart_session {
                let sessions = state.session_manager.get_all_sessions().await;
                if let Some(active) = sessions
                    .into_iter()
                    .find(|s| s.profile_id.as_deref() == Some(&profile.id))
                {
                    let _ = state.session_manager.disconnect(&active.id).await;

                    let user_auth = profile
                        .user_auth
                        .as_ref()
                        .and_then(|v| serde_json::from_value(v.clone()).ok());
                    let tls_config = profile
                        .tls_config
                        .as_ref()
                        .and_then(|v| serde_json::from_value(v.clone()).ok());
                    let config = crate::zenoh::types::SessionConfig {
                        profile_id: Some(profile.id.clone()),
                        mode: profile.mode.clone(),
                        connect_locators: profile.connect_locators.clone(),
                        listen_locators: profile.listen_locators.clone(),
                        scout_multicast: profile.scout_multicast,
                        scout_gossip: true,
                        reconnect_retry: None,
                        user_auth,
                        tls_config,
                        custom_config: profile.custom_config.clone(),
                    };
                    match state.session_manager.connect(config).await {
                        Ok(sid) => {
                            session_restarted = true;
                            new_session_id = Some(sid.to_string());
                        }
                        Err(e) => {
                            return IpcResponse::err(
                                mode,
                                format!("Profile saved, but failed to restart session: {}", e),
                            );
                        }
                    }
                }
            }

            if let Some(app) = app_handle {
                let _ = app.emit(
                    "zenohx://mcp-action",
                    json!({
                        "action": "Edit Profile",
                        "details": format!("Updated profile '{}'", profile.name)
                    }),
                );
                let _ = app.emit(
                    "zenohx://profile-updated",
                    json!({
                        "profile": profile,
                        "session_restarted": session_restarted,
                        "session_id": new_session_id
                    }),
                );
            }

            IpcResponse::ok(
                mode,
                json!({
                    "profile": profile,
                    "session_restarted": session_restarted,
                    "session_id": new_session_id
                }),
            )
        }

        "zenoh_publish" => {
            let key_expr = match args.get("key_expr").and_then(|v| v.as_str()) {
                Some(k) if !k.trim().is_empty() => k.trim().to_string(),
                _ => return IpcResponse::err(mode, "Parameter 'key_expr' is required"),
            };
            let payload_str = match args.get("payload") {
                Some(v) => match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                },
                None => return IpcResponse::err(mode, "Parameter 'payload' is required"),
            };
            let encoding = args
                .get("encoding")
                .and_then(|v| v.as_str())
                .unwrap_or("text/plain")
                .to_string();

            let sid = match resolve_session_id(&args, state).await {
                Ok(id) => id,
                Err(e) => return IpcResponse::err(mode, e),
            };

            let publish_options = args.get("priority").and_then(|v| v.as_str()).map(|p| {
                crate::zenoh::types::PublishOptions {
                    priority: Some(p.to_string()),
                    congestion_control: Some("drop".to_string()),
                    express: Some(false),
                    attachment: None,
                }
            });

            let payload_bytes = payload_str.as_bytes().to_vec();
            match state
                .session_manager
                .publish_with_options(
                    &sid,
                    &key_expr,
                    payload_bytes.clone(),
                    &encoding,
                    "put",
                    publish_options,
                )
                .await
            {
                Ok(_) => {
                    let profile_id = state
                        .session_manager
                        .get_session_profile_id(&sid)
                        .await
                        .unwrap_or_default();
                    let local_zid = state
                        .session_manager
                        .get_session(&sid)
                        .await
                        .map(|s| s.zid().to_string())
                        .ok();
                    let now = chrono::Utc::now().timestamp_millis();
                    let stored = crate::db::models::StoredMessage {
                        id: None,
                        profile_id,
                        direction: "outgoing".to_string(),
                        key_expr: key_expr.clone(),
                        payload: payload_bytes,
                        encoding: encoding.clone(),
                        kind: "put".to_string(),
                        timestamp: now,
                        source_id: local_zid,
                    };
                    let _ = state.db.insert_message(&stored);

                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "zenohx://mcp-action",
                            json!({
                                "action": "Publish",
                                "details": format!("Published to '{}'", key_expr)
                            }),
                        );
                    }

                    IpcResponse::ok(
                        mode,
                        json!({
                            "published": true,
                            "key_expr": key_expr,
                            "session_id": sid.to_string(),
                        }),
                    )
                }
                Err(e) => IpcResponse::err(mode, format!("Failed to publish: {}", e)),
            }
        }

        "zenoh_subscribe" => {
            let key_expr = match args.get("key_expr").and_then(|v| v.as_str()) {
                Some(k) if !k.trim().is_empty() => k.trim().to_string(),
                _ => return IpcResponse::err(mode, "Parameter 'key_expr' is required"),
            };
            let sid = match resolve_session_id(&args, state).await {
                Ok(id) => id,
                Err(e) => return IpcResponse::err(mode, e),
            };

            let sub_id = Uuid::new_v4();
            let db = state.db.clone();
            let profile_id = state
                .session_manager
                .get_session_profile_id(&sid)
                .await
                .unwrap_or_default();
            let closure_profile_id = profile_id.clone();
            let app_opt = app_handle.cloned();
            match state
                .session_manager
                .subscribe_with_options(
                    &sid,
                    sub_id,
                    &key_expr,
                    None,
                    move |sample| {
                        let stored = crate::db::models::StoredMessage {
                            id: None,
                            profile_id: closure_profile_id.clone(),
                            direction: "incoming".to_string(),
                            key_expr: sample.key_expr.clone(),
                            payload: sample.payload.clone(),
                            encoding: sample.encoding.clone(),
                            kind: sample.kind.clone(),
                            timestamp: sample.timestamp,
                            source_id: sample.source_id.clone(),
                        };
                        let _ = db.insert_message(&stored);
                        if let Some(ref app) = app_opt {
                            let _ = app.emit("zenohx://samples-batched", vec![&sample]);
                        }
                    },
                )
                .await
            {
                Ok(_) => {
                    // Persist subscription preset to SQLite DB if associated with a profile
                    if !profile_id.is_empty() {
                        let preset = crate::db::models::SubscriptionPreset {
                            id: sub_id.to_string(),
                            profile_id: profile_id.clone(),
                            key_expr: key_expr.clone(),
                            default_encoding: "json".to_string(),
                            auto_subscribe: true,
                            color_tag: None,
                        };
                        let _ = state.db.save_preset(&preset);
                    }

                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "zenohx://mcp-action",
                            json!({
                                "action": "Subscribe",
                                "details": format!("Subscribed to '{}'", key_expr)
                            }),
                        );
                        let _ = app.emit(
                            "zenohx://subscription-added",
                            json!({
                                "id": sub_id.to_string(),
                                "session_id": sid.to_string(),
                                "profile_id": profile_id,
                                "key_expr": key_expr,
                                "encoding": "json",
                                "active": true
                            }),
                        );
                    }

                    IpcResponse::ok(
                        mode,
                        json!({
                            "subscription_id": sub_id.to_string(),
                            "key_expr": key_expr,
                            "session_id": sid.to_string(),
                        }),
                    )
                }
                Err(e) => IpcResponse::err(mode, format!("Failed to subscribe: {}", e)),
            }
        }

        "zenoh_unsubscribe" => {
            let sub_id_opt = args
                .get("subscription_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());
            let sid_opt = args
                .get("session_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok());

            if let Some(sub_id) = sub_id_opt {
                let res = if let Some(sid) = sid_opt {
                    state.session_manager.unsubscribe(&sid, sub_id).await
                } else {
                    let sessions = state.session_manager.get_all_sessions().await;
                    for s in sessions {
                        let _ = state.session_manager.unsubscribe(&s.id, sub_id).await;
                    }
                    Ok(())
                };

                match res {
                    Ok(_) => {
                        let _ = state.db.delete_preset(&sub_id.to_string());
                        if let Some(app) = app_handle {
                            let _ = app.emit(
                                "zenohx://mcp-action",
                                json!({
                                    "action": "Unsubscribe",
                                    "details": format!("Unsubscribed '{}'", sub_id)
                                }),
                            );
                            let _ = app.emit(
                                "zenohx://subscription-removed",
                                json!({
                                    "id": sub_id.to_string(),
                                }),
                            );
                        }
                        IpcResponse::ok(mode, json!({ "unsubscribed": sub_id.to_string() }))
                    }
                    Err(e) => IpcResponse::err(mode, format!("Failed to unsubscribe: {}", e)),
                }
            } else {
                IpcResponse::err(mode, "Parameter 'subscription_id' is required")
            }
        }

        "zenoh_get_messages" => {
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as u32;
            let profile_id = args.get("profile_id").and_then(|v| v.as_str());
            let key_expr_filter = args.get("key_expr").and_then(|v| v.as_str());

            match state.db.get_messages(profile_id, limit, 0) {
                Ok(msgs) => {
                    let filtered: Vec<serde_json::Value> = msgs
                        .into_iter()
                        .filter(|m| {
                            if let Some(ke) = key_expr_filter {
                                m.key_expr == ke || m.key_expr.contains(ke)
                            } else {
                                true
                            }
                        })
                        .map(|m| {
                            let payload_display = match String::from_utf8(m.payload.clone()) {
                                Ok(s) => s,
                                Err(_) => hex::encode(&m.payload),
                            };
                            json!({
                                "id": m.id,
                                "profile_id": m.profile_id,
                                "direction": m.direction,
                                "key_expr": m.key_expr,
                                "payload": payload_display,
                                "encoding": m.encoding,
                                "kind": m.kind,
                                "timestamp": m.timestamp,
                                "source_id": m.source_id,
                            })
                        })
                        .collect();
                    IpcResponse::ok(mode, json!(filtered))
                }
                Err(e) => IpcResponse::err(mode, format!("Failed to get messages: {}", e)),
            }
        }

        "zenoh_query" => {
            let key_expr = match args.get("key_expr").and_then(|v| v.as_str()) {
                Some(k) if !k.trim().is_empty() => k.trim().to_string(),
                _ => return IpcResponse::err(mode, "Parameter 'key_expr' is required"),
            };
            let sid = match resolve_session_id(&args, state).await {
                Ok(id) => id,
                Err(e) => return IpcResponse::err(mode, e),
            };
            let target = args
                .get("target")
                .and_then(|v| v.as_str())
                .unwrap_or("all")
                .to_string();
            let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(5000);
            let payload = args
                .get("payload")
                .and_then(|v| v.as_str())
                .map(|s| s.as_bytes().to_vec());
            let encoding = if payload.is_some() {
                Some("text/plain".to_string())
            } else {
                None
            };

            match state
                .session_manager
                .query_get_advanced(
                    &sid,
                    &key_expr,
                    &target,
                    timeout_ms,
                    payload,
                    encoding,
                    None,
                )
                .await
            {
                Ok(replies) => {
                    let formatted_replies: Vec<serde_json::Value> = replies
                        .into_iter()
                        .map(|r| {
                            let payload_display = match String::from_utf8(r.payload.clone()) {
                                Ok(s) => s,
                                Err(_) => hex::encode(&r.payload),
                            };
                            json!({
                                "key_expr": r.key_expr,
                                "payload": payload_display,
                                "encoding": r.encoding,
                                "replier_id": r.replier_id,
                                "latency_ms": r.latency_ms,
                            })
                        })
                        .collect();
                    IpcResponse::ok(mode, json!(formatted_replies))
                }
                Err(e) => IpcResponse::err(mode, format!("Query failed: {}", e)),
            }
        }

        "zenoh_declare_queryable" => {
            let key_expr = match args.get("key_expr").and_then(|v| v.as_str()) {
                Some(k) if !k.trim().is_empty() => k.trim().to_string(),
                _ => return IpcResponse::err(mode, "Parameter 'key_expr' is required"),
            };
            let reply_payload = args.get("reply_payload").and_then(|v| v.as_str()).map(|s| s.to_string());
            let script_code = args.get("script_code").and_then(|v| v.as_str()).map(|s| s.to_string());

            if reply_payload.is_none() && script_code.is_none() {
                return IpcResponse::err(mode, "Parameter 'reply_payload' (static reply) or 'script_code' (JavaScript handler) is required");
            }

            let encoding = args
                .get("encoding")
                .and_then(|v| v.as_str())
                .unwrap_or("text/plain")
                .to_string();
            let sid = match resolve_session_id(&args, state).await {
                Ok(id) => id,
                Err(e) => return IpcResponse::err(mode, e),
            };

            let queryable_id = Uuid::new_v4();
            let profile_id = state
                .session_manager
                .get_session_profile_id(&sid)
                .await
                .unwrap_or_default();

            if let Some(ref script) = script_code {
                if !profile_id.is_empty() {
                    let preset = crate::db::models::QueryablePreset {
                        id: queryable_id.to_string(),
                        profile_id: profile_id.clone(),
                        key_expr: key_expr.clone(),
                        auto_reply: true,
                        reply_payload: Some(script.clone()),
                        reply_encoding: "script".to_string(),
                    };
                    let _ = state.db.save_queryable_preset(&preset);
                }

                if let Some(app) = app_handle {
                    let app_clone = app.clone();
                    let reg_res = state
                        .session_manager
                        .declare_queryable_routed(&sid, queryable_id, &key_expr, move |inbound| {
                            let _ = app_clone.emit("zenohx://query", inbound);
                        })
                        .await;

                    match reg_res {
                        Ok(_) => {
                            let _ = app.emit(
                                "zenohx://mcp-action",
                                json!({
                                    "action": "Declare Queryable (JS Script)",
                                    "details": format!("Declared JS queryable on '{}'", key_expr)
                                }),
                            );
                            let _ = app.emit(
                                "zenohx://queryable-added",
                                json!({
                                    "id": queryable_id.to_string(),
                                    "session_id": sid.to_string(),
                                    "profile_id": profile_id,
                                    "key_expr": key_expr,
                                    "auto_reply": true,
                                    "reply_mode": "script",
                                    "script_code": script,
                                    "reply_encoding": encoding,
                                }),
                            );

                            IpcResponse::ok(
                                mode,
                                json!({
                                    "queryable_id": queryable_id.to_string(),
                                    "key_expr": key_expr,
                                    "reply_mode": "script",
                                    "session_id": sid.to_string(),
                                }),
                            )
                        }
                        Err(e) => IpcResponse::err(mode, format!("Failed to declare queryable: {}", e)),
                    }
                } else {
                    // Headless fallback
                    let fallback_payload = reply_payload
                        .as_deref()
                        .unwrap_or("{\"status\":\"ok\"}")
                        .to_string();
                    let reply_bytes = fallback_payload.into_bytes();
                    let reply_enc = if reply_payload.is_some() { encoding.clone() } else { "application/json".to_string() };
                    let reply_key = key_expr.clone();
                    match state
                        .session_manager
                        .declare_queryable(
                            &sid,
                            queryable_id,
                            &key_expr,
                            move |qh| {
                                let bytes = reply_bytes.clone();
                                let enc = reply_enc.clone();
                                let key = reply_key.clone();
                                async move {
                                    let _ = qh.reply_with_encoding(&key, bytes, &enc).await;
                                }
                            },
                        )
                        .await
                    {
                        Ok(_) => IpcResponse::ok(
                            mode,
                            json!({
                                "queryable_id": queryable_id.to_string(),
                                "key_expr": key_expr,
                                "reply_mode": "script",
                                "session_id": sid.to_string(),
                            }),
                        ),
                        Err(e) => IpcResponse::err(mode, format!("Failed to declare queryable: {}", e)),
                    }
                }
            } else {
                let static_payload = reply_payload.unwrap();
                let reply_bytes = static_payload.as_bytes().to_vec();
                let reply_enc = encoding.clone();
                let reply_key = key_expr.clone();

                if !profile_id.is_empty() {
                    let preset = crate::db::models::QueryablePreset {
                        id: queryable_id.to_string(),
                        profile_id: profile_id.clone(),
                        key_expr: key_expr.clone(),
                        auto_reply: true,
                        reply_payload: Some(static_payload.clone()),
                        reply_encoding: encoding.clone(),
                    };
                    let _ = state.db.save_queryable_preset(&preset);
                }

                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "zenohx://mcp-action",
                        json!({
                            "action": "Declare Queryable",
                            "details": format!("Declared queryable on '{}'", key_expr)
                        }),
                    );
                    let _ = app.emit(
                        "zenohx://queryable-added",
                        json!({
                            "id": queryable_id.to_string(),
                            "session_id": sid.to_string(),
                            "profile_id": profile_id,
                            "key_expr": key_expr,
                            "auto_reply": true,
                            "reply_mode": "payload",
                            "reply_payload": static_payload,
                            "reply_encoding": encoding,
                        }),
                    );
                }

                match state
                    .session_manager
                    .declare_queryable(
                        &sid,
                        queryable_id,
                        &key_expr,
                        move |qh| {
                            let bytes = reply_bytes.clone();
                            let enc = reply_enc.clone();
                            let key = reply_key.clone();
                            async move {
                                let _ = qh.reply_with_encoding(&key, bytes, &enc).await;
                            }
                        },
                    )
                    .await
                {
                    Ok(_) => IpcResponse::ok(
                        mode,
                        json!({
                            "queryable_id": queryable_id.to_string(),
                            "key_expr": key_expr,
                            "reply_mode": "payload",
                            "session_id": sid.to_string(),
                        }),
                    ),
                    Err(e) => IpcResponse::err(mode, format!("Failed to declare queryable: {}", e)),
                }
            }
        }

        "zenoh_inspect_topology" => {
            let sid = match resolve_session_id(&args, state).await {
                Ok(id) => id,
                Err(e) => return IpcResponse::err(mode, e),
            };
            let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
            let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(2000);

            match state
                .session_manager
                .discover_admin_topology(&sid, max_depth, timeout_ms)
                .await
            {
                Ok(entries) => IpcResponse::ok(mode, json!(entries)),
                Err(e) => IpcResponse::err(mode, format!("Failed to inspect topology: {}", e)),
            }
        }

        "zenohx_gui_switch_workspace" => {
            let workspace = args
                .get("workspace")
                .and_then(|w| w.as_str())
                .unwrap_or("pubsub");
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "zenohx://gui-switch-tab",
                    json!({ "workspace": workspace }),
                );
                let _ = app.emit(
                    "zenohx://mcp-action",
                    json!({
                        "action": "Switch Workspace",
                        "details": format!("Switched to tab '{}'", workspace)
                    }),
                );
            }
            if mode == "live_gui" {
                IpcResponse::ok("live_gui", json!({ "switched_to": workspace }))
            } else {
                IpcResponse::err("headless", "Desktop GUI is not running; cannot switch workspace")
            }
        }

        "zenohx_gui_get_state" => {
            let sessions = state.session_manager.get_all_sessions().await;
            if mode == "live_gui" {
                IpcResponse::ok(
                    "live_gui",
                    json!({
                        "connected": !sessions.is_empty(),
                        "active_tab": "pubsub",
                        "session_count": sessions.len(),
                    }),
                )
            } else {
                IpcResponse::ok(
                    "headless",
                    json!({
                        "gui_running": false,
                        "connected": !sessions.is_empty(),
                        "session_count": sessions.len(),
                    }),
                )
            }
        }

        _ => IpcResponse::err(mode, format!("Unknown tool: {}", tool)),
    }
}

/// Dispatches an MCP tool call using dual-mode execution:
/// 1. Attempts connection to local IPC socket. If connected -> Live GUI mode.
/// 2. If connection fails -> Headless mode (shared database and local SessionManager).
pub async fn dispatch_mcp_tool(name: &str, args: serde_json::Value) -> McpToolResult {
    // 1. Probe local IPC socket
    match IpcClient::connect().await {
        Ok(mut client) => {
            let req = IpcRequest::ExecuteTool {
                tool: name.to_string(),
                args: args.clone(),
            };
            match client.call(&req).await {
                Ok(resp) => {
                    if resp.success {
                        let formatted = if resp.data.is_null() {
                            "Success".to_string()
                        } else {
                            serde_json::to_string_pretty(&resp.data).unwrap_or_default()
                        };
                        McpToolResult {
                            text: format!("[Mode: Live GUI]\n{}", formatted),
                            is_error: false,
                        }
                    } else {
                        McpToolResult {
                            text: format!(
                                "[Mode: Live GUI] Error: {}",
                                resp.error.unwrap_or_else(|| "Unknown error".to_string())
                            ),
                            is_error: true,
                        }
                    }
                }
                Err(_) => {
                    // GUI socket communication failed mid-session -> Fallback to Headless
                    dispatch_headless(name, args).await
                }
            }
        }
        Err(_) => {
            // IPC connection failed -> Fallback to Headless execution
            dispatch_headless(name, args).await
        }
    }
}

/// Executes a tool in headless fallback mode.
async fn dispatch_headless(name: &str, args: serde_json::Value) -> McpToolResult {
    if name == "zenohx_gui_switch_workspace" {
        return McpToolResult {
            text: "[Mode: Headless (GUI not running)] Error: Desktop GUI is not running; cannot switch workspace".to_string(),
            is_error: true,
        };
    }

    let headless_state = get_headless_state().await;
    let resp = execute_tool_on_state(name, args, headless_state, None).await;
    if resp.success {
        let formatted = if resp.data.is_null() {
            "Success".to_string()
        } else {
            serde_json::to_string_pretty(&resp.data).unwrap_or_default()
        };
        McpToolResult {
            text: format!("[Mode: Headless (GUI not running)]\n{}", formatted),
            is_error: false,
        }
    } else {
        McpToolResult {
            text: format!(
                "[Mode: Headless (GUI not running)] Error: {}",
                resp.error.unwrap_or_else(|| "Unknown error".to_string())
            ),
            is_error: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_DISPATCH_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn create_test_state() -> AppState {
        let session_manager = crate::zenoh::SessionManager::new();
        let db = crate::db::Database::new_in_memory().expect("in-memory db");
        db.init_tables().expect("init tables");
        let mdns_manager = crate::mdns::MdnsManager::new("zenohx-test", 7447);
        AppState {
            session_manager,
            db,
            mdns_manager,
        }
    }

    #[test]
    fn test_tool_definitions_validity() {
        let tools = get_tool_definitions();
        assert_eq!(
            tools.len(),
            16,
            "Expected exactly 16 MCP tools, found {}",
            tools.len()
        );
        let expected_names = [
            "zenoh_scout",
            "zenoh_connect_session",
            "zenoh_disconnect_session",
            "zenoh_get_sessions",
            "zenoh_get_profiles",
            "zenoh_create_profile",
            "zenoh_edit_profile",
            "zenoh_publish",
            "zenoh_subscribe",
            "zenoh_unsubscribe",
            "zenoh_get_messages",
            "zenoh_query",
            "zenoh_declare_queryable",
            "zenoh_inspect_topology",
            "zenohx_gui_switch_workspace",
            "zenohx_gui_get_state",
        ];
        for name in &expected_names {
            let tool = tools
                .iter()
                .find(|t| t.get("name").and_then(|n| n.as_str()) == Some(*name));
            assert!(tool.is_some(), "Missing expected tool definition: {}", name);
            let tool_obj = tool.unwrap();
            assert!(tool_obj.get("description").is_some());
            assert!(tool_obj.get("inputSchema").is_some());
            assert_eq!(tool_obj["inputSchema"]["type"], "object");
        }
    }

    #[test]
    fn test_resource_definitions_validity() {
        let resources = get_resource_definitions();
        assert_eq!(
            resources.len(),
            4,
            "Expected exactly 4 MCP resources, found {}",
            resources.len()
        );
        let expected_uris = [
            "zenohx://sessions",
            "zenohx://profiles",
            "zenohx://messages/recent",
            "zenohx://topology",
        ];
        for uri in &expected_uris {
            let res = resources
                .iter()
                .find(|r| r.get("uri").and_then(|u| u.as_str()) == Some(*uri));
            assert!(res.is_some(), "Missing expected resource definition: {}", uri);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_scout() {
        let state = create_test_state();
        let resp = execute_tool_on_state("zenoh_scout", json!({ "timeout_ms": 50 }), &state, None).await;
        assert!(resp.success);
        assert_eq!(resp.mode, "headless");
        assert!(resp.data.is_array());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_session_lifecycle() {
        let state = create_test_state();

        // 1. Initially 0 sessions
        let resp = execute_tool_on_state("zenoh_get_sessions", json!({}), &state, None).await;
        assert!(resp.success);
        let sessions = resp.data.as_array().unwrap();
        assert_eq!(sessions.len(), 0);

        // 2. Connect session (peer mode, ephemeral)
        let connect_resp = execute_tool_on_state(
            "zenoh_connect_session",
            json!({
                "mode": "peer",
                "connect_locators": [],
                "listen_locators": []
            }),
            &state,
            None,
        ).await;
        assert!(connect_resp.success);
        let sid_val = &connect_resp.data["id"];
        let sid_str = sid_val.as_str().unwrap();

        // 3. Verify session active
        let resp = execute_tool_on_state("zenoh_get_sessions", json!({}), &state, None).await;
        assert!(resp.success);
        let sessions = resp.data.as_array().unwrap();
        assert_eq!(sessions.len(), 1);

        // 4. Disconnect session by ID
        let disc_resp = execute_tool_on_state(
            "zenoh_disconnect_session",
            json!({ "session_id": sid_str }),
            &state,
            None,
        ).await;
        assert!(disc_resp.success);

        // 5. Verify 0 sessions
        let resp = execute_tool_on_state("zenoh_get_sessions", json!({}), &state, None).await;
        assert!(resp.success);
        let sessions = resp.data.as_array().unwrap();
        assert_eq!(sessions.len(), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_publish_without_session_fails() {
        let state = create_test_state();
        let resp = execute_tool_on_state(
            "zenoh_publish",
            json!({
                "key_expr": "demo/test",
                "payload": "hello"
            }),
            &state,
            None,
        ).await;
        assert!(!resp.success);
        assert!(resp
            .error
            .unwrap()
            .contains("No active Zenoh session. Please connect a session first using zenoh_connect_session."));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_pubsub_and_get_messages() {
        let state = create_test_state();

        // Connect session
        let connect_resp = execute_tool_on_state(
            "zenoh_connect_session",
            json!({ "mode": "peer" }),
            &state,
            None,
        ).await;
        assert!(connect_resp.success);

        // Subscribe
        let sub_resp = execute_tool_on_state(
            "zenoh_subscribe",
            json!({ "key_expr": "demo/mcp/**" }),
            &state,
            None,
        ).await;
        assert!(sub_resp.success);
        let sub_id = sub_resp.data["subscription_id"].as_str().unwrap();

        // Publish sample
        let pub_resp = execute_tool_on_state(
            "zenoh_publish",
            json!({
                "key_expr": "demo/mcp/sample",
                "payload": "{\"status\":\"ok\"}",
                "encoding": "application/json"
            }),
            &state,
            None,
        ).await;
        assert!(pub_resp.success);

        // Allow loopback message reception
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Query messages from DB
        let msg_resp = execute_tool_on_state(
            "zenoh_get_messages",
            json!({ "key_expr": "demo/mcp" }),
            &state,
            None,
        ).await;
        assert!(msg_resp.success, "msg_resp failed with: {:?}", msg_resp.error);
        let messages = msg_resp.data.as_array().unwrap();
        assert!(!messages.is_empty());

        // Unsubscribe
        let unsub_resp = execute_tool_on_state(
            "zenoh_unsubscribe",
            json!({ "subscription_id": sub_id }),
            &state,
            None,
        ).await;
        assert!(unsub_resp.success);

        // Cleanup
        let _ = execute_tool_on_state("zenoh_disconnect_session", json!({}), &state, None).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_gui_switch_workspace_headless_fails() {
        let state = create_test_state();
        let resp = execute_tool_on_state(
            "zenohx_gui_switch_workspace",
            json!({ "workspace": "query" }),
            &state,
            None,
        ).await;
        assert!(!resp.success);
        assert_eq!(
            resp.error,
            Some("Desktop GUI is not running; cannot switch workspace".to_string())
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_gui_get_state() {
        let state = create_test_state();
        let resp = execute_tool_on_state("zenohx_gui_get_state", json!({}), &state, None).await;
        assert!(resp.success);
        assert_eq!(resp.data["gui_running"], false);
        assert_eq!(resp.data["connected"], false);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_unknown() {
        let state = create_test_state();
        let resp = execute_tool_on_state("non_existent_tool", json!({}), &state, None).await;
        assert!(!resp.success);
        assert!(resp.error.unwrap().contains("Unknown tool: non_existent_tool"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_dispatch_mcp_tool_headless_fallback() {
        let _lock = TEST_DISPATCH_MUTEX.lock().await;
        let prev_sock = std::env::var("ZENOHX_IPC_SOCKET").ok();
        let id = Uuid::new_v4().simple().to_string();
        #[cfg(unix)]
        let non_existent = PathBuf::from(format!("/tmp/zx-none-{}.sock", &id[..8]));
        #[cfg(not(unix))]
        let non_existent = std::env::temp_dir().join(format!("zenohx-nonexistent-{}.sock", &id[..8]));
        std::env::set_var("ZENOHX_IPC_SOCKET", &non_existent);

        // Since no IPC server is running on the specified socket,
        // dispatch_mcp_tool should automatically fall back to headless.
        let resp = dispatch_mcp_tool("zenoh_get_sessions", json!({})).await;

        // GUI-specific tools should cleanly fail in headless mode
        let gui_resp = dispatch_mcp_tool(
            "zenohx_gui_switch_workspace",
            json!({ "workspace": "topology" }),
        ).await;

        if let Some(prev) = prev_sock {
            std::env::set_var("ZENOHX_IPC_SOCKET", prev);
        } else {
            std::env::remove_var("ZENOHX_IPC_SOCKET");
        }

        assert!(!resp.is_error);
        assert!(resp.text.contains("[Mode: Headless (GUI not running)]"));

        assert!(gui_resp.is_error);
        assert!(gui_resp.text.contains("Desktop GUI is not running; cannot switch workspace"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_query_and_queryable() {
        let state = create_test_state();

        // Connect session
        let connect_resp = execute_tool_on_state(
            "zenoh_connect_session",
            json!({ "mode": "peer" }),
            &state,
            None,
        ).await;
        assert!(connect_resp.success);

        // Declare queryable
        let q_resp = execute_tool_on_state(
            "zenoh_declare_queryable",
            json!({
                "key_expr": "demo/test/query",
                "reply_payload": "pong_payload"
            }),
            &state,
            None,
        ).await;
        assert!(q_resp.success);

        // Allow queryable registration
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Query
        let query_resp = execute_tool_on_state(
            "zenoh_query",
            json!({
                "key_expr": "demo/test/query",
                "timeout_ms": 1000
            }),
            &state,
            None,
        ).await;
        assert!(query_resp.success);
        let replies = query_resp.data.as_array().unwrap();
        assert!(!replies.is_empty());
        assert_eq!(replies[0]["payload"], "pong_payload");

        // Disconnect
        let _ = execute_tool_on_state("zenoh_disconnect_session", json!({}), &state, None).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_declare_queryable_script_mode() {
        let state = create_test_state();

        let profile = crate::db::models::ConnectionProfile {
            id: "mcp-queryable-profile".to_string(),
            name: "MCP Queryable Profile".to_string(),
            mode: "peer".to_string(),
            connect_locators: vec![],
            listen_locators: vec![],
            scout_multicast: true,
            user_auth: None,
            tls_config: None,
            custom_config: None,
            created_at: 0,
            updated_at: 0,
        };
        state.db.save_profile(&profile).expect("save profile");

        let connect_resp = execute_tool_on_state(
            "zenoh_connect_session",
            json!({ "profile_id": "mcp-queryable-profile" }),
            &state,
            None,
        ).await;
        assert!(connect_resp.success);

        // Declare with script_code
        let q_resp = execute_tool_on_state(
            "zenoh_declare_queryable",
            json!({
                "key_expr": "demo/test/script",
                "script_code": "return { message: 'hello from script' };"
            }),
            &state,
            None,
        ).await;
        assert!(q_resp.success);

        // Verify queryable preset saved in DB
        let presets = state.db.get_queryable_presets("mcp-queryable-profile").expect("get presets");
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].key_expr, "demo/test/script");
        assert_eq!(presets[0].reply_encoding, "script");
        assert_eq!(presets[0].reply_payload.as_deref(), Some("return { message: 'hello from script' };"));

        let _ = execute_tool_on_state("zenoh_disconnect_session", json!({}), &state, None).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_inspect_topology() {
        let state = create_test_state();

        // Connect session
        let connect_resp = execute_tool_on_state(
            "zenoh_connect_session",
            json!({ "mode": "peer" }),
            &state,
            None,
        ).await;
        assert!(connect_resp.success);

        let topo_resp = execute_tool_on_state(
            "zenoh_inspect_topology",
            json!({
                "max_depth": 1,
                "timeout_ms": 500
            }),
            &state,
            None,
        ).await;
        assert!(topo_resp.success);
        assert!(topo_resp.data.is_array());

        // Disconnect
        let _ = execute_tool_on_state("zenoh_disconnect_session", json!({}), &state, None).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_connect_from_db_profile() {
        let state = create_test_state();

        // Save a test profile in DB
        let profile = crate::db::models::ConnectionProfile {
            id: "mcp-test-profile".to_string(),
            name: "MCP Test Profile".to_string(),
            mode: "peer".to_string(),
            connect_locators: vec![],
            listen_locators: vec![],
            scout_multicast: true,
            user_auth: None,
            tls_config: None,
            custom_config: None,
            created_at: 0,
            updated_at: 0,
        };
        state.db.save_profile(&profile).expect("save profile");

        // Connect using profile_id
        let connect_resp = execute_tool_on_state(
            "zenoh_connect_session",
            json!({ "profile_id": "mcp-test-profile" }),
            &state,
            None,
        ).await;
        assert!(connect_resp.success);
        assert_eq!(connect_resp.data["profile_id"], "mcp-test-profile");

        // Disconnect
        let _ = execute_tool_on_state("zenoh_disconnect_session", json!({}), &state, None).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_subscribe_persists_preset_and_unsubscribe_removes() {
        let state = create_test_state();

        let profile = crate::db::models::ConnectionProfile {
            id: "mcp-sub-profile".to_string(),
            name: "MCP Sub Profile".to_string(),
            mode: "peer".to_string(),
            connect_locators: vec![],
            listen_locators: vec![],
            scout_multicast: true,
            user_auth: None,
            tls_config: None,
            custom_config: None,
            created_at: 0,
            updated_at: 0,
        };
        state.db.save_profile(&profile).expect("save profile");

        let connect_resp = execute_tool_on_state(
            "zenoh_connect_session",
            json!({ "profile_id": "mcp-sub-profile" }),
            &state,
            None,
        ).await;
        assert!(connect_resp.success);

        // Subscribe to a topic
        let sub_resp = execute_tool_on_state(
            "zenoh_subscribe",
            json!({ "key_expr": "test/mcp/preset" }),
            &state,
            None,
        ).await;
        assert!(sub_resp.success);
        let sub_id = sub_resp.data["subscription_id"].as_str().unwrap();

        // Check preset in DB
        let presets = state.db.get_presets("mcp-sub-profile").expect("get presets");
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].id, sub_id);
        assert_eq!(presets[0].key_expr, "test/mcp/preset");

        // Unsubscribe
        let unsub_resp = execute_tool_on_state(
            "zenoh_unsubscribe",
            json!({ "subscription_id": sub_id }),
            &state,
            None,
        ).await;
        assert!(unsub_resp.success);

        // Check preset removed from DB
        let presets_after = state.db.get_presets("mcp-sub-profile").expect("get presets");
        assert_eq!(presets_after.len(), 0);

        let _ = execute_tool_on_state("zenoh_disconnect_session", json!({}), &state, None).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_get_profiles() {
        let state = create_test_state();

        let profile = crate::db::models::ConnectionProfile {
            id: "profile-123".to_string(),
            name: "Test Router Node".to_string(),
            mode: "router".to_string(),
            connect_locators: vec![],
            listen_locators: vec!["tcp/127.0.0.1:7447".to_string()],
            scout_multicast: true,
            user_auth: None,
            tls_config: None,
            custom_config: None,
            created_at: 0,
            updated_at: 0,
        };
        state.db.save_profile(&profile).expect("save profile");

        let resp = execute_tool_on_state("zenoh_get_profiles", json!({}), &state, None).await;
        assert!(resp.success);
        let profiles = resp.data.as_array().expect("profiles array");
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0]["id"], "profile-123");
        assert_eq!(profiles[0]["name"], "Test Router Node");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_edit_profile_success() {
        let state = create_test_state();

        let profile = crate::db::models::ConnectionProfile {
            id: "profile-edit-1".to_string(),
            name: "Initial Node Name".to_string(),
            mode: "peer".to_string(),
            connect_locators: vec!["tcp/127.0.0.1:7447".to_string()],
            listen_locators: vec![],
            scout_multicast: true,
            user_auth: None,
            tls_config: None,
            custom_config: None,
            created_at: 100,
            updated_at: 100,
        };
        state.db.save_profile(&profile).expect("save initial profile");

        let resp = execute_tool_on_state(
            "zenoh_edit_profile",
            json!({
                "profile_id": "profile-edit-1",
                "name": "Updated Node Name",
                "connect_locators": ["tcp/10.0.0.1:7447"],
                "listen_locators": ["tcp/0.0.0.0:7447"],
                "scout_multicast": false
            }),
            &state,
            None,
        ).await;

        assert!(resp.success);
        assert_eq!(resp.data["profile"]["name"], "Updated Node Name");
        assert_eq!(resp.data["profile"]["mode"], "peer");
        assert_eq!(resp.data["profile"]["connect_locators"], json!(["tcp/10.0.0.1:7447"]));
        assert_eq!(resp.data["profile"]["listen_locators"], json!(["tcp/0.0.0.0:7447"]));
        assert_eq!(resp.data["profile"]["scout_multicast"], false);

        // Check database
        let updated = state.db.get_profile_by_id("profile-edit-1").expect("db").expect("profile");
        assert_eq!(updated.name, "Updated Node Name");
        assert_eq!(updated.mode, "peer");
        assert_eq!(updated.connect_locators, vec!["tcp/10.0.0.1:7447"]);
        assert_eq!(updated.listen_locators, vec!["tcp/0.0.0.0:7447"]);
        assert!(!updated.scout_multicast);
        assert_eq!(updated.created_at, 100);
        assert!(updated.updated_at > 100);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_edit_profile_rejects_mode_change() {
        let state = create_test_state();

        let profile = crate::db::models::ConnectionProfile {
            id: "profile-edit-mode".to_string(),
            name: "Mode Test Node".to_string(),
            mode: "peer".to_string(),
            connect_locators: vec![],
            listen_locators: vec![],
            scout_multicast: true,
            user_auth: None,
            tls_config: None,
            custom_config: None,
            created_at: 100,
            updated_at: 100,
        };
        state.db.save_profile(&profile).expect("save initial profile");

        let resp = execute_tool_on_state(
            "zenoh_edit_profile",
            json!({
                "profile_id": "profile-edit-mode",
                "mode": "router"
            }),
            &state,
            None,
        ).await;

        assert!(!resp.success);
        assert!(resp.error.unwrap().contains("mode"));

        // Mode in DB remains unchanged
        let unchanged = state.db.get_profile_by_id("profile-edit-mode").expect("db").expect("profile");
        assert_eq!(unchanged.mode, "peer");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_edit_profile_not_found() {
        let state = create_test_state();

        let resp = execute_tool_on_state(
            "zenoh_edit_profile",
            json!({
                "profile_id": "non-existent-profile",
                "name": "New Name"
            }),
            &state,
            None,
        ).await;

        assert!(!resp.success);
        assert!(resp.error.unwrap().contains("not found"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_edit_profile_restart_session() {
        let state = create_test_state();

        let profile = crate::db::models::ConnectionProfile {
            id: "profile-edit-restart".to_string(),
            name: "Restart Node".to_string(),
            mode: "peer".to_string(),
            connect_locators: vec![],
            listen_locators: vec![],
            scout_multicast: true,
            user_auth: None,
            tls_config: None,
            custom_config: None,
            created_at: 100,
            updated_at: 100,
        };
        state.db.save_profile(&profile).expect("save initial profile");

        // Connect session for this profile
        let conn_resp = execute_tool_on_state(
            "zenoh_connect_session",
            json!({ "profile_id": "profile-edit-restart" }),
            &state,
            None,
        ).await;
        assert!(conn_resp.success);
        let old_session_id = conn_resp.data["id"].as_str().unwrap().to_string();

        // Edit with restart_session: true
        let edit_resp = execute_tool_on_state(
            "zenoh_edit_profile",
            json!({
                "profile_id": "profile-edit-restart",
                "name": "Restart Node Renamed",
                "restart_session": true
            }),
            &state,
            None,
        ).await;

        assert!(edit_resp.success);
        assert_eq!(edit_resp.data["session_restarted"], true);
        let new_session_id = edit_resp.data["session_id"].as_str().unwrap();
        assert_ne!(new_session_id, old_session_id);

        // Verify active sessions has new session ID
        let sessions = state.session_manager.get_all_sessions().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id.to_string(), new_session_id);

        let _ = execute_tool_on_state("zenoh_disconnect_session", json!({}), &state, None).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_create_profile_success() {
        let state = create_test_state();

        let resp = execute_tool_on_state(
            "zenoh_create_profile",
            json!({
                "name": "Router R3",
                "mode": "router",
                "connect_locators": ["tcp/127.0.0.1:7448"],
                "listen_locators": ["tcp/0.0.0.0:7449"],
                "scout_multicast": true
            }),
            &state,
            None,
        ).await;

        assert!(resp.success);
        let profile_id = resp.data["profile"]["id"].as_str().unwrap();
        assert_eq!(resp.data["profile"]["name"], "Router R3");
        assert_eq!(resp.data["profile"]["mode"], "router");
        assert_eq!(resp.data["profile"]["connect_locators"], json!(["tcp/127.0.0.1:7448"]));
        assert_eq!(resp.data["profile"]["listen_locators"], json!(["tcp/0.0.0.0:7449"]));

        // Verify in DB
        let profile_in_db = state.db.get_profile_by_id(profile_id).expect("db").expect("profile in db");
        assert_eq!(profile_in_db.name, "Router R3");
        assert_eq!(profile_in_db.mode, "router");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_create_profile_validation_errors() {
        let state = create_test_state();

        // Missing name
        let resp = execute_tool_on_state(
            "zenoh_create_profile",
            json!({
                "mode": "router"
            }),
            &state,
            None,
        ).await;
        assert!(!resp.success);

        // Invalid mode
        let resp = execute_tool_on_state(
            "zenoh_create_profile",
            json!({
                "name": "Bad Mode Node",
                "mode": "invalid"
            }),
            &state,
            None,
        ).await;
        assert!(!resp.success);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_execute_tool_create_profile_connect_now() {
        let state = create_test_state();

        let resp = execute_tool_on_state(
            "zenoh_create_profile",
            json!({
                "name": "Router Instant",
                "mode": "router",
                "listen_locators": ["tcp/0.0.0.0:7455"],
                "connect_now": true
            }),
            &state,
            None,
        ).await;

        assert!(resp.success);
        assert_eq!(resp.data["session_started"], true);
        let session_id = resp.data["session_id"].as_str().unwrap();

        let sessions = state.session_manager.get_all_sessions().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id.to_string(), session_id);

        let _ = execute_tool_on_state("zenoh_disconnect_session", json!({}), &state, None).await;
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_dispatch_mcp_tool_live_gui_mode() {
        let _lock = TEST_DISPATCH_MUTEX.lock().await;
        let id = Uuid::new_v4().simple().to_string();
        let socket_path = PathBuf::from(format!("/tmp/zx-mcp-{}.sock", &id[..8]));
        let _ = std::fs::remove_file(&socket_path);

        let listener = crate::ipc::server::bind_unix_listener(&socket_path).expect("bind listener");
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let server_task = tokio::spawn(async move {
            tokio::select! {
                _ = async {
                    while let Ok((stream, _)) = listener.accept().await {
                        tokio::spawn(async move {
                            crate::ipc::server::handle_connection(stream, None, None).await;
                        });
                    }
                } => {},
                _ = &mut shutdown_rx => {},
            }
        });

        // Set ZENOHX_IPC_SOCKET to point directly to our test socket
        let prev_sock = std::env::var("ZENOHX_IPC_SOCKET").ok();
        std::env::set_var("ZENOHX_IPC_SOCKET", &socket_path);

        let resp = dispatch_mcp_tool(
            "zenohx_gui_switch_workspace",
            json!({ "workspace": "query" }),
        ).await;

        if let Some(prev) = prev_sock {
            std::env::set_var("ZENOHX_IPC_SOCKET", prev);
        } else {
            std::env::remove_var("ZENOHX_IPC_SOCKET");
        }

        assert!(!resp.is_error);
        assert!(resp.text.contains("[Mode: Live GUI]"));
        assert!(resp.text.contains("query"));

        let _ = shutdown_tx.send(());
        let _ = server_task.await;
        let _ = std::fs::remove_file(&socket_path);
    }
}
