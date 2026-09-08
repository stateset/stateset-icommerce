//! Stable machine-readable error codes at the Node binding boundary.
//!
//! Before this module every failure crossed into JavaScript as
//! `Error::from_reason(..)`, which napi stamps with `Status::GenericFailure`.
//! JavaScript callers saw `err.code === 'GenericFailure'` for a missing cart, a
//! duplicate SKU and a dead database alike, so the only way to branch was to
//! pattern-match the English sentence in `err.message`.
//!
//! Errors built here carry a JSON envelope in the napi *reason*:
//!
//! ```json
//! {"code":"NOT_FOUND","message":"Cart not found","details":{"httpStatus":404}}
//! ```
//!
//! `bindings/node/index.js` unpacks that envelope once, on the way out, so what
//! JavaScript actually observes is a normal `Error` with:
//!
//! - `err.code` — the stable code from [`ErrCode`], the thing to branch on;
//! - `err.message` — the human sentence, unchanged from before;
//! - `err.details` — `{ httpStatus, invariant? }` when the failure came from a
//!   [`CommerceError`];
//! - `err.napiStatus` — the napi status (`InvalidArg` / `GenericFailure`).
//!
//! The code is derived from the [`CommerceError`] variant whenever one is
//! available, so it stays correct as the engine grows new variants. Call sites
//! that only have an ad-hoc string pass an explicit [`ErrCode`] instead.

use napi::{Error, Status};
use stateset_core::CommerceError;
use stateset_core::errors::InventoryError;

/// Stable error codes surfaced to JavaScript as `err.code`.
///
/// These are part of the binding's public contract: they may gain members, but
/// an existing code never changes meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErrCode {
    /// The addressed record does not exist.
    NotFound,
    /// The request lost a uniqueness or version race (duplicate SKU/slug/email,
    /// optimistic-lock failure, tampered kernel receipt).
    Conflict,
    /// The caller's input is malformed or fails a business rule that is checked
    /// before anything is written. Maps to `Status::InvalidArg`.
    Validation,
    /// Not enough stock to satisfy the request. Split out from `Validation`
    /// because agents retry it differently (wait and re-quote, not re-prompt).
    InsufficientStock,
    /// Refused by policy: the operation is understood but not permitted.
    PolicyRejected,
    /// The storage layer failed.
    Database,
    /// A payment/shipping/tax provider or other external service failed.
    ExternalService,
    /// Anything else — a bug in the binding or the engine.
    Internal,
    /// A Rust panic was contained at the boundary and turned into this error
    /// instead of taking the host process down. See [`guard`].
    InternalPanic,
}

impl ErrCode {
    /// The wire form of the code, as JavaScript sees it in `err.code`.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "NOT_FOUND",
            Self::Conflict => "CONFLICT",
            Self::Validation => "VALIDATION",
            Self::InsufficientStock => "INSUFFICIENT_STOCK",
            Self::PolicyRejected => "POLICY_REJECTED",
            Self::Database => "DATABASE",
            Self::ExternalService => "EXTERNAL_SERVICE",
            Self::Internal => "INTERNAL",
            Self::InternalPanic => "INTERNAL_PANIC",
        }
    }

    /// The napi status to throw with.
    ///
    /// Only bad input gets `InvalidArg`; everything else stays
    /// `GenericFailure`, which is what napi has always thrown here.
    const fn status(self) -> Status {
        match self {
            Self::Validation => Status::InvalidArg,
            _ => Status::GenericFailure,
        }
    }
}

/// Classify a [`CommerceError`] into its stable code.
///
/// Ordering matters: `InsufficientStock` is checked first because the engine
/// also reports it through `invariant_code`, and the not-found/conflict/
/// validation predicates are the engine's own definitions, so new variants are
/// classified without touching this function.
pub(crate) fn commerce_code(error: &CommerceError) -> ErrCode {
    match error {
        CommerceError::InsufficientStock { .. }
        | CommerceError::Inventory(InventoryError::InsufficientStock { .. }) => {
            ErrCode::InsufficientStock
        }
        _ if error.is_not_found() => ErrCode::NotFound,
        _ if error.is_conflict() => ErrCode::Conflict,
        _ if error.is_validation() => ErrCode::Validation,
        _ if error.is_not_permitted() => ErrCode::PolicyRejected,
        _ if error.is_database() => ErrCode::Database,
        _ if error.is_external_service() => ErrCode::ExternalService,
        _ => ErrCode::Internal,
    }
}

