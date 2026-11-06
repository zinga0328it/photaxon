use super::{
    key_metadata::{KeyMetadata, KeyState},
    provider::{CredentialInput, CredentialProvider, CredentialProviderError},
};
use crate::security::{principal::ClientPrincipal, scopes::Scope};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    env, io,
    time::{SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq;

struct StoredCredential {
    metadata: KeyMetadata,
    token_hash: [u8; 32],
}

pub struct EnvCredentialProvider {
    credentials: Vec<StoredCredential>,
}

impl EnvCredentialProvider {
    pub fn from_env() -> Result<Self, io::Error> {
        let Some(token) = env::var("FIBERSPIDER_ALEX_READ_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return Self::new(Vec::new());
        };
        let key_id = env::var("FIBERSPIDER_ALEX_READ_KEY_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "alex-web-local".to_owned());
        let expires_at_unix = env::var("FIBERSPIDER_ALEX_READ_EXPIRES_AT")
            .ok()
            .map(|value| {
                value.parse::<u64>().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "ALEX read credential expiration is invalid",
                    )
                })
            })
            .transpose()?;
        let metadata = KeyMetadata::new(
            "alex-web",
            key_id,
            [Scope::Dashboard],
            KeyState::Active,
            expires_at_unix,
        );
        Self::new(vec![CredentialInput::new(metadata, token)])
    }

    pub fn new(inputs: Vec<CredentialInput>) -> Result<Self, io::Error> {
        let mut key_ids = HashSet::new();
        let mut credentials = Vec::with_capacity(inputs.len());
        for input in inputs {
            if input.metadata.client_id.trim().is_empty()
                || input.metadata.key_id.trim().is_empty()
                || input.token.len() < 64
                || is_weak(&input.token)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "client credentials require identifiers and high-entropy tokens",
                ));
            }
            if !key_ids.insert(input.metadata.key_id.clone()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "client credential key IDs must be unique",
                ));
            }
            credentials.push(StoredCredential {
                metadata: input.metadata,
                token_hash: token_hash(&input.token),
            });
        }
        Ok(Self { credentials })
    }
}

impl CredentialProvider for EnvCredentialProvider {
    fn authenticate(
        &self,
        token: &str,
        now: SystemTime,
    ) -> Result<Option<ClientPrincipal>, CredentialProviderError> {
        let presented_hash = token_hash(token);
        let now_unix = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let mut principal = None;
        for credential in &self.credentials {
            let matches = bool::from(credential.token_hash.ct_eq(&presented_hash));
            let not_expired = credential
                .metadata
                .expires_at_unix
                .is_none_or(|expires_at| expires_at > now_unix);
            let active = matches!(
                credential.metadata.state,
                KeyState::Active | KeyState::Grace
            );
            if matches && active && not_expired {
                principal = Some(ClientPrincipal::new(
                    credential.metadata.client_id.clone(),
                    credential.metadata.key_id.clone(),
                    credential.metadata.scopes.iter().copied(),
                ));
            }
        }
        Ok(principal)
    }

    fn get_client_credential(
        &self,
        client_id: &str,
    ) -> Result<Vec<KeyMetadata>, CredentialProviderError> {
        Ok(self
            .credentials
            .iter()
            .filter(|credential| credential.metadata.client_id == client_id)
            .map(|credential| credential.metadata.clone())
            .collect())
    }

    fn get_current_key(
        &self,
        key_id: &str,
        now: SystemTime,
    ) -> Result<Option<KeyMetadata>, CredentialProviderError> {
        let now_unix = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        Ok(self
            .credentials
            .iter()
            .find(|credential| {
                credential.metadata.key_id == key_id
                    && matches!(
                        credential.metadata.state,
                        KeyState::Active | KeyState::Grace
                    )
                    && credential
                        .metadata
                        .expires_at_unix
                        .is_none_or(|expires_at| expires_at > now_unix)
            })
            .map(|credential| credential.metadata.clone()))
    }

    fn get_key_metadata(
        &self,
        key_id: &str,
    ) -> Result<Option<KeyMetadata>, CredentialProviderError> {
        Ok(self
            .credentials
            .iter()
            .find(|credential| credential.metadata.key_id == key_id)
            .map(|credential| credential.metadata.clone()))
    }

