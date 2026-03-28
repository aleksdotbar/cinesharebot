use reqwest::{StatusCode, header};
use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::{app::TmdbApi, locale::Locale};

use super::models::{
    MediaType, NormalizedDetails, SearchResults, TmdbListItem, TmdbMovieDetails, TmdbPaged,
    TmdbTvDetails,
};

#[derive(Clone)]
pub struct TmdbClient {
    http: reqwest::Client,
    token: String,
}

impl TmdbClient {
    pub fn new(token: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            token,
        }
    }

    async fn get_trending_results(&self, page: u32, locale: Locale) -> Result<SearchResults, TmdbError> {
        let response: TmdbPaged<TmdbListItem> = self
            .get("/3/trending/all/week", page_query(page, locale))
            .await?;
        let next_page = if page < response.total_pages {
            Some(page + 1)
        } else {
            None
        };

        Ok(SearchResults {
            results: response
                .results
                .into_iter()
                .filter_map(|item| item.try_into().ok())
                .collect(),
            next_page,
        })
    }

    async fn get_search_results(
        &self,
        query: &str,
        page: u32,
        locale: Locale,
    ) -> Result<SearchResults, TmdbError> {
        let first_page: TmdbPaged<TmdbListItem> = self
            .get("/3/search/multi", search_query(query, page, locale))
            .await?;
        let first_results = normalize_list_results(first_page.results);

        if !first_results.is_empty() || page >= first_page.total_pages {
            return Ok(SearchResults {
                results: first_results,
                next_page: next_page(page, first_page.total_pages),
            });
        }

        let second_page: TmdbPaged<TmdbListItem> = self
            .get("/3/search/multi", search_query(query, page + 1, locale))
            .await?;

        Ok(build_topped_up_search_results(first_page.total_pages, second_page))
    }

    async fn get<T>(&self, path: &str, query: Vec<(&str, String)>) -> Result<T, TmdbError>
    where
        T: DeserializeOwned,
    {
        let url = format!("https://api.themoviedb.org{path}");
        let request = self
            .http
            .get(url)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .query(
                &query
                    .iter()
                    .map(|(key, value)| (*key, value.as_str()))
                    .collect::<Vec<_>>(),
            );
        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(parse_tmdb_error(status, body));
        }

        serde_json::from_str(&body).map_err(|source| TmdbError::InvalidResponse {
            status,
            body,
            source,
        })
    }
}

fn normalize_list_results(items: Vec<TmdbListItem>) -> Vec<super::models::NormalizedSearchResult> {
    items.into_iter().filter_map(|item| item.try_into().ok()).collect()
}

fn next_page(last_fetched_page: u32, total_pages: u32) -> Option<u32> {
    (last_fetched_page < total_pages).then_some(last_fetched_page + 1)
}

fn build_topped_up_search_results(
    first_total_pages: u32,
    second_page: TmdbPaged<TmdbListItem>,
) -> SearchResults {
    let last_fetched_page = second_page._page;
    let total_pages = first_total_pages.max(second_page.total_pages);
    let results = normalize_list_results(second_page.results);

    SearchResults {
        results,
        next_page: next_page(last_fetched_page, total_pages),
    }
}

#[async_trait::async_trait]
impl TmdbApi for TmdbClient {
    async fn get_search_or_trending_results(
        &self,
        query: &str,
        page: u32,
        locale: Locale,
    ) -> Result<SearchResults, TmdbError> {
        let query = query.trim();
        if query.is_empty() {
            self.get_trending_results(page, locale).await
        } else {
            self.get_search_results(query, page, locale).await
        }
    }

    async fn get_details(
        &self,
        media_type: MediaType,
        id: u64,
        locale: Locale,
    ) -> Result<NormalizedDetails, TmdbError> {
        match media_type {
            MediaType::Movie => self
                .get::<TmdbMovieDetails>(&format!("/3/movie/{id}"), locale_query(locale))
                .await
                .map(Into::into),
            MediaType::Tv => self
                .get::<TmdbTvDetails>(&format!("/3/tv/{id}"), locale_query(locale))
                .await
                .map(Into::into),
        }
    }
}

fn locale_query(locale: Locale) -> Vec<(&'static str, String)> {
    let mut query = Vec::new();
    if let Some(language) = locale.tmdb_language() {
        query.push(("language", language.to_string()));
    }
    query
}

