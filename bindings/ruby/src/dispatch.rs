//! Engine dispatcher for the Ruby binding.
//!
//! The Ruby extension is deliberately thin: every Ruby API call becomes one
//! `Engine::call(op, args_json)` here, which deserializes the arguments into
//! the engine's own input types, calls the real `stateset_embedded::Commerce`,
//! and serializes the engine's own output types back. This module has no
//! Ruby dependency, so it is unit-tested with plain `cargo test` and linted by
//! clippy without Ruby headers.
//!
//! Wire contract (shared with `lib/stateset_embedded/native.rb`):
//!   * `args_json` is a JSON object of named arguments.
//!   * Money and quantities are `rust_decimal::Decimal`, which (de)serializes
//!     as an exact decimal STRING. The Ruby side sends `BigDecimal#to_s('F')`
//!     and refuses `Float` before anything reaches this layer.
//!   * The reply is always an envelope, never a raised error:
//!     `{"ok":true,"value":...}` or
//!     `{"ok":false,"error":{"kind":..,"code":..,"status":..,"message":..}}`.
//!     Ruby maps `kind` onto its exception hierarchy.
//!   * Unknown fields inside an engine input are refused (`serde_ignored`), so
//!     a misspelled keyword fails loudly instead of being silently dropped.
//!   * Panics never cross the boundary: `call` runs under `catch_unwind` and a
//!     panic becomes an `internal` error envelope.

use std::panic::{AssertUnwindSafe, catch_unwind};

use rust_decimal::Decimal;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use stateset_core::errors::InventoryError;
use stateset_core::{
    AddCartItem, CartAddress, CartFilter, CartId, CommerceError, CreateCart, CreateCustomer,
    CreateInventoryItem, CreateOrder, CreatePayment, CreateProduct, CreateProductVariant,
    CreateRefund, CreateReturn, CreateShipment, CreateShipmentItem, CustomerFilter, CustomerId,
    InventoryFilter, OrderFilter, OrderId, OrderStatus, PaymentFilter, PaymentId, ProductFilter,
    ProductId, ReturnFilter, ReturnId, SetCartPayment, SetCartShipping, SetReturnDisposition,
    ShipmentFilter, ShipmentId, ShipmentLineInput, UpdateCartItem, UpdateCustomer, UpdateProduct,
};
use stateset_embedded::Commerce;
use uuid::Uuid;

/// Error category, mapped 1:1 onto a Ruby exception class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// `StateSet::NotFoundError`
    NotFound,
    /// `StateSet::ValidationError` (also bad binding arguments)
    Validation,
    /// `StateSet::InsufficientStockError`
    InsufficientStock,
    /// `StateSet::ConflictError` (duplicates, optimistic-lock failures)
    Conflict,
    /// `StateSet::NotPermittedError`
    NotPermitted,
    /// `StateSet::InvalidOperationError`: well-formed, but refused in the
    /// entity's current state (bad status transition, not cancellable, ...)
    InvalidOperation,
    /// `StateSet::DatabaseError`
    Database,
    /// `StateSet::ExternalServiceError`
    ExternalService,
    /// `StateSet::InternalError` (engine `Internal`, or a caught panic)
    Internal,
}

/// A failure, ready to cross into Ruby as data.
#[derive(Debug, Clone, Serialize)]
pub struct BindingError {
    pub kind: ErrorKind,
    /// Stable machine code: the engine invariant code when there is one
    /// (`commerce.refund.exceeds_captured`), else the engine error variant
    /// (`OrderNotFound`), else a `binding.*` code.
    pub code: String,
    /// Suggested HTTP-style status, from the engine's own classification.
    pub status: u16,
    pub message: String,
}

impl BindingError {
    fn argument(message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Validation,
            code: "binding.invalid_argument".to_owned(),
            status: 400,
            message: message.into(),
        }
    }

    fn unknown_op(op: &str) -> Self {
        Self {
            kind: ErrorKind::Validation,
            code: "binding.unknown_operation".to_owned(),
            status: 400,
            message: format!("unknown operation `{op}`"),
        }
    }

    fn panic(detail: &str) -> Self {
        Self {
            kind: ErrorKind::Internal,
            code: "binding.panic".to_owned(),
            status: 500,
            message: format!("engine panicked: {detail}"),
        }
    }
}

