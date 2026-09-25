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

//! Web access mode: serves the ZenohX UI to regular browsers.
//!
//! Started with `zenohx --web`. The embedded frontend is served over HTTP and a
//! WebSocket at `/ws` carries what the Tauri IPC normally does: `invoke` calls
//! (see `dispatch`) and backend events. The browser side lives in
//! `src/lib/webBridge.ts`. Access requires a token, passed as `?token=` in the
//! printed URL.

pub mod dispatch;

use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tauri::{AppHandle, Listener};
use tokio::sync::{broadcast, mpsc};

pub const DEFAULT_WEB_ADDR: &str = "127.0.0.1:7880";

/// Backend events forwarded to web clients (everything the backend emits).
const FORWARDED_EVENTS: &[&str] = &[
    "zenohx://samples-batched",
    "zenohx://query",
    "zenohx://session-status",
    "zenohx://sessions",
    "zenohx://profiles",
    "zenohx://profile-updated",
    "zenohx://topology",
    "zenohx://subscription-added",
    "zenohx://subscription-removed",
    "zenohx://queryable-added",
    "zenohx://mcp-action",
    "zenohx://gui-switch-tab",
];

/// WebSocket close code for a missing or wrong token (mirrored in webBridge.ts).
const CLOSE_UNAUTHORIZED: u16 = 4401;

#[derive(Debug, Clone, PartialEq)]
pub struct WebConfig {
    pub addr: SocketAddr,
    pub token: String,
    /// Keep the desktop window visible as well.
    pub keep_window: bool,
}

impl WebConfig {
    /// Web mode is enabled by `--web` or `ZENOHX_WEB=1`.
    ///
    /// Options: `--web-addr <host:port>` / `ZENOHX_WEB_ADDR` (default 127.0.0.1:7880),
    /// `--web-token <token>` / `ZENOHX_WEB_TOKEN` (default: random per run),
    /// `--web-keep-window` to also show the desktop window.
    pub fn from_args_and_env(
        args: &[String],
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<Option<WebConfig>, String> {
        let flag_value = |name: &str| -> Option<String> {
            let prefix = format!("{name}=");
            args.iter().enumerate().find_map(|(i, a)| {
                if a == name {
                    args.get(i + 1).cloned()
                } else {
                    a.strip_prefix(&prefix).map(str::to_string)
                }
            })
        };
        let has_flag = |name: &str| args.iter().any(|a| a == name || a.starts_with(&format!("{name}=")));
        let env_on = |name: &str| env(name).is_some_and(|v| matches!(v.trim(), "1" | "true" | "yes" | "on"));

        let enabled = has_flag("--web")
            || has_flag("--web-addr")
            || has_flag("--web-token")
            || env_on("ZENOHX_WEB");
        if !enabled {
            return Ok(None);
        }

        let addr_str = flag_value("--web-addr")
            .or_else(|| env("ZENOHX_WEB_ADDR"))
            .unwrap_or_else(|| DEFAULT_WEB_ADDR.to_string());
        let addr: SocketAddr = addr_str
            .parse()
            .map_err(|e| format!("invalid web address '{addr_str}' (expected host:port): {e}"))?;

        let token = flag_value("--web-token")
            .or_else(|| env("ZENOHX_WEB_TOKEN"))
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());

        Ok(Some(WebConfig {
            addr,
            token,
            keep_window: has_flag("--web-keep-window") || env_on("ZENOHX_WEB_KEEP_WINDOW"),
        }))
    }
}

#[derive(Clone)]
struct WebState {
    app: AppHandle,
    token: Arc<String>,
    events: broadcast::Sender<String>,
    /// In `tauri dev` the frontend is not embedded; pages are served by the dev server.
    dev_url: Option<String>,
}

#[derive(Deserialize)]
struct Request {
    id: u64,
    cmd: String,
    #[serde(default)]
    args: Value,
}

