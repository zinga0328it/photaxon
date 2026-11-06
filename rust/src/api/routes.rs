use super::{AppState, cabinets, health, lab};
use axum::{Router, routing::get};
use std::sync::Arc;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health::api))
        .route("/api/health", get(health::dashboard_api))
        .route("/api/v1/cabinets", get(cabinets::list))
        .route("/api/v1/admin/database", get(health::admin_database))
        .route("/api/v1/internal/cabinets", get(cabinets::list_internal))
        .route("/api/lab/openstack", get(lab::openstack))
        .route("/api/lab/cabinets", get(lab::cabinets))
        .route("/api/lab/roes", get(lab::roes))
        .route("/api/lab/topology", get(lab::topology))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::router;
    use crate::{
        api::AppState,
        credentials::{
            env_provider::EnvCredentialProvider,
            key_metadata::{KeyMetadata, KeyState},
            provider::CredentialInput,
        },
        security::{scopes::Scope, tokens::AuthTokens},
    };
    use axum::{
        Router,
        body::Body,
        http::{HeaderValue, Method, Request, StatusCode},
    };
    use sqlx::postgres::PgPoolOptions;
    use std::sync::Arc;
    use tower::ServiceExt;

    const ALEX_TOKEN: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

    fn app() -> Router {
        let tokens = AuthTokens::new("a".repeat(64), "b".repeat(64), "c".repeat(64)).unwrap();
        let credentials = EnvCredentialProvider::new(vec![CredentialInput::new(
            KeyMetadata::new(
                "alex-web",
                "alex-web-test-key",
                [Scope::Dashboard],
                KeyState::Active,
                None,
            ),
            ALEX_TOKEN.to_owned(),
        )])
        .unwrap();
        let pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(20))
            .connect_lazy("postgres://localhost/fiberspider_test")
            .unwrap();
        router(AppState::new(pool, tokens, None, Arc::new(credentials)))
    }

    fn request(method: Method, path: &str, token: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            builder = builder.header(
                axum::http::header::AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
            );
        }
        builder.body(Body::empty()).unwrap()
    }

    #[tokio::test]
    async fn alex_dashboard_gets_pass_authentication() {
        let app = app();
        for path in [
            "/api/health",
            "/api/lab/openstack",
            "/api/lab/cabinets",
            "/api/lab/roes",
            "/api/lab/topology",
        ] {
            let response = app
                .clone()
                .oneshot(request(Method::GET, path, Some(ALEX_TOKEN)))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{path}");
        }
    }

    #[tokio::test]
    async fn alex_token_is_denied_for_missing_admin_and_legacy_scopes() {
        let app = app();
        for path in ["/api/health", "/api/lab/cabinets"] {
            let response = app
                .clone()
                .oneshot(request(Method::GET, path, None))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");

            let response = app
                .clone()
                .oneshot(request(Method::GET, path, Some(&"x".repeat(64))))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        }
        for path in [
            "/api/v1/admin/database",
            "/api/v1/internal/cabinets",
            "/api/v1/cabinets",
        ] {
            let response = app
                .clone()
                .oneshot(request(Method::GET, path, Some(ALEX_TOKEN)))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{path}");
        }
        let response = app
            .clone()
            .oneshot(request(Method::GET, "/api/health", Some(&"b".repeat(64))))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn dashboard_token_cannot_use_write_or_secret_routes() {
        let app = app();
        for path in [
            "/api/health",
            "/api/lab/openstack",
            "/api/lab/cabinets",
            "/api/lab/roes",
            "/api/lab/topology",
        ] {
            for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
                let response = app
                    .clone()
                    .oneshot(request(method.clone(), path, Some(ALEX_TOKEN)))
                    .await
                    .unwrap();
                assert_eq!(
                    response.status(),
                    StatusCode::METHOD_NOT_ALLOWED,
                    "{method} {path}"
                );
            }
        }
        let response = app
            .clone()
            .oneshot(request(Method::POST, "/mcp/tools/write", Some(ALEX_TOKEN)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response = app
            .oneshot(request(Method::GET, "/api/secrets", Some(ALEX_TOKEN)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
