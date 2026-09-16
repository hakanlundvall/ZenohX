pub mod client;
pub mod server;
pub mod types;

#[cfg(test)]
mod tests;

pub use client::IpcClient;
pub use types::{IpcRequest, IpcResponse};

use std::path::PathBuf;

pub fn get_socket_path() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(r"\\.\pipe\zenohx-ipc")
    }
    #[cfg(not(windows))]
    {
        if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            if !runtime_dir.trim().is_empty() {
                return PathBuf::from(runtime_dir).join("zenohx.sock");
            }
        }

        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".zenohx").join("zenohx.sock")
    }
}