/// Starts the web server and returns the URL to open (including the token).
pub fn start(app: &AppHandle, config: &WebConfig) -> Result<String, String> {
    let listener = std::net::TcpListener::bind(config.addr)
        .map_err(|e| format!("cannot listen on {}: {e}", config.addr))?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let bound = listener.local_addr().map_err(|e| e.to_string())?;

    let (events, _) = broadcast::channel::<String>(1024);
    for &name in FORWARDED_EVENTS {
        let tx = events.clone();
        app.listen_any(name, move |event| {
            if tx.receiver_count() == 0 {
                return;
            }
            let payload = match event.payload() {
                "" => "null",
                p => p,
            };
            let _ = tx.send(format!(
                r#"{{"type":"event","event":{},"payload":{}}}"#,
                Value::String(name.to_string()),
                payload
            ));
        });
    }

    let dev_url = if tauri::is_dev() {
        app.config().build.dev_url.as_ref().map(|u| u.to_string())
    } else {
        None
    };

    let host = match bound.ip() {
        ip if ip.is_unspecified() => "localhost".to_string(),
        std::net::IpAddr::V6(ip) => format!("[{ip}]"),
        ip => ip.to_string(),
    };
    let url = match &dev_url {
        Some(dev) => format!("{}?ws={host}:{}&token={}", dev, bound.port(), config.token),
        None => format!("http://{host}:{}/?token={}", bound.port(), config.token),
    };

    let state = WebState {
        app: app.clone(),
        token: Arc::new(config.token.clone()),
        events,
        dev_url,
    };
    let router = Router::new()
        .route("/ws", get(ws_handler))
        .fallback(get(asset_handler))
        .with_state(state);

    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[web] listener error: {e}");
                return;
            }
        };
        if let Err(e) = axum::serve(listener, router).await {
            eprintln!("[web] server stopped: {e}");
        }
    });

    Ok(url)
}

/// Startup message with the address to open, made to stand out in busy terminals.
pub fn startup_banner(url: &str, url_file: &std::path::Path) -> String {
    let rule = "=".repeat(72);
    format!(
        "\n{rule}\n ZenohX web access is on. Open this address in a browser:\n\n   {url}\n\n (also saved to {})\n{rule}\n",
        url_file.display()
    )
}

/// Dev-server URL for a request to this server, with `ws=<host>` added unless present.
fn dev_redirect_target(dev_url: &str, uri: &Uri, host: Option<&str>) -> String {
    let query = uri.query().unwrap_or("");
    let has_ws = query.split('&').any(|p| p == "ws" || p.starts_with("ws="));
    let mut params: Vec<String> = query.split('&').filter(|p| !p.is_empty()).map(str::to_string).collect();
    if let (false, Some(host)) = (has_ws, host) {
        params.push(format!("ws={host}"));
    }
    let query = if params.is_empty() { String::new() } else { format!("?{}", params.join("&")) };
    format!("{}{}{}", dev_url.trim_end_matches('/'), uri.path(), query)
}

