//! The Claude Messages API over plain HTTPS (there is no official Rust SDK).

use serde_json::{json, Map, Value};

use super::stream::Update;
use super::{Role, Turn, SYSTEM};

pub const URL: &str = "https://api.anthropic.com/v1/messages";
pub const API_VERSION: &str = "2026-10-21";
/// Lets Sonnet / Opus hand a declined request to another model (routed by category).
pub const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// Streaming, so a generous cap costs nothing unless used.
const MAX_TOKENS: u32 = 64_000;

/// Models offered in settings; the first is the app's default.
pub const MODELS: [&str; 3] = ["claude-haiku-5-5", "claude-sonnet-5-5", "claude-opus-5-5"];

/// Server-side refusal fallbacks exist for these models (not for Haiku).
pub fn has_fallbacks(model: &str) -> bool {
    model == "claude-sonnet-5-5" || model == "claude-opus-5-5"
}

/// The conversation in Claude's format. Claude's own earlier replies are sent
/// back exactly as received (thinking blocks included); others as plain text.
pub fn messages(turns: &[Turn]) -> Vec<Value> {
    turns
        .iter()
        .map(|turn| match (turn.role, &turn.raw) {
            (Role::Assistant, Some(raw)) if turn.provider.as_deref() == Some("claude") => {
                json!({ "role": "assistant", "content": raw })
            }
            (Role::Assistant, _) => json!({ "role": "assistant", "content": turn.text }),
            (Role::User, _) => json!({ "role": "user", "content": turn.text }),
        })
        .collect()
}

/// The streaming request body.
pub fn request_body(model: &str, turns: &[Turn]) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "system": SYSTEM,
        // Caches the growing conversation, so each turn re-reads it cheaply.
        "cache_control": { "type": "ephemeral" },
        // Chat rarely gains from deep thinking; low effort keeps answers quick.
        "output_config": { "effort": "low" },
        "messages": messages(turns),
    });
    if has_fallbacks(model) {
        body["fallbacks"] = json!("default");
    }
    body
}

/// Collects the reply's content blocks from stream events.
#[derive(Default)]
pub struct Reply {
    blocks: Vec<Value>,
}

impl Reply {
    pub fn apply(&mut self, event: &Value) -> Update {
        match event["type"].as_str() {
            Some("content_block_start") => {
                let index = event["index"].as_u64().unwrap_or(0) as usize;
                if self.blocks.len() <= index {
                    self.blocks.resize(index + 1, Value::Null);
                }
                let block = event["content_block"].clone();
                let text = block["text"].as_str().unwrap_or_default().to_owned();
                self.blocks[index] = block;
                if text.is_empty() {
                    Update::None
                } else {
                    Update::Text(text)
                }
            }
            Some("content_block_delta") => {
                let index = event["index"].as_u64().unwrap_or(0) as usize;
                let Some(block) = self.blocks.get_mut(index).and_then(Value::as_object_mut) else {
                    return Update::None;
                };
                let delta = &event["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => {
                        let text = delta["text"].as_str().unwrap_or_default();
                        append(block, "text", text);
                        Update::Text(text.to_owned())
                    }
                    Some("thinking_delta") => {
                        append(
                            block,
                            "thinking",
                            delta["thinking"].as_str().unwrap_or_default(),
                        );
                        Update::None
                    }
                    Some("signature_delta") => {
                        block.insert("signature".into(), delta["signature"].clone());
                        Update::None
                    }
                    _ => Update::None,
                }
            }
            Some("message_delta") => match event["delta"]["stop_reason"].as_str() {
                Some(reason) => Update::Finish {
                    refused: reason == "refusal",
                    truncated: reason == "max_tokens",
                },
                None => Update::None,
            },
            Some("error") => Update::Error(
                event["error"]["message"]
                    .as_str()
                    .unwrap_or("ошибка Claude")
                    .to_owned(),
            ),
            _ => Update::None,
        }
    }

    /// The finished reply, ready to send back next turn.
    pub fn into_content(self) -> Value {
        Value::Array(
            self.blocks
                .into_iter()
                .filter(|b| !b.is_null())
                // A thinking block without its signature (cut short) cannot be replayed.
                .filter(|b| b["type"] != "thinking" || b.get("signature").is_some())
                .collect(),
        )
    }
}

fn append(block: &mut Map<String, Value>, field: &str, more: &str) {
    let current = block.get(field).and_then(Value::as_str).unwrap_or_default();
    let joined = format!("{current}{more}");
    block.insert(field.into(), Value::String(joined));
}

/// A message the user can act on, for an HTTP error status.
pub fn error_text(status: u16, body: &str) -> String {
    let api_message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_owned));
    match status {
        401 => "Ключ Anthropic не подходит — проверьте его в настройках".into(),
        403 => "У ключа нет доступа к этой модели".into(),
        429 => "Слишком много запросов — попробуйте через минуту".into(),
        529 => "Claude сейчас перегружен — попробуйте чуть позже".into(),
        500..=599 => "Сервис Claude временно недоступен".into(),
        _ => api_message.unwrap_or_else(|| format!("Ошибка Claude ({status})")),
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
        let body = request_body("claude-haiku-5-5", &[user("Привет")]);
        assert_eq!(body["stream"], true);
        assert_eq!(
            body["messages"][0],
            json!({ "role": "user", "content": "Привет" })
        );
        assert_eq!(body["output_config"]["effort"], "low");
        assert!(body.get("fallbacks").is_none());
        assert_eq!(
            request_body("claude-opus-5-5", &[user("x")])["fallbacks"],
            "default"
        );
    }

    #[test]
    fn replays_its_own_replies_and_plain_text_of_others() {
        let own = Turn {
            role: Role::Assistant,
            text: "Париж".into(),
            provider: Some("claude".into()),
            raw: Some(
                json!([{ "type": "thinking", "signature": "s" }, { "type": "text", "text": "Париж" }]),
            ),
        };
        let other = Turn {
            role: Role::Assistant,
            text: "Рим".into(),
            provider: Some("gemini".into()),
            raw: Some(json!([{ "text": "Рим" }])),
        };
        let sent = messages(&[user("a"), own, user("b"), other]);
        assert_eq!(sent[1]["content"][0]["signature"], "s");
        assert_eq!(sent[3], json!({ "role": "assistant", "content": "Рим" }));
    }

    #[test]
    fn assembles_the_reply_with_thinking_kept() {
        let mut reply = Reply::default();
        let events = [
            json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig"}}),
            json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Па"}}),
            json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"риж"}}),
            json!({"type":"message_delta","delta":{"stop_reason":"refusal"}}),
        ];
        let updates: Vec<Update> = events.iter().map(|e| reply.apply(e)).collect();
        assert_eq!(updates[3], Update::Text("Па".into()));
        assert_eq!(
            updates[5],
            Update::Finish {
                refused: true,
                truncated: false
            }
        );
        let content = reply.into_content();
        assert_eq!(content[0]["signature"], "sig");
        assert_eq!(content[1]["text"], "Париж");
    }

    #[test]
    fn explains_errors() {
        assert!(error_text(401, "").contains("Ключ"));
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"bad"}}"#;
        assert_eq!(error_text(400, body), "bad");
        assert!(error_text(529, "").contains("перегружен"));
    }
}
