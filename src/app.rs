use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use tracing::{error, info, warn};

use crate::{
    locale::Locale,
    telegram::{
        AnswerInlineQueryRequest, ChosenInlineResult, EditMessageMediaRequest,
        EditMessageTextRequest, InlineKeyboardButton, InlineKeyboardMarkup, InlineQuery,
        InlineQueryResultArticle, InputMediaPhoto, InputTextMessageContent, Message, ParseMode,
        SendMessageRequest, TelegramError, Update, User, WebhookReply,
    },
    tmdb::{MediaType, NormalizedDetails, NormalizedSearchResult, SearchResults, TmdbError},
};

const PARSE_MODE: ParseMode = ParseMode::Html;

#[async_trait]
pub trait TelegramApi: Send + Sync {
    async fn send_message(&self, request: SendMessageRequest) -> Result<(), TelegramError>;
}

#[async_trait]
pub trait TmdbApi: Send + Sync {
    async fn get_search_or_trending_results(
        &self,
        query: &str,
        page: u32,
        locale: Locale,
    ) -> Result<SearchResults, TmdbError>;

    async fn get_details(
        &self,
        media_type: MediaType,
        id: u64,
        locale: Locale,
    ) -> Result<NormalizedDetails, TmdbError>;
}

#[derive(Clone)]
pub struct AppService {
    bot_id: i64,
    bot_username: String,
    analytics_chat_id: i64,
    telegram: Arc<dyn TelegramApi>,
    tmdb: Arc<dyn TmdbApi>,
}

impl AppService {
    pub fn new(
        bot_id: i64,
        bot_username: String,
        analytics_chat_id: i64,
        telegram: Arc<dyn TelegramApi>,
        tmdb: Arc<dyn TmdbApi>,
    ) -> Self {
        Self {
            bot_id,
            bot_username,
            analytics_chat_id,
            telegram,
            tmdb,
        }
    }

    pub async fn handle_update(&self, update: Update) -> Option<WebhookReply> {
        if let Some(message) = update.message {
            self.handle_message(message).await;
            return None;
        }

        if let Some(inline_query) = update.inline_query {
            return Some(self.handle_inline_query(inline_query).await);
        }

        if let Some(chosen_inline_result) = update.chosen_inline_result {
            return self.handle_chosen_inline_result(chosen_inline_result).await;
        }

        info!("ignoring unsupported telegram update");
        None
    }

    async fn handle_message(&self, message: Message) {
        if message
            .via_bot
            .as_ref()
            .is_some_and(|via_bot| via_bot.id == self.bot_id)
        {
            info!(
                chat_id = message.chat.id,
                "ignoring message sent via this bot"
            );
            return;
        }

        info!(chat_id = message.chat.id, "handling message update");
        let locale = locale_for_message_help(message.from.as_ref());

        let request = SendMessageRequest {
            chat_id: message.chat.id,
            text: build_description_message(&self.bot_username, locale),
            parse_mode: Some(PARSE_MODE),
            disable_notification: Some(true),
            reply_markup: Some(InlineKeyboardMarkup {
                inline_keyboard: vec![vec![InlineKeyboardButton::switch_inline_current(
                    try_now_label(locale),
                    try_now_query(locale),
                )]],
            }),
        };

        if let Err(error) = self.telegram.send_message(request).await {
            self.report_error(format!("message reply failed: {error}"));
        }
    }

    async fn handle_inline_query(&self, inline_query: InlineQuery) -> WebhookReply {
        let query = inline_query.query.trim();
        let locale = locale_for_inline_query(query, inline_query.from.language_code.as_deref());
        let page = inline_query
            .offset
            .as_deref()
            .and_then(|offset| offset.parse::<u32>().ok())
            .filter(|page| *page > 0)
            .unwrap_or(1);

        info!(
            inline_query_id = %inline_query.id,
            page,
            is_trending = query.is_empty(),
            query_len = query.chars().count(),
            "handling inline query"
        );

        let search_results = match self
            .tmdb
            .get_search_or_trending_results(&inline_query.query, page, locale)
            .await
        {
            Ok(search_results) => search_results,
            Err(error) => {
                error!(page, is_trending = query.is_empty(), error = %error, "tmdb lookup failed for inline query");
                self.report_error(error.to_string());
                SearchResults {
                    results: Vec::new(),
                    next_page: None,
                }
            }
        };

        let mut seen_ids = HashSet::new();
        let results: Vec<InlineQueryResultArticle> = search_results
            .results
            .into_iter()
            .map(|result| build_query_result(result, locale))
            .filter(|result| seen_ids.insert(result.id.clone()))
            .collect();

        info!(
            inline_query_id = %inline_query.id,
            results_count = results.len(),
            has_next_page = search_results.next_page.is_some(),
            "built inline query reply"
        );

        WebhookReply::AnswerInlineQuery(AnswerInlineQueryRequest {
            inline_query_id: inline_query.id,
            results,
            next_offset: search_results.next_page.map(|page| page.to_string()),
        })
    }

