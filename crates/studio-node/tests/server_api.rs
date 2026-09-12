//! API HTTP do studio-node em ciclo real request/response.

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::response::Response;
    use axum::Router;
    use studio_node::protocol::{
        AdminEnvelope, AdminOp, OperationId, ProtocolVersion, NODE_PROTOCOL_VERSION,
    };
    use studio_node::server::{router, NodeState};
    use tower::ServiceExt;

    fn app() -> Router {
        router(NodeState::new(vec![String::from("dds-agent")]))
    }

    fn running(service: &str) -> AdminOp {
        AdminOp::SetService {
            service: String::from(service),
            running: true,
        }
    }

    fn post_apply(envelope: &AdminEnvelope) -> Request<Body> {
        Request::post("/apply")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(envelope).expect("envelope serializa"),
            ))
            .expect("request valido")
    }

    fn envelope(id: &str, op: AdminOp) -> AdminEnvelope {
        AdminEnvelope {
            protocol: NODE_PROTOCOL_VERSION,
            operation_id: OperationId(String::from(id)),
            op,
        }
    }

    async fn body_json(response: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 64)
            .await
            .expect("corpo deve ser legivel");
        serde_json::from_slice(&bytes).expect("corpo deve ser JSON")
    }

    #[tokio::test]
    async fn version_endpoint_announces_node_protocol() {
        let response = app()
            .oneshot(
                Request::get("/version")
                    .body(Body::empty())
                    .expect("request valida"),
            )
            .await
            .expect("rota existe");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            body_json(response).await,
            serde_json::json!({"major": 1, "minor": 0})
        );
    }

    #[tokio::test]
    async fn repeated_apply_on_same_state_returns_already_applied() {
        let app = app();
        let first = app
            .clone()
            .oneshot(post_apply(&envelope("op-1", running("dds-agent"))))
            .await
            .expect("rota existe");
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(
            body_json(first).await["outcome"],
            serde_json::json!("applied")
        );

        let second = app
            .oneshot(post_apply(&envelope("op-1", running("dds-agent"))))
            .await
            .expect("rota existe");
        assert_eq!(second.status(), StatusCode::OK);
        assert_eq!(
            body_json(second).await["outcome"],
            serde_json::json!("already_applied")
        );
    }

    #[tokio::test]
    async fn conflicting_payload_returns_409_without_overwrite() {
        let app = router(NodeState::new(vec![String::from("dds-agent")]));
        let stopped = AdminOp::SetService {
            service: String::from("dds-agent"),
            running: false,
        };
        let first = app
            .clone()
            .oneshot(post_apply(&envelope("op-2", running("dds-agent"))))
            .await
            .expect("rota existe");
        assert_eq!(first.status(), StatusCode::OK);

        let conflict = app
            .oneshot(post_apply(&envelope("op-2", stopped)))
            .await
            .expect("rota existe");
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        assert_eq!(
            body_json(conflict).await["code"],
            serde_json::json!("operation_id_conflict")
        );
    }

    #[tokio::test]
    async fn foreign_service_returns_403() {
        let response = app()
            .oneshot(post_apply(&envelope("op-3", running("postgres-alheio"))))
            .await
            .expect("rota existe");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            body_json(response).await["code"],
            serde_json::json!("out_of_scope")
        );
    }

    #[tokio::test]
    async fn incompatible_peer_protocol_returns_400() {
        let newer = AdminEnvelope {
            protocol: ProtocolVersion { major: 1, minor: 1 },
            operation_id: OperationId(String::from("op-4")),
            op: running("dds-agent"),
        };
        let response = app()
            .oneshot(post_apply(&newer))
            .await
            .expect("rota existe");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body_json(response).await["code"],
            serde_json::json!("incompatible_protocol")
        );
    }

    #[tokio::test]
    async fn reconcile_finds_applied_and_misses_unknown() {
        let app = app();
        let applied = app
            .clone()
            .oneshot(post_apply(&envelope("op-5", running("dds-agent"))))
            .await
            .expect("rota existe");
        assert_eq!(applied.status(), StatusCode::OK);

        let found = app
            .clone()
            .oneshot(
                Request::get("/operations/op-5")
                    .body(Body::empty())
                    .expect("request valida"),
            )
            .await
            .expect("rota existe");
        assert_eq!(found.status(), StatusCode::OK);

        let missing = app
            .oneshot(
                Request::get("/operations/inexistente")
                    .body(Body::empty())
                    .expect("request valida"),
            )
            .await
            .expect("rota existe");
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            body_json(missing).await["code"],
            serde_json::json!("unknown_operation")
        );
    }

    #[tokio::test]
    async fn apply_persists_log_and_reloads_after_restart() {
        use studio_node::operations::OperationLog;

        let path =
            std::env::temp_dir().join(format!("studio-node-api-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let state = NodeState::with_db(path.clone(), vec![String::from("dds-agent")])
            .expect("primeiro boot cria log vazio");

        let applied = router(state)
            .oneshot(post_apply(&envelope("op-6", running("dds-agent"))))
            .await
            .expect("rota existe");
        assert_eq!(applied.status(), StatusCode::OK);

        let restarted = NodeState::with_db(path.clone(), vec![String::from("dds-agent")])
            .expect("segundo boot recarrega o log");
        let found = router(restarted)
            .oneshot(
                Request::get("/operations/op-6")
                    .body(Body::empty())
                    .expect("request valida"),
            )
            .await
            .expect("rota existe");
        assert_eq!(found.status(), StatusCode::OK);
        let _ = std::fs::remove_file(&path);

        assert!(OperationLog::load(&path).is_err());
    }

    #[tokio::test]
    async fn services_show_wanted_vs_active_without_effects() {
        use studio_node::probe::FakeProbe;
        use studio_node::server::ServiceStatus;

        let probe = FakeProbe::with(&[("s-on", true), ("s-off", false)]);
        let state = NodeState::with_probe(vec![String::from("s-on"), String::from("s-off")], probe);
        let app = router(state);
        let set = |id: &str, service: &str, running: bool| {
            Request::post("/apply")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "protocol": {"major": 1, "minor": 0},
                        "operation_id": id,
                        "op": {"kind": "set_service", "service": service, "running": running},
                    })
                    .to_string(),
                ))
                .expect("request valido")
        };
        for (id, service, running) in [("w-1", "s-on", true), ("w-2", "s-off", true)] {
            let applied = app
                .clone()
                .oneshot(set(id, service, running))
                .await
                .expect("rota existe");
            assert_eq!(applied.status(), StatusCode::OK);
        }

        let response = app
            .oneshot(
                Request::get("/services")
                    .body(Body::empty())
                    .expect("request valida"),
            )
            .await
            .expect("rota existe");
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        let rows: Vec<ServiceStatus> = serde_json::from_value(body).expect("lista de servicos");
        assert_eq!(
            rows,
            vec![
                ServiceStatus {
                    service: String::from("s-off"),
                    wanted: Some(true),
                    active: false,
                },
                ServiceStatus {
                    service: String::from("s-on"),
                    wanted: Some(true),
                    active: true,
                },
            ]
        );
    }
}
