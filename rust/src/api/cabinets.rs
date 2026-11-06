use super::{AppState, auth, errors};
use crate::{cabinet::model::Cabinet, security::tokens::TokenRole};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn list(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    list_with_role(state, &headers, TokenRole::Api).await
}

pub async fn list_internal(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    list_with_role(state, &headers, TokenRole::Internal).await
}

async fn list_with_role(
    state: Arc<AppState>,
    headers: &HeaderMap,
    role: TokenRole,
) -> (StatusCode, Json<Value>) {
    if !auth::authorized(headers, &state, role) {
        return errors::unauthorized();
    }
    match crate::db::cabinets::list(&state.pool).await {
        Ok(cabinets) => cabinet_response(cabinets),
        Err(_) => errors::unavailable("cabinet_store_unavailable"),
    }
}

fn cabinet_response(cabinets: Vec<Cabinet>) -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(json!({"cabinets": cabinets})))
}
