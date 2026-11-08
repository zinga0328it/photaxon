use sqlx::PgPool;
use std::{env, io};
use uuid::Uuid;

pub const CONTROL_PLANE_ADMIN_PRINCIPAL: &str = "CONTROL_PLANE_ADMIN";

#[derive(Clone)]
pub struct InvocationContext {
    pub invocation_id: String,
    pub technician_session: String,
    pub technician_id: String,
}

impl InvocationContext {
    pub async fn from_verified_activation(pool: &PgPool) -> Result<Self, io::Error> {
        let invocation_id = required("FIBERSPIDER_MCP_INVOCATION_ID")?;
        let technician_session = required("FIBERSPIDER_MCP_ACTIVATION_ID")?;
        let technician_id = required("FIBERSPIDER_MCP_TECHNICIAN_ID")?;
        if invocation_id.trim().is_empty()
            || technician_session.trim().is_empty()
            || technician_id.trim().is_empty()
        {
            return Err(io::Error::other("MCP activation context is missing"));
        }
        let authorized = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM activation_requests a JOIN technicians t ON t.technician_id=a.technician_id WHERE a.activation_id=$1 AND a.technician_id=$2 AND a.otp_verified_at IS NOT NULL AND t.status='ACTIVE')",
        )
        .bind(&technician_session)
        .bind(&technician_id)
        .fetch_one(pool)
        .await
        .map_err(|_| io::Error::other("MCP activation authorization lookup failed"))?;
        if !authorized {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "MCP requires an ACTIVE technician with a verified OTP activation",
            ));
        }
        Ok(Self {
            invocation_id,
            technician_session,
            technician_id,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ControlPlaneAdminContext {
    pub invocation_id: String,
    pub principal_id: &'static str,
    writes_authorized: bool,
}

impl ControlPlaneAdminContext {
    pub fn for_private_tunnel() -> Self {
        Self {
            invocation_id: Uuid::new_v4().to_string(),
            principal_id: CONTROL_PLANE_ADMIN_PRINCIPAL,
            writes_authorized: false,
        }
    }

    pub fn from_authorized_tunnel_parent() -> Result<Self, io::Error> {
        let status = std::fs::read_to_string("/proc/self/status")?;
        let parent = status.lines().find_map(|line| {
            line.strip_prefix("PPid:")?.trim().parse::<u32>().ok()
        }).ok_or_else(|| io::Error::other("MCP parent identity unavailable"))?;
        let executable = std::fs::read_link(format!("/proc/{parent}/exe"))?;
        let trusted = std::path::Path::new("/home/ragno/.local/bin/tunnel-client").canonicalize()?;
        let command = std::fs::read(format!("/proc/{parent}/cmdline"))?;
        let arguments: Vec<_> = command.split(|byte| *byte == 0).filter(|argument| !argument.is_empty()).collect();
        if executable != trusted
            || !arguments.iter().any(|argument| *argument == b"run")
            || !arguments.windows(2).any(|pair| pair[0] == b"--profile" && pair[1] == b"fiberspider-ragno")
        {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied,
                "administrative MCP requires the authorized private tunnel parent"));
        }
        Ok(Self {
            invocation_id: Uuid::new_v4().to_string(),
            principal_id: CONTROL_PLANE_ADMIN_PRINCIPAL,
            writes_authorized: true,
        })
    }

    pub fn require_write_authorization(&self) -> Result<(), &'static str> {
        if self.writes_authorized {
            Ok(())
        } else {
            Err(
                "administrative writes require the authorized private tunnel launch",
            )
        }
    }

    #[cfg(test)]
    pub(crate) fn for_authorized_rollback_test() -> Self {
        Self {
            invocation_id: Uuid::new_v4().to_string(),
            principal_id: CONTROL_PLANE_ADMIN_PRINCIPAL,
            writes_authorized: true,
        }
    }
}

fn required(name: &'static str) -> Result<String, io::Error> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| io::Error::other("MCP activation context is missing"))
}

#[cfg(test)]
mod tests {
    use super::required;

    #[test]
    fn missing_context_variable_is_rejected() {
        assert!(required("FIBERSPIDER_MCP_INVOCATION_ID").is_err());
    }

    #[test]
    fn private_tunnel_is_not_an_authenticated_administrator() {
        let actor = super::ControlPlaneAdminContext::for_private_tunnel();
        assert!(actor.require_write_authorization().is_err());
    }
}
