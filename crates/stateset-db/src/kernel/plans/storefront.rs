//! Plans for the governed storefront commands — the customer journey from
//! account to delivered return, so a strict kernel endpoint can run a whole
//! checkout: `customers.create`, `carts.create`, `carts.item.add`,
//! `carts.shipping_address.set`, `carts.payment_method.set`,
//! `carts.coupon.apply`, `carts.tax.calculate`, `payments.complete`,
//! `shipments.create`, `returns.create`, `returns.tracking.add`.
//!
//! These commands delegate their business rules to the same repository
//! functions the ungoverned API uses (run on the kernel's transaction), so a
//! governed mutation can never be more permissive than an ungoverned one.
//! What the kernel adds is the envelope: policy, tenant/store scope,
//! delegation, idempotent replay, a sealed receipt and a command-context
//! outbox event.
//!
//! Both backends run the domain step inside a savepoint and seal its outcome
//! through one decision ([`seal_decision`]): a domain refusal becomes a typed
//! rejection receipt, a preview rolls the savepoint back after proving the
//! apply would succeed (so a preview can never promise what apply refuses),
//! and an apply releases the savepoint and seals success.

use crate::kernel::envelope::GuardRejection;
use serde::Serialize;
use stateset_core::{
    AddCartItemCommand, AddReturnTracking, ApplyCartCoupon, CalculateCartTax, Cart, CartId,
    CommerceError, CompletePayment, CreateCart, CreateCustomer, CreateReturn, CreateShipment,
    ProductTaxCategory, SetCartPaymentMethod, SetCartShippingAddress, TaxAddress,
    TaxCalculationRequest, TaxLineItem, Validate,
};

const VALIDATION: &str = "commerce.validation_failed";

/// Message used with [`crate::kernel::EnvelopeGuard::unversioned`] for payments.
pub const PAYMENT_UNVERSIONED: &str = "payment completion does not take an aggregate version";
/// Message used with [`crate::kernel::EnvelopeGuard::unversioned`] for return tracking.
pub const RETURN_TRACKING_UNVERSIONED: &str =
    "return tracking is guarded by the return state machine, not an expected version";

/// The aggregate a storefront command acts on; selects its receipt codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorefrontAggregate {
    Customer,
    Cart,
    Payment,
    Shipment,
    Return,
}

impl StorefrontAggregate {
    /// Receipt `aggregate_type` label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Customer => "customer",
            Self::Cart => "cart",
            Self::Payment => "payment",
            Self::Shipment => "shipment",
            Self::Return => "return",
        }
    }

    const fn not_found(self) -> &'static str {
        match self {
            Self::Customer => "commerce.customer_not_found",
            Self::Cart => "commerce.cart_not_found",
            Self::Payment => "commerce.payment_not_found",
            // A shipment is created against an order; the missing thing is the order.
            Self::Shipment => "commerce.order_not_found",
            Self::Return => "commerce.return_not_found",
        }
    }

    const fn rejected(self) -> &'static str {
        match self {
            Self::Customer => "commerce.customer.rejected",
            Self::Cart => "commerce.cart.rejected",
            Self::Payment => "commerce.payment.rejected",
            Self::Shipment => "commerce.shipment.rejected",
            Self::Return => "commerce.return.rejected",
        }
    }
}

/// Typed rejection for a business refusal raised by the delegated repository
/// function. `None` means the error is infrastructure (database, internal,
/// external service) and must abort the transaction instead of being sealed
/// as a durable business answer.
#[must_use]
pub fn domain_rejection(
    aggregate: StorefrontAggregate,
    error: &CommerceError,
) -> Option<GuardRejection> {
    if error.is_server_error() {
        return None;
    }
    let message = error.to_string();
    if let Some(code) = error.invariant_code() {
        return Some(GuardRejection::never(code, message));
    }
    let rejection = match error {
        CommerceError::EmailAlreadyExists(_)
        | CommerceError::Customer(stateset_core::CustomerError::EmailAlreadyExists(_)) => {
            GuardRejection::never("commerce.customer.email_conflict", message)
        }
        CommerceError::VersionConflict { .. } | CommerceError::OptimisticLockFailure => {
            GuardRejection::after_conflict("kernel.version_conflict", message)
        }
        _ if error.is_not_found() => GuardRejection::never(aggregate.not_found(), message),
        _ if error.is_conflict() => GuardRejection::after_conflict("commerce.conflict", message),
        _ if error.is_validation() => GuardRejection::never(VALIDATION, message),
        _ => GuardRejection::never(aggregate.rejected(), message),
    };
    Some(rejection)
}

