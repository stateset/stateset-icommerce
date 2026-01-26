//! Service modules for external integrations

#[cfg(feature = "embeddings")]
pub mod embeddings;

#[cfg(feature = "embeddings")]
pub use embeddings::*;
