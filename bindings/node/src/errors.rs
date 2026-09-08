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
use stateset_core::errors::{
    CustomerError, InventoryError, OrderError, PaymentError, ProductError, ReturnError,
    ShippingError,
};

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
    /// The request is well-formed but the record is not in a state that allows
    /// it: an order too far along to cancel, an expired reservation, a product
    /// that is not purchasable. Distinct from `Validation` — resending the same
    /// request after the record moves on may well succeed, so an agent should
    /// re-read state rather than re-prompt.
    PreconditionFailed,
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
            Self::PreconditionFailed => "PRECONDITION_FAILED",
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

    /// The HTTP status a gateway should give this failure, reported as
    /// `details.httpStatus`.
    ///
    /// Every arm mirrors `crates/stateset-http/src/error.rs`, which is the
    /// platform's actual mapping, rather than
    /// `CommerceError::suggested_status_code` — that helper has no arm for the
    /// business-rule and stock failures and sends them all to 500, while the
    /// HTTP layer answers 400. Keeping the binding and the HTTP surface in
    /// agreement matters more than agreeing with the helper, so where the two
    /// disagreed the HTTP layer won: `Validation` is 422 (`HttpError::
    /// ValidationError` -> `UNPROCESSABLE_ENTITY`), not the helper's 400, and
    /// `ExternalService` is 500 (a carrier failure has no arm in
    /// `HttpError::classify`, so it falls through to `InternalError`), not the
    /// helper's 502. Both previously followed `suggested_status_code` while
    /// this comment claimed otherwise.
    const fn http_status(self) -> u16 {
        match self {
            Self::NotFound => 404,
            Self::Conflict => 409,
            // `HttpError::ValidationError` -> 422.
            Self::Validation => 422,
            // `HttpError::BadRequest` — see the `InsufficientStock`,
            // `ShipmentExceedsOrdered`, `OrderCannotBeCancelled`,
            // `ReturnPeriodExpired`, `CustomerNotActive` and
            // `ProductNotPurchasable` arms of `HttpError::from`.
            Self::InsufficientStock | Self::PreconditionFailed => 400,
            Self::PolicyRejected => 403,
            // No `HttpError` arm covers a carrier failure, so `classify`
            // answers `InternalError`.
            Self::ExternalService => 500,
            Self::Database | Self::Internal | Self::InternalPanic => 500,
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

        // Business-rule rejections: the record exists and the input parses, but
        // its current state forbids the operation. `CommerceError` has no
        // predicate for these, so they are listed explicitly — without this arm
        // they all fall through to `INTERNAL`, which reads as "the binding is
        // broken" when it means "the order already shipped".
        CommerceError::OrderCannotBeCancelled(_)
        | CommerceError::OrderCannotBeRefunded(_)
        | CommerceError::InvalidOrderStatusTransition { .. }
        | CommerceError::ReservationExpired(_)
        | CommerceError::CustomerNotActive
        | CommerceError::ProductNotPurchasable
        | CommerceError::ReturnCannotBeApproved(_)
        | CommerceError::ReturnPeriodExpired
        | CommerceError::ItemNotEligibleForReturn
        | CommerceError::ShipmentExceedsOrdered { .. }
        | CommerceError::Order(
            OrderError::CannotCancel { .. }
            | OrderError::CannotRefund { .. }
            | OrderError::InvalidTransition(_),
        )
        | CommerceError::Payment(
            PaymentError::Declined { .. }
            | PaymentError::RefundFailed { .. }
            | PaymentError::InvalidTransition(_),
        )
        | CommerceError::Return(
            ReturnError::CannotApprove { .. }
            | ReturnError::PeriodExpired
            | ReturnError::ItemNotEligible
            | ReturnError::InvalidTransition(_),
        )
        | CommerceError::Inventory(InventoryError::ReservationExpired(_))
        | CommerceError::Customer(CustomerError::NotActive)
        | CommerceError::Product(ProductError::NotPurchasable) => ErrCode::PreconditionFailed,

        // A carrier API that failed is an upstream outage, not a bad request.
        CommerceError::Shipping(ShippingError::CarrierError { .. }) => ErrCode::ExternalService,
        CommerceError::Shipping(ShippingError::InvalidTrackingNumber(_))
        | CommerceError::Payment(PaymentError::CurrencyMismatch { .. }) => ErrCode::Validation,

        _ if error.is_not_found() => ErrCode::NotFound,
        _ if error.is_conflict() => ErrCode::Conflict,
        _ if error.is_validation() => ErrCode::Validation,
        _ if error.is_not_permitted() => ErrCode::PolicyRejected,
        _ if error.is_database() => ErrCode::Database,
        _ if error.is_external_service() => ErrCode::ExternalService,
        _ => ErrCode::Internal,
    }
}

