//! AI chat with the user's own API key: Claude (Anthropic) or Gemini (Google)
//! (PLAN.md stage 6).
//!
//! Network traffic goes only to the chosen provider's API, and only when the
//! user sends a message. Keys stay in Rust (Credential Manager); the page gets
//! the streamed text through events. One conversation is kept (`chat.json` in
//! the app data folder) until the user starts a new one. Each reply is stored
//! with its provider's raw content, which is sent back unchanged to the same
//! provider (thinking blocks / thought signatures) and as plain text to the other.

mod claude;
mod gemini;
#[cfg(windows)]
mod key;
mod stream;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{AppError, AppResult};
use stream::Update;

/// Longest message the user can send (characters).
const MAX_INPUT: usize = 20_000;

/// Shared by both providers.
const SYSTEM: &str = "You are an AI assistant built into Escape, a Dynamic Island for Windows. \
The user asks quick questions from a small panel at the top of the screen. \
Answer briefly and to the point, in the language the user writes in. \
Replies are shown as plain text: do not use Markdown headings, tables or bold; \
use short paragraphs and simple \"-\" lists when a list helps.";

/// Every model offered in settings; the first is the app's default.
pub const MODELS: [&str; 6] = [
    claude::MODELS[0],
    claude::MODELS[1],
    claude::MODELS[2],
    gemini::MODELS[0],
    gemini::MODELS[1],
    gemini::MODELS[2],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    Claude,
    Gemini,
}

impl Provider {
    fn of_model(model: &str) -> Self {
        if model.starts_with("gemini-") {
            Self::Gemini
        } else {
            Self::Claude
        }
    }

    fn parse(id: &str) -> AppResult<Self> {
        match id {
            "claude" => Ok(Self::Claude),
            "gemini" => Ok(Self::Gemini),
            other => Err(AppError::InvalidArgument(format!(
                "неизвестный сервис {other}"
            ))),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Gemini => "gemini",
        }
    }

    fn host(self) -> &'static str {
        match self {
            Self::Claude => "api.anthropic.com",
            Self::Gemini => "generativelanguage.googleapis.com",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// One message of the stored conversation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Turn {
    pub role: Role,
    pub text: String,
    /// Who wrote an assistant reply, and its content exactly as received.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

pub struct ChatShared {
    file: PathBuf,
    history: Mutex<Vec<Turn>>,
    streaming: AtomicBool,
    cancel: AtomicBool,
}

type Shared = Arc<ChatShared>;

impl ChatShared {
    fn history(&self) -> std::sync::MutexGuard<'_, Vec<Turn>> {
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
    pub role: Role,
    pub text: String,
}

/// Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub streaming: bool,
    /// The provider of the model chosen in settings.
    pub provider: &'static str,
    /// A key for that provider is saved.
    pub has_key: bool,
}

/// End of a reply. Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatDone {
    /// Something went wrong; the text says what.
    pub error: Option<String>,
    /// The model declined to answer.
    pub refused: bool,
    /// The answer was cut at the length limit.
    pub truncated: bool,
}

#[cfg(windows)]
fn load_key(provider: Provider) -> Option<String> {
    key::load(provider)
}

#[cfg(not(windows))]
fn load_key(_provider: Provider) -> Option<String> {
    None
}

pub fn start(app: &AppHandle) -> AppResult<()> {
    // reqwest is built without a default TLS crypto provider (as in the updater).
    let _ = rustls::crypto::ring::default_provider().install_default();
    let file = app.path().app_data_dir()?.join("chat.json");
    let history = std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| serde_json::from_str::<Vec<Turn>>(&text).ok())
        .unwrap_or_default();
    app.manage(Arc::new(ChatShared {
        file,
        history: Mutex::new(history),
        streaming: AtomicBool::new(false),
        cancel: AtomicBool::new(false),
    }));
    Ok(())
}

fn state(app: &AppHandle, shared: &ChatShared) -> ChatState {
    let provider = Provider::of_model(&crate::settings::chat_model(app));
    ChatState {
        messages: shared
            .history()
            .iter()
            .filter(|t| !t.text.is_empty())
            .map(|t| ChatMessage {
                role: t.role,
                text: t.text.clone(),
            })
            .collect(),
        streaming: shared.streaming.load(Ordering::SeqCst),
        provider: provider.id(),
        has_key: load_key(provider).is_some(),
    }
}

fn emit<P: Serialize + Clone>(app: &AppHandle, event: &str, payload: P) {
    if let Err(err) = app.emit(event, payload) {
        log::warn!("failed to emit {event}: {err}");
    }
}

#[tauri::command]
pub fn chat_get(app: AppHandle, shared: State<'_, Shared>) -> AppResult<ChatState> {
    Ok(state(&app, &shared))
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
    let model = crate::settings::chat_model(&app);
    let provider = Provider::of_model(&model);
    let key = load_key(provider)
        .ok_or_else(|| AppError::Unavailable("сначала добавьте API-ключ в настройках".into()))?;
    if shared.streaming.swap(true, Ordering::SeqCst) {
        return Err(AppError::Unavailable("ответ ещё печатается".into()));
    }
    shared.cancel.store(false, Ordering::SeqCst);
    shared.history().push(Turn {
        role: Role::User,
        text,
        provider: None,
        raw: None,
    });
    shared.save();

    let history = shared.history().clone();
    let task_shared = Arc::clone(&shared);
    let task_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let done = stream_reply(&task_app, &task_shared, provider, &key, &model, &history).await;
        task_shared.streaming.store(false, Ordering::SeqCst);
        emit(&task_app, "chat://done", done);
    });
    Ok(state(&app, &shared))
}

