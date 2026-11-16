//! The Claude Messages API over plain HTTP (there is no official Rust SDK):
//! request body, Server-Sent Events parsing and assembling the streamed reply.

use serde_json::{json, Map, Value};

pub const URL: &str = "https://api.anthropic.com/v1/messages";
pub const API_VERSION: &str = "2026-10-21";
/// Lets Sonnet / Opus hand a declined request to another model (routed by category).
pub const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// Streaming, so a generous cap costs nothing unless used.
const MAX_TOKENS: u32 = 64_000;

/// Models offered in settings; the first is the default.
pub const MODELS: [&str; 3] = ["claude-haiku-5-5", "claude-sonnet-5-5", "claude-opus-5-5"];

const SYSTEM: &str = "You are Claude, built into Escape, a Dynamic Island for Windows. \
The user asks quick questions from a small panel at the top of the screen. \
Answer briefly and to the point, in the language the user writes in. \
Replies are shown as plain text: do not use Markdown headings, tables or bold; \
use short paragraphs and simple \"-\" lists when a list helps.";

/// Server-side refusal fallbacks exist for these models (not for Haiku).
pub fn has_fallbacks(model: &str) -> bool {
    model == "claude-sonnet-5-5" || model == "claude-opus-5-5"
}

/// The streaming request body for `history` (the conversation so far, ending with
/// the user's message). Blocks from earlier replies are sent back unchanged.
pub fn request_body(model: &str, history: &[Value]) -> Value {
    let mut body = json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "stream": true,
        "system": SYSTEM,
        // Caches the growing conversation, so each turn re-reads it cheaply.
        "cache_control": { "type": "ephemeral" },
        // Chat rarely gains from deep thinking; low effort keeps answers quick.
        "output_config": { "effort": "low" },
        "messages": history,
    });
    if has_fallbacks(model) {
        body["fallbacks"] = json!("default");
    }
    body
}

/// Splits a byte stream into SSE `data:` payloads, keeping partial events
/// (and partial UTF-8) until the rest arrives.
#[derive(Default)]
pub struct SseParser {
    buffer: Vec<u8>,
}

impl SseParser {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Value> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(end) = find_event_end(&self.buffer) {
            let raw: Vec<u8> = self.buffer.drain(..end.0 + end.1).collect();
            let text = String::from_utf8_lossy(&raw[..end.0]);
            let data: Vec<&str> = text
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(str::trim_start)
                .collect();
            if data.is_empty() {
                continue;
            }
            if let Ok(value) = serde_json::from_str(&data.join("\n")) {
                events.push(value);
            }
        }
        events
    }
}

/// Position and length of the first blank-line separator (`\n\n` or `\r\n\r\n`).
fn find_event_end(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer.windows(2).position(|w| w == b"\n\n").map(|i| (i, 2));
    let crlf = buffer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| (i, 4));
    match (lf, crlf) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
        (a, b) => a.or(b),
    }
}

/// What a stream event means for the conversation.
#[derive(Debug, PartialEq)]
pub enum Update {
    /// New visible text.
    Text(String),
    /// Why the model stopped (`end_turn`, `max_tokens`, `refusal`, …).
    Stop(String),
    /// An error event inside the stream (e.g. overloaded).
    Error(String),
    None,
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
                Some(reason) => Update::Stop(reason.to_owned()),
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
    pub fn into_content(self) -> Vec<Value> {
        self.blocks
            .into_iter()
            .filter(|b| !b.is_null())
            // A thinking block without its signature (cut short) cannot be replayed.
            .filter(|b| b["type"] != "thinking" || b.get("signature").is_some())
            .collect()
    }

    /// Only the text, for a reply that was stopped midway.
    pub fn into_text_only(self) -> Vec<Value> {
        let text: String = self
            .blocks
            .iter()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect();
        if text.is_empty() {
            Vec::new()
        } else {
            vec![json!({ "type": "text", "text": text })]
        }
    }
}

fn append(block: &mut Map<String, Value>, field: &str, more: &str) {
    let current = block.get(field).and_then(Value::as_str).unwrap_or_default();
    let joined = format!("{current}{more}");
    block.insert(field.into(), Value::String(joined));
}

/// Visible text of a message's content (a string or a list of blocks).
pub fn text_of(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|b| b["type"] == "text")
            .filter_map(|b| b["text"].as_str())
            .collect(),
        _ => String::new(),
    }
}

/// A message the user can act on, for an HTTP error status.
pub fn error_text(status: u16, body: &str) -> String {
    let api_message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v["error"]["message"].as_str().map(str::to_owned));
    match status {
        401 => "Ключ не подходит — проверьте его в настройках".into(),
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

    #[test]
    fn builds_the_request() {
        let history = vec![json!({ "role": "user", "content": "Привет" })];
        let body = request_body("claude-haiku-5-5", &history);
        assert_eq!(body["stream"], true);
        assert_eq!(body["messages"][0]["content"], "Привет");
        assert_eq!(body["output_config"]["effort"], "low");
        assert!(body.get("fallbacks").is_none());
        assert_eq!(
            request_body("claude-opus-5-5", &history)["fallbacks"],
            "default"
        );
    }

    #[test]
    fn parses_events_split_across_chunks() {
        let mut parser = SseParser::default();
        let first = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"При";
        assert!(parser.push(first.as_bytes()).is_empty());
        // The second chunk starts mid-character: "и" is split between chunks.
        let rest = "вет\"}}\r\n\r\nevent: ping\ndata: {\"type\":\"ping\"}\n\n";
        let bytes = rest.as_bytes();
        let events = parser.push(bytes);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["delta"]["text"], "Привет");
        assert_eq!(events[1]["type"], "ping");
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
            json!({"type":"message_delta","delta":{"stop_reason":"end_turn"}}),
        ];
        let updates: Vec<Update> = events.iter().map(|e| reply.apply(e)).collect();
        assert_eq!(updates[3], Update::Text("Па".into()));
        assert_eq!(updates[5], Update::Stop("end_turn".into()));
        let content = reply.into_content();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["signature"], "sig");
        assert_eq!(text_of(&Value::Array(content)), "Париж");
    }

    #[test]
    fn keeps_only_text_when_stopped() {
        let mut reply = Reply::default();
        reply.apply(&json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}));
        assert!(reply.into_text_only().is_empty());
    }

    #[test]
    fn explains_errors() {
        assert!(error_text(401, "").contains("Ключ"));
        let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"bad"}}"#;
        assert_eq!(error_text(400, body), "bad");
        assert!(error_text(529, "").contains("перегружен"));
    }
}
