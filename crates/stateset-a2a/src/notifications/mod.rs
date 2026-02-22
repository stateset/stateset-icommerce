//! Webhook notification service types and logic.
//!
//! Provides HMAC-SHA256 signed webhook delivery and SSRF validation.

pub mod hmac;
pub mod ssrf;

pub use self::hmac::{sign_webhook, verify_webhook};
pub use ssrf::validate_url;
