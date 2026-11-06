use sqlx::PgPool;
use std::{env, io};

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
}
