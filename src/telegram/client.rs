use reqwest::StatusCode;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

use crate::{
    app::TelegramApi,
    http_client::{
        AttemptResult, RetryPolicy, build_client, execute_with_retry, retry_after_from_headers,
        retryable, should_retry_request_error, should_retry_status,
    },
};

use super::{SendMessageRequest, User};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SetWebhookRequest {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_updates: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_pending_updates: Option<bool>,
}

#[derive(Clone)]
pub struct TelegramClient {
    http: reqwest::Client,
    token: String,
}

impl TelegramClient {
    pub fn new(token: String) -> Self {
        Self {
            http: build_client(),
            token,
        }
    }

    pub async fn get_me(&self) -> Result<User, TelegramError> {
        self.call::<(), User>("getMe", None).await
    }

    pub async fn set_webhook(&self, request: SetWebhookRequest) -> Result<bool, TelegramError> {
        self.call("setWebhook", Some(&request)).await
    }

    pub fn build_method_url(&self, method: &str) -> String {
        format!("https://api.telegram.org/bot{}/{}", self.token, method)
    }

    async fn call<TRequest, TResponse>(
        &self,
        method: &str,
        payload: Option<&TRequest>,
    ) -> Result<TResponse, TelegramError>
    where
        TRequest: Serialize + ?Sized,
        TResponse: DeserializeOwned,
    {
        let url = self.build_method_url(method);
        execute_with_retry(method, RetryPolicy::default(), |_| {
            let url = url.clone();
            let request = self.http.post(&url);
            let request = if let Some(payload) = payload {
                request.json(payload)
            } else {
                request
            };

            async move {
                let response = match request.send().await {
                    Ok(response) => response,
                    Err(error) => {
                        return if should_retry_request_error(&error) {
                            retryable(
                                error.into(),
                                "transport failure",
                                None,
                            )
                        } else {
                            AttemptResult::Final(error.into())
                        };
                    }
                };

                let status = response.status();
                let headers = response.headers().clone();
                let body = match response.text().await {
                    Ok(body) => body,
                    Err(error) => {
                        return if should_retry_request_error(&error) {
                            retryable(error.into(), "response body read failed", None)
                        } else {
                            AttemptResult::Final(error.into())
                        };
                    }
                };

                let api_response = match serde_json::from_str::<TelegramApiResponse<TResponse>>(&body) {
                    Ok(api_response) => api_response,
                    Err(source) => {
                        return if should_retry_status(status) {
                            retryable(
                                TelegramError::InvalidResponse { status, body, source },
                                "retryable non-json response",
                                retry_after_from_headers(&headers),
                            )
                        } else {
                            AttemptResult::Final(TelegramError::InvalidResponse { status, body, source })
                        };
                    }
                };

                if api_response.ok {
                    return match api_response.result {
                        Some(result) => AttemptResult::Success(result),
                        None => AttemptResult::Final(TelegramError::MissingResult { status }),
                    };
                }

                let should_retry = should_retry_telegram_api_response(status, &api_response);
                let error = TelegramError::Api {
                    status,
                    description: api_response
                        .description
                        .unwrap_or_else(|| "unknown telegram api error".to_string()),
                    error_code: api_response.error_code,
                };

                if should_retry {
                    return retryable(
                        error,
                        "retryable api response",
                        api_response
                            .parameters
                            .as_ref()
                            .and_then(|parameters| parameters.retry_after)
                            .map(std::time::Duration::from_secs)
                            .or_else(|| retry_after_from_headers(&headers)),
                    );
                }

                AttemptResult::Final(error)
            }
        })
        .await
    }
}

#[async_trait::async_trait]
impl TelegramApi for TelegramClient {
    async fn send_message(&self, request: SendMessageRequest) -> Result<(), TelegramError> {
        let _: serde_json::Value = self.call("sendMessage", Some(&request)).await?;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
struct TelegramApiResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
    error_code: Option<u16>,
    parameters: Option<TelegramResponseParameters>,
}

#[derive(Debug, Deserialize)]
struct TelegramResponseParameters {
    retry_after: Option<u64>,
}

#[derive(Debug, Error)]
pub enum TelegramError {
    #[error("telegram request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("telegram api error ({status}): {description}")]
    Api {
        status: StatusCode,
        description: String,
        error_code: Option<u16>,
    },
    #[error("telegram response missing result ({status})")]
    MissingResult { status: StatusCode },
    #[error("invalid telegram response ({status}): {body}")]
    InvalidResponse {
        status: StatusCode,
        body: String,
        source: serde_json::Error,
    },
}

fn should_retry_telegram_api_response<T>(
    status: StatusCode,
    api_response: &TelegramApiResponse<T>,
) -> bool {
    should_retry_status(status) || api_response.error_code == Some(StatusCode::TOO_MANY_REQUESTS.as_u16())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{SetWebhookRequest, TelegramApiResponse, TelegramClient, should_retry_telegram_api_response};
    use crate::telegram::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode, SendMessageRequest};

    #[test]
    fn serializes_outbound_requests_and_path() {
        let client = TelegramClient::new("123:abc".to_string());
        assert_eq!(
            client.build_method_url("getMe"),
            "https://api.telegram.org/bot123:abc/getMe"
        );

        let send_message = serde_json::to_value(SendMessageRequest {
            chat_id: 1,
            text: "hello".to_string(),
            parse_mode: Some(ParseMode::Html),
            disable_notification: Some(true),
            reply_markup: Some(InlineKeyboardMarkup {
                inline_keyboard: vec![vec![InlineKeyboardButton::switch_inline_current(
                    "Try now",
                    "john wick",
                )]],
            }),
        })
        .unwrap();
        assert_eq!(send_message["parse_mode"], "HTML");

        let set_webhook = serde_json::to_value(SetWebhookRequest {
            url: "https://example.com/bot/123:abc".to_string(),
            secret_token: Some("secret".to_string()),
            allowed_updates: Some(vec!["message".to_string()]),
            drop_pending_updates: Some(false),
        })
        .unwrap();
        assert_eq!(set_webhook["url"], json!("https://example.com/bot/123:abc"));
    }

    #[test]
    fn telegram_api_retry_policy_respects_error_code() {
        let response = TelegramApiResponse::<serde_json::Value> {
            ok: false,
            result: None,
            description: None,
            error_code: Some(reqwest::StatusCode::TOO_MANY_REQUESTS.as_u16()),
            parameters: None,
        };

        assert!(should_retry_telegram_api_response(reqwest::StatusCode::OK, &response));
    }
}
