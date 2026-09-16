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

pub mod installer;
pub mod protocol;
pub mod tools;

#[cfg(test)]
mod protocol_tests;

pub use protocol::{
    handle_jsonrpc_message, run_mcp_server_stream, run_mcp_stdio_server, JsonRpcError,
    JsonRpcRequest, JsonRpcResponse, McpResource, McpTool,
};
pub use tools::{
    dispatch_mcp_tool, execute_tool_on_state, execute_tool_on_state_with_mode,
    get_resource_definitions, get_tool_definitions, McpToolResult,
};
