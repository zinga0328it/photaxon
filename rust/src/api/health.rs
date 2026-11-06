use super::{AppState, auth, errors};
use crate::security::tokens::TokenRole;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, Method, StatusCode},
};
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn api(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    if !auth::authorized(&headers, &state, TokenRole::Api) {
        return errors::unauthorized();
    }
    database_health(&state).await
}

pub async fn dashboard_api(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    if let Err(error) = auth::authorize_route(&headers, &state, &Method::GET, "/api/health") {
        return auth::denial(error);
    }
    database_health(&state).await
}

async fn database_health(state: &AppState) -> (StatusCode, Json<Value>) {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))),
        Err(_) => errors::unavailable("database_unavailable"),
    }
}

pub async fn admin_database(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    if !auth::authorized(&headers, &state, TokenRole::Admin) {
        return errors::unauthorized();
    }
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({"database": "ok"}))),
        Err(_) => errors::unavailable("database_unavailable"),
    }
}
