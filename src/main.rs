use std::sync::Arc;

use tokio::net::TcpListener;
use tracing::info;

use cinesharebot::{
    app::AppService,
    config::{Config, ConfigError},
    telegram::{SetWebhookRequest, TelegramClient},
    tmdb::TmdbClient,
    web::{AppState, WEBHOOK_PATH, build_router},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
            url: format!(
                "{}{}",
                webhook_base_url.trim_end_matches('/'),
                WEBHOOK_PATH
            ),
            secret_token: config.webhook_secret.clone(),
            allowed_updates: Some(vec![
                "message".to_string(),
                "inline_query".to_string(),
                "chosen_inline_result".to_string(),
            ]),
            drop_pending_updates: Some(false),
        };

        telegram_client.set_webhook(request).await?;
        info!("telegram webhook configured");
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
