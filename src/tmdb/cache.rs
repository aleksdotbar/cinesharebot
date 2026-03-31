use std::{
    collections::HashMap,
    hash::Hash,
    time::{Duration, Instant},
};

use crate::locale::Locale;

use super::models::{MediaType, NormalizedDetails, SearchResults};

const TRENDING_TTL: Duration = Duration::from_secs(3 * 24 * 60 * 60);
const SEARCH_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const DETAILS_TTL: Duration = Duration::from_secs(2 * 24 * 60 * 60);

const TRENDING_CACHE_MAX_ENTRIES: usize = 32;
const SEARCH_CACHE_MAX_ENTRIES: usize = 512;
const DETAILS_CACHE_MAX_ENTRIES: usize = 2_048;

#[derive(Clone, Debug, Default)]
pub struct TmdbCache {
    trending: TimedCache<TrendingCacheKey, SearchResults>,
    search: TimedCache<SearchCacheKey, SearchResults>,
    details: TimedCache<DetailsCacheKey, NormalizedDetails>,
}

impl TmdbCache {
    pub fn new() -> Self {
        Self {
            trending: TimedCache::new(TRENDING_CACHE_MAX_ENTRIES),
            search: TimedCache::new(SEARCH_CACHE_MAX_ENTRIES),
            details: TimedCache::new(DETAILS_CACHE_MAX_ENTRIES),
        }
    }

    pub fn get_trending(&mut self, page: u32, locale: Locale) -> Option<SearchResults> {
        self.trending
            .get(&TrendingCacheKey { page, locale }, Instant::now())
    }

    pub fn insert_trending(&mut self, page: u32, locale: Locale, results: SearchResults) {
        self.trending.insert(
            TrendingCacheKey { page, locale },
            results,
            Instant::now(),
            TRENDING_TTL,
        );
    }

    pub fn get_search(&mut self, query: &str, page: u32, locale: Locale) -> Option<SearchResults> {
        self.search
            .get(&SearchCacheKey::new(query, page, locale), Instant::now())
    }

    pub fn insert_search(
        &mut self,
        query: &str,
        page: u32,
        locale: Locale,
        results: SearchResults,
    ) {
        self.search.insert(
            SearchCacheKey::new(query, page, locale),
            results,
            Instant::now(),
            SEARCH_TTL,
        );
    }

    pub fn get_details(
        &mut self,
        media_type: MediaType,
        id: u64,
        locale: Locale,
    ) -> Option<NormalizedDetails> {
        self.details
            .get(&DetailsCacheKey { media_type, id, locale }, Instant::now())
    }

    pub fn insert_details(
        &mut self,
        media_type: MediaType,
        id: u64,
        locale: Locale,
        details: NormalizedDetails,
    ) {
        self.details.insert(
            DetailsCacheKey { media_type, id, locale },
            details,
            Instant::now(),
            DETAILS_TTL,
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct TrendingCacheKey {
    page: u32,
    locale: Locale,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SearchCacheKey {
    normalized_query: String,
    page: u32,
    locale: Locale,
}

impl SearchCacheKey {
    fn new(query: &str, page: u32, locale: Locale) -> Self {
        Self {
            normalized_query: query.trim().to_lowercase(),
            page,
            locale,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DetailsCacheKey {
    media_type: MediaType,
    id: u64,
    locale: Locale,
}

#[derive(Clone, Debug)]
struct TimedCache<K, V> {
    entries: HashMap<K, CacheEntry<V>>,
    max_entries: usize,
}

impl<K, V> Default for TimedCache<K, V> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            max_entries: 0,
        }
    }
}

impl<K, V> TimedCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    fn get(&mut self, key: &K, now: Instant) -> Option<V> {
        self.prune_expired(now);

        self.entries.get(key).map(|entry| entry.value.clone())
    }

    fn insert(&mut self, key: K, value: V, now: Instant, ttl: Duration) {
        self.prune_expired(now);
        self.entries.insert(
            key,
            CacheEntry {
                value,
                expires_at: now + ttl,
                inserted_at: now,
            },
        );
        self.evict_if_needed();
    }

    fn prune_expired(&mut self, now: Instant) {
        self.entries.retain(|_, entry| entry.expires_at > now);
    }

    fn evict_if_needed(&mut self) {
        while self.entries.len() > self.max_entries {
            let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.inserted_at)
                .map(|(key, _)| key.clone())
            else {
                break;
            };

            self.entries.remove(&oldest_key);
        }
    }
}

#[derive(Clone, Debug)]
struct CacheEntry<V> {
    value: V,
    expires_at: Instant,
    inserted_at: Instant,
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::locale::Locale;

    use super::{SearchCacheKey, TimedCache};

    #[test]
    fn search_cache_key_normalizes_query_for_case_and_whitespace() {
        assert_eq!(
            SearchCacheKey::new("  John Wick  ", 1, Locale::En),
            SearchCacheKey::new("john wick", 1, Locale::En)
        );
    }

    #[test]
    fn timed_cache_expires_entries() {
        let mut cache = TimedCache::new(8);
        let now = Instant::now();

        cache.insert("key", "value", now, Duration::from_secs(10));
        assert_eq!(cache.get(&"key", now + Duration::from_secs(9)), Some("value"));
        assert_eq!(cache.get(&"key", now + Duration::from_secs(10)), None);
    }

    #[test]
    fn timed_cache_evicts_oldest_entry_when_full() {
        let mut cache = TimedCache::new(2);
        let now = Instant::now();

        cache.insert("first", 1, now, Duration::from_secs(60));
        cache.insert("second", 2, now + Duration::from_secs(1), Duration::from_secs(60));
        cache.insert("third", 3, now + Duration::from_secs(2), Duration::from_secs(60));

        assert_eq!(cache.get(&"first", now + Duration::from_secs(2)), None);
        assert_eq!(cache.get(&"second", now + Duration::from_secs(2)), Some(2));
        assert_eq!(cache.get(&"third", now + Duration::from_secs(2)), Some(3));
    }
}
