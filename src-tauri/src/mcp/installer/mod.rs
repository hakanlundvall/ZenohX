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

pub mod cli;
pub mod mutator;
pub mod registry;
pub mod types;

#[cfg(test)]
mod mutator_tests;
#[cfg(test)]
mod tests;

pub use cli::handle_cli_args;
pub use mutator::{install_agent, install_agent_by_id, uninstall_agent, uninstall_agent_by_id};
pub use registry::{get_agent_by_id, get_all_agents, get_home_dir, resolve_binary_command};
pub use types::{AgentTarget, ConfigFormat};

