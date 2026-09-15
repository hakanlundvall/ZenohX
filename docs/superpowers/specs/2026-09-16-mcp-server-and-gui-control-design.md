# Design Specification: ZenohX Model Context Protocol (MCP) Server & Live GUI Control

**Date:** 2026-09-16  
**Status:** Proposed  
**Scope:** Architectural (MCP Server Subsystem, Local IPC Bridge, CLI Binary, and React GUI Synchronization)

---

## 1. Overview & Problem Statement

As AI coding assistants and autonomous agents (e.g. Antigravity, Claude Desktop, Cursor, Zed) become central to robotics, IoT, and distributed systems workflows, developers need AI models to interact directly with the Eclipse Zenoh middleware layer: scouting the network, inspecting topologies, connecting sessions, publishing sensor/control samples, querying queryables, and verifying message feeds.

Currently, ZenohX provides a desktop GUI (built with Tauri 2 and React) and stores profiles, query history, and messages in SQLite. However, there is no standardized protocol interface for AI agents to control ZenohX or orchestrate Zenoh operations.

This specification defines the architecture, protocol, and implementation of a native Model Context Protocol (MCP) server for ZenohX that:
1. Speaks the standard MCP JSON-RPC 2.0 protocol over `stdio`.
2. Connects to the running ZenohX desktop application via local Inter-Process Communication (IPC) to provide **Live GUI Control**, synchronizing UI workspaces, active sessions, and publishing/querying in real-time.
3. Automatically falls back to **Headless Execution** if the desktop GUI is not running, sharing the SQLite database and executing Zenoh operations independently in the background.

---

## 2. Architecture & Data Flow

```
+-------------------------------------------------------------------------------+
|                      AI Assistant / MCP Client                                |
|        (Antigravity, Claude Desktop, Cursor, Custom Agent)                    |
+---------------------------------------+---------------------------------------+
                                        | stdio (JSON-RPC 2.0)
+---------------------------------------v---------------------------------------+
|                    zenohx-mcp (Native Rust CLI)                               |
|                                                                               |
|  +-------------------------------------------------------------------------+  |
|  |                           MCP Protocol Engine                           |  |
|  |  - initialize, tools/list, tools/call, resources/list, resources/read   |  |
|  +------------------------------------+------------------------------------+  |
|                                       |
|                  Probe IPC Socket / Named Pipe?
|                                       |
|             [Socket Active]           |           [Socket Inactive]
|                    |                  |                   |
|                    v                  |                   v
|   +--------------------------------+  |  +---------------------------------+
|   |         IPC Client             |  |  |       Headless Engine           |
|   |  - Forward tool calls to GUI   |  |  |  - Local SessionManager (Zenoh) |
|   |  - Receive live GUI responses  |  |  |  - Direct SQLite Database       |
|   +--------------------------------+  |  +---------------------------------+
+--------------------+--------------------------------------+-------------------+
                     | Local IPC (Unix Socket / Pipe)       |
                     v                                      | Direct Zenoh API
+----------------------------------------------------+      |
|               ZenohX Desktop Application           |      |
|                                                    |      |
|  +----------------------------------------------+  |      |
|  |           Tauri IPC Listener Task            |  |      |
|  |  - Dispatches commands to SessionManager/DB  |  |      |
|  |  - Emits events to Webview                   |  |      |
|  +----------------------+-----------------------+  |      |
|                         | Tauri Events             |      |
|  +----------------------v-----------------------+  |      |
|  |       React Webview (Frontend)               |  |      |
|  |  - Active workspace switch                   |  |      |
|  |  - AI action notification toast              |  |      |
|  |  - Live Zustand store updates                |  |      |
|  +----------------------------------------------+  |      |
|                                                    |      |
|  +----------------------------------------------+  |      |
|  |     SessionManager & SQLite Database         |  |      |
|  +----------------------+-----------------------+  |      |
+-------------------------|--------------------------+      |
                          |                                 |
                          +----------------+----------------+
                                           |
                                           v
                             Eclipse Zenoh Network (LAN/WAN)
```

---

## 3. Detailed Component Specifications

### 3.1 Local IPC Subsystem (`src-tauri/src/ipc/`)

#### 3.1.1 Socket Transport
* **Unix (Linux & macOS)**:
  * Path: `$XDG_RUNTIME_DIR/zenohx.sock` if `$XDG_RUNTIME_DIR` is set, otherwise `$HOME/.zenohx/zenohx.sock`.
  * Permissions: Enforces file mode `0600` (read/write by owning user only).
  * Cleanup: Automatically unlinks existing stale socket files on startup after confirming connection failure. Unlinks cleanly on app exit.