/// The `details` object for a [`CommerceError`]: the HTTP status the engine
/// suggests plus, for a published commerce invariant, its stable code.
fn commerce_details(error: &CommerceError) -> serde_json::Value {
    match error.invariant_code() {
        Some(invariant) => serde_json::json!({
            "httpStatus": error.suggested_status_code(),
            "invariant": invariant,
        }),
        None => serde_json::json!({ "httpStatus": error.suggested_status_code() }),
    }
}

/// Build the napi error carrying the JSON envelope.
fn envelope(code: ErrCode, message: &str, details: Option<serde_json::Value>) -> Error {
    let payload = match details {
        Some(details) => {
            serde_json::json!({ "code": code.as_str(), "message": message, "details": details })
        }
        None => serde_json::json!({ "code": code.as_str(), "message": message }),
    };
    Error::new(code.status(), payload.to_string())
}

/// An error with an explicitly chosen code and no underlying cause.
///
/// Use this where the binding itself rejects the call — an unparsable UUID, an
/// unknown enum string, a key of the wrong length.
pub(crate) fn coded(code: ErrCode, message: impl std::fmt::Display) -> Error {
    envelope(code, &message.to_string(), None)
}

/// Classify a cause: the [`CommerceError`] variant wins when there is one,
/// otherwise the call site's `fallback` stands.
fn classify_cause(
    fallback: ErrCode,
    cause: &dyn std::any::Any,
) -> (ErrCode, Option<serde_json::Value>) {
    match cause.downcast_ref::<CommerceError>() {
        Some(commerce) => (commerce_code(commerce), Some(commerce_details(commerce))),
        None => (fallback, None),
    }
}

/// An error wrapping an underlying failure, rendered exactly as
/// `"{context}: {cause}"` — the message shape the binding has always produced.
///
/// When `cause` is a [`CommerceError`] the code comes from its variant and
/// `fallback` is ignored; otherwise `fallback` decides. That is what lets the
/// 700-odd `"Failed to …"` sites keep a single call shape while still reporting
/// `NOT_FOUND` / `CONFLICT` / `INSUFFICIENT_STOCK` when the engine says so.
pub(crate) fn wrap<E>(fallback: ErrCode, context: &str, cause: E) -> Error
where
    E: std::fmt::Display + std::any::Any,
{
    let (code, details) = classify_cause(fallback, &cause);
    envelope(code, &format!("{context}: {cause}"), details)
}

/// Like [`wrap`], but the message is the cause's own `Display` with no added
/// context — for the handful of sites that forwarded `error.to_string()`.
pub(crate) fn from_cause<E>(fallback: ErrCode, cause: E) -> Error
where
    E: std::fmt::Display + std::any::Any,
{
    let (code, details) = classify_cause(fallback, &cause);
    envelope(code, &cause.to_string(), details)
}

/// Turn a caught panic payload into an `INTERNAL_PANIC` error.
fn panic_error(payload: &(dyn std::any::Any + Send)) -> Error {
    let message = payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&'static str>().map(|text| (*text).to_owned()))
        .unwrap_or_else(|| "unknown panic payload".to_owned());
    envelope(ErrCode::InternalPanic, &format!("panic in native code: {message}"), None)
}

/// Run a synchronous entry point with panic containment.
///
/// The workspace release profile is `panic = "abort"`, which makes a panic
/// anywhere under a `#[napi]` call kill the host Node process. `release-node`
/// (see the root `Cargo.toml`) inherits `release` but keeps `panic = "unwind"`
/// so this guard can convert the unwind into a JavaScript exception. Under
/// `panic = "abort"` the guard is inert but harmless.
pub(crate) fn guard<T, F>(operation: F) -> napi::Result<T>
where
    F: FnOnce() -> napi::Result<T>,
{
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(result) => result,
        Err(payload) => Err(panic_error(&*payload)),
    }
}