impl From<CommerceError> for BindingError {
    fn from(err: CommerceError) -> Self {
        let kind = classify(&err);
        let code = err.invariant_code().map_or_else(|| variant_name(&err), str::to_owned);
        Self { kind, code, status: err.suggested_status_code(), message: err.to_string() }
    }
}

/// Classification uses the engine's own predicates so the Ruby hierarchy
/// tracks the engine (and the HTTP layer) instead of a parallel list.
fn classify(err: &CommerceError) -> ErrorKind {
    if matches!(
        err,
        CommerceError::InsufficientStock { .. }
            | CommerceError::Inventory(InventoryError::InsufficientStock { .. })
    ) {
        ErrorKind::InsufficientStock
    } else if err.is_not_found() {
        ErrorKind::NotFound
    } else if err.is_validation() {
        ErrorKind::Validation
    } else if err.is_conflict() {
        ErrorKind::Conflict
    } else if err.is_not_permitted() {
        ErrorKind::NotPermitted
    } else if err.is_database() {
        ErrorKind::Database
    } else if err.is_external_service() {
        ErrorKind::ExternalService
    } else if matches!(err, CommerceError::Internal(_)) {
        ErrorKind::Internal
    } else {
        ErrorKind::InvalidOperation
    }
}

/// `OrderNotFound(..)` -> `OrderNotFound`; `Order(NotFound(..))` -> `Order`.
fn variant_name(err: &CommerceError) -> String {
    let debug = format!("{err:?}");
    debug
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .next()
        .unwrap_or("CommerceError")
        .to_owned()
}

type CallResult = Result<Value, BindingError>;

/// Named arguments for one call.
struct Args<'a> {
    op: &'a str,
    map: Map<String, Value>,
}

impl<'a> Args<'a> {
    fn parse(op: &'a str, raw: &str) -> Result<Self, BindingError> {
        let value: Value = if raw.trim().is_empty() {
            Value::Object(Map::new())
        } else {
            serde_json::from_str(raw)
                .map_err(|e| BindingError::argument(format!("{op}: arguments are not JSON: {e}")))?
        };
        match value {
            Value::Object(map) => Ok(Self { op, map }),
            _ => Err(BindingError::argument(format!("{op}: arguments must be a JSON object"))),
        }
    }

    /// A required scalar argument (id, string, decimal, ...).
    fn req<T: DeserializeOwned>(&self, key: &str) -> Result<T, BindingError> {
        match self.map.get(key) {
            None | Some(Value::Null) => Err(BindingError::argument(format!(
                "{}: missing required argument `{key}`",
                self.op
            ))),
            Some(v) => serde_json::from_value(v.clone())
                .map_err(|e| BindingError::argument(format!("{}: invalid `{key}`: {e}", self.op))),
        }
    }

    /// An optional scalar argument; `nil` and absent both mean `None`.
    fn opt<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, BindingError> {
        match self.map.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(_) => self.req(key).map(Some),
        }
    }

    /// An engine input struct. Unknown fields anywhere inside it are refused.
    fn input<T: DeserializeOwned>(&self, key: &str) -> Result<T, BindingError> {
        let value = self.map.get(key).cloned().unwrap_or_else(|| Value::Object(Map::new()));
        strict_from_value(self.op, key, value)
    }

    /// An optional engine input struct (e.g. a list filter).
    fn input_or_default<T: DeserializeOwned + Default>(
        &self,
        key: &str,
    ) -> Result<T, BindingError> {
        match self.map.get(key) {
            None | Some(Value::Null) => Ok(T::default()),
            Some(v) => strict_from_value(self.op, key, v.clone()),
        }
    }
}

fn strict_from_value<T: DeserializeOwned>(
    op: &str,
    key: &str,
    value: Value,
) -> Result<T, BindingError> {
    let mut unknown = Vec::new();
    let parsed: T = serde_ignored::deserialize(value, |path| unknown.push(path.to_string()))
        .map_err(|e| BindingError::argument(format!("{op}: invalid `{key}`: {e}")))?;
    if unknown.is_empty() {
        Ok(parsed)
    } else {
        Err(BindingError::argument(format!(
            "{op}: unknown field(s) in `{key}`: {}",
            unknown.join(", ")
        )))
    }
}

