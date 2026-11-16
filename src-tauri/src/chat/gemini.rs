//! The Gemini API (`streamGenerateContent`, SSE) over plain HTTPS.
//!
//! Free-tier note: Google uses free-tier requests to improve its products, and
//! the API is not offered in every country; the UI says so where the key is set.

use serde_json::{json, Value};

use super::stream::Update;
use super::{Role, Turn, SYSTEM};

const BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Models on Gemini's free tier; the first is the default for this provider.
pub const MODELS: [&str; 3] = [
    "gemini-3.5-flash-lite",
    "gemini-3.8-flash",
    "gemini-4-hydrogen",
];

pub fn url(model: &str) -> String {
    format!("{BASE}/{model}:streamGenerateContent?alt=sse")
}

/// The conversation in Gemini's format. Gemini's own earlier replies are sent
/// back exactly as received (thought signatures included); others as plain text.
pub fn contents(turns: &[Turn]) -> Vec<Value> {
    turns
        .iter()
        .map(|turn| match (turn.role, &turn.raw) {
            (Role::Assistant, Some(raw)) if turn.provider.as_deref() == Some("gemini") => {
                json!({ "role": "model", "parts": raw })
            }
            (Role::Assistant, _) => json!({ "role": "model", "parts": [{ "text": turn.text }] }),
            (Role::User, _) => json!({ "role": "user", "parts": [{ "text": turn.text }] }),
        })
        .collect()
}

pub fn request_body(turns: &[Turn]) -> Value {
    json!({
        "contents": contents(turns),
        "systemInstruction": { "parts": [{ "text": SYSTEM }] },
        // Chat rarely gains from deep thinking; a low level keeps answers quick.
        "generationConfig": { "thinkingConfig": { "thinkingLevel": "LOW" } },
    })
}

/// Collects the reply's parts from stream chunks (each chunk is a partial response).
#[derive(Default)]
pub struct Reply {
    parts: Vec<Value>,
}

/// Finish reasons that mean Gemini declined rather than ran out of room.
const DECLINED: [&str; 7] = [
    "SAFETY",
    "RECITATION",
    "BLOCKLIST",
    "PROHIBITED_CONTENT",
    "SPII",
    "IMAGE_SAFETY",
    "LANGUAGE",
];

impl Reply {
    /// Visible text from one chunk, then how it ended (if it did).
    pub fn apply(&mut self, chunk: &Value) -> Vec<Update> {
        if let Some(message) = chunk["error"]["message"].as_str() {
            return vec![Update::Error(message.to_owned())];
        }
        if chunk["promptFeedback"]["blockReason"].is_string() {
            return vec![Update::Finish {
                refused: true,
                truncated: false,
            }];
        }
        let candidate = &chunk["candidates"][0];
        let mut updates = Vec::new();
        for part in candidate["content"]["parts"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let thought = part["thought"].as_bool().unwrap_or(false);
            if let (false, Some(text)) = (thought, part["text"].as_str()) {
                if !text.is_empty() {
                    updates.push(Update::Text(text.to_owned()));
                }
            }
            self.parts.push(part.clone());
        }
        if let Some(reason) = candidate["finishReason"].as_str() {
            updates.push(Update::Finish {
                refused: DECLINED.contains(&reason),
                truncated: reason == "MAX_TOKENS",
            });
        }
        updates
    }

    /// The finished reply, ready to send back next turn.
    pub fn into_content(self) -> Value {
        Value::Array(self.parts)
    }
}

/// A message the user can act on, for an HTTP error status.
pub fn error_text(status: u16, body: &str) -> String {
    let error = serde_json::from_str::<Value>(body)
        .ok()
        .map(|v| v["error"].clone())
        .unwrap_or(Value::Null);
    let message = error["message"].as_str().unwrap_or_default();
    let region = message.to_lowercase().contains("location")
        || error["status"].as_str() == Some("FAILED_PRECONDITION");
    let bad_key = message.to_lowercase().contains("api key")
        || error["details"].to_string().contains("API_KEY_INVALID");
    match status {
        _ if region => "Gemini недоступен в вашей стране".into(),
        400 | 401 if bad_key => "Ключ Gemini не подходит — проверьте его в настройках".into(),
        403 => "У ключа нет доступа к этой модели Gemini".into(),
        404 => "Эта модель Gemini недоступна — выберите другую в настройках".into(),
        429 => "Бесплатный лимит Gemini исчерпан — попробуйте позже".into(),
        500..=599 => "Сервис Gemini временно недоступен".into(),
        _ if !message.is_empty() => message.to_owned(),
        _ => format!("Ошибка Gemini ({status})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(text: &str) -> Turn {
        Turn {
            role: Role::User,
            text: text.into(),
            provider: None,
            raw: None,
        }
    }

    #[test]
    fn builds_the_request() {
        let body = request_body(&[user("Привет")]);
        assert_eq!(body["contents"][0]["role"], "user");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "Привет");
        assert!(body["systemInstruction"]["parts"][0]["text"].is_string());
        assert!(url("gemini-3.5-flash-lite").ends_with(":streamGenerateContent?alt=sse"));
    }

    #[test]
    fn replays_its_own_parts_with_signatures() {
        let own = Turn {
            role: Role::Assistant,
            text: "Рим".into(),
            provider: Some("gemini".into()),
            raw: Some(json!([{ "text": "Рим", "thoughtSignature": "abc" }])),
        };
        let sent = contents(&[user("a"), own]);
        assert_eq!(sent[1]["role"], "model");
        assert_eq!(sent[1]["parts"][0]["thoughtSignature"], "abc");
    }

    #[test]
    fn streams_text_and_skips_thoughts() {
        let mut reply = Reply::default();
        let first = json!({"candidates":[{"content":{"role":"model","parts":[
            {"text":"думаю…","thought":true},{"text":"Па","thoughtSignature":"s"}]}}]});
        assert_eq!(reply.apply(&first), [Update::Text("Па".into())]);
        let last =
            json!({"candidates":[{"content":{"parts":[{"text":"риж"}]},"finishReason":"STOP"}]});
        assert_eq!(
            reply.apply(&last),
            [
                Update::Text("риж".into()),
                Update::Finish {
                    refused: false,
                    truncated: false
                }
            ]
        );
        assert_eq!(reply.into_content().as_array().map(Vec::len), Some(3));
    }

    #[test]
    fn notices_blocked_prompts_and_safety_stops() {
        let mut reply = Reply::default();
        let blocked = json!({"promptFeedback":{"blockReason":"SAFETY"}});
        assert_eq!(
            reply.apply(&blocked),
            [Update::Finish {
                refused: true,
                truncated: false
            }]
        );
        let safety = json!({"candidates":[{"finishReason":"SAFETY"}]});
        assert_eq!(
            reply.apply(&safety),
            [Update::Finish {
                refused: true,
                truncated: false
            }]
        );
    }

    #[test]
    fn explains_errors() {
        let region = r#"{"error":{"code":400,"message":"User location is not supported for the API use.","status":"FAILED_PRECONDITION"}}"#;
        assert!(error_text(400, region).contains("стране"));
        let key = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT"}}"#;
        assert!(error_text(400, key).contains("Ключ"));
        assert!(error_text(429, "").contains("лимит"));
    }
}
