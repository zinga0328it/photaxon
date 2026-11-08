use super::DatabaseConfig;
use crate::security::secrets;
use std::{env, error::Error, net::Ipv6Addr, path::PathBuf};

pub struct TelegramRuntimeConfig {
    pub database: DatabaseConfig,
    pub bot_token: String,
}

pub fn load() -> Result<TelegramRuntimeConfig, Box<dyn Error>> {
    let runtime_file = env::var_os("FIBERSPIDER_ENV_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("secrets/runtime.env"));
    let database = DatabaseConfig {
        host: secrets::required_from_file(&runtime_file, "FIBERSPIDER_DB_HOST")?
            .parse::<Ipv6Addr>()?,
        port: secrets::required_from_file(&runtime_file, "FIBERSPIDER_DB_PORT")?.parse::<u16>()?,
        name: secrets::required_from_file(&runtime_file, "FIBERSPIDER_DB_NAME")?,
        user: secrets::required_from_file(&runtime_file, "FIBERSPIDER_DB_USER")?,
        password: secrets::required_from_file(&runtime_file, "FIBERSPIDER_DB_PASSWORD")?,
    };
    let telegram_file = env::var_os("FIBERSPIDER_TELEGRAM_ENV_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("secrets/telegram-bot.env"));
    let bot_token = secrets::required_from_file(&telegram_file, "TELEGRAM_BOT_TOKEN")?;
    Ok(TelegramRuntimeConfig {
        database,
        bot_token,
    })
}
