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

use zenohx_lib::mcp::installer::handle_cli_args;
use zenohx_lib::mcp::protocol::run_mcp_stdio_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let handled = handle_cli_args(&args)?;
        if handled {
            return Ok(());
        }
    }

    eprintln!(
        "[zenohx-mcp] Starting ZenohX MCP server on stdio (PID: {})...",
        std::process::id()
    );

    if let Err(e) = run_mcp_stdio_server().await {
        eprintln!("[zenohx-mcp] Server exited with error: {}", e);
        std::process::exit(1);
    }

    eprintln!("[zenohx-mcp] Stdio stream closed, exiting.");
    Ok(())
}