fn page_query(page: u32, locale: Locale) -> Vec<(&'static str, String)> {
    let mut query = vec![("page", page.to_string())];
    if let Some(language) = locale.tmdb_language() {
        query.push(("language", language.to_string()));
    }
    query
}

fn search_query(query: &str, page: u32, locale: Locale) -> Vec<(&'static str, String)> {
    let mut params = vec![("query", query.to_string()), ("page", page.to_string())];
    if let Some(language) = locale.tmdb_language() {
        params.push(("language", language.to_string()));
    }
    params
}

#[derive(Debug, serde::Deserialize)]
struct TmdbErrorResponse {
    status_code: i64,
    status_message: String,
}

fn parse_tmdb_error(status: StatusCode, body: String) -> TmdbError {
    if let Ok(error) = serde_json::from_str::<TmdbErrorResponse>(&body) {
        return TmdbError::Api {
            status,
            status_code: error.status_code,
            status_message: error.status_message,
        };
    }

    TmdbError::Protocol(format!("tmdb request failed ({status}): {body}"))
}

#[derive(Debug, Error)]
pub enum TmdbError {
    #[error("tmdb request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("tmdb api error ({status_code}, {status}): {status_message}")]
    Api {
        status: StatusCode,
        status_code: i64,
        status_message: String,
    },
    #[error("invalid tmdb response ({status}): {body}")]
    InvalidResponse {
        status: StatusCode,
        body: String,
        source: serde_json::Error,
    },
    #[error("{0}")]
    Protocol(String),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        build_topped_up_search_results, locale_query, next_page, normalize_list_results,
        page_query, search_query,
    };
    use crate::{locale::Locale, tmdb::models::{MediaType, TmdbListItem, TmdbPaged}};

    #[test]
    fn keeps_one_to_one_next_page_when_results_exist() {
        assert_eq!(next_page(1, 3), Some(2));
        assert_eq!(next_page(3, 3), None);
    }

    #[test]
    fn top_up_uses_next_page_results_and_skips_consumed_page() {
        let second_page: TmdbPaged<TmdbListItem> = serde_json::from_value(json!({
            "page": 2,
            "total_pages": 4,
            "results": [
                {
                    "media_type": "movie",
                    "id": 1,
                    "title": "John Wick",
                    "original_title": "John Wick",
                    "overview": "desc",
                    "poster_path": "/poster.jpg",
                    "release_date": "2014-10-22",
                    "popularity": 10.0
                }
            ]
        }))
        .unwrap();

        let search_results = build_topped_up_search_results(4, second_page);

        assert_eq!(search_results.results.len(), 1);
        assert_eq!(search_results.results[0].media_type, MediaType::Movie);
        assert_eq!(search_results.next_page, Some(3));
    }

    #[test]
    fn normalization_drops_people() {
        let page: TmdbPaged<TmdbListItem> = serde_json::from_value(json!({
            "page": 1,
            "total_pages": 2,
            "results": [
                { "media_type": "person", "id": 1 },
                {
                    "media_type": "tv",
                    "id": 2,
                    "name": "Lost",
                    "original_name": "Lost",
                    "overview": "desc",
                    "poster_path": "/poster2.jpg",
                    "first_air_date": "2004-09-22",
                    "popularity": 5.0
                }
            ]
        }))
        .unwrap();

        let results = normalize_list_results(page.results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].media_type, MediaType::Tv);
    }

    #[test]
    fn russian_locale_adds_tmdb_language_param() {
        assert_eq!(
            search_query("джон уик", 2, Locale::Ru),
            vec![
                ("query", "джон уик".to_string()),
                ("page", "2".to_string()),
                ("language", "ru-RU".to_string()),
            ]
        );
        assert_eq!(
            page_query(3, Locale::Ru),
            vec![
                ("page", "3".to_string()),
                ("language", "ru-RU".to_string()),
            ]
        );
        assert_eq!(locale_query(Locale::Ru), vec![("language", "ru-RU".to_string())]);
    }

    #[test]
    fn english_locale_keeps_default_tmdb_request_behavior() {
        assert_eq!(
            search_query("john wick", 1, Locale::En),
            vec![("query", "john wick".to_string()), ("page", "1".to_string())]
        );
        assert_eq!(page_query(1, Locale::En), vec![("page", "1".to_string())]);
        assert!(locale_query(Locale::En).is_empty());
    }
}