/// What a storefront domain step committed (or, under preview, would commit).
#[derive(Debug, Clone)]
pub struct DomainOutcome<T> {
    /// Receipt result.
    pub result: T,
    /// Receipt aggregate id.
    pub aggregate_id: String,
    /// Aggregate version before the step, when the aggregate is versioned.
    pub version_before: Option<i32>,
    /// Aggregate version after the step, when the aggregate is versioned.
    pub version_after: Option<i32>,
    /// Outbox event type.
    pub event_type: &'static str,
    /// Outbox event payload.
    pub event_payload: serde_json::Value,
}

/// How a backend must finish a storefront domain step that ran in a savepoint.
#[derive(Debug)]
pub enum SealDecision<T> {
    /// Roll the savepoint back and seal this rejection.
    Reject(GuardRejection),
    /// Roll the savepoint back and seal a preview (the step proved it would apply).
    Preview(DomainOutcome<T>),
    /// Release the savepoint, append the event and seal success.
    Apply(DomainOutcome<T>),
}

/// The single decision both backends take on a savepointed domain step.
///
/// # Errors
///
/// Returns the step's error unchanged when it is infrastructure rather than a
/// business refusal (see [`domain_rejection`]).
pub fn seal_decision<T>(
    aggregate: StorefrontAggregate,
    attempted: Result<DomainOutcome<T>, CommerceError>,
    preview: bool,
) -> Result<SealDecision<T>, CommerceError> {
    match attempted {
        Ok(outcome) if preview => Ok(SealDecision::Preview(outcome)),
        Ok(outcome) => Ok(SealDecision::Apply(outcome)),
        Err(error) => domain_rejection(aggregate, &error).map(SealDecision::Reject).ok_or(error),
    }
}

/// Build a [`DomainOutcome`] whose event payload is the serialized result
/// summary.
pub fn outcome<T>(
    result: T,
    aggregate_id: impl Into<String>,
    event_type: &'static str,
    event_payload: impl Serialize,
) -> Result<DomainOutcome<T>, CommerceError> {
    Ok(DomainOutcome {
        result,
        aggregate_id: aggregate_id.into(),
        version_before: None,
        version_after: None,
        event_type,
        event_payload: serde_json::to_value(event_payload)
            .map_err(|error| CommerceError::Internal(error.to_string()))?,
    })
}

fn validation(error: &CommerceError) -> GuardRejection {
    GuardRejection::never(VALIDATION, error.to_string())
}

fn required(field: &str, value: &str) -> Option<GuardRejection> {
    value
        .trim()
        .is_empty()
        .then(|| GuardRejection::never(VALIDATION, format!("{field} is required")))
}

fn cart_id_guard(cart_id: CartId) -> Option<GuardRejection> {
    cart_id.is_nil().then(|| GuardRejection::never(VALIDATION, "cart_id is required"))
}

/// Static payload checks for `customers.create`.
#[must_use]
pub fn create_customer_guard(input: &CreateCustomer) -> Option<GuardRejection> {
    input.validate().err().map(|error| validation(&error))
}

/// Static payload checks for `carts.create`.
#[must_use]
pub fn create_cart_guard(input: &CreateCart) -> Option<GuardRejection> {
    input.validate().err().map(|error| validation(&error))
}

/// Static payload checks for `carts.item.add`.
#[must_use]
pub fn add_cart_item_guard(input: &AddCartItemCommand) -> Option<GuardRejection> {
    cart_id_guard(input.cart_id).or_else(|| input.item.validate().err().map(|e| validation(&e)))
}

/// Static payload checks for `carts.shipping_address.set`.
#[must_use]
pub fn set_cart_shipping_address_guard(input: &SetCartShippingAddress) -> Option<GuardRejection> {
    let address = &input.address;
    cart_id_guard(input.cart_id)
        .or_else(|| required("address.line1", &address.line1))
        .or_else(|| required("address.city", &address.city))
        .or_else(|| required("address.postal_code", &address.postal_code))
        .or_else(|| required("address.country", &address.country))
}