fn ok<T: Serialize>(value: T) -> CallResult {
    serde_json::to_value(value).map_err(|e| BindingError {
        kind: ErrorKind::Internal,
        code: "binding.serialize".to_owned(),
        status: 500,
        message: format!("could not serialize engine result: {e}"),
    })
}

/// One open store. Wraps the real engine; there is no other state.
pub struct Engine {
    commerce: Commerce,
}

impl Engine {
    /// Open (creating if needed) the SQLite store at `path`; `":memory:"` is
    /// an ephemeral in-process store.
    pub fn open(path: &str) -> Result<Self, BindingError> {
        guard(|| Commerce::new(path).map(|commerce| Self { commerce }).map_err(BindingError::from))
    }

    /// Run one operation and return the JSON envelope. Never panics.
    #[must_use]
    pub fn call(&self, op: &str, args_json: &str) -> String {
        let result = guard(|| {
            let args = Args::parse(op, args_json)?;
            self.dispatch(op, &args)
        });
        envelope(result)
    }

    #[allow(clippy::too_many_lines)]
    fn dispatch(&self, op: &str, a: &Args<'_>) -> CallResult {
        let c = &self.commerce;
        match op {
            // ---------------------------------------------------------------
            // Customers
            // ---------------------------------------------------------------
            "customers.create" => ok(c.customers().create(a.input::<CreateCustomer>("input")?)?),
            "customers.get" => ok(c.customers().get(a.req::<CustomerId>("id")?)?),
            "customers.get_by_email" => {
                ok(c.customers().get_by_email(&a.req::<String>("email")?)?)
            }
            "customers.update" => ok(c
                .customers()
                .update(a.req::<CustomerId>("id")?, a.input::<UpdateCustomer>("input")?)?),
            "customers.list" => {
                ok(c.customers().list(a.input_or_default::<CustomerFilter>("filter")?)?)
            }
            "customers.count" => {
                ok(c.customers().count(a.input_or_default::<CustomerFilter>("filter")?)?)
            }
            "customers.delete" => ok(c.customers().delete(a.req::<CustomerId>("id")?)?),

            // ---------------------------------------------------------------
            // Products
            // ---------------------------------------------------------------
            "products.create" => ok(c.products().create(a.input::<CreateProduct>("input")?)?),
            "products.get" => ok(c.products().get(a.req::<ProductId>("id")?)?),
            "products.get_by_slug" => ok(c.products().get_by_slug(&a.req::<String>("slug")?)?),
            "products.update" => ok(c
                .products()
                .update(a.req::<ProductId>("id")?, a.input::<UpdateProduct>("input")?)?),
            "products.list" => {
                ok(c.products().list(a.input_or_default::<ProductFilter>("filter")?)?)
            }
            "products.count" => {
                ok(c.products().count(a.input_or_default::<ProductFilter>("filter")?)?)
            }
            "products.search" => ok(c.products().search(&a.req::<String>("query")?)?),
            "products.delete" => ok(c.products().delete(a.req::<ProductId>("id")?)?),
            "products.activate" => ok(c.products().activate(a.req::<ProductId>("id")?)?),
            "products.archive" => ok(c.products().archive(a.req::<ProductId>("id")?)?),
            "products.add_variant" => ok(c.products().add_variant(
                a.req::<ProductId>("product_id")?,
                a.input::<CreateProductVariant>("input")?,
            )?),
            "products.get_variant" => ok(c.products().get_variant(a.req::<Uuid>("id")?)?),
            "products.get_variant_by_sku" => {
                ok(c.products().get_variant_by_sku(&a.req::<String>("sku")?)?)
            }
            "products.get_variants" => {
                ok(c.products().get_variants(a.req::<ProductId>("product_id")?)?)
            }

            // ---------------------------------------------------------------
            // Inventory
            // ---------------------------------------------------------------
            "inventory.create_item" => {
                ok(c.inventory().create_item(a.input::<CreateInventoryItem>("input")?)?)
            }
            "inventory.get_item" => ok(c.inventory().get_item(a.req::<i64>("id")?)?),
            "inventory.get_item_by_sku" => {
                ok(c.inventory().get_item_by_sku(&a.req::<String>("sku")?)?)
            }
            "inventory.get_stock" => ok(c.inventory().get_stock(&a.req::<String>("sku")?)?),
            "inventory.list" => {
                ok(c.inventory().list(a.input_or_default::<InventoryFilter>("filter")?)?)
            }
            "inventory.adjust" => ok(c.inventory().adjust(
                &a.req::<String>("sku")?,
                a.req::<Decimal>("quantity")?,
                &a.req::<String>("reason")?,
            )?),
            "inventory.reserve" => ok(c.inventory().reserve(
                &a.req::<String>("sku")?,
                a.req::<Decimal>("quantity")?,
                &a.req::<String>("reference_type")?,
                &a.req::<String>("reference_id")?,
                a.opt::<i64>("expires_in_seconds")?,
            )?),
            "inventory.get_reservation" => {
                ok(c.inventory().get_reservation(a.req::<Uuid>("id")?)?)
            }
            "inventory.confirm_reservation" => {
                ok(c.inventory().confirm_reservation(a.req::<Uuid>("id")?)?)
            }
            "inventory.release_reservation" => {
                ok(c.inventory().release_reservation(a.req::<Uuid>("id")?)?)
            }
            "inventory.get_transactions" => ok(c.inventory().get_transactions(
                a.req::<i64>("item_id")?,
                a.opt::<u32>("limit")?.unwrap_or(100),
            )?),
            "inventory.has_stock" => ok(c
                .inventory()
                .has_stock(&a.req::<String>("sku")?, a.req::<Decimal>("quantity")?)?),

            // ---------------------------------------------------------------
            // Carts & checkout
            // ---------------------------------------------------------------
            "carts.create" => ok(c.carts().create(a.input::<CreateCart>("input")?)?),
            "carts.get" => ok(c.carts().get(a.req::<CartId>("id")?)?),
            "carts.get_by_number" => {
                ok(c.carts().get_by_number(&a.req::<String>("cart_number")?)?)
            }
            "carts.list" => ok(c.carts().list(a.input_or_default::<CartFilter>("filter")?)?),
            "carts.count" => ok(c.carts().count(a.input_or_default::<CartFilter>("filter")?)?),
            "carts.for_customer" => {
                ok(c.carts().for_customer(a.req::<CustomerId>("customer_id")?)?)
            }
            "carts.delete" => ok(c.carts().delete(a.req::<CartId>("id")?)?),
            "carts.add_item" => ok(c
                .carts()
                .add_item(a.req::<CartId>("cart_id")?, a.input::<AddCartItem>("input")?)?),
            "carts.update_item" => ok(c
                .carts()
                .update_item(a.req::<Uuid>("item_id")?, a.input::<UpdateCartItem>("input")?)?),
            "carts.remove_item" => ok(c.carts().remove_item(a.req::<Uuid>("item_id")?)?),
            "carts.get_items" => ok(c.carts().get_items(a.req::<CartId>("cart_id")?)?),
            "carts.clear_items" => ok(c.carts().clear_items(a.req::<CartId>("cart_id")?)?),
            "carts.set_shipping_address" => ok(c.carts().set_shipping_address(
                a.req::<CartId>("id")?,
                a.input::<CartAddress>("address")?,
            )?),
            "carts.set_billing_address" => ok(c
                .carts()
                .set_billing_address(a.req::<CartId>("id")?, a.input::<CartAddress>("address")?)?),
            "carts.set_shipping" => ok(c
                .carts()
                .set_shipping(a.req::<CartId>("id")?, a.input::<SetCartShipping>("input")?)?),
            "carts.get_shipping_rates" => {
                ok(c.carts().get_shipping_rates(a.req::<CartId>("id")?)?)
            }
            "carts.set_payment" => ok(c
                .carts()
                .set_payment(a.req::<CartId>("id")?, a.input::<SetCartPayment>("input")?)?),
            "carts.set_tax" => {
                ok(c.carts().set_tax(a.req::<CartId>("id")?, a.req::<Decimal>("tax_amount")?)?)
            }
            "carts.apply_discount" => ok(c
                .carts()
                .apply_discount(a.req::<CartId>("id")?, &a.req::<String>("coupon_code")?)?),
            "carts.remove_discount" => ok(c.carts().remove_discount(a.req::<CartId>("id")?)?),
            "carts.recalculate" => ok(c.carts().recalculate(a.req::<CartId>("id")?)?),
            "carts.reserve_inventory" => ok(c.carts().reserve_inventory(a.req::<CartId>("id")?)?),
            "carts.release_inventory" => ok(c.carts().release_inventory(a.req::<CartId>("id")?)?),
            "carts.mark_ready_for_payment" => {
                ok(c.carts().mark_ready_for_payment(a.req::<CartId>("id")?)?)
            }
            "carts.begin_checkout" => ok(c.carts().begin_checkout(a.req::<CartId>("id")?)?),
            "carts.complete" => ok(c.carts().complete(a.req::<CartId>("id")?)?),
            "carts.cancel" => ok(c.carts().cancel(a.req::<CartId>("id")?)?),
            "carts.abandon" => ok(c.carts().abandon(a.req::<CartId>("id")?)?),

            // ---------------------------------------------------------------
            // Orders
            // ---------------------------------------------------------------
            "orders.create" => ok(c.orders().create(a.input::<CreateOrder>("input")?)?),
            "orders.get" => ok(c.orders().get(a.req::<OrderId>("id")?)?),
            "orders.get_by_number" => {
                ok(c.orders().get_by_number(&a.req::<String>("order_number")?)?)
            }
            "orders.list" => ok(c.orders().list(a.input_or_default::<OrderFilter>("filter")?)?),
            "orders.count" => ok(c.orders().count(a.input_or_default::<OrderFilter>("filter")?)?),
            "orders.list_for_customer" => {
                ok(c.orders().list_for_customer(a.req::<CustomerId>("customer_id")?)?)
            }
            "orders.update_status" => ok(c
                .orders()
                .update_status(a.req::<OrderId>("id")?, a.req::<OrderStatus>("status")?)?),
            "orders.cancel" => ok(c.orders().cancel(a.req::<OrderId>("id")?)?),
            "orders.ship" => {
                let tracking = a.opt::<String>("tracking_number")?;
                let lines = match a.map.get("lines") {
                    None | Some(Value::Null) => None,
                    Some(_) => Some(a.input::<Vec<ShipmentLineInput>>("lines")?),
                };
                ok(c.orders().ship_lines(a.req::<OrderId>("id")?, tracking.as_deref(), lines)?)
            }
            "orders.deliver" => ok(c.orders().deliver(a.req::<OrderId>("id")?)?),

            // ---------------------------------------------------------------
            // Payments & refunds
            // ---------------------------------------------------------------
            "payments.create" => ok(c.payments().create(a.input::<CreatePayment>("input")?)?),
            "payments.get" => ok(c.payments().get(a.req::<PaymentId>("id")?)?),
            "payments.get_by_number" => {
                ok(c.payments().get_by_number(&a.req::<String>("payment_number")?)?)
            }
            "payments.list" => {
                ok(c.payments().list(a.input_or_default::<PaymentFilter>("filter")?)?)
            }
            "payments.count" => {
                ok(c.payments().count(a.input_or_default::<PaymentFilter>("filter")?)?)
            }
            "payments.for_order" => ok(c.payments().for_order(a.req::<OrderId>("order_id")?)?),
            "payments.mark_processing" => {
                ok(c.payments().mark_processing(a.req::<PaymentId>("id")?)?)
            }
            "payments.mark_completed" => {
                ok(c.payments().mark_completed(a.req::<PaymentId>("id")?)?)
            }
            "payments.mark_failed" => ok(c.payments().mark_failed(
                a.req::<PaymentId>("id")?,
                &a.req::<String>("reason")?,
                a.opt::<String>("code")?.as_deref(),
            )?),
            "payments.cancel" => ok(c.payments().cancel(a.req::<PaymentId>("id")?)?),
            "payments.create_refund" => {
                ok(c.payments().create_refund(a.input::<CreateRefund>("input")?)?)
            }
            "payments.get_refund" => ok(c.payments().get_refund(a.req::<Uuid>("id")?)?),
            "payments.get_refunds" => {
                ok(c.payments().get_refunds(a.req::<PaymentId>("payment_id")?)?)
            }
            "payments.complete_refund" => ok(c.payments().complete_refund(a.req::<Uuid>("id")?)?),
            "payments.fail_refund" => {
                ok(c.payments().fail_refund(a.req::<Uuid>("id")?, &a.req::<String>("reason")?)?)
            }

            // ---------------------------------------------------------------
            // Returns
            // ---------------------------------------------------------------
            "returns.create" => ok(c.returns().create(a.input::<CreateReturn>("input")?)?),
            "returns.get" => ok(c.returns().get(a.req::<ReturnId>("id")?)?),
            "returns.list" => {
                ok(c.returns().list(a.input_or_default::<ReturnFilter>("filter")?)?)
            }
            "returns.count" => {
                ok(c.returns().count(a.input_or_default::<ReturnFilter>("filter")?)?)
            }
            "returns.list_for_order" => {
                ok(c.returns().list_for_order(a.req::<OrderId>("order_id")?)?)
            }
            "returns.approve" => ok(c.returns().approve(a.req::<ReturnId>("id")?)?),
            "returns.reject" => {
                ok(c.returns().reject(a.req::<ReturnId>("id")?, &a.req::<String>("reason")?)?)
            }
            "returns.mark_received" => ok(c.returns().mark_received(a.req::<ReturnId>("id")?)?),
            "returns.complete" => ok(c.returns().complete(a.req::<ReturnId>("id")?)?),
            "returns.cancel" => ok(c.returns().cancel(a.req::<ReturnId>("id")?)?),
            "returns.set_item_disposition" => ok(c.returns().set_item_disposition(
                a.req::<ReturnId>("id")?,
                a.req::<Uuid>("item_id")?,
                a.input::<SetReturnDisposition>("input")?,
            )?),
            "returns.add_tracking" => ok(c
                .returns()
                .add_tracking(a.req::<ReturnId>("id")?, &a.req::<String>("tracking_number")?)?),

            // ---------------------------------------------------------------
            // Shipments
            // ---------------------------------------------------------------
            "shipments.create" => ok(c.shipments().create(a.input::<CreateShipment>("input")?)?),
            "shipments.get" => ok(c.shipments().get(a.req::<ShipmentId>("id")?)?),
            "shipments.get_by_number" => {
                ok(c.shipments().get_by_number(&a.req::<String>("shipment_number")?)?)
            }
            "shipments.get_by_tracking" => {
                ok(c.shipments().get_by_tracking(&a.req::<String>("tracking_number")?)?)
            }
            "shipments.list" => {
                ok(c.shipments().list(a.input_or_default::<ShipmentFilter>("filter")?)?)
            }
            "shipments.count" => {
                ok(c.shipments().count(a.input_or_default::<ShipmentFilter>("filter")?)?)
            }
            "shipments.for_order" => ok(c.shipments().for_order(a.req::<OrderId>("order_id")?)?),
            "shipments.get_items" => ok(c.shipments().get_items(a.req::<ShipmentId>("id")?)?),
            "shipments.add_item" => ok(c
                .shipments()
                .add_item(a.req::<ShipmentId>("id")?, a.input::<CreateShipmentItem>("input")?)?),
            "shipments.remove_item" => ok(c.shipments().remove_item(a.req::<Uuid>("item_id")?)?),
            "shipments.mark_processing" => {
                ok(c.shipments().mark_processing(a.req::<ShipmentId>("id")?)?)
            }
            "shipments.mark_ready" => ok(c.shipments().mark_ready(a.req::<ShipmentId>("id")?)?),
            "shipments.ship" => ok(c
                .shipments()
                .ship(a.req::<ShipmentId>("id")?, a.opt::<String>("tracking_number")?)?),
            "shipments.mark_in_transit" => {
                ok(c.shipments().mark_in_transit(a.req::<ShipmentId>("id")?)?)
            }
            "shipments.mark_out_for_delivery" => {
                ok(c.shipments().mark_out_for_delivery(a.req::<ShipmentId>("id")?)?)
            }
            "shipments.mark_delivered" => {
                ok(c.shipments().mark_delivered(a.req::<ShipmentId>("id")?)?)
            }
            "shipments.mark_failed" => ok(c.shipments().mark_failed(a.req::<ShipmentId>("id")?)?),
            "shipments.hold" => ok(c.shipments().hold(a.req::<ShipmentId>("id")?)?),
            "shipments.cancel" => ok(c.shipments().cancel(a.req::<ShipmentId>("id")?)?),

            _ => Err(BindingError::unknown_op(op)),
        }
    }
}

