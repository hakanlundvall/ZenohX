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

#[cfg(test)]
mod tests {
    use super::super::manager::*;
    use super::super::types::*;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    fn test_peer_config() -> SessionConfig {
        let mut config = SessionConfig::default_peer();
        config.scout_multicast = false;
        config.scout_gossip = false;
        config
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_zenoh_query_and_queryable_roundtrip() {
        let manager = SessionManager::new();
        let session_id = manager.connect(test_peer_config()).await.unwrap();
        let q_id = Uuid::new_v4();
        let test_key = format!("roundtrip/rpc/{}", Uuid::new_v4().simple());

        // Declare a mock Queryable
        manager
            .declare_queryable(&session_id, q_id, &test_key, move |query| async move {
                query
                    .reply(&query.key_expr, b"{\"status\": \"ok\"}".to_vec())
                    .await
                    .unwrap();
            })
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Execute Query
        let replies = manager
            .query_get(&session_id, &test_key, "all", 3000)
            .await
            .unwrap();
        assert!(!replies.is_empty());
        assert_eq!(replies[0].payload, b"{\"status\": \"ok\"}");
        assert_eq!(replies[0].is_err, false);

        manager.disconnect(&session_id).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_zenoh_query_with_custom_encoding() {
        let manager = SessionManager::new();
        let session_id = manager.connect(test_peer_config()).await.unwrap();
        let q_id = Uuid::new_v4();
        let key_expr = format!("sensor/telemetry/{}", Uuid::new_v4().simple());

        let reply_key = key_expr.clone();
        manager
            .declare_queryable(&session_id, q_id, &key_expr, move |query| {
                let k = reply_key.clone();
                async move {
                    query
                        .reply_with_encoding(
                            &k,
                            b"temperature=24.5".to_vec(),
                            "text/plain",
                        )
                        .await
                        .unwrap();
                }
            })
            .await
            .unwrap();

        let replies = manager
            .query_get(&session_id, &key_expr, "best_matching", 2000)
            .await
            .unwrap();

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].key_expr, key_expr);
        assert_eq!(replies[0].payload, b"temperature=24.5");
        assert_eq!(replies[0].encoding, "text/plain");
        assert!(!replies[0].is_err);

        manager.disconnect(&session_id).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_zenoh_query_error_reply() {
        let manager = SessionManager::new();
        let session_id = manager.connect(test_peer_config()).await.unwrap();
        let q_id = Uuid::new_v4();
        let key_expr = format!("service/error_test/{}", Uuid::new_v4().simple());

        manager
            .declare_queryable(&session_id, q_id, &key_expr, |query| async move {
                query.reply_err("internal service failure").await.unwrap();
            })
            .await
            .unwrap();

        let replies = manager
            .query_get(&session_id, &key_expr, "all", 2000)
            .await
            .unwrap();

        assert_eq!(replies.len(), 1);
        assert!(replies[0].is_err);
        assert!(
            replies[0]
                .error_message
                .as_ref()
                .unwrap()
                .contains("internal service failure")
        );

        manager.disconnect(&session_id).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_zenoh_query_delete_reply() {
        let manager = SessionManager::new();
        let session_id = manager.connect(test_peer_config()).await.unwrap();
        let q_id = Uuid::new_v4();
        let key_expr = format!("cache/item/{}", Uuid::new_v4().simple());

        let reply_key = key_expr.clone();
        manager
            .declare_queryable(&session_id, q_id, &key_expr, move |query| {
                let k = reply_key.clone();
                async move {
                    query.reply_del(&k).await.unwrap();
                }
            })
            .await
            .unwrap();

        let replies = manager
            .query_get(&session_id, &key_expr, "all", 2000)
            .await
            .unwrap();

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].key_expr, key_expr);
        assert!(!replies[0].is_err);

        manager.disconnect(&session_id).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_zenoh_query_undeclare_queryable() {
        let manager = SessionManager::new();
        let mut config = SessionConfig::default_peer();
        config.scout_multicast = false;
        config.scout_gossip = false;
        let session_id = manager.connect(config).await.unwrap();
        let q_id = Uuid::new_v4();
        let test_key = format!("service/unreg/{}", Uuid::new_v4().simple());

        let reply_key = test_key.clone();
        manager
            .declare_queryable(&session_id, q_id, &test_key, move |query| {
                let k = reply_key.clone();
                async move {
                    query.reply(&k, b"active".to_vec()).await.unwrap();
                }
            })
            .await
            .unwrap();

        let replies = manager
            .query_get(&session_id, &test_key, "all", 1000)
            .await
            .unwrap();
        assert_eq!(replies.len(), 1);

        // Undeclare queryable
        manager.undeclare_queryable(&session_id, q_id).await.unwrap();

        // Allow Zenoh runtime to propagate undeclaration, then verify 0 replies using condition-based waiting
        let mut replies_after = Vec::new();
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            tokio::time::sleep(Duration::from_millis(50)).await;
            replies_after = manager
                .query_get(&session_id, &test_key, "all", 300)
                .await
                .unwrap();
            if replies_after.is_empty() {
                break;
            }
        }
        assert_eq!(
            replies_after.len(),
            0,
            "expected 0 replies after undeclare, got: {:?}",
            replies_after
        );

        manager.disconnect(&session_id).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_zenoh_query_routed_and_reply_token() {
        let manager = SessionManager::new();
        let session_id = manager.connect(test_peer_config()).await.unwrap();
        let q_id = Uuid::new_v4();
        let prefix = format!("ipc_rpc_{}", Uuid::new_v4().simple());
        let wildcard = format!("{prefix}/ipc/rpc/**");
        let query_key = format!("{prefix}/ipc/rpc/calculate");

        let (tx, mut rx) = mpsc::channel::<InboundQuery>(10);

        // Declare routed queryable (emulating Tauri IPC event emission)
        manager
            .declare_queryable_routed(&session_id, q_id, &wildcard, move |inbound| {
                let _ = tx.try_send(inbound);
            })
            .await
            .unwrap();

        // Spawn a background querier task
        let mgr_clone = manager.clone();
        let query_selector = format!("{query_key}?x=10&y=20");
        let query_handle = tokio::spawn(async move {
            mgr_clone
                .query_get(&session_id, &query_selector, "all", 3000)
                .await
        });

        // Wait for inbound query notification
        let inbound = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout waiting for inbound query")
            .expect("channel closed");

        assert_eq!(inbound.key_expr, query_key);
        assert_eq!(inbound.parameters, "x=10&y=20");

        // Reply using the token via SessionManager
        manager
            .reply_query(
                &inbound.token,
                &query_key,
                b"{\"result\": 30}".to_vec(),
                "application/json",
            )
            .await
            .unwrap();

        // Collect query result
        let replies = query_handle.await.unwrap().unwrap();
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].payload, b"{\"result\": 30}");
        assert_eq!(replies[0].encoding, "application/json");
        assert!(!replies[0].is_err);

