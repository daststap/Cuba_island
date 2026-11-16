//! AI chat with the user's own Anthropic API key (PLAN.md stage 6).
//!
//! The only network traffic is to `api.anthropic.com`, and only when the user
//! sends a message. The key stays in Rust (Credential Manager); the page gets
//! the streamed text through events. One conversation is kept (`chat.json` in
//! the app data folder) until the user starts a new one.

mod api;
#[cfg(windows)]
mod key;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{AppError, AppResult};
pub use api::MODELS;

/// Longest message the user can send (characters).
const MAX_INPUT: usize = 20_000;

pub struct ChatShared {
    file: PathBuf,
    /// API messages exactly as sent: `{role, content}`.
    history: Mutex<Vec<Value>>,
    streaming: AtomicBool,
    cancel: AtomicBool,
}

type Shared = Arc<ChatShared>;

impl ChatShared {
    fn history(&self) -> std::sync::MutexGuard<'_, Vec<Value>> {
        self.history.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn save(&self) {
        let text = serde_json::to_string(&*self.history()).unwrap_or_default();
        if let Err(err) = std::fs::write(&self.file, text) {
            log::warn!("failed to save the chat: {err}");
        }
    }
}

/// One message as the page shows it. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub text: String,
}

/// Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub streaming: bool,
    pub has_key: bool,
}

/// End of a reply. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatDone {
    /// Something went wrong; the text says what.
    pub error: Option<String>,
    /// Claude declined to answer.
    pub refused: bool,
    /// The answer was cut at the length limit.
    pub truncated: bool,
}

#[cfg(windows)]
fn load_key() -> Option<String> {
    key::load()
}

#[cfg(not(windows))]
fn load_key() -> Option<String> {
    None
}

pub fn start(app: &AppHandle) -> AppResult<()> {
    // reqwest is built without a default TLS crypto provider (as in the updater).
    let _ = rustls::crypto::ring::default_provider().install_default();
    let file = app.path().app_data_dir()?.join("chat.json");
    let history = std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<Value>>(&text).ok())
        .unwrap_or_default();
    app.manage(Arc::new(ChatShared {
        file,
        history: Mutex::new(history),
        streaming: AtomicBool::new(false),
        cancel: AtomicBool::new(false),
    }));
    Ok(())
}

fn state(shared: &ChatShared) -> ChatState {
    ChatState {
        messages: shared
            .history()
            .iter()
            .map(|m| ChatMessage {
                role: m["role"].as_str().unwrap_or("user").to_owned(),
                text: api::text_of(&m["content"]),
            })
            .filter(|m| !m.text.is_empty())
            .collect(),
        streaming: shared.streaming.load(Ordering::SeqCst),
        has_key: load_key().is_some(),
    }
}

fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(err) = app.emit(event, payload) {
        log::warn!("failed to emit {event}: {err}");
    }
}

#[tauri::command]
pub fn chat_get(shared: State<'_, Shared>) -> AppResult<ChatState> {
    Ok(state(&shared))
}

/// Sends the user's message; the reply streams as `chat://delta`, then `chat://done`.
#[tauri::command]
pub fn chat_send(text: String, app: AppHandle, shared: State<'_, Shared>) -> AppResult<ChatState> {
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Err(AppError::InvalidArgument("пустое сообщение".into()));
    }
    if text.chars().count() > MAX_INPUT {
        return Err(AppError::InvalidArgument(
            "слишком длинное сообщение".into(),
        ));
    }
    let key = load_key()
        .ok_or_else(|| AppError::Unavailable("сначала добавьте API-ключ в настройках".into()))?;
    if shared.streaming.swap(true, Ordering::SeqCst) {
        return Err(AppError::Unavailable("Claude ещё отвечает".into()));
    }
    shared.cancel.store(false, Ordering::SeqCst);
    shared
        .history()
        .push(json!({ "role": "user", "content": text }));
    shared.save();

    let model = crate::settings::chat_model(&app);
    let history = shared.history().clone();
    let task_shared = Arc::clone(&shared);
    tauri::async_runtime::spawn(async move {
        let done = stream_reply(&app, &task_shared, &key, &model, &history).await;
        task_shared.streaming.store(false, Ordering::SeqCst);
        emit(&app, "chat://done", done);
    });
    Ok(state(&shared))
}