/// Run an async entry point with panic containment.
///
/// Polls the future inside `catch_unwind`; a panic on any poll resolves the
/// call to an `INTERNAL_PANIC` error and drops the future.
pub(crate) async fn guard_async<T, F>(future: F) -> napi::Result<T>
where
    F: std::future::Future<Output = napi::Result<T>>,
{
    let mut future = Box::pin(future);
    std::future::poll_fn(move |context| {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            future.as_mut().poll(context)
        })) {
            Ok(poll) => poll,
            Err(payload) => std::task::Poll::Ready(Err(panic_error(&*payload))),
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn reason(error: &Error) -> serde_json::Value {
        serde_json::from_str(error.reason.as_str()).expect("reason is a JSON envelope")
    }

    #[test]
    fn commerce_variants_map_to_stable_codes() {
        assert_eq!(commerce_code(&CommerceError::NotFound), ErrCode::NotFound);
        assert_eq!(commerce_code(&CommerceError::OrderNotFound(Uuid::nil())), ErrCode::NotFound);
        assert_eq!(commerce_code(&CommerceError::DuplicateSku("x".into())), ErrCode::Conflict);
        assert_eq!(commerce_code(&CommerceError::OptimisticLockFailure), ErrCode::Conflict);
        assert_eq!(commerce_code(&CommerceError::ValidationError("x".into())), ErrCode::Validation);
        assert_eq!(
            commerce_code(&CommerceError::NotPermitted("x".into())),
            ErrCode::PolicyRejected
        );
        assert_eq!(commerce_code(&CommerceError::DatabaseError("x".into())), ErrCode::Database);
        assert_eq!(
            commerce_code(&CommerceError::ExternalServiceError("x".into())),
            ErrCode::ExternalService
        );
        assert_eq!(commerce_code(&CommerceError::Internal("x".into())), ErrCode::Internal);
        assert_eq!(
            commerce_code(&CommerceError::InsufficientStock {
                sku: "x".into(),
                requested: "2".into(),
                available: "1".into(),
            }),
            ErrCode::InsufficientStock
        );
    }

    #[test]
    fn validation_is_the_only_invalid_arg() {
        assert_eq!(ErrCode::Validation.status(), Status::InvalidArg);
        for code in [
            ErrCode::NotFound,
            ErrCode::Conflict,
            ErrCode::InsufficientStock,
            ErrCode::PolicyRejected,
            ErrCode::Database,
            ErrCode::ExternalService,
            ErrCode::Internal,
            ErrCode::InternalPanic,
        ] {
            assert_eq!(code.status(), Status::GenericFailure, "{}", code.as_str());
        }
    }

    #[test]
    fn wrap_preserves_the_legacy_message_shape() {
        let error = wrap(ErrCode::Internal, "Failed to get cart", CommerceError::NotFound);
        let payload = reason(&error);
        assert_eq!(payload["code"], "NOT_FOUND");
        assert_eq!(payload["message"], "Failed to get cart: Record not found");
        assert_eq!(payload["details"]["httpStatus"], 404);
    }

    #[test]
    fn wrap_falls_back_for_non_commerce_causes() {
        let parse: Result<i32, _> = "x".parse::<i32>();
        let error = wrap(ErrCode::Validation, "Invalid quantity", parse.unwrap_err());
        let payload = reason(&error);
        assert_eq!(payload["code"], "VALIDATION");
        assert_eq!(error.status, Status::InvalidArg);
        assert!(payload["details"].is_null());
    }

    #[test]
    fn invariants_are_reported_in_details() {
        let error = wrap(
            ErrCode::Internal,
            "Failed to reserve inventory",
            CommerceError::InsufficientStock {
                sku: "SKU-1".into(),
                requested: "9".into(),
                available: "1".into(),
            },
        );
        let payload = reason(&error);
        assert_eq!(payload["code"], "INSUFFICIENT_STOCK");
        assert_eq!(payload["details"]["invariant"], "commerce.inventory.insufficient_available");
    }

    #[test]
    fn coded_errors_carry_no_details() {
        let error = coded(ErrCode::Validation, "Invalid cart UUID");
        let payload = reason(&error);
        assert_eq!(payload["code"], "VALIDATION");
        assert_eq!(payload["message"], "Invalid cart UUID");
        assert!(payload.get("details").is_none());
    }

    #[test]
    fn envelope_escapes_hostile_messages() {
        let error = coded(ErrCode::Internal, "quote \" brace } newline \n");
        let payload = reason(&error);
        assert_eq!(payload["message"], "quote \" brace } newline \n");
    }

    #[test]
    fn guard_contains_a_panic() {
        let error = guard::<(), _>(|| panic!("boom")).unwrap_err();
        let payload = reason(&error);
        assert_eq!(payload["code"], "INTERNAL_PANIC");
        assert_eq!(payload["message"], "panic in native code: boom");
    }

    #[test]
    fn guard_passes_success_through() {
        assert_eq!(guard(|| Ok(7)).unwrap(), 7);
    }

    #[test]
    fn from_cause_keeps_the_engine_message() {
        let error = from_cause(ErrCode::Internal, CommerceError::CustomerNotActive);
        let payload = reason(&error);
        assert_eq!(payload["code"], "INTERNAL");
        assert_eq!(payload["message"], "Customer is not active");
        assert_eq!(payload["details"]["httpStatus"], 500);
    }
}
