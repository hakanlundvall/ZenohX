use super::get_socket_path;
use super::types::{IpcRequest, IpcResponse};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct IpcClient {
    #[cfg(unix)]
    stream: tokio::net::UnixStream,
}

impl IpcClient {
    pub async fn connect() -> Result<Self, String> {
        let path = get_socket_path();
        Self::connect_to(&path).await
    }

    pub async fn connect_to(path: &Path) -> Result<Self, String> {
        #[cfg(unix)]
        {
            let stream = tokio::net::UnixStream::connect(path)
                .await
                .map_err(|e| format!("Failed to connect to IPC socket at {:?}: {}", path, e))?;
            Ok(Self { stream })
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Err("IPC on non-unix not implemented yet".to_string())
        }
    }

    pub async fn call(&mut self, req: &IpcRequest) -> Result<IpcResponse, String> {
        #[cfg(unix)]
        {
            let mut line = serde_json::to_string(req).map_err(|e| e.to_string())?;
            line.push('\n');

            let (reader, mut writer) = self.stream.split();
            writer
                .write_all(line.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
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

            serde_json::from_str(&response_line)
                .map_err(|e| format!("Failed to parse response: {}", e))
        }
        #[cfg(not(unix))]
        {
            let _ = req;
            Err("IPC on non-unix not implemented yet".to_string())
        }
    }
}