async fn stream_reply(
    app: &AppHandle,
    shared: &ChatShared,
    key: &str,
    model: &str,
    history: &[Value],
) -> ChatDone {
    let mut done = ChatDone {
        error: None,
        refused: false,
        truncated: false,
    };
    let client = reqwest::Client::new();
    let mut request = client
        .post(api::URL)
        .header("x-api-key", key)
        .header("anthropic-version", api::API_VERSION)
        .header("content-type", "application/json")
        .json(&api::request_body(model, history));
    if api::has_fallbacks(model) {
        request = request.header("anthropic-beta", api::FALLBACK_BETA);
    }
    let mut response = match request.send().await {
        Ok(response) => response,
        Err(err) => {
            log::warn!("chat request failed: {err}");
            done.error = Some("Нет связи с api.anthropic.com".into());
            return done;
        }
    };
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        log::warn!("chat request returned {status}");
        done.error = Some(api::error_text(status.as_u16(), &body));
        return done;
    }

    let mut parser = api::SseParser::default();
    let mut reply = api::Reply::default();
    let mut cancelled = false;
    loop {
        if shared.cancel.load(Ordering::SeqCst) {
            cancelled = true;
            break;
        }
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(err) => {
                log::warn!("chat stream broke: {err}");
                done.error = Some("Связь оборвалась — ответ неполный".into());
                break;
            }
        };
        for event in parser.push(&chunk) {
            match reply.apply(&event) {
                api::Update::Text(text) => emit(app, "chat://delta", text),
                api::Update::Stop(reason) => {
                    done.refused = reason == "refusal";
                    done.truncated = reason == "max_tokens";
                }
                api::Update::Error(message) => done.error = Some(message),
                api::Update::None => {}
            }
        }
    }

    // A stopped or broken reply keeps only its text; thinking without its end can't be sent back.
    let content = if cancelled || done.error.is_some() {
        reply.into_text_only()
    } else {
        reply.into_content()
    };
    if !content.is_empty() {
        shared
            .history()
            .push(json!({ "role": "assistant", "content": content }));
        shared.save();
    }
    done
}

/// Stops the reply being streamed (what arrived so far is kept).
#[tauri::command]
pub fn chat_stop(shared: State<'_, Shared>) -> AppResult<()> {
    shared.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

/// Starts a new conversation.
#[tauri::command]
pub fn chat_new(shared: State<'_, Shared>) -> AppResult<ChatState> {
    if shared.streaming.load(Ordering::SeqCst) {
        return Err(AppError::Unavailable("Claude ещё отвечает".into()));
    }
    shared.history().clear();
    shared.save();
    Ok(state(&shared))
}

/// Saves (or, with an empty string, removes) the API key.
#[tauri::command]
pub fn chat_set_key(key: String, shared: State<'_, Shared>) -> AppResult<ChatState> {
    let key = key.trim();
    #[cfg(windows)]
    {
        if key.is_empty() {
            self::key::delete();
        } else if !key.starts_with("sk-ant-") {
            return Err(AppError::InvalidArgument(
                "это не похоже на ключ Anthropic (он начинается с sk-ant-)".into(),
            ));
        } else {
            self::key::save(key)?;
        }
    }
    #[cfg(not(windows))]
    let _ = key;
    Ok(state(&shared))
}

#[cfg(test)]
mod tests {
    #[test]
    fn https_client_starts_with_our_tls_setup() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        assert!(reqwest::Client::builder().build().is_ok());
    }
}