/// Compares tokens without leaking the position of the first mismatch.
fn token_matches(given: &str, expected: &str) -> bool {
    given.len() == expected.len()
        && given
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

async fn ws_handler(
    State(state): State<WebState>,
    Query(query): Query<HashMap<String, String>>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let authorized = query
        .get("token")
        .is_some_and(|t| token_matches(t, &state.token));
    upgrade.on_upgrade(move |mut socket| async move {
        if authorized {
            serve_socket(socket, state).await;
        } else {
            let _ = socket
                .send(Message::Close(Some(CloseFrame {
                    code: CLOSE_UNAUTHORIZED,
                    reason: "invalid or missing token".into(),
                })))
                .await;
        }
    })
}

async fn serve_socket(socket: WebSocket, state: WebState) {
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    let mut events = state.events.subscribe();

    let writer = tokio::spawn(async move {
        loop {
            let msg = tokio::select! {
                msg = out_rx.recv() => match msg {
                    Some(m) => m,
                    None => break,
                },
                ev = events.recv() => match ev {
                    Ok(m) => m,
                    // A slow client misses some events rather than stalling everyone.
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        eprintln!("[web] client lagging, dropped {n} events");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
            };
            if sink.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = stream.next().await {
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };
        let request: Request = match serde_json::from_str(&text) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[web] malformed request: {e}");
                continue;
            }
        };
        let app = state.app.clone();
        let out = out_tx.clone();
        // Commands run concurrently, like Tauri's async commands.
        tokio::spawn(async move {
            let response = match dispatch::dispatch(&app, &request.cmd, request.args).await {
                Ok(result) => json!({ "type": "response", "id": request.id, "ok": true, "result": result }),
                Err(error) => json!({ "type": "response", "id": request.id, "ok": false, "error": error }),
            };
            let _ = out.send(response.to_string());
        });
    }

    drop(out_tx);
    writer.abort();
}

async fn asset_handler(State(state): State<WebState>, headers: HeaderMap, uri: Uri) -> Response {
    if let Some(dev) = &state.dev_url {
        // `tauri dev`: the dev server has the pages. Keep the query (token) and
        // tell the page where this server is, so it can open the WebSocket here.
        let host = headers.get(header::HOST).and_then(|h| h.to_str().ok());
        return Redirect::temporary(&dev_redirect_target(dev, &uri, host)).into_response();
    }

    let path = uri.path().trim_start_matches('/');
    let resolver = state.app.asset_resolver();
    let asset = if path.is_empty() {
        None
    } else {
        resolver.get(path.to_string())
    };
    // Unknown paths get the app shell (client-side routing).
    let (asset, is_index) = match asset {
        Some(a) => (a, path == "index.html"),
        None => match resolver.get("index.html".to_string()) {
            Some(a) => (a, true),
            None => return (StatusCode::NOT_FOUND, "frontend assets not found").into_response(),
        },
    };

    if is_index {
        // Marks the page as served by ZenohX so it installs the WebSocket bridge.
        let html = String::from_utf8_lossy(&asset.bytes).replacen(
            "<head>",
            "<head><script>window.__ZENOHX_WEB__=true</script>",
            1,
        );
        return (
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8".to_string()),
                (header::CACHE_CONTROL, "no-store".to_string()),
            ],
            html,
        )
            .into_response();
    }

    ([(header::CONTENT_TYPE, asset.mime_type.clone())], asset.bytes.clone()).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(args: &[&str], env: &[(&str, &str)]) -> Result<Option<WebConfig>, String> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let env: HashMap<String, String> = env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        WebConfig::from_args_and_env(&args, |k| env.get(k).cloned())
    }

    #[test]
    fn disabled_without_flag_or_env() {
        assert_eq!(cfg(&["zenohx"], &[]).unwrap(), None);
        assert_eq!(cfg(&["zenohx"], &[("ZENOHX_WEB", "0")]).unwrap(), None);
    }

    #[test]
    fn defaults_to_localhost_with_random_token() {
        let a = cfg(&["zenohx", "--web"], &[]).unwrap().unwrap();
        let b = cfg(&["zenohx", "--web"], &[]).unwrap().unwrap();
        assert_eq!(a.addr, DEFAULT_WEB_ADDR.parse().unwrap());
        assert_eq!(a.token.len(), 32);
        assert_ne!(a.token, b.token);
        assert!(!a.keep_window);
    }

    #[test]
    fn reads_flags_in_both_forms_and_env() {
        let c = cfg(&["zenohx", "--web-addr", "0.0.0.0:9000", "--web-token=abc", "--web-keep-window"], &[])
            .unwrap()
            .unwrap();
        assert_eq!(c.addr, "0.0.0.0:9000".parse().unwrap());
        assert_eq!(c.token, "abc");
        assert!(c.keep_window);

        let e = cfg(
            &["zenohx"],
            &[("ZENOHX_WEB", "1"), ("ZENOHX_WEB_ADDR", "127.0.0.1:1234"), ("ZENOHX_WEB_TOKEN", "xyz")],
        )
        .unwrap()
        .unwrap();
        assert_eq!(e.addr, "127.0.0.1:1234".parse().unwrap());
        assert_eq!(e.token, "xyz");
    }

    #[test]
    fn rejects_bad_address() {
        assert!(cfg(&["zenohx", "--web-addr", "nope"], &[]).is_err());
    }

    #[test]
    fn dev_redirect_points_the_page_back_at_this_server() {
        let uri: Uri = "/?token=abc".parse().unwrap();
        assert_eq!(
            dev_redirect_target("http://localhost:1420/", &uri, Some("127.0.0.1:7880")),
            "http://localhost:1420/?token=abc&ws=127.0.0.1:7880"
        );
        let uri: Uri = "/?ws=h:1&token=abc".parse().unwrap();
        assert_eq!(
            dev_redirect_target("http://localhost:1420/", &uri, Some("127.0.0.1:7880")),
            "http://localhost:1420/?ws=h:1&token=abc"
        );
        let uri: Uri = "/".parse().unwrap();
        assert_eq!(
            dev_redirect_target("http://localhost:1420", &uri, Some("localhost:7880")),
            "http://localhost:1420/?ws=localhost:7880"
        );
    }

    #[test]
    fn token_comparison() {
        assert!(token_matches("abc", "abc"));
        assert!(!token_matches("abd", "abc"));
        assert!(!token_matches("ab", "abc"));
    }
}
