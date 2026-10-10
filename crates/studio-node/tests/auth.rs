//! Autenticação por token do studio-node (T-840-01): `GET /version` aberto;
//! demais rotas exigem `Authorization: Bearer <token>` quando configurado.

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use studio_node::server::{router, NodeState};
    use tower::ServiceExt;

    const TOKEN: &str = "token-de-teste-0123456789abcdef";

    fn app_with_token() -> Router {
        router(NodeState::new(vec![String::from("dds-agent")]).with_token(TOKEN))
    }

    fn app_without_token() -> Router {
        router(NodeState::new(vec![String::from("dds-agent")]))
    }

    async fn status_of(app: Router, path: &str, token: Option<&str>) -> StatusCode {
        let mut builder = Request::get(path);
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        let response = app
            .oneshot(builder.body(Body::empty()).expect("request valido"))
            .await
            .expect("responde");
        response.status()
    }

    #[tokio::test]
    async fn versao_fica_aberta_sem_token() {
        let status = status_of(app_with_token(), "/version", None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn rota_protegida_sem_token_e_401() {
        let status = status_of(app_with_token(), "/operations", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rota_protegida_com_token_errado_e_401() {
        let status = status_of(app_with_token(), "/operations", Some("errado")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rota_protegida_com_token_valido_passa() {
        let status = status_of(app_with_token(), "/operations", Some(TOKEN)).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn sem_token_configurado_tudo_passa_localhost() {
        let status = status_of(app_without_token(), "/operations", None).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn prefixo_diferente_de_bearer_e_401() {
        let app = app_with_token();
        let response = app
            .oneshot(
                Request::get("/operations")
                    .header("authorization", format!("Basic {TOKEN}"))
                    .body(Body::empty())
                    .expect("request valido"),
            )
            .await
            .expect("responde");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