    async fn handle_chosen_inline_result(
        &self,
        chosen_inline_result: ChosenInlineResult,
    ) -> Option<WebhookReply> {
        let inline_message_id = chosen_inline_result.inline_message_id.or_else(|| {
            warn!(
                result_id = %chosen_inline_result.result_id,
                "ignoring chosen inline result without inline message id"
            );
            None
        })?;

        info!(
            result_id = %chosen_inline_result.result_id,
            has_query = chosen_inline_result.query.is_some(),
            "handling chosen inline result"
        );
        let locale = locale_for_chosen_result(
            chosen_inline_result.query.as_deref(),
            chosen_inline_result.from.language_code.as_deref(),
        );

        let (media_type, id) = parse_result_id(&chosen_inline_result.result_id).or_else(|| {
            warn!(
                result_id = %chosen_inline_result.result_id,
                "ignoring chosen inline result with invalid result id"
            );
            None
        })?;

        let details = match self.tmdb.get_details(media_type, id, locale).await {
            Ok(details) => details,
            Err(error) => {
                error!(
                    media_type = %media_type,
                    tmdb_id = id,
                    error = %error,
                    "tmdb details lookup failed for chosen inline result"
                );
                self.report_error(error.to_string());
                return Some(WebhookReply::EditMessageText(EditMessageTextRequest {
                    inline_message_id,
                    text: details_load_failed_message(locale).to_string(),
                    parse_mode: Some(PARSE_MODE),
                }));
            }
        };

        let caption = build_caption(
            locale,
            details.media_type,
            &details.title,
            &details.original_title,
            details.year.as_deref(),
            Some(&details.genres),
        );

        let reply = match details.poster_path.as_deref() {
            Some(poster_path) => {
                info!(
                    media_type = %details.media_type,
                    tmdb_id = details.id,
                    "replying to chosen inline result with media edit"
                );
                WebhookReply::EditMessageMedia(EditMessageMediaRequest {
                    inline_message_id,
                    media: InputMediaPhoto::new(
                        image_url(poster_path, 780),
                        Some(caption),
                        Some(PARSE_MODE),
                    ),
                })
            }
            None => {
                info!(
                    media_type = %details.media_type,
                    tmdb_id = details.id,
                    "replying to chosen inline result with text edit"
                );
                WebhookReply::EditMessageText(EditMessageTextRequest {
                    inline_message_id,
                    text: caption,
                    parse_mode: Some(PARSE_MODE),
                })
            }
        };

        self.spawn_analytics(
            chosen_inline_result.from,
            details,
            chosen_inline_result.query,
        );

        Some(reply)
    }

    fn spawn_analytics(&self, user: User, details: NormalizedDetails, query: Option<String>) {
        let request = SendMessageRequest {
            chat_id: self.analytics_chat_id,
            text: build_analytics_message(user.username.as_deref(), &details, query.as_deref()),
            parse_mode: Some(PARSE_MODE),
            disable_notification: Some(true),
            reply_markup: None,
        };
        let telegram = self.telegram.clone();

        tokio::spawn(async move {
            info!(
                media_type = %details.media_type,
                tmdb_id = details.id,
                username = user.username.as_deref().unwrap_or(""),
                has_query = query.is_some(),
                "sending analytics message"
            );

            if let Err(error) = telegram.send_message(request).await {
                error!(error = %error, "best-effort telegram send failed");
            }
        });
    }

