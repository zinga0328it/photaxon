use super::scopes::Scope;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientPrincipal {
    pub client_id: String,
    pub key_id: String,
    scopes: BTreeSet<Scope>,
}

impl ClientPrincipal {
    pub fn new(
        client_id: impl Into<String>,
        key_id: impl Into<String>,
        scopes: impl IntoIterator<Item = Scope>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            key_id: key_id.into(),
            scopes: scopes.into_iter().collect(),
        }
    }

    pub fn has_scope(&self, scope: Scope) -> bool {
        self.scopes.contains(&scope)
    }

    pub fn scopes(&self) -> impl Iterator<Item = Scope> + '_ {
        self.scopes.iter().copied()
    }
}
