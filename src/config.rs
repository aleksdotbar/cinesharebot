use std::{env, net::SocketAddr};

use thiserror::Error;

#[derive(Clone, Debug)]
pub struct Config {
    pub bot_token: String,
    pub tmdb_key: String,
    pub analytics_chat_id: i64,
    pub webhook_secret: Option<String>,
    pub webhook_base_url: Option<String>,
    pub bind_address: SocketAddr,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required environment variable {0}")]
    MissingEnv(&'static str),
    #[error("failed to parse environment variable {name}: {value}")]
    InvalidEnv { name: &'static str, value: String },
    #[error("telegram getMe did not return a username")]
    MissingBotUsername,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bot_token = required_env("BOT_TOKEN")?;
        let tmdb_key = required_env("TMDB_KEY")?;
        let analytics_chat_id = parse_env_i64("ANALYTICS_CHAT_ID")?;
        let webhook_secret = optional_env("WEBHOOK_SECRET");
        let webhook_base_url = optional_env("WEBHOOK_BASE_URL");
        let host = optional_env("HOST").unwrap_or_else(|| "0.0.0.0".to_string());
        let port = parse_optional_env_u16("PORT")?.unwrap_or(3000);
        let bind_address = format!("{host}:{port}")
            .parse()
            .map_err(|_| ConfigError::InvalidEnv {
                name: "HOST/PORT",
                value: format!("{host}:{port}"),
            })?;

        Ok(Self {
            bot_token,
            tmdb_key,
            analytics_chat_id,
            webhook_secret,
            webhook_base_url,
            bind_address,
        })
    }
}

fn required_env(name: &'static str) -> Result<String, ConfigError> {
    env::var(name).map_err(|_| ConfigError::MissingEnv(name))
}

fn optional_env(name: &'static str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_env_i64(name: &'static str) -> Result<i64, ConfigError> {
    let value = required_env(name)?;
    value.parse::<i64>().map_err(|_| ConfigError::InvalidEnv {
        name,
        value,
    })
}

fn parse_optional_env_u16(name: &'static str) -> Result<Option<u16>, ConfigError> {
    match optional_env(name) {
        Some(value) => value
            .parse::<u16>()
            .map(Some)
            .map_err(|_| ConfigError::InvalidEnv { name, value }),
        None => Ok(None),
    }
}
