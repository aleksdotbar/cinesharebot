use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use tracing::{info, warn};

use crate::{
    app::AppService,
    config::Config,
    telegram::Update,
};

const TELEGRAM_SECRET_HEADER: &str = "x-telegram-bot-api-secret-token";
pub const WEBHOOK_PATH: &str = "/telegram/webhook";

#[derive(Clone)]
pub struct AppState {
    config: Config,
    service: AppService,
}

impl AppState {
    pub fn new(config: Config, service: AppService) -> Self {
        Self { config, service }
    }

    pub fn bind_address(&self) -> std::net::SocketAddr {
        self.config.bind_address
    }
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route(WEBHOOK_PATH, post(handle_webhook))
        .with_state(state)
}

async fn healthz() -> StatusCode {
    StatusCode::OK
}

async fn handle_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(update): Json<Update>,
) -> Response {
    if let Some(expected_secret) = &state.config.webhook_secret {
        let actual_secret = headers
            .get(TELEGRAM_SECRET_HEADER)
            .and_then(|value| value.to_str().ok());

        if actual_secret != Some(expected_secret.as_str()) {
            warn!("rejected webhook request with invalid telegram secret");
            return StatusCode::UNAUTHORIZED.into_response();
        }
    }

    info!(
        update_id = update.update_id,
        update_kind = update_kind(&update),
        "received telegram update"
    );

    match state.service.handle_update(update).await {
        Some(reply) => reply.into_response(),
        None => StatusCode::OK.into_response(),
    }
}

fn update_kind(update: &Update) -> &'static str {
    if update.message.is_some() {
        "message"
    } else if update.inline_query.is_some() {
        "inline_query"
    } else if update.chosen_inline_result.is_some() {
        "chosen_inline_result"
    } else {
        "unsupported"
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use axum::{body::Body, http::{Request, StatusCode}};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use crate::{
        app::{AppService, TelegramApi, TmdbApi},
        config::Config,
        locale::Locale,
        telegram::{SendMessageRequest, TelegramError},
        tmdb::{MediaType, NormalizedDetails, NormalizedSearchResult, SearchResults, TmdbError},
    };

    use super::{AppState, WEBHOOK_PATH, build_router};

    #[derive(Default)]
    struct MockTelegram {
        sent_messages: Mutex<Vec<SendMessageRequest>>,
    }

    #[async_trait]
    impl TelegramApi for MockTelegram {
        async fn send_message(&self, request: SendMessageRequest) -> Result<(), TelegramError> {
            self.sent_messages.lock().unwrap().push(request);
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockTmdb {
        search_response: Mutex<Option<SearchResults>>,
        details_response: Mutex<Option<NormalizedDetails>>,
    }

    #[async_trait]
    impl TmdbApi for MockTmdb {
        async fn get_search_or_trending_results(
            &self,
            _query: &str,
            _page: u32,
            _locale: Locale,
        ) -> Result<SearchResults, TmdbError> {
            Ok(self
                .search_response
                .lock()
                .unwrap()
                .clone()
                .unwrap_or(SearchResults {
                    results: Vec::new(),
                    next_page: None,
                }))
        }

        async fn get_details(
            &self,
            _media_type: MediaType,
            _id: u64,
            _locale: Locale,
        ) -> Result<NormalizedDetails, TmdbError> {
            self.details_response
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| TmdbError::Protocol("missing details fixture".to_string()))
        }
    }

    fn test_state(telegram: Arc<MockTelegram>, tmdb: Arc<MockTmdb>) -> AppState {
        let config = Config {
            bot_token: "secret-token".to_string(),
            tmdb_key: "tmdb".to_string(),
            analytics_chat_id: -100,
            webhook_secret: Some("header-secret".to_string()),
            webhook_base_url: None,
            bind_address: "127.0.0.1:3000".parse().unwrap(),
        };
        let service = AppService::new(777, "cinebot".to_string(), -100, telegram, tmdb);

        AppState::new(config, service)
    }

    #[tokio::test]
    async fn rejects_unknown_path() {
        let app = build_router(test_state(
            Arc::new(MockTelegram::default()),
            Arc::new(MockTmdb::default()),
        ));

        let response = app
            .oneshot(
                Request::post("/wrong")
                    .header("content-type", "application/json")
                    .header("x-telegram-bot-api-secret-token", "header-secret")
                    .body(Body::from(r#"{"update_id":1}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn healthz_returns_ok() {
        let app = build_router(test_state(
            Arc::new(MockTelegram::default()),
            Arc::new(MockTmdb::default()),
        ));

        let response = app
            .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_invalid_secret_header() {
        let app = build_router(test_state(
            Arc::new(MockTelegram::default()),
            Arc::new(MockTmdb::default()),
        ));

        let response = app
            .oneshot(
                Request::post(WEBHOOK_PATH)
                    .header("content-type", "application/json")
                    .header("x-telegram-bot-api-secret-token", "wrong")
                    .body(Body::from(r#"{"update_id":1}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn inline_query_returns_webhook_reply_json() {
        let telegram = Arc::new(MockTelegram::default());
        let tmdb = Arc::new(MockTmdb::default());
        *tmdb.search_response.lock().unwrap() = Some(SearchResults {
            results: vec![NormalizedSearchResult {
                media_type: MediaType::Movie,
                id: 42,
                title: "John Wick".to_string(),
                original_title: "John Wick".to_string(),
                overview: "An ex-hitman comes out of retirement.".to_string(),
                poster_path: Some("/john.jpg".to_string()),
                year: Some("2014".to_string()),
                popularity: 1.0,
            }],
            next_page: Some(2),
        });
        let app = build_router(test_state(telegram, tmdb));

        let response = app
            .oneshot(
                Request::post(WEBHOOK_PATH)
                    .header("content-type", "application/json")
                    .header("x-telegram-bot-api-secret-token", "header-secret")
                    .body(Body::from(
                        r#"{"update_id":1,"inline_query":{"id":"iq1","from":{"id":1,"username":"neo"},"query":"john wick","offset":""}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(body["method"], "answerInlineQuery");
        assert_eq!(body["inline_query_id"], "iq1");
        assert_eq!(body["results"][0]["id"], "movie:42");
    }

    #[tokio::test]
    async fn unsupported_updates_return_empty_ok() {
        let app = build_router(test_state(
            Arc::new(MockTelegram::default()),
            Arc::new(MockTmdb::default()),
        ));

        let response = app
            .oneshot(
                Request::post(WEBHOOK_PATH)
                    .header("content-type", "application/json")
                    .header("x-telegram-bot-api-secret-token", "header-secret")
                    .body(Body::from(json!({"update_id": 1, "edited_message": {}}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(bytes.is_empty());
    }
}
