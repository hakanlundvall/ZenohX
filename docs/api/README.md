# ZenohX MCP API Documentation

This document describes the available Model Context Protocol (MCP) tools and resources for controlling ZenohX.

## Tools

### `zenoh_scout`

Scans the local physical network via multicast for external Zenoh routers, peers, and locators. NOTE: To inspect or interact with nodes inside the ZenohX app, use 'zenoh_get_sessions' or 'zenoh_get_profiles' instead.

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "timeout_ms": {
      "type": "integer",
      "description": "Scouting timeout in milliseconds (default: 1000)",
      "default": 1000
    }
  }
}
```

### `zenoh_connect_session`

Starts/opens a Zenoh session. To start an existing node configured in the app, pass 'profile_id' (retrieve IDs via 'zenoh_get_profiles'). Only pass mode/locators without profile_id if an ad-hoc session is explicitly requested.

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "profile_id": {
      "type": "string",
      "description": "Optional saved profile ID to load connection configuration from"
    },
    "mode": {
      "type": "string",
      "enum": [
        "peer",
        "client",
        "router"
      ],
      "description": "Zenoh session mode (default: client)"
    },
    "connect_locators": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "List of endpoint locators to connect to"
    },
    "listen_locators": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "List of endpoint locators to listen on"
    },
    "username": {
      "type": "string",
      "description": "Optional username for user authentication"
    },
    "password": {
      "type": "string",
      "description": "Optional password for user authentication"
    },
    "token": {
      "type": "string",
      "description": "Optional token for token-based authentication"
    },
    "user_auth": {
      "type": "object",
      "properties": {
        "username": {
          "type": "string"
        },
        "password": {
          "type": "string"
        },
        "token": {
          "type": "string"
        }
      },
      "description": "Optional user authentication object with username/password/token"
    }
  }
}
```

### `zenoh_disconnect_session`

Disconnects an active Zenoh session, or all active sessions if session_id is omitted.

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "session_id": {
      "type": "string",
      "description": "UUID of the session to disconnect. If omitted, disconnects all sessions."
    }
  }
}
```

### `zenoh_get_sessions`

Returns active Zenoh nodes/sessions currently running in the ZenohX app. ALWAYS call this first to discover active node sessions before subscribing, publishing, or creating new sessions.

#### Input Schema

```json
{
  "type": "object",
  "properties": {}
}
```

### `zenoh_get_profiles`

Lists all saved connection profiles (nodes) configured in the ZenohX app (e.g. Local Router, Edge Client). Use this to see configured nodes and retrieve their profile IDs.

#### Input Schema

```json
{
  "type": "object",
  "properties": {}
}
```

### `zenoh_create_profile`

Creates a new connection profile (node) in ZenohX, saves it to SQLite so it appears in the GUI, and optionally connects it immediately.

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "name": {
      "type": "string",
      "description": "Display name of the new node profile (e.g. 'R3', 'Edge Sensor')"
    },
    "mode": {
      "type": "string",
      "enum": [
        "peer",
        "client",
        "router"
      ],
      "description": "Zenoh operation mode"
    },
    "connect_locators": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "List of connect locators (e.g. ['tcp/127.0.0.1:7448'])"
    },
    "listen_locators": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "List of listen locators (e.g. ['tcp/0.0.0.0:7449'])"
    },
    "scout_multicast": {
      "type": "boolean",
      "description": "Enable or disable multicast scouting (default: true)",
      "default": true
    },
    "connect_now": {
      "type": "boolean",
      "description": "If true, immediately opens an active Zenoh session for this new profile after saving",
      "default": false
    },
    "username": {
      "type": "string",
      "description": "Optional username for user authentication"
    },
    "password": {
      "type": "string",
      "description": "Optional password for user authentication"
    },
    "token": {
      "type": "string",
      "description": "Optional token for token-based authentication"
    },
    "user_auth": {
      "type": "object",
      "properties": {
        "username": {
          "type": "string"
        },
        "password": {
          "type": "string"
        },
        "token": {
          "type": "string"
        }
      },
      "description": "Optional user authentication object with username/password/token"
    }
  },
  "required": [
    "name",
    "mode"
  ]
}
```

### `zenoh_edit_profile`

Edits an existing connection profile (node) in ZenohX. You can update its name, connect locators, listen locators, multicast scouting, or user authentication. NOTE: The Zenoh operation mode (peer/client/router) CANNOT be changed.

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "profile_id": {
      "type": "string",
      "description": "ID of the profile to edit"
    },
    "name": {
      "type": "string",
      "description": "New display name for the profile"
    },
    "connect_locators": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "New list of connect locators (e.g. ['tcp/192.168.1.50:7447'])"
    },
    "listen_locators": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "New list of listen locators (e.g. ['tcp/0.0.0.0:7447'])"
    },
    "scout_multicast": {
      "type": "boolean",
      "description": "Enable or disable multicast scouting"
    },
    "username": {
      "type": "string",
      "description": "Optional username for user authentication"
    },
    "password": {
      "type": "string",
      "description": "Optional password for user authentication"
    },
    "token": {
      "type": "string",
      "description": "Optional token for token-based authentication"
    },
    "user_auth": {
      "type": "object",
      "properties": {
        "username": {
          "type": "string"
        },
        "password": {
          "type": "string"
        },
        "token": {
          "type": "string"
        }
      },
      "description": "Optional user authentication object with username/password/token"
    },
    "restart_session": {
      "type": "boolean",
      "description": "If true and the profile currently has an active running session, disconnect and restart the session with the updated profile configuration"
    }
  },
  "required": [
    "profile_id"
  ]
}
```

### `zenoh_publish`

Publishes a data sample to the specified Zenoh key expression. Specify 'session_id' to publish through an existing running node session (from zenoh_get_sessions).

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "key_expr": {
      "type": "string",
      "description": "Zenoh key expression to publish to"
    },
    "payload": {
      "type": "string",
      "description": "Payload data (string or serialized JSON)"
    },
    "encoding": {
      "type": "string",
      "description": "MIME type / encoding (default: text/plain)",
      "default": "text/plain"
    },
    "priority": {
      "type": "string",
      "enum": [
        "real_time",
        "interactive_high",
        "interactive_low",
        "data_high",
        "data",
        "data_low",
        "background"
      ],
      "description": "Zenoh priority QoS"
    },
    "session_id": {
      "type": "string",
      "description": "Optional session UUID to use for publishing"
    }
  },
  "required": [
    "key_expr",
    "payload"
  ]
}
```

