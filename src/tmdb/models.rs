use std::fmt;

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaType {
    Movie,
    Tv,
}

impl fmt::Display for MediaType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Movie => formatter.write_str("movie"),
            Self::Tv => formatter.write_str("tv"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedSearchResult {
    pub media_type: MediaType,
    pub id: u64,
    pub title: String,
    pub original_title: String,
    pub overview: String,
    pub poster_path: Option<String>,
    pub year: Option<String>,
    pub popularity: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedDetails {
    pub media_type: MediaType,
    pub id: u64,
    pub title: String,
    pub original_title: String,
    pub overview: String,
    pub poster_path: Option<String>,
    pub year: Option<String>,
    pub popularity: f64,
    pub genres: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResults {
    pub results: Vec<NormalizedSearchResult>,
    pub next_page: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbPaged<T> {
    #[serde(rename = "page")]
    pub _page: u32,
    pub results: Vec<T>,
    pub total_pages: u32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "media_type", rename_all = "snake_case")]
pub enum TmdbListItem {
    Movie(TmdbMovieSummary),
    Tv(TmdbTvSummary),
    Person(TmdbPersonSummary),
}

#[derive(Debug, Deserialize)]
pub struct TmdbMovieSummary {
    pub id: u64,
    pub title: String,
    pub original_title: String,
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub popularity: f64,
}

#[derive(Debug, Deserialize)]
pub struct TmdbTvSummary {
    pub id: u64,
    pub name: String,
    pub original_name: String,
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub first_air_date: Option<String>,
    #[serde(default)]
    pub popularity: f64,
}

#[derive(Debug, Deserialize)]
pub struct TmdbPersonSummary {
    pub id: u64,
}

#[derive(Debug, Deserialize)]
pub struct TmdbMovieDetails {
    pub id: u64,
    pub title: String,
    pub original_title: String,
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub popularity: f64,
    #[serde(default)]
    pub genres: Vec<TmdbGenre>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbTvDetails {
    pub id: u64,
    pub name: String,
    pub original_name: String,
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub first_air_date: Option<String>,
    #[serde(default)]
    pub popularity: f64,
    #[serde(default)]
    pub genres: Vec<TmdbGenre>,
}

#[derive(Debug, Deserialize)]
pub struct TmdbGenre {
    pub name: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TmdbListItemConversionError {
    UnsupportedPerson,
}

impl From<TmdbMovieSummary> for NormalizedSearchResult {
    fn from(movie: TmdbMovieSummary) -> Self {
        Self {
            media_type: MediaType::Movie,
            id: movie.id,
            title: movie.title,
            original_title: movie.original_title,
            overview: movie.overview,
            poster_path: movie.poster_path,
            year: extract_year(movie.release_date.as_deref()),
            popularity: movie.popularity,
        }
    }
}

impl From<TmdbTvSummary> for NormalizedSearchResult {
    fn from(tv: TmdbTvSummary) -> Self {
        Self {
            media_type: MediaType::Tv,
            id: tv.id,
            title: tv.name,
            original_title: tv.original_name,
            overview: tv.overview,
            poster_path: tv.poster_path,
            year: extract_year(tv.first_air_date.as_deref()),
            popularity: tv.popularity,
        }
    }
}

impl TryFrom<TmdbListItem> for NormalizedSearchResult {
    type Error = TmdbListItemConversionError;

    fn try_from(item: TmdbListItem) -> Result<Self, Self::Error> {
        match item {
            TmdbListItem::Movie(movie) => Ok(movie.into()),
            TmdbListItem::Tv(tv) => Ok(tv.into()),
            TmdbListItem::Person(_) => Err(TmdbListItemConversionError::UnsupportedPerson),
        }
    }
}

impl From<TmdbMovieDetails> for NormalizedDetails {
    fn from(movie: TmdbMovieDetails) -> Self {
        Self {
            media_type: MediaType::Movie,
            id: movie.id,
            title: movie.title,
            original_title: movie.original_title,
            overview: movie.overview,
            poster_path: movie.poster_path,
            year: extract_year(movie.release_date.as_deref()),
            popularity: movie.popularity,
            genres: movie.genres.into_iter().map(|genre| genre.name).collect(),
        }
    }
}

impl From<TmdbTvDetails> for NormalizedDetails {
    fn from(tv: TmdbTvDetails) -> Self {
        Self {
            media_type: MediaType::Tv,
            id: tv.id,
            title: tv.name,
            original_title: tv.original_name,
            overview: tv.overview,
            poster_path: tv.poster_path,
            year: extract_year(tv.first_air_date.as_deref()),
            popularity: tv.popularity,
            genres: tv.genres.into_iter().map(|genre| genre.name).collect(),
        }
    }
}

fn extract_year(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    value.split('-').next().map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        MediaType, NormalizedDetails, NormalizedSearchResult, TmdbListItem, TmdbMovieDetails,
        TmdbPaged, TmdbTvDetails,
    };

    #[test]
    fn deserializes_tmdb_search_trending_and_details_payloads() {
        let paged: TmdbPaged<TmdbListItem> = serde_json::from_value(json!({
            "page": 1,
            "total_pages": 2,
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
                },
                {
                    "media_type": "tv",
                    "id": 2,
                    "name": "Lost",
                    "original_name": "Lost",
                    "overview": "desc",
                    "poster_path": "/poster2.jpg",
                    "first_air_date": "2004-09-22",
                    "popularity": 5.0
                },
                {
                    "media_type": "person",
                    "id": 3
                }
            ]
        }))
        .unwrap();
        let normalized = paged
            .results
            .into_iter()
            .filter_map(|item| NormalizedSearchResult::try_from(item).ok())
            .collect::<Vec<_>>();
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].media_type, MediaType::Movie);
        assert_eq!(normalized[1].media_type, MediaType::Tv);

        let movie: TmdbMovieDetails = serde_json::from_value(json!({
            "id": 1,
            "title": "John Wick",
            "original_title": "John Wick",
            "overview": "desc",
            "poster_path": "/poster.jpg",
            "release_date": "2014-10-22",
            "popularity": 10.0,
            "genres": [{ "name": "Action" }]
        }))
        .unwrap();
        assert_eq!(NormalizedDetails::from(movie).genres, vec!["Action"]);

        let tv: TmdbTvDetails = serde_json::from_value(json!({
            "id": 2,
            "name": "Lost",
            "original_name": "Lost",
            "overview": "desc",
            "poster_path": "/poster2.jpg",
            "first_air_date": "2004-09-22",
            "popularity": 5.0,
            "genres": [{ "name": "Drama" }]
        }))
        .unwrap();
        assert_eq!(NormalizedDetails::from(tv).genres, vec!["Drama"]);
    }
}
