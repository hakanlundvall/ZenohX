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
    use crate::mcp::protocol::{
        handle_jsonrpc_message, run_mcp_server_stream, JsonRpcError, JsonRpcRequest,
        JsonRpcResponse, McpResource, McpTool,
    };
    use serde_json::json;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

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
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert!(result["capabilities"]["tools"].is_object());
        assert!(result["capabilities"]["resources"].is_object());
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

    #[tokio::test]
    async fn test_mcp_notifications_initialized() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };
        let resp = handle_jsonrpc_message(req).await;
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, None);
        assert!(resp.error.is_none());
        assert_eq!(resp.result, Some(json!({})));
    }

    #[tokio::test]
    async fn test_mcp_ping() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!("ping-1")),
            method: "ping".to_string(),
            params: None,
        };
        let resp = handle_jsonrpc_message(req).await;
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Some(json!("ping-1")));
        assert!(resp.error.is_none());
        assert_eq!(resp.result, Some(json!({})));
    }

    #[tokio::test]
    async fn test_mcp_resources_list() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(3)),
            method: "resources/list".to_string(),
            params: None,
        };
        let resp = handle_jsonrpc_message(req).await;
        let result = resp.result.expect("result present");
        let resources = result["resources"].as_array().expect("resources array");
        assert!(resources.iter().any(|r| r["uri"] == "zenohx://sessions"));
    }

    #[tokio::test]
    async fn test_mcp_tools_call() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(4)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "zenoh_get_sessions",
                "arguments": {}
            })),
        };
        let resp = handle_jsonrpc_message(req).await;
        assert_eq!(resp.id, Some(json!(4)));
        let result = resp.result.expect("result present");
        let content = result["content"].as_array().expect("content array");
        assert!(!content.is_empty());
        assert_eq!(content[0]["type"], "text");
        assert_eq!(result["isError"], false);
    }

    #[tokio::test]
    async fn test_mcp_unknown_method_returns_32601() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(5)),
            method: "unsupported_method".to_string(),
            params: None,
        };
        let resp = handle_jsonrpc_message(req).await;
        assert_eq!(resp.id, Some(json!(5)));
        assert!(resp.result.is_none());
        let err = resp.error.expect("error present");
        assert_eq!(err.code, -32601);
        assert!(err.message.contains("unsupported_method"));
    }

    #[test]
    fn test_mcp_models_serialization_roundtrip() {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(10)),
            method: "tools/list".to_string(),
            params: Some(json!({ "cursor": "abc" })),
        };
        let serialized = serde_json::to_string(&req).expect("serialize req");
        let deserialized: JsonRpcRequest = serde_json::from_str(&serialized).expect("deserialize req");
        assert_eq!(deserialized.method, "tools/list");
        assert_eq!(deserialized.id, Some(json!(10)));

        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(10)),
            result: Some(json!({ "ok": true })),
            error: None,
        };
        let serialized_resp = serde_json::to_string(&resp).expect("serialize resp");
        assert!(!serialized_resp.contains("error")); // skip_serializing_if Option::is_none

        let err_resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(11)),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        };
        let serialized_err = serde_json::to_string(&err_resp).expect("serialize err");
        assert!(!serialized_err.contains("result")); // skip_serializing_if Option::is_none

        let tool = McpTool {
            name: "test_tool".to_string(),
            description: Some("desc".to_string()),
            input_schema: json!({ "type": "object" }),
        };
        let tool_ser = serde_json::to_string(&tool).expect("tool ser");
        assert!(tool_ser.contains("inputSchema"));

        let resource = McpResource {
            uri: "zenoh://test".to_string(),
            name: "Test Resource".to_string(),
            description: Some("Resource desc".to_string()),
            mime_type: Some("application/json".to_string()),
        };
        let res_ser = serde_json::to_string(&resource).expect("res ser");
        assert!(res_ser.contains("mimeType"));
    }

    #[tokio::test]
    async fn test_run_mcp_server_stream_handles_requests_and_parse_errors() {
        let (client_reader, server_writer) = tokio::io::duplex(4096);
        let (server_reader, mut client_writer) = tokio::io::duplex(4096);

        // Run server task in background
        let server_handle = tokio::spawn(async move {
            run_mcp_server_stream(server_reader, server_writer).await
        });

        // 1. Send valid ping request
        let ping_req = json!({
            "jsonrpc": "2.0",
            "id": "test-ping",
            "method": "ping"
        });
        client_writer
            .write_all(format!("{}\n", ping_req).as_bytes())
            .await
            .expect("write ping");
        client_writer.flush().await.expect("flush");

        // 2. Send notification (no id) -> should NOT produce a response line
        let notif = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        client_writer
            .write_all(format!("{}\n", notif).as_bytes())
            .await
            .expect("write notif");
        client_writer.flush().await.expect("flush");

        // 3. Send invalid json line
        client_writer
            .write_all(b"not-json\n")
            .await
            .expect("write invalid json");
        client_writer.flush().await.expect("flush");

        // Close client writer so server stream reaches EOF
        drop(client_writer);

        let mut lines = tokio::io::BufReader::new(client_reader).lines();

        // Check line 1 (ping response)
        let line1 = lines.next_line().await.expect("read line 1").expect("line 1 present");
        let resp1: JsonRpcResponse = serde_json::from_str(&line1).expect("parse line 1");
        assert_eq!(resp1.id, Some(json!("test-ping")));
        assert!(resp1.error.is_none());

        // Check line 2 (parse error response - notification produced NO line!)
        let line2 = lines.next_line().await.expect("read line 2").expect("line 2 present");
        let resp2: JsonRpcResponse = serde_json::from_str(&line2).expect("parse line 2");
        assert!(resp2.id.is_none());
        let err = resp2.error.expect("parse error present");
        assert_eq!(err.code, -32700);
        assert!(err.message.contains("Parse error"));

        // Server should shut down cleanly on EOF
        let res = server_handle.await.expect("join handle");
        assert!(res.is_ok());
    }
}