    fn report_error(&self, message: String) {
        error!(message = %message, "reporting bot error");

        let telegram = self.telegram.clone();
        let analytics_chat_id = self.analytics_chat_id;

        tokio::spawn(async move {
            let request = SendMessageRequest {
                chat_id: analytics_chat_id,
                text: format!("<b>Error:</b> {}", escape_html(&message)),
                parse_mode: Some(PARSE_MODE),
                disable_notification: None,
                reply_markup: None,
            };

            if let Err(error) = telegram.send_message(request).await {
                error!(error = %error, "best-effort telegram send failed");
            }
        });
    }
}

fn parse_result_id(result_id: &str) -> Option<(MediaType, u64)> {
    let (media_type, id) = result_id.split_once(':')?;
    let media_type = match media_type {
        "movie" => MediaType::Movie,
        "tv" => MediaType::Tv,
        _ => return None,
    };
    let id = id.parse::<u64>().ok()?;

    Some((media_type, id))
}

fn locale_for_message_help(user: Option<&User>) -> Locale {
    Locale::from_language_code(user.and_then(|user| user.language_code.as_deref()))
}

fn locale_for_inline_query(query: &str, language_code: Option<&str>) -> Locale {
    Locale::from_query(query).unwrap_or_else(|| Locale::from_language_code(language_code))
}

fn locale_for_chosen_result(query: Option<&str>, language_code: Option<&str>) -> Locale {
    query
        .and_then(Locale::from_query)
        .unwrap_or_else(|| Locale::from_language_code(language_code))
}

fn build_query_result(result: NormalizedSearchResult, locale: Locale) -> InlineQueryResultArticle {
    let mut article = InlineQueryResultArticle::article(
        format!("{}:{}", result.media_type, result.id),
        build_article_title(&result, locale),
        truncate_chars(&result.overview, 512),
        InputTextMessageContent {
            message_text: build_caption(
                locale,
                result.media_type,
                &result.title,
                &result.original_title,
                result.year.as_deref(),
                None,
            ),
            parse_mode: Some(PARSE_MODE),
        },
    );
    article.thumbnail_url = result.poster_path.as_ref().map(|path| image_url(path, 154));
    article.thumbnail_width = Some(154);
    article.thumbnail_height = Some(231);
    article.reply_markup = Some(InlineKeyboardMarkup {
        inline_keyboard: vec![vec![InlineKeyboardButton::callback("...", "loading")]],
    });
    article
}

fn build_caption(
    locale: Locale,
    media_type: MediaType,
    title: &str,
    original_title: &str,
    year: Option<&str>,
    genres: Option<&[String]>,
) -> String {
    let mut caption = format!("<b>{}</b>", build_result_title(title, year));

    if original_title != title {
        caption.push('\n');
        caption.push_str(&escape_html(original_title));
    }

    caption.push_str("\n\n<i>");
    caption.push_str(build_media_label(media_type, locale));
    caption.push_str("</i>");

    if let Some(genres) = genres.filter(|genres| !genres.is_empty()) {
        caption.push_str(" • <i>");
        caption.push_str(
            &genres
                .iter()
                .map(|genre| format_genre_label(genre, locale))
                .collect::<Vec<_>>()
                .join(", "),
        );
        caption.push_str("</i>");
    }

    caption
}

fn build_article_title(result: &NormalizedSearchResult, locale: Locale) -> String {
    let mut title = escape_html(&result.title);

    if let Some(year) = &result.year {
        title.push_str(" | ");
        title.push_str(year);
    }

    title.push_str(" | ");
    title.push_str(build_media_label(result.media_type, locale));

    title
}

fn build_result_title(title: &str, year: Option<&str>) -> String {
    let mut result = escape_html(title);

    if let Some(year) = year {
        result.push_str(" (");
        result.push_str(year);
        result.push(')');
    }

    result
}

fn build_analytics_message(
    username: Option<&str>,
    details: &NormalizedDetails,
    query: Option<&str>,
) -> String {
    let who = username
        .map(|username| format!("@{}", escape_html(username)))
        .unwrap_or_else(|| "Someone".to_string());
    let what = build_result_title(&details.title, details.year.as_deref());
    let where_text = query
        .map(|query| format!("for query <code>{}</code>", escape_html(query)))
        .unwrap_or_else(|| "from trends".to_string());

    format!("{who} chose <b>{what}</b> {where_text}")
}

