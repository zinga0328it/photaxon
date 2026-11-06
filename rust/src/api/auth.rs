use super::{AppState, errors};
use crate::{
    credentials::provider::CredentialProviderError,
    security::{
        principal::ClientPrincipal,
        scopes::required_scope,
        tokens::{self, TokenRole},
    },
};
use axum::{
    Json,
    http::{HeaderMap, Method, StatusCode},
};
use serde_json::Value;
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientAuthorizationError {
    Unauthorized,
    Forbidden,
    ProviderUnavailable,
}

pub fn authorized(headers: &HeaderMap, state: &AppState, role: TokenRole) -> bool {
    state.tokens.authorizes(headers, role)
}

pub fn authorize_route(
    headers: &HeaderMap,
    state: &AppState,
    method: &Method,
    path: &str,
) -> Result<Option<ClientPrincipal>, ClientAuthorizationError> {
    let required = required_scope(method, path).ok_or(ClientAuthorizationError::Forbidden)?;

    if state.tokens.authorizes(headers, TokenRole::Api) {
        return Ok(None);
    }
    if state.tokens.authorizes(headers, TokenRole::Admin)
        || state.tokens.authorizes(headers, TokenRole::Internal)
    {
        return Err(ClientAuthorizationError::Forbidden);
    }

    let token = tokens::bearer_token(headers).ok_or(ClientAuthorizationError::Unauthorized)?;
    let principal = state
        .client_credentials
        .authenticate(token, SystemTime::now())
        .map_err(|error| match error {
            CredentialProviderError::Unavailable => ClientAuthorizationError::ProviderUnavailable,
        })?
        .ok_or(ClientAuthorizationError::Unauthorized)?;
    if principal.has_scope(required) {
        Ok(Some(principal))
    } else {
        Err(ClientAuthorizationError::Forbidden)
    }
}

pub fn denial(error: ClientAuthorizationError) -> (StatusCode, Json<Value>) {
    match error {
        ClientAuthorizationError::Unauthorized => errors::unauthorized(),
        ClientAuthorizationError::Forbidden => errors::forbidden(),
        ClientAuthorizationError::ProviderUnavailable => {
            errors::unavailable("credential_provider_unavailable")
        }
    }
}
