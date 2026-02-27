//! Webhook notification service types and logic.
//!
//! Provides HMAC-SHA256 signed webhook delivery and SSRF validation.

pub mod hmac;
pub mod ssrf;

pub use self::hmac::{
    WebhookHmacError, sign_webhook, try_sign_webhook, try_verify_webhook, verify_webhook,
};
pub use ssrf::{UrlValidationOptions, validate_url, validate_url_with_options};
