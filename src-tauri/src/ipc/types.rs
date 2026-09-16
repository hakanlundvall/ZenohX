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
