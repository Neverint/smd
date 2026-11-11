//! Typed errors returned by every backend command.
//!
//! The frontend receives `{ code, message, details }`: `code` picks the friendly
//! translated text, `details` goes behind the "Copy details" button.

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
#[allow(
    dead_code,
    reason = "variants are used by the module commands from Phase 1 on"
)]
pub enum AppError {
    #[error("access denied: {0}")]
    AccessDenied(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unsupported on this system: {0}")]
    Unsupported(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    fn code(&self) -> &'static str {
        match self {
            AppError::AccessDenied(_) => "accessDenied",
            AppError::NotFound(_) => "notFound",
            AppError::Unsupported(_) => "unsupported",
            AppError::Io(_) => "io",
            AppError::Internal(_) => "internal",
        }
    }
}

#[derive(Serialize)]
struct WireError<'a> {
    code: &'a str,
    message: String,
    details: String,
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        WireError {
            code: self.code(),
            message: self.to_string(),
            details: format!("{self:?}"),
        }
        .serialize(serializer)
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_code() {
        let json = serde_json::to_value(AppError::NotFound("C:\\x".into())).unwrap();
        assert_eq!(json["code"], "notFound");
        assert_eq!(json["message"], "not found: C:\\x");
    }
}
