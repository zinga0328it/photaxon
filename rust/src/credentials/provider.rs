use super::key_metadata::KeyMetadata;
use crate::security::principal::ClientPrincipal;
use std::{error::Error, fmt, time::SystemTime};

pub struct CredentialInput {
    pub metadata: KeyMetadata,
    pub token: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialProviderError {
    Unavailable,
}

impl fmt::Display for CredentialProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable => formatter.write_str("credential provider unavailable"),
        }
    }
}

impl Error for CredentialProviderError {}

pub trait CredentialProvider: Send + Sync {
    fn authenticate(
        &self,
        token: &str,
        now: SystemTime,
    ) -> Result<Option<ClientPrincipal>, CredentialProviderError>;

    fn get_client_credential(
        &self,
        client_id: &str,
    ) -> Result<Vec<KeyMetadata>, CredentialProviderError>;

    fn get_service_credential(
        &self,
        service_id: &str,
    ) -> Result<Vec<KeyMetadata>, CredentialProviderError> {
        self.get_client_credential(service_id)
    }

    fn get_current_key(
        &self,
        key_id: &str,
        now: SystemTime,
    ) -> Result<Option<KeyMetadata>, CredentialProviderError>;

    fn get_key_metadata(
        &self,
        key_id: &str,
    ) -> Result<Option<KeyMetadata>, CredentialProviderError>;

    fn is_revoked(&self, key_id: &str) -> Result<bool, CredentialProviderError>;
}

impl CredentialInput {
    pub fn new(metadata: KeyMetadata, token: String) -> Self {
        Self { metadata, token }
    }
}
