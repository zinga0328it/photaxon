use crate::telegram::otp::{OtpMaterial, verify_otp as verify_otp_code};
use sqlx::{PgPool, Row};
use std::{error::Error, fmt};
use uuid::Uuid;

const OTP_TTL_SECONDS: i64 = 300;
const SESSION_TTL_SECONDS: i64 = 8 * 60 * 60;
const OTP_RATE_LIMIT_SECONDS: i64 = 30;
const OTP_MAX_ATTEMPTS: i32 = 5;

pub struct OtpDelivery {
    pub session_id: String,
    pub code: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TechnicianSession {
    pub session_id: String,
    pub technician_id: String,
    pub telegram_user_id: String,
    pub telegram_chat_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TechnicianAuthError {
    Denied,
    RateLimited,
    InvalidOtp,
    Expired,
    Locked,
    DatabaseUnavailable,
}

impl fmt::Display for TechnicianAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Denied => "technician is not authorized",
            Self::RateLimited => "OTP request rate limit reached",
            Self::InvalidOtp => "OTP is invalid",
            Self::Expired => "OTP session expired",
            Self::Locked => "OTP session is locked",
            Self::DatabaseUnavailable => "technician authentication store unavailable",
        })
    }
}

impl Error for TechnicianAuthError {}

pub fn valid_telegram_identity(user_id: &str, chat_id: &str) -> bool {
    user_id == chat_id
        && user_id
            .parse::<u64>()
            .is_ok_and(|identity| identity > 0 && identity.to_string() == user_id)
}

pub async fn authenticate_telegram_user(
    pool: &PgPool,
    telegram_user_id: &str,
    telegram_chat_id: &str,
) -> Result<TechnicianSession, TechnicianAuthError> {
    if !valid_telegram_identity(telegram_user_id, telegram_chat_id) {
        return Err(TechnicianAuthError::Denied);
    }
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let technician_status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM technicians WHERE technician_id=$1 FOR UPDATE",
    )
    .bind(telegram_user_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    if technician_status.as_deref() != Some("ACTIVE") {
        return Err(TechnicianAuthError::Denied);
    }
    let existing_session = sqlx::query_scalar::<_, String>(
        "SELECT session_id FROM field_technician_sessions WHERE telegram_user_id=$1 AND telegram_chat_id=$2 AND technician_id=$1 AND status='ACTIVE' AND expires_at>now() ORDER BY created_at DESC LIMIT 1 FOR UPDATE",
    )
    .bind(telegram_user_id)
    .bind(telegram_chat_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let session_id = if let Some(session_id) = existing_session {
        sqlx::query("UPDATE field_technician_sessions SET expires_at=now()+($2*interval '1 second') WHERE session_id=$1")
            .bind(&session_id)
            .bind(SESSION_TTL_SECONDS)
            .execute(&mut *transaction)
            .await
            .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
        session_id
    } else {
        let session_id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO field_technician_sessions (session_id,technician_id,telegram_user_id,telegram_chat_id,status,expires_at) VALUES ($1,$2,$2,$3,'ACTIVE',now()+($4*interval '1 second'))")
            .bind(&session_id)
            .bind(telegram_user_id)
            .bind(telegram_chat_id)
            .bind(SESSION_TTL_SECONDS)
            .execute(&mut *transaction)
            .await
            .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
        session_id
    };
    transaction
        .commit()
        .await
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    Ok(TechnicianSession {
        session_id,
        technician_id: telegram_user_id.to_owned(),
        telegram_user_id: telegram_user_id.to_owned(),
        telegram_chat_id: telegram_chat_id.to_owned(),
    })
}

pub async fn begin_otp(
    pool: &PgPool,
    telegram_user_id: &str,
    telegram_chat_id: &str,
) -> Result<OtpDelivery, TechnicianAuthError> {
    if telegram_user_id.trim().is_empty() || telegram_chat_id.trim().is_empty() {
        return Err(TechnicianAuthError::Denied);
    }
    let technician_id = telegram_user_id.trim();
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let technician_status = sqlx::query_scalar::<_, String>(
        "SELECT status FROM technicians WHERE technician_id=$1 FOR UPDATE",
    )
    .bind(technician_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    if technician_status.as_deref() != Some("ACTIVE") {
        return Err(TechnicianAuthError::Denied);
    }
    let rate_limited = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM field_technician_sessions WHERE technician_id=$1 AND created_at > now() - ($2 * interval '1 second'))",
    )
    .bind(technician_id)
    .bind(OTP_RATE_LIMIT_SECONDS)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    if rate_limited {
        return Err(TechnicianAuthError::RateLimited);
    }

    let session_id = Uuid::new_v4().to_string();
    let challenge_id = Uuid::new_v4().to_string();
    let material = OtpMaterial::generate();
    sqlx::query("INSERT INTO field_technician_sessions (session_id,technician_id,telegram_user_id,telegram_chat_id,status,expires_at) VALUES ($1,$2,$3,$4,'OTP_PENDING',now() + ($5 * interval '1 second'))")
        .bind(&session_id)
        .bind(technician_id)
        .bind(telegram_user_id)
        .bind(telegram_chat_id)
        .bind(OTP_TTL_SECONDS)
        .execute(&mut *transaction)
        .await
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    sqlx::query("INSERT INTO field_otp_challenges (challenge_id,session_id,otp_hash,otp_salt,expires_at) VALUES ($1,$2,$3,$4,now() + ($5 * interval '1 second'))")
        .bind(&challenge_id)
        .bind(&session_id)
        .bind(material.hash())
        .bind(material.salt())
        .bind(OTP_TTL_SECONDS)
        .execute(&mut *transaction)
        .await
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    transaction
        .commit()
        .await
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    Ok(OtpDelivery {
        session_id,
        code: material.code().to_owned(),
    })
}