/// Every operation name `dispatch` accepts. Kept in step with the `match`
/// by `tests::every_listed_op_is_dispatched`; the Ruby specs assert the Ruby
/// surface calls only these.
pub const OPERATIONS: &[&str] = &[
    "customers.create",
    "customers.get",
    "customers.get_by_email",
    "customers.update",
    "customers.list",
    "customers.count",
    "customers.delete",
    "products.create",
    "products.get",
    "products.get_by_slug",
    "products.update",
    "products.list",
    "products.count",
    "products.search",
    "products.delete",
    "products.activate",
    "products.archive",
    "products.add_variant",
    "products.get_variant",
    "products.get_variant_by_sku",
    "products.get_variants",
    "inventory.create_item",
    "inventory.get_item",
    "inventory.get_item_by_sku",
    "inventory.get_stock",
    "inventory.list",
    "inventory.adjust",
    "inventory.reserve",
    "inventory.get_reservation",
    "inventory.confirm_reservation",
    "inventory.release_reservation",
    "inventory.get_transactions",
    "inventory.has_stock",
    "carts.create",
    "carts.get",
    "carts.get_by_number",
    "carts.list",
    "carts.count",
    "carts.for_customer",
    "carts.delete",
    "carts.add_item",
    "carts.update_item",
    "carts.remove_item",
    "carts.get_items",
    "carts.clear_items",
    "carts.set_shipping_address",
    "carts.set_billing_address",
    "carts.set_shipping",
    "carts.get_shipping_rates",
    "carts.set_payment",
    "carts.set_tax",
    "carts.apply_discount",
    "carts.remove_discount",
    "carts.recalculate",
    "carts.reserve_inventory",
    "carts.release_inventory",
    "carts.mark_ready_for_payment",
    "carts.begin_checkout",
    "carts.complete",
    "carts.cancel",
    "carts.abandon",
    "orders.create",
    "orders.get",
    "orders.get_by_number",
    "orders.list",
    "orders.count",
    "orders.list_for_customer",
    "orders.update_status",
    "orders.cancel",
    "orders.ship",
    "orders.deliver",
    "payments.create",
    "payments.get",
    "payments.get_by_number",
    "payments.list",
    "payments.count",
    "payments.for_order",
    "payments.mark_processing",
    "payments.mark_completed",
    "payments.mark_failed",
    "payments.cancel",
    "payments.create_refund",
    "payments.get_refund",
    "payments.get_refunds",
    "payments.complete_refund",
    "payments.fail_refund",
    "returns.create",
    "returns.get",
    "returns.list",
    "returns.count",
    "returns.list_for_order",
    "returns.approve",
    "returns.reject",
    "returns.mark_received",
    "returns.complete",
    "returns.cancel",
    "returns.add_tracking",
    "returns.set_item_disposition",
    "shipments.create",
    "shipments.get",
    "shipments.get_by_number",
    "shipments.get_by_tracking",
    "shipments.list",
    "shipments.count",
    "shipments.for_order",
    "shipments.get_items",
    "shipments.add_item",
    "shipments.remove_item",
    "shipments.mark_processing",
    "shipments.mark_ready",
    "shipments.ship",
    "shipments.mark_in_transit",
    "shipments.mark_out_for_delivery",
    "shipments.mark_delivered",
    "shipments.mark_failed",
    "shipments.hold",
    "shipments.cancel",
];

