use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ParseMode {
    #[serde(rename = "HTML")]
    Html,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Update {
    pub update_id: i64,
    #[serde(default)]
    pub message: Option<Message>,
    #[serde(default)]
    pub inline_query: Option<InlineQuery>,
    #[serde(default)]
    pub chosen_inline_result: Option<ChosenInlineResult>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Message {
    pub chat: Chat,
    #[serde(default)]
    pub from: Option<User>,
    #[serde(default)]
    pub via_bot: Option<User>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Chat {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: ChatKind,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChatKind {
    #[default]
    Private,
    Group,
    Supergroup,
    Channel,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct InlineQuery {
    pub id: String,
    pub from: User,
    pub query: String,
    #[serde(default)]
    pub offset: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct ChosenInlineResult {
    pub result_id: String,
    pub from: User,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub inline_message_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub language_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SendMessageRequest {
    pub chat_id: i64,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<ParseMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_notification: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AnswerInlineQueryRequest {
    pub inline_query_id: String,
    pub results: Vec<InlineQueryResultArticle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_personal: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EditMessageTextRequest {
    pub inline_message_id: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<ParseMode>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EditMessageMediaRequest {
    pub inline_message_id: String,
    pub media: InputMediaPhoto,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InlineQueryResultArticle {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_width: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_height: Option<u16>,
    pub input_message_content: InputTextMessageContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
}

impl InlineQueryResultArticle {
    pub fn article(
        id: String,
        title: String,
        description: String,
        input_message_content: InputTextMessageContent,
    ) -> Self {
        Self {
            kind: "article",
            id,
            title,
            description,
            thumbnail_url: None,
            thumbnail_width: None,
            thumbnail_height: None,
            input_message_content,
            reply_markup: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InputTextMessageContent {
    pub message_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<ParseMode>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InputMediaPhoto {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub media: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<ParseMode>,
}

impl InputMediaPhoto {
    pub fn new(media: String, caption: Option<String>, parse_mode: Option<ParseMode>) -> Self {
        Self {
            kind: "photo",
            media,
            caption,
            parse_mode,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InlineKeyboardMarkup {
    pub inline_keyboard: Vec<Vec<InlineKeyboardButton>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InlineKeyboardButton {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub switch_inline_query_current_chat: Option<String>,
}

impl InlineKeyboardButton {
    pub fn callback(text: impl Into<String>, callback_data: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            callback_data: Some(callback_data.into()),
            switch_inline_query_current_chat: None,
        }
    }

    pub fn switch_inline_current(
        text: impl Into<String>,
        query: impl Into<String>,
    ) -> Self {
        Self {
            text: text.into(),
            callback_data: None,
            switch_inline_query_current_chat: Some(query.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        AnswerInlineQueryRequest, EditMessageMediaRequest, EditMessageTextRequest,
        InlineQueryResultArticle, InputMediaPhoto, InputTextMessageContent, ParseMode, Update,
    };

    #[test]
    fn deserializes_message_inline_query_and_chosen_inline_result() {
        let message_update: Update = serde_json::from_value(json!({
            "update_id": 1,
            "message": {
                "chat": { "id": 123, "type": "private" },
                "from": { "id": 10, "language_code": "ru-RU" }
            }
        }))
        .unwrap();
        assert_eq!(message_update.message.unwrap().chat.id, 123);

        let inline_query_update: Update = serde_json::from_value(json!({
            "update_id": 2,
            "inline_query": {
                "id": "iq1",
                "from": { "id": 1, "username": "neo", "language_code": "en" },
                "query": "john wick",
                "offset": ""
            }
        }))
        .unwrap();
        assert_eq!(inline_query_update.inline_query.unwrap().id, "iq1");

        let chosen_update: Update = serde_json::from_value(json!({
            "update_id": 3,
            "chosen_inline_result": {
                "result_id": "movie:1",
                "from": { "id": 1, "username": "neo", "language_code": "ru" },
                "query": "john wick",
                "inline_message_id": "msg-1"
            }
        }))
        .unwrap();
        assert_eq!(
            chosen_update
                .chosen_inline_result
                .unwrap()
                .inline_message_id
                .as_deref(),
            Some("msg-1")
        );
    }

    #[test]
    fn serializes_webhook_payload_shapes() {
        let answer = serde_json::to_value(AnswerInlineQueryRequest {
            inline_query_id: "iq1".to_string(),
            results: vec![InlineQueryResultArticle::article(
                "movie:1".to_string(),
                "John Wick".to_string(),
                "desc".to_string(),
                InputTextMessageContent {
                    message_text: "hello".to_string(),
                    parse_mode: Some(ParseMode::Html),
                },
            )],
            is_personal: Some(true),
            next_offset: Some("2".to_string()),
        })
        .unwrap();
        assert_eq!(answer["inline_query_id"], "iq1");
        assert_eq!(answer["is_personal"], true);

        let edit_text = serde_json::to_value(EditMessageTextRequest {
            inline_message_id: "msg-1".to_string(),
            text: "hello".to_string(),
            parse_mode: Some(ParseMode::Html),
        })
        .unwrap();
        assert_eq!(edit_text["inline_message_id"], "msg-1");

        let edit_media = serde_json::to_value(EditMessageMediaRequest {
            inline_message_id: "msg-1".to_string(),
            media: InputMediaPhoto::new(
                "https://example.com/poster.jpg".to_string(),
                Some("hello".to_string()),
                Some(ParseMode::Html),
            ),
        })
        .unwrap();
        assert_eq!(edit_media["media"]["type"], "photo");
    }
}