fn build_description_message(bot_username: &str, locale: Locale) -> String {
    let mention = format!("@{}", escape_html(bot_username));
    match locale {
        Locale::En => format!(
            "\
This bot can help you find and share movies and tv shows.

It works automatically, no need to add it anywhere.

Simply open any of your chats, type <code>{mention}</code> and a movie or a tv show title after a space. Then tap on one of the results.

If you just type <code>{mention}</code> and a space, you'll see trending results.

If you already have some messages from this bot in your chat, you can just tap on bot's username and it will be pasted into the input field.

You can try it right here.
"
        ),
        Locale::Ru => format!(
            "\
Этот бот помогает искать и делиться фильмами и сериалами.

Он работает автоматически, его не нужно никуда добавлять.

Просто откройте любой чат, введите <code>{mention}</code> и после пробела название фильма или сериала. Затем выберите один из результатов.

Если просто ввести <code>{mention}</code> и пробел, бот покажет тренды.

Если в чате уже есть сообщения от этого бота, можно просто нажать на имя бота, и оно вставится в поле ввода.

Попробуйте прямо здесь.
"
        ),
    }
}

fn image_url(path: &str, size: u16) -> String {
    format!("https://image.tmdb.org/t/p/w{size}{path}")
}

fn build_media_label(media_type: MediaType, locale: Locale) -> &'static str {
    match (media_type, locale) {
        (MediaType::Movie, Locale::En) => "Movie",
        (MediaType::Tv, Locale::En) => "TV",
        (MediaType::Movie, Locale::Ru) => "Фильм",
        (MediaType::Tv, Locale::Ru) => "Сериал",
    }
}

fn try_now_label(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Try now",
        Locale::Ru => "Попробовать",
    }
}

fn try_now_query(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "john wick",
        Locale::Ru => "джон уик",
    }
}

fn details_load_failed_message(locale: Locale) -> &'static str {
    match locale {
        Locale::En => "Couldn’t load details for that title right now. Please try again.",
        Locale::Ru => "Сейчас не удалось загрузить информацию об этом тайтле. Попробуйте еще раз.",
    }
}

fn format_genre_label(genre: &str, locale: Locale) -> String {
    match locale {
        Locale::En => escape_html(genre),
        Locale::Ru => escape_html(&title_case_words(genre)),
    }
}

