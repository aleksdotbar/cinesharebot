use axum::{
    Json,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::{Value, to_value};

use super::{
    AnswerInlineQueryRequest, EditMessageMediaRequest, EditMessageTextRequest,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebhookReply {
    AnswerInlineQuery(AnswerInlineQueryRequest),
    EditMessageText(EditMessageTextRequest),
    EditMessageMedia(EditMessageMediaRequest),
}

impl WebhookReply {
    pub fn to_json(&self) -> Value {
        match self {
            Self::AnswerInlineQuery(payload) => webhook_envelope("answerInlineQuery", payload),
            Self::EditMessageText(payload) => webhook_envelope("editMessageText", payload),
            Self::EditMessageMedia(payload) => webhook_envelope("editMessageMedia", payload),
        }
    }
}

impl IntoResponse for WebhookReply {
    fn into_response(self) -> Response {
        Json(self.to_json()).into_response()
    }
}

fn webhook_envelope<T>(method: &'static str, payload: &T) -> Value
where
    T: Serialize,
{
    let mut value = to_value(payload).expect("webhook reply payload must serialize");
    let object = value
        .as_object_mut()
        .expect("webhook reply payload must serialize to an object");
    object.insert("method".to_string(), Value::String(method.to_string()));
    Value::Object(object.clone())
}

#[cfg(test)]
mod tests {
    use super::WebhookReply;
    use crate::telegram::{
        AnswerInlineQueryRequest, EditMessageMediaRequest, EditMessageTextRequest,
        InputMediaPhoto, ParseMode,
    };

    #[test]
    fn serializes_method_envelope() {
        let answer = WebhookReply::AnswerInlineQuery(AnswerInlineQueryRequest {
            inline_query_id: "iq1".to_string(),
            results: Vec::new(),
            next_offset: None,
        });
        assert_eq!(answer.to_json()["method"], "answerInlineQuery");

        let edit_text = WebhookReply::EditMessageText(EditMessageTextRequest {
            inline_message_id: "msg-1".to_string(),
            text: "hello".to_string(),
            parse_mode: Some(ParseMode::Html),
        });
        assert_eq!(edit_text.to_json()["method"], "editMessageText");

        let edit_media = WebhookReply::EditMessageMedia(EditMessageMediaRequest {
            inline_message_id: "msg-1".to_string(),
            media: InputMediaPhoto::new(
                "https://example.com/photo.jpg".to_string(),
                Some("caption".to_string()),
                Some(ParseMode::Html),
            ),
        });
        assert_eq!(edit_media.to_json()["method"], "editMessageMedia");
    }
}