/// The reply being assembled, per provider.
enum Reply {
    Claude(claude::Reply),
    Gemini(gemini::Reply),
}

impl Reply {
    fn apply(&mut self, event: &Value) -> Vec<Update> {
        match self {
            Self::Claude(reply) => vec![reply.apply(event)],
            Self::Gemini(reply) => reply.apply(event),
        }
    }

    fn into_content(self) -> Value {
        match self {
            Self::Claude(reply) => reply.into_content(),
            Self::Gemini(reply) => reply.into_content(),
        }
    }
}

async fn stream_reply(
    app: &AppHandle,
    shared: &ChatShared,
    provider: Provider,
    key: &str,
    model: &str,
    history: &[Turn],
) -> ChatDone {
    let mut done = ChatDone {
        error: None,
        refused: false,
        truncated: false,
    };
    let client = reqwest::Client::new();
    let request = match provider {
        Provider::Claude => {
            let mut request = client
                .post(claude::URL)
                .header("x-api-key", key)
                .header("anthropic-version", claude::API_VERSION)
                .json(&claude::request_body(model, history));
            if claude::has_fallbacks(model) {
                request = request.header("anthropic-beta", claude::FALLBACK_BETA);
            }
            request
        }
        Provider::Gemini => client
            .post(gemini::url(model))
            .header("x-goog-api-key", key)
            .json(&gemini::request_body(history)),
    };
    let mut response = match request.send().await {
        Ok(response) => response,
        Err(err) => {
            log::warn!("chat request failed: {err}");
            done.error = Some(format!("Нет связи с {}", provider.host()));
            return done;
        }
    };
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        log::warn!("chat request to {} returned {status}", provider.id());
        done.error = Some(match provider {
            Provider::Claude => claude::error_text(status.as_u16(), &body),
            Provider::Gemini => gemini::error_text(status.as_u16(), &body),
        });
        return done;
    }

    let mut parser = stream::SseParser::default();
    let mut reply = match provider {
        Provider::Claude => Reply::Claude(claude::Reply::default()),
        Provider::Gemini => Reply::Gemini(gemini::Reply::default()),
    };
    let mut text = String::new();
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
            for update in reply.apply(&event) {
                match update {
                    Update::Text(delta) => {
                        text.push_str(&delta);
                        emit(app, "chat://delta", delta);
                    }
                    Update::Finish { refused, truncated } => {
                        done.refused = refused;
                        done.truncated = truncated;
                    }
                    Update::Error(message) => done.error = Some(message),
                    Update::None => {}
                }
            }
        }
    }

    if !text.is_empty() {
        // A stopped or broken reply keeps only its text: unfinished thinking can't be replayed.
        let complete = !cancelled && done.error.is_none();
        shared.history().push(Turn {
            role: Role::Assistant,
            text,
            provider: complete.then(|| provider.id().to_owned()),
            raw: complete.then(|| reply.into_content()),
        });
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
pub fn chat_new(app: AppHandle, shared: State<'_, Shared>) -> AppResult<ChatState> {
    if shared.streaming.load(Ordering::SeqCst) {
        return Err(AppError::Unavailable("ответ ещё печатается".into()));
    }
    shared.history().clear();
    shared.save();
    Ok(state(&app, &shared))
}

/// Saves (or, with an empty string, removes) the API key of `provider`.
#[tauri::command]
pub fn chat_set_key(
    provider: String,
    key: String,
    app: AppHandle,
    shared: State<'_, Shared>,
) -> AppResult<ChatState> {
    let provider = Provider::parse(&provider)?;
    let key = key.trim();
    if key.chars().any(char::is_whitespace) {
        return Err(AppError::InvalidArgument(
            "в ключе не должно быть пробелов".into(),
        ));
    }
    if provider == Provider::Claude && !key.is_empty() && !key.starts_with("sk-ant-") {
        return Err(AppError::InvalidArgument(
            "это не похоже на ключ Anthropic (он начинается с sk-ant-)".into(),
        ));
    }
    #[cfg(windows)]
    {
        if key.is_empty() {
            self::key::delete(provider);
        } else {
            self::key::save(provider, key)?;
        }
    }
    Ok(state(&app, &shared))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_client_starts_with_our_tls_setup() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        assert!(reqwest::Client::builder().build().is_ok());
    }

    #[test]
    fn picks_the_provider_by_model() {
        assert_eq!(
            Provider::of_model("gemini-3.5-flash-lite"),
            Provider::Gemini
        );
        assert_eq!(Provider::of_model("claude-haiku-5-5"), Provider::Claude);
        assert!(Provider::parse("openai").is_err());
    }

    #[test]
    fn stored_turns_round_trip() {
        let turn = Turn {
            role: Role::Assistant,
            text: "Париж".into(),
            provider: Some("claude".into()),
            raw: Some(serde_json::json!([{ "type": "text", "text": "Париж" }])),
        };
        let json = serde_json::to_string(&[turn]).unwrap();
        let back: Vec<Turn> = serde_json::from_str(&json).unwrap();
        assert_eq!(back[0].role, Role::Assistant);
        assert_eq!(back[0].raw.as_ref().unwrap()[0]["text"], "Париж");
        // A user turn stores only its text.
        assert!(!serde_json::to_string(&Turn {
            role: Role::User,
            text: "x".into(),
            provider: None,
            raw: None
        })
        .unwrap()
        .contains("raw"));
    }
}
