use std::{future::Future, time::Duration};

use reqwest::{StatusCode, header};
use tokio::time::sleep;
use tracing::warn;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_ATTEMPTS: usize = 3;
const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub initial_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: MAX_ATTEMPTS,
            initial_delay: INITIAL_RETRY_DELAY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryInstruction {
    pub reason: &'static str,
    pub retry_after: Option<Duration>,
}

#[derive(Debug)]
pub enum AttemptResult<T, E> {
    Success(T),
    Retryable {
        error: E,
        instruction: RetryInstruction,
    },
    Final(E),
}

pub fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .expect("http client configuration must be valid")
}

pub fn should_retry_request_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect() || error.is_request()
}

pub fn should_retry_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

pub async fn execute_with_retry<T, E, MakeAttempt, AttemptFuture>(
    operation: &str,
    policy: RetryPolicy,
    mut make_attempt: MakeAttempt,
) -> Result<T, E>
where
    E: std::fmt::Display,
    MakeAttempt: FnMut(usize) -> AttemptFuture,
    AttemptFuture: Future<Output = AttemptResult<T, E>>,
{
    for attempt in 1..=policy.max_attempts {
        match make_attempt(attempt).await {
            AttemptResult::Success(value) => return Ok(value),
            AttemptResult::Final(error) => return Err(error),
            AttemptResult::Retryable { error, instruction } => {
                if attempt == policy.max_attempts {
                    return Err(error);
                }

                let delay = retry_delay_for_attempt(policy, attempt, instruction.retry_after);
                warn!(
                    operation,
                    attempt,
                    delay_ms = delay.as_millis(),
                    reason = instruction.reason,
                    error = %error,
                    "retrying http request"
                );
                sleep(delay).await;
            }
        }
    }

    unreachable!("retry loop must return before exhausting attempts");
}

pub fn retryable<T, E>(
    error: E,
    reason: &'static str,
    retry_after: Option<Duration>,
) -> AttemptResult<T, E> {
    AttemptResult::Retryable {
        error,
        instruction: RetryInstruction {
            reason,
            retry_after,
        },
    }
}

pub fn retry_delay_for_attempt(
    policy: RetryPolicy,
    attempt: usize,
    retry_after: Option<Duration>,
) -> Duration {
    retry_after.unwrap_or_else(|| policy.initial_delay.mul_f64(2_f64.powi((attempt - 1) as i32)))
}

pub fn retry_after_from_headers(headers: &header::HeaderMap) -> Option<Duration> {
    headers
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::Duration;

    use super::{
        AttemptResult, RetryPolicy, execute_with_retry, retry_delay_for_attempt, retryable,
        should_retry_status,
    };

    #[test]
    fn retries_only_retryable_statuses() {
        assert!(should_retry_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(should_retry_status(StatusCode::BAD_GATEWAY));
        assert!(!should_retry_status(StatusCode::BAD_REQUEST));
    }

    #[test]
    fn retry_backoff_is_exponential_without_retry_after() {
        let policy = RetryPolicy::default();
        assert_eq!(retry_delay_for_attempt(policy, 1, None), policy.initial_delay);
        assert_eq!(
            retry_delay_for_attempt(policy, 2, None),
            policy.initial_delay.mul_f64(2.0)
        );
    }

    #[tokio::test]
    async fn execute_with_retry_retries_and_then_succeeds() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_closure = Arc::clone(&attempts);

        let result = execute_with_retry("test", RetryPolicy::default(), move |_| {
            let attempts = Arc::clone(&attempts_for_closure);
            async move {
                let current = attempts.fetch_add(1, Ordering::SeqCst);
                if current == 0 {
                    retryable("retry", "test", Some(Duration::ZERO))
                } else {
                    AttemptResult::Success("ok")
                }
            }
        })
        .await;

        assert_eq!(result, Ok("ok"));
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
}