/// Static payload checks for `carts.payment_method.set`.
#[must_use]
pub fn set_cart_payment_method_guard(input: &SetCartPaymentMethod) -> Option<GuardRejection> {
    cart_id_guard(input.cart_id)
        .or_else(|| required("payment.payment_method", &input.payment.payment_method))
}

/// Static payload checks for `carts.coupon.apply`.
#[must_use]
pub fn apply_cart_coupon_guard(input: &ApplyCartCoupon) -> Option<GuardRejection> {
    cart_id_guard(input.cart_id).or_else(|| required("coupon_code", &input.coupon_code))
}

/// Static payload checks for `carts.tax.calculate`.
#[must_use]
pub fn calculate_cart_tax_guard(input: &CalculateCartTax) -> Option<GuardRejection> {
    cart_id_guard(input.cart_id)
}

/// Static payload checks for `payments.complete`.
#[must_use]
pub fn complete_payment_guard(input: &CompletePayment) -> Option<GuardRejection> {
    input
        .payment_id
        .into_uuid()
        .is_nil()
        .then(|| GuardRejection::never(VALIDATION, "payment_id is required"))
}

/// Static payload checks for `shipments.create`.
#[must_use]
pub fn create_shipment_guard(input: &CreateShipment) -> Option<GuardRejection> {
    if input.order_id.into_uuid().is_nil() {
        return Some(GuardRejection::never(VALIDATION, "order_id is required"));
    }
    required("recipient_name", &input.recipient_name)
        .or_else(|| required("shipping_address", &input.shipping_address))
        .or_else(|| {
            input.items.iter().flatten().find(|item| item.quantity <= 0).map(|item| {
                GuardRejection::never(
                    VALIDATION,
                    format!("shipment item {} quantity must be positive", item.sku),
                )
            })
        })
}

/// `payments.complete` captures a payment once. The repository treats a
/// repeated completion as a no-op status write; the governed command refuses
/// it, so a second command cannot seal a second `payments.completed.v1` fact
/// for money that was captured once.
///
/// # Errors
///
/// `NotPermitted` when the payment is already completed.
pub fn refuse_recapture(status: &str) -> Result<(), CommerceError> {
    if status == "completed" {
        return Err(CommerceError::NotPermitted("payment is already completed".into()));
    }
    Ok(())
}

/// Order statuses a shipment may not be created against: closed to
/// fulfilment (every unit shipped, or cancelled/refunded). The same rule as
/// the repositories' manifest check.
#[must_use]
pub fn order_status_refuses_shipment(status: &str) -> bool {
    crate::shipment_allocations::validate_order_status(status).is_err()
}

/// Static payload checks for `returns.create`.
#[must_use]
pub fn create_return_guard(input: &CreateReturn) -> Option<GuardRejection> {
    input.validate().err().map(|error| validation(&error))
}

/// Static payload checks for `returns.tracking.add`.
#[must_use]
pub fn add_return_tracking_guard(input: &AddReturnTracking) -> Option<GuardRejection> {
    if input.return_id.into_uuid().is_nil() {
        return Some(GuardRejection::never(VALIDATION, "return_id is required"));
    }
    required("tracking_number", &input.tracking_number)
}

/// The exact tax request a cart prices to — the same mapping
/// `Commerce::calculate_cart_tax` uses: the shipping address, every line at
/// the standard tax category, the cart's customer, currency and shipping.
///
/// # Errors
///
/// A cart without a shipping address cannot be taxed.
pub fn cart_tax_request(cart: &Cart) -> Result<TaxCalculationRequest, CommerceError> {
    let address = cart.shipping_address.clone().ok_or_else(|| {
        CommerceError::ValidationError("Shipping address required to calculate tax".into())
    })?;
    Ok(TaxCalculationRequest {
        line_items: cart
            .items
            .iter()
            .map(|item| TaxLineItem {
                id: item.id.to_string(),
                sku: Some(item.sku.clone()),
                product_id: item.product_id,
                quantity: rust_decimal::Decimal::from(item.quantity),
                unit_price: item.unit_price,
                discount_amount: item.discount_amount,
                tax_category: ProductTaxCategory::Standard,
                tax_code: None,
                description: Some(item.name.clone()),
            })
            .collect(),
        shipping_address: TaxAddress {
            country: address.country,
            state: address.state,
            city: Some(address.city),
            postal_code: Some(address.postal_code),
            line1: Some(address.line1),
            line2: address.line2,
        },
        customer_id: cart.customer_id.map(Into::into),
        currency: cart.currency,
        shipping_amount: Some(cart.shipping_amount),
        ..Default::default()
    })
}

