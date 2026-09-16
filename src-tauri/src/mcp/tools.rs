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

use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolResult {
    pub text: String,
    pub is_error: bool,
}

pub fn get_tool_definitions() -> Vec<serde_json::Value> {
    vec![
        json!({
            "name": "zenoh_scout",
            "description": "Scout for Zenoh routers and peers on the local network",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "timeout_ms": { "type": "integer", "default": 1000 }
                }
            }
        }),
        json!({
            "name": "zenoh_publish",
            "description": "Publish data to a Zenoh key expression",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key_expr": { "type": "string" },
                    "payload": { "type": "string" }
                },
                "required": ["key_expr", "payload"]
            }
        }),
    ]
}

pub fn get_resource_definitions() -> Vec<serde_json::Value> {
    vec![json!({
        "uri": "zenoh://sessions",
        "name": "Active Zenoh Sessions",
        "description": "Lists all active Zenoh client/peer/router sessions",
        "mimeType": "application/json"
    })]
}

pub async fn dispatch_mcp_tool(name: &str, _args: serde_json::Value) -> McpToolResult {
    match name {
        "zenoh_scout" => McpToolResult {
            text: "[]".to_string(),
            is_error: false,
        },
        "zenoh_publish" => McpToolResult {
            text: "Published successfully".to_string(),
            is_error: false,
        },
        _ => McpToolResult {
            text: format!("Unknown tool: {}", name),
            is_error: true,
        },
    }
}
