#[cfg(test)]
mod tests {
    use crate::ipc::client::IpcClient;
    use crate::ipc::get_socket_path;
    use crate::ipc::types::{IpcRequest, IpcResponse};
    use serde_json::json;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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
    fn test_ipc_request_ping_and_get_state() {
        let ping = IpcRequest::Ping;
        let serialized = serde_json::to_string(&ping).expect("serialize");
        assert!(serialized.contains(r#""method":"ping""#));
        let deserialized: IpcRequest = serde_json::from_str(&serialized).expect("deserialize");
        match deserialized {
            IpcRequest::Ping => {}
            _ => panic!("expected Ping variant"),
        }

        let state_req = IpcRequest::GetState;
        let serialized = serde_json::to_string(&state_req).expect("serialize");
        assert!(serialized.contains(r#""method":"get_state""#));
        let deserialized: IpcRequest = serde_json::from_str(&serialized).expect("deserialize");
        match deserialized {
            IpcRequest::GetState => {}
            _ => panic!("expected GetState variant"),
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
        assert!(deserialized.error.is_none());
    }

    #[test]
    fn test_ipc_response_helpers() {
        let ok_resp = IpcResponse::ok("headless", json!({ "status": "active" }));
        assert!(ok_resp.success);
        assert_eq!(ok_resp.mode, "headless");
        assert_eq!(ok_resp.data["status"], "active");
        assert!(ok_resp.error.is_none());

        let err_resp = IpcResponse::err("live_gui", "session failed");
        assert!(!err_resp.success);
        assert_eq!(err_resp.mode, "live_gui");
        assert_eq!(err_resp.data, serde_json::Value::Null);
        assert_eq!(err_resp.error, Some("session failed".to_string()));
    }

    #[test]
    fn test_get_socket_path() {
        let path = get_socket_path();
        assert!(path.ends_with("zenohx.sock"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_ipc_client_call_mock_server() {
        let socket_path = std::env::temp_dir().join(format!("zenohx_test_{}.sock", uuid::Uuid::new_v4()));
        let _ = std::fs::remove_file(&socket_path);

        let listener = tokio::net::UnixListener::bind(&socket_path).expect("bind socket");

        // Spawn mock server task
        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let (reader, mut writer) = stream.into_split();
            let mut buf_reader = BufReader::new(reader);
            let mut line = String::new();
            buf_reader.read_line(&mut line).await.expect("read line");

            let req: IpcRequest = serde_json::from_str(&line).expect("parse request");
            let resp = match req {
                IpcRequest::Ping => IpcResponse::ok("live_gui", json!({ "status": "pong" })),
                _ => IpcResponse::err("live_gui", "unexpected"),
            };

            let mut resp_str = serde_json::to_string(&resp).expect("serialize");
            resp_str.push('\n');
            writer.write_all(resp_str.as_bytes()).await.expect("write");
            writer.flush().await.expect("flush");
        });

        let mut client = IpcClient::connect_to(&socket_path).await.expect("connect");
        let resp = client.call(&IpcRequest::Ping).await.expect("call");

        assert!(resp.success);
        assert_eq!(resp.mode, "live_gui");
        assert_eq!(resp.data["status"], "pong");

        server_handle.await.expect("server finish");
        let _ = std::fs::remove_file(&socket_path);
    }
}