/// Run `f`, turning a panic into an `internal` error.
fn guard<T>(f: impl FnOnce() -> Result<T, BindingError>) -> Result<T, BindingError> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|payload| {
        let detail = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic payload".to_owned());
        Err(BindingError::panic(&detail))
    })
}

fn envelope(result: CallResult) -> String {
    let value = match result {
        Ok(value) => json!({ "ok": true, "value": value }),
        Err(error) => json!({ "ok": false, "error": error }),
    };
    // Serializing a `Value` cannot fail; fall back to a fixed envelope anyway
    // so this function is total.
    serde_json::to_string(&value).unwrap_or_else(|_| {
        r#"{"ok":false,"error":{"kind":"internal","code":"binding.serialize","status":500,"message":"could not encode reply"}}"#.to_owned()
    })
}

/// Encode an `open` failure in the same envelope shape as `call`.
#[must_use]
pub fn error_envelope(error: BindingError) -> String {
    envelope(Err(error))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(engine: &Engine, op: &str, args: Value) -> Value {
        let reply: Value =
            serde_json::from_str(&engine.call(op, &args.to_string())).expect("envelope is JSON");
        reply
    }

    fn value(engine: &Engine, op: &str, args: Value) -> Value {
        let reply = call(engine, op, args);
        assert_eq!(reply["ok"], json!(true), "{op} failed: {reply}");
        reply["value"].clone()
    }

    #[test]
    fn every_listed_op_is_dispatched() {
        let engine = Engine::open(":memory:").expect("open");
        for op in OPERATIONS {
            let reply = call(&engine, op, json!({}));
            assert_ne!(
                reply["error"]["code"],
                json!("binding.unknown_operation"),
                "{op} is listed but not dispatched"
            );
        }
        let reply = call(&engine, "customers.nope", json!({}));
        assert_eq!(reply["error"]["code"], json!("binding.unknown_operation"));
    }

    #[test]
    fn persists_to_the_sqlite_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("store.db");
        let path = path.to_str().expect("utf8 path");
        let id = {
            let engine = Engine::open(path).expect("open");
            let customer = value(
                &engine,
                "customers.create",
                json!({"input": {"email": "p@example.com", "first_name": "P", "last_name": "Q"}}),
            );
            customer["id"].as_str().expect("id").to_owned()
        };
        let reopened = Engine::open(path).expect("reopen");
        let found = value(&reopened, "customers.get", json!({"id": id}));
        assert_eq!(found["email"], json!("p@example.com"));
    }

    #[test]
    fn unknown_input_fields_are_refused() {
        let engine = Engine::open(":memory:").expect("open");
        let reply = call(
            &engine,
            "customers.create",
            json!({"input": {"email": "a@example.com", "first_name": "A", "last_name": "B", "not_a_field": "x"}}),
        );
        assert_eq!(reply["error"]["kind"], json!("validation"));
        assert!(reply["error"]["message"].as_str().unwrap_or_default().contains("not_a_field"));
    }

    #[test]
    fn engine_errors_are_classified() {
        let engine = Engine::open(":memory:").expect("open");
        let input =
            json!({"input": {"email": "dup@example.com", "first_name": "A", "last_name": "B"}});
        value(&engine, "customers.create", input.clone());
        let dup = call(&engine, "customers.create", input);
        assert_eq!(dup["error"]["kind"], json!("conflict"), "{dup}");

        let invalid = call(
            &engine,
            "customers.create",
            json!({"input": {"email": "not-an-email", "first_name": "A", "last_name": "B"}}),
        );
        assert_eq!(invalid["error"]["kind"], json!("validation"), "{invalid}");

        let bad_id = call(&engine, "customers.get", json!({"id": "nope"}));
        assert_eq!(bad_id["error"]["code"], json!("binding.invalid_argument"));
    }

    #[test]
    fn money_round_trips_exactly() {
        let engine = Engine::open(":memory:").expect("open");
        let payment = value(
            &engine,
            "payments.create",
            json!({"input": {"amount": "0.30", "payment_method": "credit_card", "currency": "USD"}}),
        );
        assert_eq!(payment["amount"], json!("0.30"));
    }

    #[test]
    fn panics_become_internal_errors() {
        let result: Result<(), BindingError> = guard(|| panic!("boom"));
        let err = result.expect_err("panic is caught");
        assert_eq!(err.kind, ErrorKind::Internal);
        assert!(err.message.contains("boom"));
    }
}
