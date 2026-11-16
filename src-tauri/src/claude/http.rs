//! Minimal HTTP/1.1 request parsing for the local hook endpoint — just enough for
//! `curl -X POST --data-binary @-`, no dependency needed.

use std::io::{BufRead, BufReader, Read};

/// Named before the rename to Cuba; hooks already in Claude Code's settings post here.
pub const PATH: &str = "/escape/claude-code";
const MAX_BODY: usize = 256 * 1024;
const MAX_HEADER_LINES: usize = 64;

#[derive(Debug, PartialEq, Eq)]
pub enum Rejection {
    BadRequest,
    NotFound,
    /// Not JSON, or sent by a web page (has `Origin`): browsers cannot send this
    /// combination cross-site without a preflight, which we never answer.
    Forbidden,
    TooLarge,
}

impl Rejection {
    pub fn status_line(&self) -> &'static str {
        match self {
            Self::BadRequest => "400 Bad Request",
            Self::NotFound => "404 Not Found",
            Self::Forbidden => "403 Forbidden",
            Self::TooLarge => "413 Payload Too Large",
        }
    }
}

/// Reads one request and returns its body if it is an acceptable hook POST.
pub fn read_hook_request(stream: impl Read) -> Result<Vec<u8>, Rejection> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|_| Rejection::BadRequest)?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next(), parts.next());
    if method != Some("POST") {
        return Err(Rejection::NotFound);
    }
    if path != Some(PATH) {
        return Err(Rejection::NotFound);
    }

    let mut length = None;
    let mut json = false;
    for _ in 0..MAX_HEADER_LINES {
        line.clear();
        reader
            .read_line(&mut line)
            .map_err(|_| Rejection::BadRequest)?;
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else {
            return Err(Rejection::BadRequest);
        };
        let (name, value) = (name.trim().to_ascii_lowercase(), value.trim());
        match name.as_str() {
            "content-length" => {
                length = Some(value.parse::<usize>().map_err(|_| Rejection::BadRequest)?);
            }
            "content-type" => json = value.to_ascii_lowercase().starts_with("application/json"),
            "origin" => return Err(Rejection::Forbidden),
            _ => {}
        }
    }
    if !json {
        return Err(Rejection::Forbidden);
    }
    let length = length.ok_or(Rejection::BadRequest)?;
    if length > MAX_BODY {
        return Err(Rejection::TooLarge);
    }
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .map_err(|_| Rejection::BadRequest)?;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(head: &str, body: &str) -> Vec<u8> {
        format!("{head}\r\nContent-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
    }

    #[test]
    fn accepts_a_curl_post() {
        let raw = request(
            "POST /escape/claude-code HTTP/1.1\r\nHost: 127.0.0.1:47821\r\nContent-Type: application/json",
            r#"{"a":1}"#,
        );
        assert_eq!(read_hook_request(&raw[..]).unwrap(), br#"{"a":1}"#);
    }

    #[test]
    fn rejects_browsers_wrong_paths_and_types() {
        let from_page = request(
            "POST /escape/claude-code HTTP/1.1\r\nContent-Type: application/json\r\nOrigin: https://evil.example",
            "{}",
        );
        assert_eq!(read_hook_request(&from_page[..]), Err(Rejection::Forbidden));

        let text = request(
            "POST /escape/claude-code HTTP/1.1\r\nContent-Type: text/plain",
            "{}",
        );
        assert_eq!(read_hook_request(&text[..]), Err(Rejection::Forbidden));

        let get = request("GET /escape/claude-code HTTP/1.1", "");
        assert_eq!(read_hook_request(&get[..]), Err(Rejection::NotFound));

        let other = request(
            "POST /other HTTP/1.1\r\nContent-Type: application/json",
            "{}",
        );
        assert_eq!(read_hook_request(&other[..]), Err(Rejection::NotFound));
    }

    #[test]
    fn rejects_huge_bodies() {
        let raw = format!(
            "POST {PATH} HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY + 1
        );
        assert_eq!(read_hook_request(raw.as_bytes()), Err(Rejection::TooLarge));
    }
}