/// The `details` object for a [`CommerceError`].
///
/// Always carries `httpStatus`; adds `invariant` for a published commerce
/// invariant, and the variant's own fields where it has them, so a caller can
/// read `details.available` instead of parsing it back out of the sentence.
fn commerce_details(code: ErrCode, error: &CommerceError) -> serde_json::Value {
    let mut details = serde_json::Map::new();
    details.insert("httpStatus".to_owned(), code.http_status().into());
    if let Some(invariant) = error.invariant_code() {
        details.insert("invariant".to_owned(), invariant.into());
    }
    match error {
        CommerceError::InsufficientStock { sku, requested, available }
        | CommerceError::Inventory(InventoryError::InsufficientStock {
            sku,
            requested,
            available,
        }) => {
            details.insert("sku".to_owned(), sku.clone().into());
            details.insert("requested".to_owned(), requested.clone().into());
            details.insert("available".to_owned(), available.clone().into());
        }
        CommerceError::VersionConflict { entity, id, expected_version } => {
            details.insert("entity".to_owned(), entity.clone().into());
            details.insert("id".to_owned(), id.clone().into());
            details.insert("expectedVersion".to_owned(), (*expected_version).into());
        }
        CommerceError::ShipmentExceedsOrdered { order_item_id, requested, remaining } => {
            details.insert("orderItemId".to_owned(), order_item_id.to_string().into());
            details.insert("requested".to_owned(), (*requested).into());
            details.insert("remaining".to_owned(), (*remaining).into());
        }
        _ => {}
    }
    serde_json::Value::Object(details)
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
        Some(commerce) => {
            let code = commerce_code(commerce);
            (code, Some(commerce_details(code, commerce)))
        }
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
        // Business-rule rejections used to fall through to INTERNAL.
        assert_eq!(
            commerce_code(&CommerceError::CustomerNotActive),
            ErrCode::PreconditionFailed,
            "a customer that exists but is inactive is a precondition failure, not a bug"
        );
        assert_eq!(
            commerce_code(&CommerceError::OrderCannotBeCancelled("shipped".into())),
            ErrCode::PreconditionFailed
        );
        assert_eq!(
            commerce_code(&CommerceError::ProductNotPurchasable),
            ErrCode::PreconditionFailed
        );
        assert_eq!(
            commerce_code(&CommerceError::Return(ReturnError::PeriodExpired)),
            ErrCode::PreconditionFailed
        );
        assert_eq!(
            commerce_code(&CommerceError::Shipping(ShippingError::CarrierError {
                carrier: "ups".into(),
                reason: "503".into(),
            })),
            ErrCode::ExternalService
        );
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
            ErrCode::PreconditionFailed,
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
    fn http_status_matches_the_platform_http_layer() {
        // Mirrors `HttpError::from(CommerceError)` + `HttpError::status_code()`
        // in crates/stateset-http. The two arms that used to disagree with it
        // are pinned explicitly below.
        assert_eq!(ErrCode::NotFound.http_status(), 404);
        assert_eq!(ErrCode::Conflict.http_status(), 409);
        assert_eq!(ErrCode::InsufficientStock.http_status(), 400);
        assert_eq!(ErrCode::PreconditionFailed.http_status(), 400);
        assert_eq!(ErrCode::PolicyRejected.http_status(), 403);
        assert_eq!(ErrCode::Database.http_status(), 500);
        assert_eq!(ErrCode::Internal.http_status(), 500);
        assert_eq!(ErrCode::InternalPanic.http_status(), 500);

        // `HttpError::ValidationError` is UNPROCESSABLE_ENTITY, not 400: that
        // 400 came from `CommerceError::suggested_status_code`, which this
        // mapping deliberately does not follow.
        assert_eq!(ErrCode::Validation.http_status(), 422);
        // A carrier failure has no `HttpError` arm, so `classify` sends it to
        // `InternalError` (500). 502 was, again, the helper's answer.
        assert_eq!(ErrCode::ExternalService.http_status(), 500);
    }

    #[test]
    fn variant_fields_are_reported_in_details() {
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
        assert_eq!(payload["details"]["sku"], "SKU-1");
        assert_eq!(payload["details"]["requested"], "9");
        assert_eq!(payload["details"]["available"], "1");

        let error = wrap(
            ErrCode::Internal,
            "Failed to update order",
            CommerceError::VersionConflict {
                entity: "order".into(),
                id: "abc".into(),
                expected_version: 3,
            },
        );
        let payload = reason(&error);
        assert_eq!(payload["code"], "CONFLICT");
        assert_eq!(payload["details"]["entity"], "order");
        assert_eq!(payload["details"]["id"], "abc");
        assert_eq!(payload["details"]["expectedVersion"], 3);
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
        assert_eq!(payload["details"]["httpStatus"], 400);
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
        assert_eq!(payload["code"], "PRECONDITION_FAILED");
        assert_eq!(payload["message"], "Customer is not active");
        assert_eq!(payload["details"]["httpStatus"], 400);
    }
}
