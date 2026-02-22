use thiserror::Error;

/// Errors that can occur during policy evaluation, loading, or configuration.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PolicyError {
    /// The operator string does not match any known [`Operator`](crate::Operator) variant.
    #[error("Unknown operator: {0}")]
    UnknownOperator(String),

    /// A condition definition is malformed (e.g., missing required fields).
    #[error("Invalid condition: {0}")]
    InvalidCondition(String),

    /// No policy sets are registered for the requested domain.
    #[error("Policy set not found for domain: {0}")]
    DomainNotFound(String),

    /// A policy file could not be loaded from disk.
    #[error("Failed to load policy file {path}: {message}")]
    LoadError {
        /// Filesystem path that was attempted.
        path: String,
        /// Human-readable description of what went wrong.
        message: String,
    },

    /// YAML parsing failed.
    #[error("YAML parse error: {0}")]
    YamlError(String),

    /// JSON parsing failed.
    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Filesystem I/O failed.
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// A regular expression was invalid.
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),

    /// A generic evaluation-time error.
    #[error("Policy evaluation error: {0}")]
    EvaluationError(String),
}

/// Convenience alias used throughout this crate.
pub type Result<T> = std::result::Result<T, PolicyError>;