fn title_case_words(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut capitalize_next = true;

    for character in value.chars() {
        if capitalize_next {
            for uppercase in character.to_uppercase() {
                result.push(uppercase);
            }
        } else {
            result.push(character);
        }

        capitalize_next = character.is_whitespace() || character == '-';
    }

    result
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::{
        AppService, TelegramApi, TmdbApi, build_caption, build_description_message,
        details_load_failed_message, locale_for_chosen_result, locale_for_inline_query,
        locale_for_message_help,
    };
    use crate::{
        locale::Locale,
        telegram::{SendMessageRequest, TelegramError, Update, User},
        tmdb::{MediaType, NormalizedDetails, NormalizedSearchResult, SearchResults, TmdbError},
    };

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

    struct MockTmdb {
        search_results: SearchResults,
        details: NormalizedDetails,
        search_locales: Mutex<Vec<Locale>>,
        details_locales: Mutex<Vec<Locale>>,
    }

    #[async_trait]
    impl TmdbApi for MockTmdb {
        async fn get_search_or_trending_results(
            &self,
            _query: &str,
            _page: u32,
            locale: Locale,
        ) -> Result<SearchResults, TmdbError> {
            self.search_locales.lock().unwrap().push(locale);
            Ok(self.search_results.clone())
        }

        async fn get_details(
            &self,
            _media_type: MediaType,
            _id: u64,
            locale: Locale,
        ) -> Result<NormalizedDetails, TmdbError> {
            self.details_locales.lock().unwrap().push(locale);
            Ok(self.details.clone())
        }
    }

    fn sample_result() -> NormalizedSearchResult {
        NormalizedSearchResult {
            media_type: MediaType::Movie,
            id: 1,
            title: "John Wick".to_string(),
            original_title: "John Wick".to_string(),
            overview: "An ex-hitman comes out of retirement.".to_string(),
            poster_path: Some("/poster.jpg".to_string()),
            year: Some("2014".to_string()),
            popularity: 10.0,
        }
    }

    fn sample_details() -> NormalizedDetails {
        NormalizedDetails {
            media_type: MediaType::Movie,
            id: 1,
            title: "John Wick".to_string(),
            original_title: "John Wick".to_string(),
            overview: "An ex-hitman comes out of retirement.".to_string(),
            poster_path: Some("/poster.jpg".to_string()),
            year: Some("2014".to_string()),
            popularity: 10.0,
            genres: vec!["Action".to_string()],
        }
    }

    #[tokio::test]
    async fn message_updates_send_outbound_message() {
        let telegram = Arc::new(MockTelegram::default());
        let tmdb = Arc::new(MockTmdb {
            search_results: SearchResults {
                results: vec![sample_result()],
                next_page: None,
            },
            details: sample_details(),
            search_locales: Mutex::new(Vec::new()),
            details_locales: Mutex::new(Vec::new()),
        });
        let service = AppService::new(777, "cinebot".to_string(), -100, telegram.clone(), tmdb);

        service
            .handle_update(
                serde_json::from_str::<Update>(
                    r#"{"update_id":1,"message":{"chat":{"id":123},"from":{"id":1,"language_code":"en"}}}"#,
                )
                .unwrap(),
            )
            .await;

        let sent_messages = telegram.sent_messages.lock().unwrap();
        assert_eq!(sent_messages.len(), 1);
        assert_eq!(sent_messages[0].chat_id, 123);
    }

    #[tokio::test]
    async fn ignores_messages_sent_via_this_bot() {
        let telegram = Arc::new(MockTelegram::default());
        let tmdb = Arc::new(MockTmdb {
            search_results: SearchResults {
                results: vec![sample_result()],
                next_page: None,
            },
            details: sample_details(),
            search_locales: Mutex::new(Vec::new()),
            details_locales: Mutex::new(Vec::new()),
        });
        let service = AppService::new(777, "cinebot".to_string(), -100, telegram.clone(), tmdb);

        service
            .handle_update(serde_json::from_str::<Update>(
                r#"{"update_id":1,"message":{"chat":{"id":123},"via_bot":{"id":777,"username":"cinebot"}}}"#,
            )
            .unwrap())
            .await;

        let sent_messages = telegram.sent_messages.lock().unwrap();
        assert!(sent_messages.is_empty());
    }

    #[tokio::test]
    async fn chosen_inline_result_returns_media_reply_and_sends_analytics() {
        let telegram = Arc::new(MockTelegram::default());
        let tmdb = Arc::new(MockTmdb {
            search_results: SearchResults {
                results: vec![sample_result()],
                next_page: None,
            },
            details: sample_details(),
            search_locales: Mutex::new(Vec::new()),
            details_locales: Mutex::new(Vec::new()),
        });
        let service = AppService::new(777, "cinebot".to_string(), -100, telegram.clone(), tmdb);

        let reply = service
            .handle_update(serde_json::from_str::<Update>(
                r#"{"update_id":1,"chosen_inline_result":{"result_id":"movie:1","from":{"id":1,"username":"neo","language_code":"en"},"query":"john wick","inline_message_id":"abc"}}"#,
            )
            .unwrap())
            .await;

        let reply_json = reply.unwrap().to_json();
        assert_eq!(reply_json["method"], "editMessageMedia");

        tokio::task::yield_now().await;

        let sent_messages = telegram.sent_messages.lock().unwrap();
        assert_eq!(sent_messages.len(), 1);
        assert_eq!(sent_messages[0].chat_id, -100);
    }

    #[test]
    fn resolves_locales_correctly() {
        let user = User {
            id: 1,
            username: None,
            language_code: Some("ru-RU".to_string()),
        };

        assert_eq!(locale_for_message_help(Some(&user)), Locale::Ru);
        assert_eq!(locale_for_message_help(None), Locale::En);
        assert_eq!(locale_for_inline_query("джон уик", Some("en")), Locale::Ru);
        assert_eq!(locale_for_inline_query("john wick", Some("ru")), Locale::En);
        assert_eq!(locale_for_inline_query("", Some("ru")), Locale::Ru);
        assert_eq!(locale_for_chosen_result(Some("джон уик"), Some("en")), Locale::Ru);
        assert_eq!(locale_for_chosen_result(None, Some("ru")), Locale::Ru);
    }

    #[test]
    fn renders_help_in_russian() {
        let text = build_description_message("cinebot", Locale::Ru);
        assert!(text.contains("Этот бот помогает искать"));
        assert!(text.contains("@cinebot"));
    }

    #[test]
    fn renders_help_in_english() {
        let text = build_description_message("cinebot", Locale::En);
        assert!(text.contains("This bot can help you find and share"));
    }

    #[test]
    fn russian_caption_capitalizes_genres() {
        let caption = build_caption(
            Locale::Ru,
            MediaType::Movie,
            "Джон Уик",
            "Джон Уик",
            Some("2014"),
            Some(&["боевик".to_string(), "триллер".to_string(), "криминал".to_string()]),
        );

        assert!(caption.contains("<i>Фильм</i> • <i>Боевик, Триллер, Криминал</i>"));
    }

    #[test]
    fn russian_caption_title_cases_multi_word_genres() {
        let caption = build_caption(
            Locale::Ru,
            MediaType::Movie,
            "Дюна",
            "Дюна",
            Some("2021"),
            Some(&["научная фантастика".to_string(), "приключение".to_string()]),
        );

        assert!(caption.contains("<i>Фильм</i> • <i>Научная Фантастика, Приключение</i>"));
    }

    #[tokio::test]
    async fn inline_query_uses_russian_locale_and_labels_for_cyrillic_query() {
        let telegram = Arc::new(MockTelegram::default());
        let tmdb = Arc::new(MockTmdb {
            search_results: SearchResults {
                results: vec![sample_result()],
                next_page: Some(2),
            },
            details: sample_details(),
            search_locales: Mutex::new(Vec::new()),
            details_locales: Mutex::new(Vec::new()),
        });
        let service = AppService::new(777, "cinebot".to_string(), -100, telegram, tmdb.clone());

        let reply = service
            .handle_update(serde_json::from_str::<Update>(
                r#"{"update_id":1,"inline_query":{"id":"iq1","from":{"id":1,"language_code":"en"},"query":"джон уик","offset":""}}"#,
            )
            .unwrap())
            .await
            .unwrap();
        let reply_json = reply.to_json();

        assert_eq!(tmdb.search_locales.lock().unwrap().as_slice(), &[Locale::Ru]);
        assert_eq!(reply_json["results"][0]["title"], "John Wick | 2014 | Фильм");
        assert_eq!(reply_json["results"][0]["reply_markup"]["inline_keyboard"][0][0]["text"], "...");
    }

    #[tokio::test]
    async fn chosen_inline_result_error_text_is_russian_when_locale_is_russian() {
        struct ErrorTmdb {
            details_locales: Mutex<Vec<Locale>>,
        }

        #[async_trait]
        impl TmdbApi for ErrorTmdb {
            async fn get_search_or_trending_results(
                &self,
                _query: &str,
                _page: u32,
                _locale: Locale,
            ) -> Result<SearchResults, TmdbError> {
                Ok(SearchResults {
                    results: Vec::new(),
                    next_page: None,
                })
            }

            async fn get_details(
                &self,
                _media_type: MediaType,
                _id: u64,
                locale: Locale,
            ) -> Result<NormalizedDetails, TmdbError> {
                self.details_locales.lock().unwrap().push(locale);
                Err(TmdbError::Protocol("boom".to_string()))
            }
        }

        let telegram = Arc::new(MockTelegram::default());
        let tmdb = Arc::new(ErrorTmdb {
            details_locales: Mutex::new(Vec::new()),
        });
        let service = AppService::new(777, "cinebot".to_string(), -100, telegram, tmdb.clone());

        let reply = service
            .handle_update(serde_json::from_str::<Update>(
                r#"{"update_id":1,"chosen_inline_result":{"result_id":"movie:1","from":{"id":1,"language_code":"en"},"query":"джон уик","inline_message_id":"abc"}}"#,
            )
            .unwrap())
            .await
            .unwrap();
        let reply_json = reply.to_json();

        assert_eq!(tmdb.details_locales.lock().unwrap().as_slice(), &[Locale::Ru]);
        assert_eq!(reply_json["text"], details_load_failed_message(Locale::Ru));
    }
}