pub async fn verify_otp(
    pool: &PgPool,
    telegram_user_id: &str,
    telegram_chat_id: &str,
    code: &str,
) -> Result<TechnicianSession, TechnicianAuthError> {
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(TechnicianAuthError::InvalidOtp);
    }
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let row = sqlx::query("SELECT s.session_id,s.technician_id,s.telegram_user_id,s.telegram_chat_id,(s.expires_at <= now()) AS session_expired,c.challenge_id,c.otp_hash,c.otp_salt,(c.expires_at <= now()) AS challenge_expired,c.attempts,(c.consumed_at IS NOT NULL) AS otp_consumed FROM field_technician_sessions s JOIN field_otp_challenges c ON c.session_id=s.session_id WHERE s.telegram_user_id=$1 AND s.telegram_chat_id=$2 AND s.status='OTP_PENDING' ORDER BY s.created_at DESC LIMIT 1 FOR UPDATE OF s,c")
        .bind(telegram_user_id)
        .bind(telegram_chat_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?
        .ok_or(TechnicianAuthError::InvalidOtp)?;

    let session_id: String = row
        .try_get("session_id")
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let technician_id: String = row
        .try_get("technician_id")
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let stored_user_id: String = row
        .try_get("telegram_user_id")
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let stored_chat_id: String = row
        .try_get("telegram_chat_id")
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let challenge_id: String = row
        .try_get("challenge_id")
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let otp_hash: String = row
        .try_get("otp_hash")
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let otp_salt: String = row
        .try_get("otp_salt")
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let attempts: i32 = row
        .try_get("attempts")
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let otp_consumed: bool = row
        .try_get("otp_consumed")
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    if otp_consumed || attempts >= OTP_MAX_ATTEMPTS {
        return Err(TechnicianAuthError::Locked);
    }
    let session_expired: bool = row
        .try_get("session_expired")
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    let challenge_expired: bool = row
        .try_get("challenge_expired")
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    if session_expired || challenge_expired {
        sqlx::query("UPDATE field_technician_sessions SET status='EXPIRED' WHERE session_id=$1")
            .bind(&session_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
        transaction
            .commit()
            .await
            .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
        return Err(TechnicianAuthError::Expired);
    }
    if !verify_otp_code(&otp_salt, code, &otp_hash) {
        let next_attempts = attempts + 1;
        sqlx::query("UPDATE field_otp_challenges SET attempts=$2,consumed_at=CASE WHEN $2 >= $3 THEN now() ELSE consumed_at END WHERE challenge_id=$1")
            .bind(&challenge_id)
            .bind(next_attempts)
            .bind(OTP_MAX_ATTEMPTS)
            .execute(&mut *transaction)
            .await
            .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
        if next_attempts >= OTP_MAX_ATTEMPTS {
            sqlx::query("UPDATE field_technician_sessions SET status='REVOKED',revoked_at=now() WHERE session_id=$1")
                .bind(&session_id)
                .execute(&mut *transaction)
                .await
                .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
            transaction
                .commit()
                .await
                .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
            return Err(TechnicianAuthError::Locked);
        }
        transaction
            .commit()
            .await
            .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
        return Err(TechnicianAuthError::InvalidOtp);
    }

    sqlx::query("UPDATE field_otp_challenges SET consumed_at=now() WHERE challenge_id=$1")
        .bind(&challenge_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    sqlx::query("UPDATE field_technician_sessions SET status='ACTIVE',verified_at=now(),expires_at=now() + ($2 * interval '1 second') WHERE session_id=$1")
        .bind(&session_id)
        .bind(SESSION_TTL_SECONDS)
        .execute(&mut *transaction)
        .await
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    transaction
        .commit()
        .await
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    Ok(TechnicianSession {
        session_id,
        technician_id,
        telegram_user_id: stored_user_id,
        telegram_chat_id: stored_chat_id,
    })
}

pub async fn active_session(
    pool: &PgPool,
    telegram_user_id: &str,
    telegram_chat_id: &str,
) -> Result<Option<TechnicianSession>, TechnicianAuthError> {
    let row = sqlx::query("SELECT s.session_id,s.technician_id,s.telegram_user_id,s.telegram_chat_id FROM field_technician_sessions s JOIN technicians t ON t.technician_id=s.technician_id WHERE s.telegram_user_id=$1 AND s.telegram_chat_id=$2 AND s.status='ACTIVE' AND s.expires_at>now() AND t.status='ACTIVE' ORDER BY s.verified_at DESC LIMIT 1")
        .bind(telegram_user_id)
        .bind(telegram_chat_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?;
    row.map(|row| {
        Ok(TechnicianSession {
            session_id: row
                .try_get("session_id")
                .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?,
            technician_id: row
                .try_get("technician_id")
                .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?,
            telegram_user_id: row
                .try_get("telegram_user_id")
                .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?,
            telegram_chat_id: row
                .try_get("telegram_chat_id")
                .map_err(|_| TechnicianAuthError::DatabaseUnavailable)?,
        })
    })
    .transpose()
}

#[cfg(test)]
mod telegram_identity_tests {
    use super::{TechnicianAuthError, authenticate_telegram_user, valid_telegram_identity};

    #[test]
    fn only_canonical_private_telegram_identity_is_accepted() {
        assert!(valid_telegram_identity("123456", "123456"));
        for (user, chat) in [
            ("0", "0"),
            ("01", "01"),
            ("123", "-100"),
            ("123", "456"),
            ("name", "name"),
        ] {
            assert!(!valid_telegram_identity(user, chat));
        }
    }

    #[tokio::test]
    async fn group_identity_is_rejected_without_database_access() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgresql://localhost/fiberspider_unused")
            .unwrap();
        assert!(matches!(
            authenticate_telegram_user(&pool, "123", "-100").await,
            Err(TechnicianAuthError::Denied)
        ));
    }
}
