use axum::http::{HeaderMap, header};
use sha2::{Digest, Sha256};
use std::io;
use subtle::ConstantTimeEq;

#[derive(Clone, Copy)]
pub enum TokenRole {
    Api,
    Admin,
    Internal,
}

#[derive(Clone)]
pub struct AuthTokens {
    api: [u8; 32],
    admin: [u8; 32],
    internal: [u8; 32],
}

impl AuthTokens {
    pub fn new(api: String, admin: String, internal: String) -> Result<Self, io::Error> {
        let tokens = [&api, &admin, &internal];
        if tokens
            .iter()
            .any(|token| token.len() < 64 || is_weak(token))
        {
            return Err(invalid("API tokens must be high-entropy values"));
        }
        if api == admin || api == internal || admin == internal {
            return Err(invalid("API, admin, and internal tokens must be distinct"));
        }
        Ok(Self {
            api: token_hash(&api),
            admin: token_hash(&admin),
            internal: token_hash(&internal),
        })
    }

    pub fn authorizes(&self, headers: &HeaderMap, role: TokenRole) -> bool {
        let Some(token) = bearer_token(headers) else {
            return false;
        };
        let expected = match role {
            TokenRole::Api => &self.api,
            TokenRole::Admin => &self.admin,
            TokenRole::Internal => &self.internal,
        };
        bool::from(token_hash(token).ct_eq(expected))
    }
}

pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let mut parts = value.split_ascii_whitespace();
    let (Some(scheme), Some(token)) = (parts.next(), parts.next()) else {
        return None;
    };
    if !scheme.eq_ignore_ascii_case("Bearer") || parts.next().is_some() {
        return None;
    }
    Some(token)
}

fn token_hash(token: &str) -> [u8; 32] {
    let digest = Sha256::digest(token.as_bytes());
    let mut hash = [0_u8; 32];
    hash.copy_from_slice(&digest);
    hash
}

fn is_weak(token: &str) -> bool {
    ["admin", "password", "changeme", "secret", "test123"]
        .iter()
        .any(|value| token.eq_ignore_ascii_case(value))
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::{AuthTokens, TokenRole};
    use axum::http::{HeaderMap, HeaderValue, header};

    fn set_bearer(headers: &mut HeaderMap, token: &str) {
        let value = format!("Bearer {token}").parse::<HeaderValue>().unwrap();
        headers.insert(header::AUTHORIZATION, value);
    }

    fn tokens() -> AuthTokens {
        AuthTokens::new("a".repeat(64), "b".repeat(64), "c".repeat(64)).unwrap()
    }

    #[test]
    fn missing_and_wrong_tokens_are_rejected() {
        let tokens = tokens();
        let headers = HeaderMap::new();
        assert!(!tokens.authorizes(&headers, TokenRole::Api));

        let mut headers = HeaderMap::new();
        set_bearer(&mut headers, &"x".repeat(64));
        assert!(!tokens.authorizes(&headers, TokenRole::Api));
    }

    #[test]
    fn api_admin_and_internal_tokens_are_separate() {
        let tokens = tokens();
        let mut headers = HeaderMap::new();

        set_bearer(&mut headers, &"a".repeat(64));
        assert!(tokens.authorizes(&headers, TokenRole::Api));
        assert!(!tokens.authorizes(&headers, TokenRole::Admin));

        set_bearer(&mut headers, &"b".repeat(64));
        assert!(tokens.authorizes(&headers, TokenRole::Admin));
        assert!(!tokens.authorizes(&headers, TokenRole::Api));

        set_bearer(&mut headers, &"c".repeat(64));
        assert!(tokens.authorizes(&headers, TokenRole::Internal));
        assert!(!tokens.authorizes(&headers, TokenRole::Admin));
    }
}