* **Windows**:
  * Path: `\\.\pipe\zenohx-ipc`.

#### 3.1.2 Message Framing & Format
* Transport messages are line-delimited JSON (`\n`).
* **Request Structure**:
  ```json
  {
    "id": 1,
    "method": "execute_tool",
    "params": {
      "tool": "zenoh_publish",
      "args": {
        "key_expr": "demo/temperature",
        "payload": "{\"celsius\": 24.5}",
        "encoding": "application/json"
      }
    }
  }
  ```
* **Response Structure**:
  ```json
  {
    "id": 1,
    "result": {
      "mode": "live_gui",
      "success": true,
      "data": { ... }
    },
    "error": null
  }
  ```

#### 3.1.3 IPC Server in Tauri (`src-tauri/src/ipc/server.rs`)
* Runs inside `tauri::async_runtime::spawn` within `setup()` in `src-tauri/src/lib.rs`.
* Shares `AppState` (`Arc<SessionManager>`, `Arc<Database>`, `MdnsManager`) and `AppHandle`.
* When an IPC request is received:
  1. If it targets a GUI state change (e.g. `zenohx_gui_switch_workspace`), emits `zenohx://gui-switch-tab` to the webview and acknowledges.
  2. If it targets a Zenoh operation (`publish`, `query`, `connect`, etc.), dispatches to the internal command logic, emits `zenohx://mcp-action` with an action summary for the user toast, and returns the serialized outcome.

---

### 3.2 MCP Server CLI (`src-tauri/src/bin/zenohx-mcp.rs`)

#### 3.2.1 Entry Point & Lifecycle
* Configured as a Cargo binary target: `[[bin]] name = "zenohx-mcp" path = "src/bin/zenohx-mcp.rs"`.
* Invoked via CLI: `zenohx-mcp` (or launched by desktop wrappers via `npx zenohx mcp`).
* Reads JSON-RPC 2.0 lines from `stdin` and outputs to `stdout`.
* Logging is strictly directed to `stderr` to avoid corrupting stdio JSON-RPC.

#### 3.2.2 Dual-Mode Selection Logic
On startup or on each tool call:
1. Probe IPC socket with a 100ms timeout (`{"method": "ping"}`).
2. **If response received**:
   * Mode is set to `Live GUI`.
   * Forwards tool execution to the IPC server in the running ZenohX app.
   * Prepends output with `[Mode: Live GUI]`.
3. **If connection fails / timeout**:
   * Mode is set to `Headless`.
   * Locates `zenohx.db` using standard application data directory resolution.
   * Lazily initializes an internal `SessionManager` and `Database`.
   * Directly executes operations.
   * Prepends output with `[Mode: Headless (GUI not running)]`.

---

### 3.3 Exposed MCP Tools Specification

| Tool Name | Parameters | Description |
| :--- | :--- | :--- |
| `zenoh_scout` | `timeout_ms` (integer, default: 1000) | Scouts the local network for Zenoh routers, peers, and locators. |
| `zenoh_connect_session` | `profile_id` (optional string), `mode` (optional "peer"\|"client"), `connect_locators` (optional array of strings), `listen_locators` (optional array of strings) | Opens a Zenoh session using a saved profile or explicit connection locators. |
| `zenoh_disconnect_session` | `session_id` (optional string) | Disconnects an active Zenoh session. Disconnects all if omitted. |
| `zenoh_get_sessions` | None | Returns active sessions, their ZIDs, modes, and connected locators. |
| `zenoh_publish` | `key_expr` (string, required), `payload` (string/JSON, required), `encoding` (optional string, default: "text/plain"), `priority` (optional string), `session_id` (optional string) | Publishes a sample to the specified Zenoh key expression. |
| `zenoh_subscribe` | `key_expr` (string, required), `session_id` (optional string) | Declares a subscriber on the key expression to capture incoming samples. |
| `zenoh_unsubscribe` | `subscription_id` (optional string), `key_expr` (optional string) | Cancels an active Zenoh subscription. |
| `zenoh_get_messages` | `key_expr` (optional string), `limit` (optional integer, default: 50) | Reads captured message buffers or SQLite message history. |
| `zenoh_query` | `key_expr` (string, required), `target` (optional "all"\|"best_matching"\|"complete"), `timeout_ms` (optional integer, default: 5000), `payload` (optional string) | Issues a Zenoh GET query and collects all replies. |
| `zenoh_declare_queryable` | `key_expr` (string, required), `reply_payload` (string, required), `session_id` (optional string) | Registers a queryable endpoint that automatically returns a predefined response. |
| `zenoh_inspect_topology` | `session_id` (optional string) | Queries admin space (`@/admin/**`) to discover routers, peers, and links. |
| `zenohx_gui_switch_workspace` | `workspace` (string: "pubsub"\|"query"\|"traffic"\|"topology"\|"settings") | Switches the active workspace tab in the ZenohX desktop interface. |
| `zenohx_gui_get_state` | None | Retrieves GUI state: current tab, active profile, and error statuses. |