        manager.disconnect(&session_id).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_zenoh_query_reply_on_wildcard_key_expr_sanitizes() {
        let manager = SessionManager::new();
        let session_id = manager.connect(test_peer_config()).await.unwrap();
        let q_id = Uuid::new_v4();
        let prefix = format!("wildcard_rpc_{}", Uuid::new_v4().simple());
        let wildcard = format!("{prefix}/wildcard/rpc/**");
        let query_key = format!("{prefix}/wildcard/rpc");

        let (tx, mut rx) = mpsc::channel::<InboundQuery>(10);

        // Declare routed queryable on wildcard expression
        manager
            .declare_queryable_routed(&session_id, q_id, &wildcard, move |inbound| {
                let _ = tx.try_send(inbound);
            })
            .await
            .unwrap();

        // Querier sends query on wildcard selector
        let mgr_clone = manager.clone();
        let wildcard_clone = wildcard.clone();
        let query_handle = tokio::spawn(async move {
            mgr_clone
                .query_get(&session_id, &wildcard_clone, "all", 3000)
                .await
        });

        let inbound = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        // Replying with the inbound wildcard key_expr should succeed because it auto-sanitizes
        manager
            .reply_query(
                &inbound.token,
                &inbound.key_expr,
                b"{\"wildcard_reply\": true}".to_vec(),
                "application/json",
            )
            .await
            .unwrap();

        let replies = query_handle.await.unwrap().unwrap();
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].key_expr, query_key);
        assert_eq!(replies[0].payload, b"{\"wildcard_reply\": true}");
        assert!(!replies[0].is_err);

        manager.disconnect(&session_id).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_zenoh_query_multiple_queryables_scatter_gather() {
        let manager = SessionManager::new();
        let session_id = manager.connect(test_peer_config()).await.unwrap();

        let (tx, mut rx) = mpsc::channel::<InboundQuery>(10);
        let prefix = format!("scatter_{}", Uuid::new_v4().simple());

        // Declare 3 queryables on unique subpaths
        let k1 = format!("{prefix}/node1");
        let k2 = format!("{prefix}/node2");
        let k3 = format!("{prefix}/node3");
        let keys = [k1.clone(), k2.clone(), k3.clone()];
        for key in &keys {
            let tx_clone = tx.clone();
            manager
                .declare_queryable_routed(&session_id, Uuid::new_v4(), key, move |inbound| {
                    let _ = tx_clone.try_send(inbound);
                })
                .await
                .unwrap();
        }

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Query with wildcard selector
        let mgr_clone = manager.clone();
        let wildcard_query = format!("{prefix}/**");
        let query_handle = tokio::spawn(async move {
            mgr_clone
                .query_get(&session_id, &wildcard_query, "all", 3000)
                .await
        });

        // Collect and reply to all 3 incoming queries
        for _ in 0..3 {
            let inbound = tokio::time::timeout(Duration::from_secs(2), rx.recv())
                .await
                .expect("timeout waiting for query")
                .expect("channel closed");

            let reply_payload = format!("{{\"node\":\"{}\"}}", inbound.key_expr).into_bytes();
            manager
                .reply_query(
                    &inbound.token,
                    &inbound.key_expr,
                    reply_payload,
                    "application/json",
                )
                .await
                .unwrap();
        }

        let replies = query_handle.await.unwrap().unwrap();
        assert_eq!(replies.len(), 3, "Expected 3 replies from 3 distinct queryables");

        let reply_keys: Vec<String> = replies.iter().map(|r| r.key_expr.clone()).collect();
        assert!(reply_keys.contains(&k1));
        assert!(reply_keys.contains(&k2));
        assert!(reply_keys.contains(&k3));

        manager.disconnect(&session_id).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_zenoh_query_invalid_session_and_tokens() {
        let manager = SessionManager::new();
        let invalid_session_id = Uuid::new_v4();
        let q_id = Uuid::new_v4();

        // Query on invalid session
        let query_res = manager
            .query_get(&invalid_session_id, "test/topic", "all", 500)
            .await;
        assert!(query_res.is_err());

        // Declare queryable on invalid session
        let decl_res = manager
            .declare_queryable(&invalid_session_id, q_id, "test/topic", |_| async {})
            .await;
        assert!(decl_res.is_err());

        // Undeclare queryable on invalid session
        let undecl_res = manager
            .undeclare_queryable(&invalid_session_id, q_id)
            .await;
        assert!(undecl_res.is_err());

        // Reply with unknown token
        let unknown_token = Uuid::new_v4();
        let reply_res = manager
            .reply_query(&unknown_token, "test/topic", vec![], "application/json")
            .await;
        assert!(reply_res.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_undeclare_queryable_prunes_pending_queries() {
        let manager = SessionManager::new();
        let session_id = manager.connect(test_peer_config()).await.unwrap();
        let q_id = Uuid::new_v4();
        let prefix = format!("prune_test_{}", Uuid::new_v4().simple());
        let wildcard = format!("{prefix}/prune/test/**");
        let query_key = format!("{prefix}/prune/test/req");

        let (tx, mut rx) = mpsc::channel::<InboundQuery>(10);

        manager
            .declare_queryable_routed(&session_id, q_id, &wildcard, move |inbound| {
                let _ = tx.try_send(inbound);
            })
            .await
            .unwrap();

        let mgr_clone = manager.clone();
        let query_key_clone = query_key.clone();
        tokio::spawn(async move {
            let _ = mgr_clone
                .query_get(&session_id, &query_key_clone, "all", 1000)
                .await;
        });

        let inbound = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout waiting for inbound query")
            .expect("channel closed");

        // Undeclare the queryable before replying
        manager.undeclare_queryable(&session_id, q_id).await.unwrap();

        // Replying with the token should now fail because pending queries were pruned
        let reply_res = manager
            .reply_query(
                &inbound.token,
                &query_key,
                b"ok".to_vec(),
                "text/plain",
            )
            .await;
        assert!(reply_res.is_err());

        manager.disconnect(&session_id).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_zenoh_query_with_request_payload_and_consolidation() {
        let manager = SessionManager::new();
        let session_id = manager.connect(test_peer_config()).await.unwrap();
        let q_id = Uuid::new_v4();
        let key_expr = format!("rpc/compute/{}", Uuid::new_v4().simple());

        let reply_key = key_expr.clone();
        // Declare a queryable that inspects query payload and parameters
        manager
            .declare_queryable(&session_id, q_id, &key_expr, move |query| {
                let k = reply_key.clone();
                async move {
                    let p = query.payload.as_deref().unwrap_or(b"");
                    let response = format!("processed: {}", String::from_utf8_lossy(p));
                    query
                        .reply(&k, response.into_bytes())
                        .await
                        .unwrap();
                }
            })
            .await
            .unwrap();

        // Execute query with payload, custom encoding, and consolidation
        let request_payload = b"input_data_123".to_vec();
        let replies = manager
            .query_get_advanced(
                &session_id,
                &key_expr,
                "best_matching",
                2000,
                Some(request_payload),
                Some("text/plain".to_string()),
                Some("latest".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].payload, b"processed: input_data_123");
        assert!(!replies[0].is_err);

        manager.disconnect(&session_id).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_zenoh_wildcard_queryable_subpath_query() {
        let manager = SessionManager::new();
        let session_id = manager.connect(test_peer_config()).await.unwrap();
        let q_id = Uuid::new_v4();
        let prefix = format!("subpath_{}", Uuid::new_v4().simple());
        let wildcard = format!("{prefix}/rpc/**");
        let subpath = format!("{prefix}/rpc/a");

        let (tx, mut rx) = mpsc::channel::<InboundQuery>(10);

        // Declare routed queryable on wildcard
        manager
            .declare_queryable_routed(&session_id, q_id, &wildcard, move |inbound| {
                let _ = tx.try_send(inbound);
            })
            .await
            .unwrap();

        // Querier queries specific subpath
        let mgr_clone = manager.clone();
        let subpath_clone = subpath.clone();
        let query_handle = tokio::spawn(async move {
            mgr_clone
                .query_get(&session_id, &subpath_clone, "all", 3000)
                .await
        });

        let inbound = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        assert_eq!(inbound.key_expr, subpath);

        // Reply using the inbound token and key_expr
        let reply_msg = format!("{{\"msg\": \"hello {}\"}}", subpath);
        manager
            .reply_query(
                &inbound.token,
                &inbound.key_expr,
                reply_msg.into_bytes(),
                "application/json",
            )
            .await
            .unwrap();

        let replies = query_handle.await.unwrap().unwrap();
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].key_expr, subpath);
        assert_eq!(replies[0].payload, format!("{{\"msg\": \"hello {}\"}}", subpath).into_bytes());

        manager.disconnect(&session_id).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_zenoh_wildcard_queryable_subpath_query_with_wildcard_reply_key() {
        let manager = SessionManager::new();
        let session_id = manager.connect(test_peer_config()).await.unwrap();
        let q_id = Uuid::new_v4();
        let prefix = format!("subpath_wild_{}", Uuid::new_v4().simple());
        let wildcard = format!("{prefix}/rpc/**");
        let subpath = format!("{prefix}/rpc/a");

        let (tx, mut rx) = mpsc::channel::<InboundQuery>(10);

        manager
            .declare_queryable_routed(&session_id, q_id, &wildcard, move |inbound| {
                let _ = tx.try_send(inbound);
            })
            .await
            .unwrap();

        let mgr_clone = manager.clone();
        let subpath_clone = subpath.clone();
        let query_handle = tokio::spawn(async move {
            mgr_clone
                .query_get(&session_id, &subpath_clone, "all", 3000)
                .await
        });

        let inbound = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout")
            .expect("channel closed");

        // Even if frontend or client passes wildcard (the queryable's declared key),
        // the backend should sanitize using the actual query's key expression so the querier gets the reply
        let reply_msg = format!("{{\"msg\": \"hello {}\"}}", subpath);
        manager
            .reply_query(
                &inbound.token,
                &wildcard,
                reply_msg.into_bytes(),
                "application/json",
            )
            .await
            .unwrap();

        let replies = query_handle.await.unwrap().unwrap();
        assert_eq!(replies.len(), 1, "expected 1 reply for {}", subpath);
        assert_eq!(replies[0].key_expr, subpath);
        assert_eq!(replies[0].payload, format!("{{\"msg\": \"hello {}\"}}", subpath).into_bytes());

        manager.disconnect(&session_id).await.unwrap();
    }
}


