//! Common error type for all Tauri commands.
//!
//! Every command returns [`AppResult<T>`]. On the frontend an error arrives as
//! `{ kind, message }` (see `src/ipc/commands.ts`) and is shown as a toast.

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),

    #[error("ошибка Tauri: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("неверный аргумент: {0}")]
    InvalidArgument(String),

    #[error("{0}")]
    Internal(String),
}

impl AppError {
    /// Stable machine-readable error kind, mirrored in `AppErrorKind` on the frontend.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Tauri(_) => "tauri",
            Self::InvalidArgument(_) => "invalid_argument",
            Self::Internal(_) => "internal",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_kind_and_message() {
        let err = AppError::InvalidArgument("rect.w < 0".into());
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "invalid_argument", "message": "неверный аргумент: rect.w < 0" })
        );
    }

    #[test]
    fn io_errors_convert_via_question_mark() {
        fn fails() -> AppResult<()> {
            Err(std::io::Error::other("disk"))?
        }
        assert_eq!(fails().unwrap_err().kind(), "io");
    }
}