/// Canonical form of a tax request, compared under the cart lock to prove
/// the tax about to be written was priced on the cart as it now stands.
///
/// # Errors
///
/// Serialization failure (never expected for these plain types).
pub fn tax_basis(request: &TaxCalculationRequest) -> Result<serde_json::Value, CommerceError> {
    serde_json::to_value(request).map_err(|error| CommerceError::Internal(error.to_string()))
}

/// The refusal when the cart moved between pricing its tax and locking it.
#[must_use]
pub fn tax_basis_changed() -> CommerceError {
    CommerceError::Conflict(
        "the cart changed while its tax was being calculated; retry to price the current cart"
            .into(),
    )
}

/// Redact the processor token from a cart before it is sealed on a receipt.
#[must_use]
pub fn redact_payment_token(mut cart: Cart) -> Cart {
    if cart.payment_token.is_some() {
        cart.payment_token = Some("[redacted]".into());
    }
    cart
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use stateset_core::{AddCartItem, CartAddress, OrderId, PaymentId, ReturnId, SetCartPayment};

    fn address() -> CartAddress {
        CartAddress {
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            company: None,
            line1: "1 Main St".into(),
            line2: None,
            city: "Los Angeles".into(),
            state: Some("CA".into()),
            postal_code: "90001".into(),
            country: "US".into(),
            phone: None,
            email: None,
        }
    }

    #[test]
    fn customer_guard_requires_a_valid_email_and_names() {
        let mut input = CreateCustomer {
            email: "ada@example.com".into(),
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        };
        assert_eq!(create_customer_guard(&input), None);
        input.email = "not-an-email".into();
        assert_eq!(create_customer_guard(&input).map(|r| r.code), Some(VALIDATION));
    }

    #[test]
    fn cart_guards_refuse_nil_carts_and_bad_lines() {
        let item = AddCartItem {
            sku: "W-1".into(),
            name: "Widget".into(),
            quantity: 1,
            unit_price: Decimal::new(5000, 2),
            ..Default::default()
        };
        let mut add = AddCartItemCommand { cart_id: CartId::new(), item };
        assert_eq!(add_cart_item_guard(&add), None);
        add.item.quantity = 0;
        assert!(add_cart_item_guard(&add).is_some());
        add.item.quantity = 1;
        add.cart_id = CartId::from(uuid::Uuid::nil());
        assert!(add_cart_item_guard(&add).is_some());

        let mut shipping = SetCartShippingAddress { cart_id: CartId::new(), address: address() };
        assert_eq!(set_cart_shipping_address_guard(&shipping), None);
        shipping.address.postal_code = " ".into();
        assert!(set_cart_shipping_address_guard(&shipping).is_some());

        let payment = SetCartPaymentMethod {
            cart_id: CartId::new(),
            payment: SetCartPayment {
                payment_method: String::new(),
                payment_token: None,
                billing_address: None,
            },
        };
        assert!(set_cart_payment_method_guard(&payment).is_some());

        let coupon = ApplyCartCoupon { cart_id: CartId::new(), coupon_code: "  ".into() };
        assert!(apply_cart_coupon_guard(&coupon).is_some());
        assert_eq!(calculate_cart_tax_guard(&CalculateCartTax { cart_id: CartId::new() }), None);
    }

    #[test]
    fn fulfilment_guards_refuse_nil_targets() {
        assert!(
            complete_payment_guard(&CompletePayment {
                payment_id: PaymentId::from(uuid::Uuid::nil())
            })
            .is_some()
        );
        let shipment = CreateShipment {
            order_id: OrderId::new(),
            recipient_name: "Ada".into(),
            shipping_address: "1 Main St".into(),
            ..Default::default()
        };
        assert_eq!(create_shipment_guard(&shipment), None);
        assert!(
            create_shipment_guard(&CreateShipment { recipient_name: String::new(), ..shipment })
                .is_some()
        );
        assert!(create_return_guard(&CreateReturn::default()).is_some());
        let tracking =
            AddReturnTracking { return_id: ReturnId::new(), tracking_number: "1Z".into() };
        assert_eq!(add_return_tracking_guard(&tracking), None);
        assert!(
            add_return_tracking_guard(&AddReturnTracking {
                tracking_number: String::new(),
                ..tracking
            })
            .is_some()
        );
        assert!(refuse_recapture("pending").is_ok());
        assert_eq!(
            domain_rejection(
                StorefrontAggregate::Payment,
                &refuse_recapture("completed").expect_err("recapture")
            )
            .map(|r| r.code),
            Some("commerce.payment.rejected")
        );
        assert!(order_status_refuses_shipment("cancelled"));
        assert!(order_status_refuses_shipment("shipped"));
        assert!(order_status_refuses_shipment("delivered"));
        assert!(!order_status_refuses_shipment("processing"));
        assert!(!order_status_refuses_shipment("partially_shipped"));
    }

    #[test]
    fn domain_errors_become_typed_rejections_and_infrastructure_errors_do_not() {
        let cart = StorefrontAggregate::Cart;
        assert_eq!(
            domain_rejection(cart, &CommerceError::NotFound).map(|r| r.code),
            Some("commerce.cart_not_found")
        );
        assert_eq!(
            domain_rejection(cart, &CommerceError::ValidationError("bad coupon".into()))
                .map(|r| r.code),
            Some(VALIDATION)
        );
        let conflict = domain_rejection(cart, &tax_basis_changed()).expect("conflict");
        assert_eq!(conflict.code, "commerce.conflict");
        assert_eq!(conflict.retry, stateset_core::RetryDisposition::AfterConflict);
        assert_eq!(
            domain_rejection(
                StorefrontAggregate::Customer,
                &CommerceError::EmailAlreadyExists("a@b.c".into())
            )
            .map(|r| r.code),
            Some("commerce.customer.email_conflict")
        );
        assert_eq!(domain_rejection(cart, &CommerceError::DatabaseError("io".into())), None);
        assert_eq!(domain_rejection(cart, &CommerceError::Internal("bug".into())), None);
    }

    #[test]
    fn seal_decision_previews_only_what_would_apply() {
        let ok = || outcome(1_u8, "id", "carts.updated.v1", serde_json::json!({}));
        assert!(matches!(
            seal_decision(StorefrontAggregate::Cart, ok(), true),
            Ok(SealDecision::Preview(_))
        ));
        assert!(matches!(
            seal_decision(StorefrontAggregate::Cart, ok(), false),
            Ok(SealDecision::Apply(_))
        ));
        assert!(matches!(
            seal_decision::<u8>(StorefrontAggregate::Cart, Err(CommerceError::NotFound), true),
            Ok(SealDecision::Reject(_))
        ));
        assert!(
            seal_decision::<u8>(
                StorefrontAggregate::Cart,
                Err(CommerceError::DatabaseError("io".into())),
                false
            )
            .is_err()
        );
    }

    #[test]
    fn tax_request_needs_an_address_and_tracks_the_cart() {
        let mut cart = Cart {
            id: CartId::new(),
            cart_number: "C-1".into(),
            customer_id: None,
            status: stateset_core::CartStatus::Active,
            currency: stateset_core::CurrencyCode::USD,
            items: vec![],
            subtotal: Decimal::ZERO,
            tax_amount: Decimal::ZERO,
            shipping_amount: Decimal::ZERO,
            discount_amount: Decimal::ZERO,
            grand_total: Decimal::ZERO,
            customer_email: None,
            customer_phone: None,
            customer_name: None,
            shipping_address: None,
            billing_address: None,
            billing_same_as_shipping: true,
            fulfillment_type: None,
            shipping_method: None,
            shipping_carrier: None,
            estimated_delivery: None,
            payment_method: None,
            payment_token: Some("tok_secret".into()),
            payment_status: stateset_core::CartPaymentStatus::None,
            coupon_code: None,
            discount_description: None,
            order_id: None,
            order_number: None,
            notes: None,
            metadata: None,
            inventory_reserved: false,
            reservation_expires_at: None,
            x402_payment: None,
            expires_at: None,
            completed_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        assert!(cart_tax_request(&cart).is_err());
        cart.shipping_address = Some(address());
        let before = tax_basis(&cart_tax_request(&cart).expect("request")).expect("basis");
        cart.shipping_amount = Decimal::new(599, 2);
        let after = tax_basis(&cart_tax_request(&cart).expect("request")).expect("basis");
        assert_ne!(before, after, "a shipping change must invalidate the priced tax");
        assert_eq!(redact_payment_token(cart).payment_token.as_deref(), Some("[redacted]"));
    }
}
