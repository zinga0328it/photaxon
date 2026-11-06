use crate::config::DatabaseConfig;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::io;

pub async fn connect(config: &DatabaseConfig) -> Result<PgPool, io::Error> {
    let host = config.host.to_string();
    let options = sqlx::postgres::PgConnectOptions::new()
        .host(&host)
        .port(config.port)
        .username(&config.user)
        .password(&config.password)
        .database(&config.name);
    PgPoolOptions::new()
        .max_connections(3)
        .connect_with(options)
        .await
        .map_err(|_| io::Error::other("PostgreSQL connection failed"))
}