    fn is_revoked(&self, key_id: &str) -> Result<bool, CredentialProviderError> {
        Ok(self.credentials.iter().any(|credential| {
            credential.metadata.key_id == key_id && credential.metadata.state == KeyState::Revoked
        }))
    }
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

#[cfg(test)]
mod tests {
    use super::EnvCredentialProvider;
    use crate::{
        credentials::{
            key_metadata::{KeyMetadata, KeyState},
            provider::{CredentialInput, CredentialProvider, CredentialProviderError},
        },
        security::scopes::Scope,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now() -> SystemTime {
        UNIX_EPOCH + std::time::Duration::from_secs(2_000_000_000)
    }

    fn input(
        key_id: &str,
        state: KeyState,
        scope: Scope,
        expiration: Option<u64>,
        token: char,
    ) -> CredentialInput {
        CredentialInput::new(
            KeyMetadata::new("alex-web", key_id, [scope], state, expiration),
            token.to_string().repeat(64),
        )
    }

    #[test]
    fn valid_key_returns_principal_without_secret() {
        let provider = EnvCredentialProvider::new(vec![input(
            "alex-1",
            KeyState::Active,
            Scope::Dashboard,
            None,
            'a',
        )])
        .unwrap();
        let principal = provider
            .authenticate(&"a".repeat(64), now())
            .unwrap()
            .unwrap();
        assert_eq!(principal.client_id, "alex-web");
        assert_eq!(principal.key_id, "alex-1");
        assert!(principal.has_scope(Scope::Dashboard));
    }

    #[test]
    fn invalid_and_revoked_keys_are_denied() {
        let provider = EnvCredentialProvider::new(vec![input(
            "revoked",
            KeyState::Revoked,
            Scope::Dashboard,
            None,
            'r',
        )])
        .unwrap();
        assert!(
            provider
                .authenticate(&"x".repeat(64), now())
                .unwrap()
                .is_none()
        );
        assert!(
            provider
                .authenticate(&"r".repeat(64), now())
                .unwrap()
                .is_none()
        );
        assert!(provider.is_revoked("revoked").unwrap());
    }

    #[test]
    fn expired_pending_keys_are_denied() {
        let provider = EnvCredentialProvider::new(vec![
            input(
                "expired",
                KeyState::Active,
                Scope::Dashboard,
                Some(1_999_999_999),
                'e',
            ),
            input("pending", KeyState::Pending, Scope::Dashboard, None, 'p'),
        ])
        .unwrap();
        assert!(
            provider
                .authenticate(&"e".repeat(64), now())
                .unwrap()
                .is_none()
        );
        assert!(
            provider
                .authenticate(&"p".repeat(64), now())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn grace_key_and_dual_key_rotation_are_supported() {
        let provider = EnvCredentialProvider::new(vec![
            input("old", KeyState::Grace, Scope::Dashboard, None, 'o'),
            input("new", KeyState::Active, Scope::Dashboard, None, 'n'),
        ])
        .unwrap();
        assert_eq!(
            provider
                .authenticate(&"o".repeat(64), now())
                .unwrap()
                .unwrap()
                .key_id,
            "old"
        );
        assert_eq!(
            provider
                .authenticate(&"n".repeat(64), now())
                .unwrap()
                .unwrap()
                .key_id,
            "new"
        );
    }

    #[test]
    fn valid_key_without_dashboard_scope_is_not_authorized() {
        let provider = EnvCredentialProvider::new(vec![input(
            "broker",
            KeyState::Active,
            Scope::Broker,
            None,
            'b',
        )])
        .unwrap();
        let principal = provider
            .authenticate(&"b".repeat(64), now())
            .unwrap()
            .unwrap();
        assert!(!principal.has_scope(Scope::Dashboard));
    }

    struct UnavailableProvider;

    impl CredentialProvider for UnavailableProvider {
        fn authenticate(
            &self,
            _: &str,
            _: SystemTime,
        ) -> Result<Option<crate::security::principal::ClientPrincipal>, CredentialProviderError>
        {
            Err(CredentialProviderError::Unavailable)
        }
        fn get_client_credential(
            &self,
            _: &str,
        ) -> Result<Vec<KeyMetadata>, CredentialProviderError> {
            Err(CredentialProviderError::Unavailable)
        }
        fn get_current_key(
            &self,
            _: &str,
            _: SystemTime,
        ) -> Result<Option<KeyMetadata>, CredentialProviderError> {
            Err(CredentialProviderError::Unavailable)
        }
        fn get_key_metadata(
            &self,
            _: &str,
        ) -> Result<Option<KeyMetadata>, CredentialProviderError> {
            Err(CredentialProviderError::Unavailable)
        }
        fn is_revoked(&self, _: &str) -> Result<bool, CredentialProviderError> {
            Err(CredentialProviderError::Unavailable)
        }
    }

    #[test]
    fn unavailable_provider_fails_closed() {
        let provider = UnavailableProvider;
        assert_eq!(
            provider.authenticate(&"a".repeat(64), now()),
            Err(CredentialProviderError::Unavailable)
        );
    }
}
