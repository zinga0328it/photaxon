use crate::security::scopes::Scope;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyState {
    Pending,
    Active,
    Grace,
    Revoked,
    Expired,
}

impl KeyState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Active => "ACTIVE",
            Self::Grace => "GRACE",
            Self::Revoked => "REVOKED",
            Self::Expired => "EXPIRED",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyMetadata {
    pub client_id: String,
    pub key_id: String,
    pub scopes: BTreeSet<Scope>,
    pub state: KeyState,
    pub expires_at_unix: Option<u64>,
}

impl KeyMetadata {
    pub fn new(
        client_id: impl Into<String>,
        key_id: impl Into<String>,
        scopes: impl IntoIterator<Item = Scope>,
        state: KeyState,
        expires_at_unix: Option<u64>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            key_id: key_id.into(),
            scopes: scopes.into_iter().collect(),
            state,
            expires_at_unix,
        }
    }
}
