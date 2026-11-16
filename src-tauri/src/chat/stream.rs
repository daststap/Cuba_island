//! Server-Sent Events, shared by both providers.

use serde_json::Value;

/// What a stream event means for the conversation.
#[derive(Debug, PartialEq)]
pub enum Update {
    /// New visible text.
    Text(String),
    /// The model finished; `refused` = declined to answer, `truncated` = hit the length limit.
    Finish {
        refused: bool,
        truncated: bool,
    },
    /// An error reported inside the stream (e.g. overloaded).
    Error(String),
    None,
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
        while let Some((at, len)) = find_event_end(&self.buffer) {
            let raw: Vec<u8> = self.buffer.drain(..at + len).collect();
            let text = String::from_utf8_lossy(&raw[..at]);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_events_split_across_chunks() {
        let mut parser = SseParser::default();
        let first = "event: content_block_delta\ndata: {\"delta\":{\"text\":\"При";
        assert!(parser.push(first.as_bytes()).is_empty());
        let rest = "вет\"}}\r\n\r\nevent: ping\ndata: {\"type\":\"ping\"}\n\n";
        let events = parser.push(rest.as_bytes());
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["delta"]["text"], "Привет");
        assert_eq!(events[1]["type"], "ping");
    }

    #[test]
    fn keeps_a_split_multibyte_character() {
        let mut parser = SseParser::default();
        let bytes = "data: {\"t\":\"ё\"}\n\n".as_bytes();
        // Split inside the two-byte "ё".
        let cut = bytes.iter().position(|&b| b >= 0x80).unwrap_or(0) + 1;
        assert!(parser.push(&bytes[..cut]).is_empty());
        assert_eq!(parser.push(&bytes[cut..])[0]["t"], "ё");
    }
}