### `zenoh_subscribe`

Declares a subscriber on a key expression to capture incoming samples. Specify 'session_id' to attach the subscriber to a specific running node session (from zenoh_get_sessions).

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "key_expr": {
      "type": "string",
      "description": "Zenoh key expression to subscribe to"
    },
    "session_id": {
      "type": "string",
      "description": "Optional session UUID to subscribe on"
    }
  },
  "required": [
    "key_expr"
  ]
}
```

### `zenoh_unsubscribe`

Cancels an active Zenoh subscription by subscription_id.

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "subscription_id": {
      "type": "string",
      "description": "Subscription UUID to unsubscribe"
    },
    "session_id": {
      "type": "string",
      "description": "Optional session UUID"
    }
  },
  "required": [
    "subscription_id"
  ]
}
```

### `zenoh_get_messages`

Reads captured message buffers or SQLite message history.

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "key_expr": {
      "type": "string",
      "description": "Optional key expression filter"
    },
    "limit": {
      "type": "integer",
      "description": "Maximum number of messages to return (default: 50)",
      "default": 50
    },
    "profile_id": {
      "type": "string",
      "description": "Optional profile ID filter"
    }
  }
}
```

### `zenoh_query`

Issues a Zenoh GET query and collects all replies.

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "key_expr": {
      "type": "string",
      "description": "Zenoh key expression or selector to query"
    },
    "target": {
      "type": "string",
      "enum": [
        "all",
        "best_matching",
        "complete"
      ],
      "description": "Query target routing policy (default: all)",
      "default": "all"
    },
    "timeout_ms": {
      "type": "integer",
      "description": "Query timeout in milliseconds (default: 5000)",
      "default": 5000
    },
    "payload": {
      "type": "string",
      "description": "Optional query predicate payload"
    },
    "session_id": {
      "type": "string",
      "description": "Optional session UUID to use"
    }
  },
  "required": [
    "key_expr"
  ]
}
```

### `zenoh_declare_queryable`

Registers a queryable endpoint that automatically returns a predefined response or executes dynamic JavaScript script responses.

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "key_expr": {
      "type": "string",
      "description": "Zenoh key expression for the queryable"
    },
    "reply_payload": {
      "type": "string",
      "description": "Static payload string returned in replies (optional if script_code is provided)"
    },
    "script_code": {
      "type": "string",
      "description": "JavaScript code to dynamically compute query replies. Receives 'query' with { keyExpr, params, payload, timestamp }. Return a JSON object/string or explicit { payload, encoding, keyExpr }."
    },
    "encoding": {
      "type": "string",
      "description": "Reply encoding (default: text/plain)",
      "default": "text/plain"
    },
    "session_id": {
      "type": "string",
      "description": "Optional session UUID to register on"
    }
  },
  "required": [
    "key_expr"
  ]
}
```

### `zenoh_inspect_topology`

Queries admin space (@/admin/**) to discover routers, peers, and links.

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "session_id": {
      "type": "string",
      "description": "Optional session UUID"
    },
    "max_depth": {
      "type": "integer",
      "description": "Maximum recursion depth (default: 3)",
      "default": 3
    },
    "timeout_ms": {
      "type": "integer",
      "description": "Timeout in milliseconds (default: 2000)",
      "default": 2000
    }
  }
}
```

### `zenohx_gui_switch_workspace`

Switches the active workspace tab in the ZenohX desktop interface.

#### Input Schema

```json
{
  "type": "object",
  "properties": {
    "workspace": {
      "type": "string",
      "enum": [
        "pubsub",
        "query",
        "traffic",
        "topology",
        "settings"
      ],
      "description": "Target workspace tab"
    }
  },
  "required": [
    "workspace"
  ]
}
```

### `zenohx_gui_get_state`

Retrieves GUI state: current tab, active profile, and connection status.

#### Input Schema

```json
{
  "type": "object",
  "properties": {}
}
```

## Resources

### `zenohx://sessions`

**Name:** Active Zenoh Sessions
**Description:** JSON summary of active Zenoh sessions and status
**MIME Type:** `application/json`

### `zenohx://profiles`

**Name:** Connection Profiles
**Description:** List of saved connection profiles from SQLite
**MIME Type:** `application/json`

### `zenohx://messages/recent`

**Name:** Recent Messages
**Description:** Snapshot of recently published and received messages
**MIME Type:** `application/json`

### `zenohx://topology`

**Name:** Discovered Topology
**Description:** Discovered topology nodes, routers, and connectivity graph
**MIME Type:** `application/json`
