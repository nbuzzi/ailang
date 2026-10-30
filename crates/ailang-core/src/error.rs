use thiserror::Error;

#[derive(Debug, Error)]
pub enum AilangError {
    #[error("Lexer error at position {pos}: {msg}")]
    LexError { pos: usize, msg: String },

    #[error("Parse error: {msg}")]
    ParseError { msg: String },

    #[error("Runtime error in '{stage}': {msg}")]
    RuntimeError { stage: String, msg: String },

    #[error("Type error: expected {expected}, got {got}")]
    TypeError { expected: String, got: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("Regex error: {0}")]
    RegexError(String),

    #[error("Assertion failed: expected '{expected}', got '{got}'")]
    AssertionError { expected: String, got: String },

    #[error("Unknown function: '{0}'")]
    UnknownFunction(String),

    #[error("Unknown variable: '{0}'")]
    UnknownVariable(String),

    #[error("Security error: {msg}")]
    SecurityError { msg: String },

    #[error("Edit error: {msg}")]
    EditError { msg: String },
}

/// Structured error output — what the AI agent receives on failure
/// This is designed to be parseable, not human-readable.
#[derive(Debug, serde::Serialize)]
pub struct ErrorOutput {
    pub error: String,
    pub code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
}

impl From<&AilangError> for ErrorOutput {
    fn from(e: &AilangError) -> Self {
        let code = match e {
            AilangError::LexError { .. }       => 1,
            AilangError::ParseError { .. }     => 2,
            AilangError::RuntimeError { .. }   => 3,
            AilangError::TypeError { .. }      => 4,
            AilangError::IoError(_)            => 5,
            AilangError::HttpError(_)          => 6,
            AilangError::RegexError(_)         => 7,
            AilangError::AssertionError { .. } => 8,
            AilangError::UnknownFunction(_)    => 9,
            AilangError::UnknownVariable(_)    => 10,
            AilangError::SecurityError { .. }  => 11,
            AilangError::EditError { .. }      => 12,
        };
        let stage = match e {
            AilangError::RuntimeError { stage, .. } => Some(stage.clone()),
            _ => None,
        };
        ErrorOutput {
            error: e.to_string(),
            code,
            stage,
        }
    }
}

pub type AilangResult<T> = Result<T, AilangError>;