---

### 3.4 Exposed MCP Resources

* `zenohx://sessions`: JSON summary of active Zenoh sessions and status.
* `zenohx://profiles`: List of saved connection profiles from SQLite.
* `zenohx://messages/recent`: Snapshot of recently published and received messages.
* `zenohx://topology`: Discovered topology nodes, routers, and connectivity graph.

---

### 3.5 Frontend Synchronization (`src/hooks/useMcpListener.ts`)

1. **Workspace Switching**:
   * Listens for `zenohx://gui-switch-tab`.
   * Invokes `setActiveTab(payload.workspace)` in React `App.tsx`.
2. **AI Action Feedback**:
   * Listens for `zenohx://mcp-action`.
   * Displays a temporary notification badge / toast:
     `🤖 AI Action: [Action Summary]` (e.g. *"Published sample to demo/temperature"*).
3. **Reactive Stores**:
   * Because Zenoh operations in GUI mode execute through the app's `SessionManager`, existing events (`zenohx://session-status`, `zenohx://sample-stream`, `zenohx://message-received`) continue to update Zustand stores (`connectionStore`, `messageStore`, `trafficStore`, `topologyStore`) automatically with no UI regressions.

---

### 3.6 Error Handling & Edge Cases

1. **GUI Shutdown Mid-Session**: If the GUI process terminates while `zenohx-mcp` is running, the IPC client detects the broken socket on the subsequent call and switches seamlessly to headless execution.
2. **Stale Sockets**: If the operating system or previous ZenohX process was killed abruptly, `zenohx-mcp` attempts to connect before assuming the socket is valid. The GUI server also cleans up existing socket files on boot.
3. **No Active Session**: If a tool requiring a connection (like `zenoh_publish`) is called without an active session, it returns a structured error instructing the AI to invoke `zenoh_connect_session`.
4. **Invalid Key Expressions**: Validated before Zenoh dispatch; descriptive errors are returned via MCP JSON-RPC format.

---

## 4. Verification & Testing Strategy

1. **Rust Unit Tests (`src-tauri`)**:
   * IPC message serialization/deserialization.
   * MCP protocol request/response parser (`initialize`, `tools/list`, `tools/call`).
   * Tool parameter validation.
2. **IPC Integration Test**:
   * Spawn a temporary IPC server on a mock socket and verify end-to-end command dispatch.
3. **Headless E2E Verification**:
   * Run `zenohx-mcp` via stdio pipe, send standard MCP `initialize` and `tools/call` for `zenoh_scout`, and verify valid JSON-RPC output.
4. **Full Test Suite Execution**:
   * Run `npm run test:all` (`tsc --noEmit`, Node unit tests, and `cargo test`).

---

## 5. File Structure Changes

* New files:
  * `src-tauri/src/ipc/mod.rs`: IPC protocol types and framing.
  * `src-tauri/src/ipc/server.rs`: Async IPC listener for Tauri app.
  * `src-tauri/src/ipc/client.rs`: Lightweight IPC client for MCP CLI.
  * `src-tauri/src/mcp/mod.rs`: MCP protocol types, tools, and dispatcher.
  * `src-tauri/src/mcp/tools.rs`: Tool handlers (scout, connect, pub/sub, query, GUI).
  * `src-tauri/src/bin/zenohx-mcp.rs`: Standalone binary for MCP stdio.
  * `src/hooks/useMcpListener.ts`: Frontend React hook for IPC event handling.
* Modified files:
  * `src-tauri/Cargo.toml`: Add `[[bin]]` configuration and dependencies if needed.
  * `src-tauri/src/lib.rs`: Start IPC listener task in `setup()`.
  * `src/App.tsx`: Mount `useMcpListener()`.
  * `package.json`: Add CLI script shortcut (e.g. `npm run mcp`).
