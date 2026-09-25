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

//! Routes `invoke` calls from web clients to the same Tauri command functions
//! the desktop webview uses.

use crate::commands::*;
use crate::AppState;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

/// Reads one command argument. Like Tauri, argument names are camelCase and a
/// missing argument deserializes from `null` (so `Option` parameters are optional).
fn arg<T: DeserializeOwned>(args: &Value, name: &str) -> Result<T, String> {
    let value = args.get(name).cloned().unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|e| format!("invalid argument '{name}': {e}"))
}

fn to_json<T: Serialize>(result: Result<T, String>) -> Result<Value, String> {
    result.and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string()))
}

/// Runs a command by name with JSON arguments, as the Tauri IPC would.
pub async fn dispatch(app: &AppHandle, cmd: &str, a: Value) -> Result<Value, String> {
    let state = || app.state::<AppState>();
    let app = || app.clone();

    match cmd {
        // Sessions & topology
        "connect_session" => to_json(connect_session(state(), arg(&a, "config")?).await),
        "connect_node_by_zid" => to_json(connect_node_by_zid(state(), arg(&a, "zid")?).await),
        "disconnect_session" => to_json(disconnect_session(state(), arg(&a, "sessionId")?).await),
        "scout_locators" => to_json(scout_locators(state(), arg(&a, "timeoutMs")?).await),
        "get_session_info" => to_json(get_session_info(state(), arg(&a, "sessionId")?).await),
        "get_all_sessions" => to_json(get_all_sessions(state()).await),
        "get_node_configuration" => to_json(get_node_configuration(state(), arg(&a, "zid")?).await),
        "query_admin_space" => to_json(
            query_admin_space(state(), arg(&a, "sessionId")?, arg(&a, "selector")?, arg(&a, "timeoutMs")?)
                .await,
        ),
        "discover_admin_topology" => to_json(
            discover_admin_topology(state(), arg(&a, "sessionId")?, arg(&a, "maxDepth")?, arg(&a, "timeoutMs")?)
                .await,
        ),

        // Pub/Sub
        "publish_sample" => to_json(
            publish_sample(
                state(),
                arg(&a, "sessionId")?,
                arg(&a, "keyExpr")?,
                arg(&a, "payload")?,
                arg(&a, "encoding")?,
                arg(&a, "kind")?,
            )
            .await,
        ),
        "publish_sample_advanced" => to_json(
            publish_sample_advanced(
                state(),
                arg(&a, "sessionId")?,
                arg(&a, "keyExpr")?,
                arg(&a, "payload")?,
                arg(&a, "encoding")?,
                arg(&a, "kind")?,
                arg(&a, "options")?,
            )
            .await,
        ),
        "subscribe" => to_json(
            subscribe(app(), state(), arg(&a, "sessionId")?, arg(&a, "subId")?, arg(&a, "keyExpr")?).await,
        ),
        "subscribe_advanced" => to_json(
            subscribe_advanced(
                app(),
                state(),
                arg(&a, "sessionId")?,
                arg(&a, "subId")?,
                arg(&a, "keyExpr")?,
                arg(&a, "options")?,
            )
            .await,
        ),
        "unsubscribe" => to_json(unsubscribe(state(), arg(&a, "sessionId")?, arg(&a, "subId")?).await),
        "start_stream_generator" => to_json(start_stream_generator(state(), arg(&a, "config")?).await),
        "stop_stream_generator" => to_json(stop_stream_generator(state(), arg(&a, "generatorId")?).await),

        // Queries & queryables
        "query_get" => to_json(
            query_get(
                state(),
                arg(&a, "sessionId")?,
                arg(&a, "selector")?,
                arg(&a, "target")?,
                arg(&a, "timeoutMs")?,
                arg(&a, "payload")?,
                arg(&a, "encoding")?,
                arg(&a, "consolidation")?,
            )
            .await,
        ),
        "declare_queryable" => to_json(
            declare_queryable(app(), state(), arg(&a, "sessionId")?, arg(&a, "queryableId")?, arg(&a, "keyExpr")?)
                .await,
        ),
        "undeclare_queryable" => {
            to_json(undeclare_queryable(state(), arg(&a, "sessionId")?, arg(&a, "queryableId")?).await)
        }
        "reply_query" => to_json(
            reply_query(state(), arg(&a, "token")?, arg(&a, "keyExpr")?, arg(&a, "payload")?, arg(&a, "encoding")?)
                .await,
        ),
        "save_queryable_preset" => to_json(save_queryable_preset(state(), arg(&a, "preset")?).await),
        "load_queryable_presets" => to_json(load_queryable_presets(state(), arg(&a, "profileId")?).await),
        "delete_queryable_preset" => to_json(delete_queryable_preset(state(), arg(&a, "presetId")?).await),
        "save_query_execution" => to_json(save_query_execution(state(), arg(&a, "execution")?).await),
        "load_query_history" => to_json(
            load_query_history(state(), arg(&a, "profileId")?, arg(&a, "limit")?, arg(&a, "offset")?).await,
        ),
        "clear_query_history" => to_json(clear_query_history(state(), arg(&a, "profileId")?).await),
        "delete_query_execution" => to_json(delete_query_execution(state(), arg(&a, "executionId")?).await),

        // Profiles, presets & history
        "save_profile" => to_json(save_profile(state(), arg(&a, "profile")?).await),
        "load_profiles" => to_json(load_profiles(state()).await),
        "delete_profile" => to_json(delete_profile(state(), arg(&a, "profileId")?).await),
        "save_subscription_preset" => to_json(save_subscription_preset(state(), arg(&a, "preset")?).await),
        "load_subscription_presets" => to_json(load_subscription_presets(state(), arg(&a, "profileId")?).await),
        "delete_subscription_preset" => {
            to_json(delete_subscription_preset(state(), arg(&a, "presetId")?).await)
        }
        "query_messages" => to_json(
            query_messages(state(), arg(&a, "profileId")?, arg(&a, "limit")?, arg(&a, "offset")?).await,
        ),
        "save_message" => to_json(save_message(state(), arg(&a, "message")?).await),
        "clear_message_history" => to_json(clear_message_history(state(), arg(&a, "profileId")?).await),
        "delete_message" => to_json(delete_message(state(), arg(&a, "messageId")?).await),

        // mDNS
        "get_mdns_status" => to_json(get_mdns_status(state()).await),
        "set_mdns_config" => to_json(set_mdns_config(state(), arg(&a, "enabled")?, arg(&a, "hostname")?).await),
        "refresh_mdns_interfaces" => to_json(refresh_mdns_interfaces(state()).await),

        // MCP agent installer
        "get_mcp_agents" => to_json(get_mcp_agents().await),
        "get_mcp_config_json" => to_json(get_mcp_config_json().await),
        "install_mcp_agent" => to_json(install_mcp_agent(arg(&a, "agentId")?).await),
        "uninstall_mcp_agent" => to_json(uninstall_mcp_agent(arg(&a, "agentId")?).await),
        "install_all_detected_mcp_agents" => to_json(install_all_detected_mcp_agents().await),

        // Tauri core plugins the UI uses
        "plugin:app|version" => Ok(Value::String(app().package_info().version.to_string())),
        "plugin:app|name" => Ok(Value::String(app().package_info().name.clone())),

        other => Err(format!("Command '{other}' is not available in web mode")),
    }
}
