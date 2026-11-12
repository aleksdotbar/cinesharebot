use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::{info, warn};

use cinesharebot::{
    app::AppService,
    config::{Config, ConfigError},
    telegram::{SetWebhookRequest, TelegramClient},
    tmdb::TmdbClient,
    web::{AppState, WEBHOOK_PATH, build_router},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Container healthcheck: the image has no shell or HTTP client.
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        std::process::exit(if healthcheck() { 0 } else { 1 });
    }

    init_tracing();

    let config = Config::from_env()?;
    let telegram_client = Arc::new(TelegramClient::new(config.bot_token.clone()));
    let tmdb_client = Arc::new(TmdbClient::new(config.tmdb_key.clone()));

    let me = telegram_client.get_me().await?;
    let bot_id = me.id;
    let bot_username = me
        .username
        .ok_or(ConfigError::MissingBotUsername)?
        .to_string();

    if let Some(webhook_base_url) = &config.webhook_base_url {
        let request = SetWebhookRequest {
            url: format!("{}{}", webhook_base_url.trim_end_matches('/'), WEBHOOK_PATH),
            secret_token: Some(config.webhook_secret.clone()),
            allowed_updates: Some(vec![
                "message".to_string(),
                "inline_query".to_string(),
                "chosen_inline_result".to_string(),
            ]),
            drop_pending_updates: Some(false),
        };

        // The public hostname may not resolve until the platform routes this
        // instance, and Telegram rejects unresolvable webhooks; keep serving.
        tokio::spawn(register_webhook(telegram_client.clone(), request));
    }

    let service = AppService::new(
        bot_id,
        bot_username,
        config.analytics_chat_id,
        telegram_client,
        tmdb_client,
    );
    let state = AppState::new(config, service);
    let listener = TcpListener::bind(state.bind_address()).await?;
    let app = build_router(state);

    info!(address = %listener.local_addr()?, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cinesharebot=info,tower_http=info".into()),
        )
        .with_target(false)
        .compact()
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};

        if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
            sigterm.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

async fn register_webhook(client: Arc<TelegramClient>, request: SetWebhookRequest) {
    let mut delay = std::time::Duration::from_secs(2);
    loop {
        match client.set_webhook(request.clone()).await {
            Ok(_) => {
                info!("telegram webhook configured");
                return;
            }
            Err(error) => {
                warn!(%error, retry_in_s = delay.as_secs(), "telegram webhook not configured yet")
            }
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(std::time::Duration::from_secs(60));
    }
}

fn healthcheck() -> bool {
    use std::io::{Read, Write};
    use std::time::Duration;

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let Ok(address) = format!("127.0.0.1:{port}").parse() else {
        return false;
    };
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(&address, Duration::from_secs(2))
    else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    if stream
        .write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut response = [0u8; 16];
    let Ok(read) = stream.read(&mut response) else {
        return false;
    };
    is_ok_status(&response[..read])
}

fn is_ok_status(response: &[u8]) -> bool {
    response.starts_with(b"HTTP/1.0 200") || response.starts_with(b"HTTP/1.1 200")
}

#[cfg(test)]
mod tests {
    use super::is_ok_status;

    #[test]
    fn healthcheck_accepts_only_ok_status() {
        assert!(is_ok_status(b"HTTP/1.1 200 OK\r\n"));
        assert!(is_ok_status(b"HTTP/1.0 200 OK"));
        assert!(!is_ok_status(b"HTTP/1.1 503 Service Unavailable"));
        assert!(!is_ok_status(b""));
    }
}
