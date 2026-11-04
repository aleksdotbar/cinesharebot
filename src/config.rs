use std::{env, fmt, net::SocketAddr};

use thiserror::Error;

#[derive(Clone)]
pub struct Config {
    pub bot_token: String,
    pub tmdb_key: String,
    pub analytics_chat_id: i64,
    pub webhook_secret: String,
    pub webhook_base_url: Option<String>,
    pub bind_address: SocketAddr,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required environment variable {0}")]
    MissingEnv(&'static str),
    #[error("failed to parse environment variable {name}: {value}")]
    InvalidEnv { name: &'static str, value: String },
    #[error("WEBHOOK_SECRET must contain 1-256 ASCII letters, digits, underscores or hyphens")]
    InvalidWebhookSecret,
    #[error("telegram getMe did not return a username")]
    MissingBotUsername,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let bot_token = required_env("BOT_TOKEN")?;
        let tmdb_key = required_env("TMDB_KEY")?;
        let analytics_chat_id = parse_env_i64("ANALYTICS_CHAT_ID")?;
        let webhook_secret = required_env("WEBHOOK_SECRET")?;
        validate_webhook_secret(&webhook_secret)?;
        let webhook_base_url = optional_env("WEBHOOK_BASE_URL");
        let host = optional_env("HOST").unwrap_or_else(|| "0.0.0.0".to_string());
        let port = parse_optional_env_u16("PORT")?.unwrap_or(3000);
        let bind_address =
            format!("{host}:{port}")
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
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or(ConfigError::MissingEnv(name))
}

fn optional_env(name: &'static str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_env_i64(name: &'static str) -> Result<i64, ConfigError> {
    let value = required_env(name)?;
    value
        .parse::<i64>()
        .map_err(|_| ConfigError::InvalidEnv { name, value })
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

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("bot_token", &"[redacted]")
            .field("tmdb_key", &"[redacted]")
            .field("webhook_secret", &"[redacted]")
            .field("analytics_chat_id", &self.analytics_chat_id)
            .field(
                "webhook_base_url_configured",
                &self.webhook_base_url.is_some(),
            )
            .field("bind_address", &self.bind_address)
            .finish()
    }
}

fn validate_webhook_secret(value: &str) -> Result<(), ConfigError> {
    if (1..=256).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        Ok(())
    } else {
        Err(ConfigError::InvalidWebhookSecret)
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, validate_webhook_secret};

    #[test]
    fn security_webhook_secret_rejects_empty_invalid_and_oversized_values() {
        for (case, (value, valid)) in [
            ("".to_owned(), false),
            (" ".to_owned(), false),
            ("bad\nheader".to_owned(), false),
            ("é".to_owned(), false),
            ("a".repeat(257), false),
            ("a".repeat(256), true),
            ("valid_Secret-123".to_owned(), true),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                validate_webhook_secret(&value).is_ok(),
                valid,
                "secret format case {case} failed"
            );
        }
    }

    #[test]
    fn security_config_debug_does_not_disclose_credentials() {
        let config = Config {
            bot_token: "fixture-bot-secret".into(),
            tmdb_key: "fixture-tmdb-secret".into(),
            webhook_secret: "fixture-webhook-secret".into(),
            analytics_chat_id: -100,
            webhook_base_url: Some("https://fixture-url-secret@example.invalid".into()),
            bind_address: "127.0.0.1:3000".parse().unwrap(),
        };
        let rendered = format!("{config:?}");
        for secret in [
            config.bot_token.as_str(),
            config.tmdb_key.as_str(),
            config.webhook_secret.as_str(),
            "fixture-url-secret",
        ] {
            assert!(!rendered.contains(secret));
        }
    }
}
