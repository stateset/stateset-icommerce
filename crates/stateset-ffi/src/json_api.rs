//! JSON-over-C-ABI surface shared by the .NET and Swift bindings.
//!
//! The typed `#[repr(C)]` API in [`crate::api`] covers a handful of entities.
//! Language bindings that want the whole commerce surface (customers through
//! shipments) need something that does not grow a new unsafe export per
//! method, so this module exposes exactly three entry points:
//!
//! - [`stateset_json_open`] opens (or creates) a store and returns a handle;
//! - [`stateset_json_call`] invokes `"<domain>.<method>"` with a JSON object of
//!   arguments and returns a JSON envelope;
//! - [`stateset_destroy`](crate::api::stateset_destroy) and
//!   [`stateset_string_free`](crate::strings::stateset_string_free) release the
//!   handle and the returned envelope.
//!
//! Every method dispatches in safe Rust to `stateset_embedded::Commerce`; the
//! only unsafe code is the pointer handling at the three exports.
//!
//! # Envelope
//!
//! Success: `{"ok":true,"result":<value>}` (`result` is `null` for a lookup
//! that found nothing). Failure:
//! `{"ok":false,"error":{"code":<FfiErrorCode as int>,"kind":"not_found","message":"..."}}`.
//!
//! # Money
//!
//! Every monetary amount crosses the boundary as an exact decimal **string**
//! (`"19.99"`), in both directions. The engine refuses JSON numbers for money
//! arguments, so a float can never be coerced into a price.

use std::os::raw::c_char;

use rust_decimal::Decimal;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use stateset_core::{
    AddCartItem, CartAddress, CartFilter, CartId, CommerceError, CreateCart, CreateCustomer,
    CreateInventoryItem, CreateOrder, CreatePayment, CreateProduct, CreateProductVariant,
    CreateRefund, CreateReturn, CreateShipment, CustomerFilter, CustomerId, InventoryFilter,
    OrderFilter, OrderId, OrderStatus, PaymentFilter, PaymentId, ProductFilter, ProductId,
    ReturnFilter, ReturnId, SetCartPayment, SetCartShipping, SetReturnDisposition, ShipmentFilter,
    ShipmentId, UpdateCartItem, UpdateCustomer, UpdateProduct,
};
use stateset_embedded::Commerce;
use uuid::Uuid;

use crate::api::{CommerceHandle, begin_engine_use, init_engine, register_new_engine_handle};
use crate::error::{FfiErrorCode, catch_ffi_mut_ptr, clear_last_error, set_last_error};
use crate::strings::{c_string_to_rust, rust_to_c_string};

/// Version of the JSON method catalog. Bumped when a method is removed or its
/// argument/result shape changes incompatibly; additions do not bump it.
pub const JSON_API_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// A failed call: an ABI error code plus a human-readable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallError {
    /// Stable numeric code (same values as [`FfiErrorCode`]).
    pub code: FfiErrorCode,
    /// Message for humans; not a stable contract.
    pub message: String,
}

impl CallError {
    fn invalid(message: impl Into<String>) -> Self {
        Self { code: FfiErrorCode::InvalidArgument, message: message.into() }
    }
}

impl From<CommerceError> for CallError {
    fn from(err: CommerceError) -> Self {
        Self { code: FfiErrorCode::from(&err), message: err.to_string() }
    }
}

const fn kind_of(code: FfiErrorCode) -> &'static str {
    match code {
        FfiErrorCode::Ok => "ok",
        FfiErrorCode::NotFound => "not_found",
        FfiErrorCode::InvalidArgument => "invalid_argument",
        FfiErrorCode::DatabaseError => "database_error",
        FfiErrorCode::SerializationError => "serialization_error",
        FfiErrorCode::NullPointer => "null_pointer",
        FfiErrorCode::Utf8Error => "utf8_error",
        FfiErrorCode::BufferTooSmall => "buffer_too_small",
        _ => "internal_error",
    }
}

type CallResult = Result<Value, CallError>;

// ---------------------------------------------------------------------------
// Argument helpers
// ---------------------------------------------------------------------------

fn parse<T: DeserializeOwned>(method: &str, args: Value) -> Result<T, CallError> {
    serde_json::from_value(args)
        .map_err(|e| CallError::invalid(format!("invalid arguments for {method}: {e}")))
}

fn to_value<T: serde::Serialize>(value: &T) -> CallResult {
    serde_json::to_value(value).map_err(|e| CallError {
        code: FfiErrorCode::SerializationError,
        message: format!("failed to serialize result: {e}"),
    })
}

const fn ok_unit() -> CallResult {
    Ok(Value::Bool(true))
}

/// Reject JSON numbers wherever the engine expects money or quantity:
/// decimals must be exact strings so no binding can route them through a
/// float. Walks the whole argument tree once before dispatch.
fn reject_float_numbers(method: &str, value: &Value, path: &str) -> Result<(), CallError> {
    match value {
        Value::Number(n) if !(n.is_i64() || n.is_u64()) => Err(CallError::invalid(format!(
            "{method}: `{path}` is a floating-point number; pass decimals as exact strings"
        ))),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                reject_float_numbers(method, item, &format!("{path}[{i}]"))?;
            }
            Ok(())
        }
        Value::Object(map) => {
            for (k, v) in map {
                // Free-form caller metadata is opaque to the engine.
                if k == "metadata" {
                    continue;
                }
                let child = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                reject_float_numbers(method, v, &child)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[derive(Deserialize)]
struct IdArg<T> {
    id: T,
}

fn id<T: DeserializeOwned>(method: &str, args: Value) -> Result<T, CallError> {
    parse::<IdArg<T>>(method, args).map(|a| a.id)
}

fn filter_or_default<T: DeserializeOwned + Default>(
    method: &str,
    args: Value,
) -> Result<T, CallError> {
    match args {
        Value::Null => Ok(T::default()),
        Value::Object(ref m) if m.is_empty() => Ok(T::default()),
        other => parse(method, other),
    }
}

fn found<T: serde::Serialize>(value: Option<T>) -> CallResult {
    match value {
        Some(v) => to_value(&v),
        None => Ok(Value::Null),
    }
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Every method name [`call`] accepts, for discovery and binding parity checks.
pub const METHODS: &[&str] = &[
    // customers
    "customers.create",
    "customers.get",
    "customers.get_by_email",
    "customers.update",
    "customers.list",
    "customers.count",
    "customers.delete",
    // products
    "products.create",
    "products.get",
    "products.get_by_slug",
    "products.update",
    "products.list",
    "products.count",
    "products.delete",
    "products.activate",
    "products.archive",
    "products.search",
    "products.add_variant",
    "products.get_variants",
    "products.get_variant_by_sku",
    // inventory
    "inventory.create_item",
    "inventory.get_item_by_sku",
    "inventory.list",
    "inventory.get_stock",
    "inventory.adjust",
    "inventory.has_stock",
    "inventory.reserve",
    "inventory.release_reservation",
    "inventory.confirm_reservation",
    "inventory.get_reservation",
    // carts / checkout
    "carts.create",
    "carts.get",
    "carts.list",
    "carts.count",
    "carts.add_item",
    "carts.update_item",
    "carts.remove_item",
    "carts.get_items",
    "carts.clear_items",
    "carts.set_shipping_address",
    "carts.set_billing_address",
    "carts.set_shipping",
    "carts.set_payment",
    "carts.apply_discount",
    "carts.remove_discount",
    "carts.recalculate",
    "carts.mark_ready_for_payment",
    "carts.begin_checkout",
    "carts.complete",
    "carts.cancel",
    "carts.abandon",
    // orders
    "orders.create",
    "orders.get",
    "orders.get_by_number",
    "orders.list",
    "orders.list_for_customer",
    "orders.count",
    "orders.update_status",
    "orders.cancel",
    "orders.ship",
    "orders.deliver",
    // payments + refunds
    "payments.create",
    "payments.get",
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
    // returns
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
    // shipments
    "shipments.create",
    "shipments.get",
    "shipments.get_by_tracking",
    "shipments.list",
    "shipments.count",
    "shipments.for_order",
    "shipments.mark_processing",
    "shipments.mark_ready",
    "shipments.ship",
    "shipments.mark_in_transit",
    "shipments.mark_out_for_delivery",
    "shipments.mark_delivered",
    "shipments.cancel",
    // meta
    "meta.methods",
    "meta.version",
];

/// Invoke one engine method. `args` must be a JSON object (or null for
/// methods without arguments). This is the safe core of [`stateset_json_call`]
/// and is usable directly from Rust tests.
///
/// # Errors
///
/// Returns a [`CallError`] for an unknown method, malformed arguments, or any
/// engine error.
pub fn call(c: &Commerce, method: &str, args: Value) -> CallResult {
    reject_float_numbers(method, &args, "")?;
    let (domain, op) = method
        .split_once('.')
        .ok_or_else(|| CallError::invalid(format!("method must be `domain.op`: {method}")))?;
    match domain {
        "customers" => customers(c, method, op, args),
        "products" => products(c, method, op, args),
        "inventory" => inventory(c, method, op, args),
        "carts" => carts(c, method, op, args),
        "orders" => orders(c, method, op, args),
        "payments" => payments(c, method, op, args),
        "returns" => returns(c, method, op, args),
        "shipments" => shipments(c, method, op, args),
        "meta" => match op {
            "methods" => Ok(json!(METHODS)),
            "version" => Ok(json!({
                "json_api_version": JSON_API_VERSION,
                "abi_version": crate::version::ABI_VERSION,
                "engine_version": env!("CARGO_PKG_VERSION"),
            })),
            _ => Err(unknown(method)),
        },
        _ => Err(unknown(method)),
    }
}

fn unknown(method: &str) -> CallError {
    CallError::invalid(format!("unknown method: {method}"))
}

fn customers(c: &Commerce, m: &str, op: &str, args: Value) -> CallResult {
    let api = c.customers();
    match op {
        "create" => to_value(&api.create(parse::<CreateCustomer>(m, args)?)?),
        "get" => found(api.get(id::<CustomerId>(m, args)?)?),
        "get_by_email" => {
            #[derive(Deserialize)]
            struct A {
                email: String,
            }
            found(api.get_by_email(&parse::<A>(m, args)?.email)?)
        }
        "update" => {
            #[derive(Deserialize)]
            struct A {
                id: CustomerId,
                input: UpdateCustomer,
            }
            let a: A = parse(m, args)?;
            to_value(&api.update(a.id, a.input)?)
        }
        "list" => to_value(&api.list(filter_or_default::<CustomerFilter>(m, args)?)?),
        "count" => to_value(&api.count(filter_or_default::<CustomerFilter>(m, args)?)?),
        "delete" => {
            api.delete(id::<CustomerId>(m, args)?)?;
            ok_unit()
        }
        _ => Err(unknown(m)),
    }
}

fn products(c: &Commerce, m: &str, op: &str, args: Value) -> CallResult {
    let api = c.products();
    match op {
        "create" => to_value(&api.create(parse::<CreateProduct>(m, args)?)?),
        "get" => found(api.get(id::<ProductId>(m, args)?)?),
        "get_by_slug" => {
            #[derive(Deserialize)]
            struct A {
                slug: String,
            }
            found(api.get_by_slug(&parse::<A>(m, args)?.slug)?)
        }
        "update" => {
            #[derive(Deserialize)]
            struct A {
                id: ProductId,
                input: UpdateProduct,
            }
            let a: A = parse(m, args)?;
            to_value(&api.update(a.id, a.input)?)
        }
        "list" => to_value(&api.list(filter_or_default::<ProductFilter>(m, args)?)?),
        "count" => to_value(&api.count(filter_or_default::<ProductFilter>(m, args)?)?),
        "delete" => {
            api.delete(id::<ProductId>(m, args)?)?;
            ok_unit()
        }
        "activate" => to_value(&api.activate(id::<ProductId>(m, args)?)?),
        "archive" => to_value(&api.archive(id::<ProductId>(m, args)?)?),
        "search" => {
            #[derive(Deserialize)]
            struct A {
                query: String,
            }
            to_value(&api.search(&parse::<A>(m, args)?.query)?)
        }
        "add_variant" => {
            #[derive(Deserialize)]
            struct A {
                product_id: ProductId,
                variant: CreateProductVariant,
            }
            let a: A = parse(m, args)?;
            to_value(&api.add_variant(a.product_id, a.variant)?)
        }
        "get_variants" => {
            #[derive(Deserialize)]
            struct A {
                product_id: ProductId,
            }
            to_value(&api.get_variants(parse::<A>(m, args)?.product_id)?)
        }
        "get_variant_by_sku" => found(api.get_variant_by_sku(&sku_arg(m, args)?)?),
        _ => Err(unknown(m)),
    }
}

fn sku_arg(m: &str, args: Value) -> Result<String, CallError> {
    #[derive(Deserialize)]
    struct A {
        sku: String,
    }
    parse::<A>(m, args).map(|a| a.sku)
}

fn inventory(c: &Commerce, m: &str, op: &str, args: Value) -> CallResult {
    let api = c.inventory();
    match op {
        "create_item" => to_value(&api.create_item(parse::<CreateInventoryItem>(m, args)?)?),
        "get_item_by_sku" => found(api.get_item_by_sku(&sku_arg(m, args)?)?),
        "list" => to_value(&api.list(filter_or_default::<InventoryFilter>(m, args)?)?),
        "get_stock" => found(api.get_stock(&sku_arg(m, args)?)?),
        "adjust" => {
            #[derive(Deserialize)]
            struct A {
                sku: String,
                quantity: Decimal,
                reason: String,
            }
            let a: A = parse(m, args)?;
            to_value(&api.adjust(&a.sku, a.quantity, &a.reason)?)
        }
        "has_stock" => {
            #[derive(Deserialize)]
            struct A {
                sku: String,
                quantity: Decimal,
            }
            let a: A = parse(m, args)?;
            to_value(&api.has_stock(&a.sku, a.quantity)?)
        }
        "reserve" => {
            #[derive(Deserialize)]
            struct A {
                sku: String,
                quantity: Decimal,
                reference_type: String,
                reference_id: String,
                expires_in_seconds: Option<i64>,
            }
            let a: A = parse(m, args)?;
            to_value(&api.reserve(
                &a.sku,
                a.quantity,
                &a.reference_type,
                &a.reference_id,
                a.expires_in_seconds,
            )?)
        }
        "release_reservation" => {
            api.release_reservation(id::<Uuid>(m, args)?)?;
            ok_unit()
        }
        "confirm_reservation" => {
            api.confirm_reservation(id::<Uuid>(m, args)?)?;
            ok_unit()
        }
        "get_reservation" => found(api.get_reservation(id::<Uuid>(m, args)?)?),
        _ => Err(unknown(m)),
    }
}

fn carts(c: &Commerce, m: &str, op: &str, args: Value) -> CallResult {
    let api = c.carts();
    match op {
        "create" => to_value(&api.create(parse::<CreateCart>(m, args)?)?),
        "get" => found(api.get(id::<CartId>(m, args)?)?),
        "list" => to_value(&api.list(filter_or_default::<CartFilter>(m, args)?)?),
        "count" => to_value(&api.count(filter_or_default::<CartFilter>(m, args)?)?),
        "add_item" => {
            #[derive(Deserialize)]
            struct A {
                cart_id: CartId,
                item: AddCartItem,
            }
            let a: A = parse(m, args)?;
            to_value(&api.add_item(a.cart_id, a.item)?)
        }
        "update_item" => {
            #[derive(Deserialize)]
            struct A {
                item_id: Uuid,
                input: UpdateCartItem,
            }
            let a: A = parse(m, args)?;
            to_value(&api.update_item(a.item_id, a.input)?)
        }
        "remove_item" => {
            #[derive(Deserialize)]
            struct A {
                item_id: Uuid,
            }
            api.remove_item(parse::<A>(m, args)?.item_id)?;
            ok_unit()
        }
        "get_items" => to_value(&api.get_items(id::<CartId>(m, args)?)?),
        "clear_items" => {
            api.clear_items(id::<CartId>(m, args)?)?;
            ok_unit()
        }
        "set_shipping_address" | "set_billing_address" => {
            #[derive(Deserialize)]
            struct A {
                id: CartId,
                address: CartAddress,
            }
            let a: A = parse(m, args)?;
            if op == "set_shipping_address" {
                to_value(&api.set_shipping_address(a.id, a.address)?)
            } else {
                to_value(&api.set_billing_address(a.id, a.address)?)
            }
        }
        "set_shipping" => {
            #[derive(Deserialize)]
            struct A {
                id: CartId,
                shipping: SetCartShipping,
            }
            let a: A = parse(m, args)?;
            to_value(&api.set_shipping(a.id, a.shipping)?)
        }
        "set_payment" => {
            #[derive(Deserialize)]
            struct A {
                id: CartId,
                payment: SetCartPayment,
            }
            let a: A = parse(m, args)?;
            to_value(&api.set_payment(a.id, a.payment)?)
        }
        "apply_discount" => {
            #[derive(Deserialize)]
            struct A {
                id: CartId,
                coupon_code: String,
            }
            let a: A = parse(m, args)?;
            to_value(&api.apply_discount(a.id, &a.coupon_code)?)
        }
        "remove_discount" => to_value(&api.remove_discount(id::<CartId>(m, args)?)?),
        "recalculate" => to_value(&api.recalculate(id::<CartId>(m, args)?)?),
        "mark_ready_for_payment" => to_value(&api.mark_ready_for_payment(id::<CartId>(m, args)?)?),
        "begin_checkout" => to_value(&api.begin_checkout(id::<CartId>(m, args)?)?),
        "complete" => to_value(&api.complete(id::<CartId>(m, args)?)?),
        "cancel" => to_value(&api.cancel(id::<CartId>(m, args)?)?),
        "abandon" => to_value(&api.abandon(id::<CartId>(m, args)?)?),
        _ => Err(unknown(m)),
    }
}

fn orders(c: &Commerce, m: &str, op: &str, args: Value) -> CallResult {
    let api = c.orders();
    match op {
        "create" => to_value(&api.create(parse::<CreateOrder>(m, args)?)?),
        "get" => found(api.get(id::<OrderId>(m, args)?)?),
        "get_by_number" => {
            #[derive(Deserialize)]
            struct A {
                order_number: String,
            }
            found(api.get_by_number(&parse::<A>(m, args)?.order_number)?)
        }
        "list" => to_value(&api.list(filter_or_default::<OrderFilter>(m, args)?)?),
        "list_for_customer" => {
            #[derive(Deserialize)]
            struct A {
                customer_id: CustomerId,
            }
            to_value(&api.list_for_customer(parse::<A>(m, args)?.customer_id)?)
        }
        "count" => to_value(&api.count(filter_or_default::<OrderFilter>(m, args)?)?),
        "update_status" => {
            #[derive(Deserialize)]
            struct A {
                id: OrderId,
                status: OrderStatus,
            }
            let a: A = parse(m, args)?;
            to_value(&api.update_status(a.id, a.status)?)
        }
        "cancel" => to_value(&api.cancel(id::<OrderId>(m, args)?)?),
        "ship" => {
            #[derive(Deserialize)]
            struct A {
                id: OrderId,
                tracking_number: Option<String>,
            }
            let a: A = parse(m, args)?;
            to_value(&api.ship(a.id, a.tracking_number.as_deref())?)
        }
        "deliver" => to_value(&api.deliver(id::<OrderId>(m, args)?)?),
        _ => Err(unknown(m)),
    }
}

fn payments(c: &Commerce, m: &str, op: &str, args: Value) -> CallResult {
    let api = c.payments();
    match op {
        "create" => to_value(&api.create(parse::<CreatePayment>(m, args)?)?),
        "get" => found(api.get(id::<PaymentId>(m, args)?)?),
        "list" => to_value(&api.list(filter_or_default::<PaymentFilter>(m, args)?)?),
        "count" => to_value(&api.count(filter_or_default::<PaymentFilter>(m, args)?)?),
        "for_order" => {
            #[derive(Deserialize)]
            struct A {
                order_id: OrderId,
            }
            to_value(&api.for_order(parse::<A>(m, args)?.order_id)?)
        }
        "mark_processing" => to_value(&api.mark_processing(id::<PaymentId>(m, args)?)?),
        "mark_completed" => to_value(&api.mark_completed(id::<PaymentId>(m, args)?)?),
        "mark_failed" => {
            #[derive(Deserialize)]
            struct A {
                id: PaymentId,
                reason: String,
                code: Option<String>,
            }
            let a: A = parse(m, args)?;
            to_value(&api.mark_failed(a.id, &a.reason, a.code.as_deref())?)
        }
        "cancel" => to_value(&api.cancel(id::<PaymentId>(m, args)?)?),
        "create_refund" => to_value(&api.create_refund(parse::<CreateRefund>(m, args)?)?),
        "get_refund" => found(api.get_refund(id::<Uuid>(m, args)?)?),
        "get_refunds" => {
            #[derive(Deserialize)]
            struct A {
                payment_id: PaymentId,
            }
            to_value(&api.get_refunds(parse::<A>(m, args)?.payment_id)?)
        }
        "complete_refund" => to_value(&api.complete_refund(id::<Uuid>(m, args)?)?),
        "fail_refund" => {
            #[derive(Deserialize)]
            struct A {
                id: Uuid,
                reason: String,
            }
            let a: A = parse(m, args)?;
            to_value(&api.fail_refund(a.id, &a.reason)?)
        }
        _ => Err(unknown(m)),
    }
}

fn returns(c: &Commerce, m: &str, op: &str, args: Value) -> CallResult {
    let api = c.returns();
    match op {
        "create" => to_value(&api.create(parse::<CreateReturn>(m, args)?)?),
        "get" => found(api.get(id::<ReturnId>(m, args)?)?),
        "list" => to_value(&api.list(filter_or_default::<ReturnFilter>(m, args)?)?),
        "count" => to_value(&api.count(filter_or_default::<ReturnFilter>(m, args)?)?),
        "list_for_order" => {
            #[derive(Deserialize)]
            struct A {
                order_id: OrderId,
            }
            to_value(&api.list_for_order(parse::<A>(m, args)?.order_id)?)
        }
        "approve" => to_value(&api.approve(id::<ReturnId>(m, args)?)?),
        "reject" => {
            #[derive(Deserialize)]
            struct A {
                id: ReturnId,
                reason: String,
            }
            let a: A = parse(m, args)?;
            to_value(&api.reject(a.id, &a.reason)?)
        }
        "mark_received" => to_value(&api.mark_received(id::<ReturnId>(m, args)?)?),
        "complete" => to_value(&api.complete(id::<ReturnId>(m, args)?)?),
        "cancel" => to_value(&api.cancel(id::<ReturnId>(m, args)?)?),
        "add_tracking" => {
            #[derive(Deserialize)]
            struct A {
                id: ReturnId,
                tracking_number: String,
            }
            let a: A = parse(m, args)?;
            to_value(&api.add_tracking(a.id, &a.tracking_number)?)
        }
        "set_item_disposition" => {
            #[derive(Deserialize)]
            struct A {
                id: ReturnId,
                item_id: Uuid,
                #[serde(flatten)]
                input: SetReturnDisposition,
            }
            let a: A = parse(m, args)?;
            to_value(&api.set_item_disposition(a.id, a.item_id, a.input)?)
        }
        _ => Err(unknown(m)),
    }
}

fn shipment_filter(m: &str, args: Value) -> Result<ShipmentFilter, CallError> {
    #[derive(Deserialize, Default)]
    struct A {
        order_id: Option<OrderId>,
        limit: Option<u32>,
        offset: Option<u32>,
    }
    let a: A = filter_or_default(m, args)?;
    Ok(ShipmentFilter {
        order_id: a.order_id,
        limit: a.limit,
        offset: a.offset,
        ..ShipmentFilter::default()
    })
}

fn shipments(c: &Commerce, m: &str, op: &str, args: Value) -> CallResult {
    let api = c.shipments();
    match op {
        "create" => to_value(&api.create(parse::<CreateShipment>(m, args)?)?),
        "get" => found(api.get(id::<ShipmentId>(m, args)?)?),
        "get_by_tracking" => {
            #[derive(Deserialize)]
            struct A {
                tracking_number: String,
            }
            found(api.get_by_tracking(&parse::<A>(m, args)?.tracking_number)?)
        }
        "list" => to_value(&api.list(shipment_filter(m, args)?)?),
        "count" => to_value(&api.count(shipment_filter(m, args)?)?),
        "for_order" => {
            #[derive(Deserialize)]
            struct A {
                order_id: OrderId,
            }
            to_value(&api.for_order(parse::<A>(m, args)?.order_id)?)
        }
        "mark_processing" => to_value(&api.mark_processing(id::<ShipmentId>(m, args)?)?),
        "mark_ready" => to_value(&api.mark_ready(id::<ShipmentId>(m, args)?)?),
        "ship" => {
            #[derive(Deserialize)]
            struct A {
                id: ShipmentId,
                tracking_number: Option<String>,
            }
            let a: A = parse(m, args)?;
            to_value(&api.ship(a.id, a.tracking_number)?)
        }
        "mark_in_transit" => to_value(&api.mark_in_transit(id::<ShipmentId>(m, args)?)?),
        "mark_out_for_delivery" => {
            to_value(&api.mark_out_for_delivery(id::<ShipmentId>(m, args)?)?)
        }
        "mark_delivered" => to_value(&api.mark_delivered(id::<ShipmentId>(m, args)?)?),
        "cancel" => to_value(&api.cancel(id::<ShipmentId>(m, args)?)?),
        _ => Err(unknown(m)),
    }
}

// ---------------------------------------------------------------------------
// Envelope
// ---------------------------------------------------------------------------

/// Render a call outcome as the envelope string returned across the ABI.
#[must_use]
pub fn envelope(result: &CallResult) -> String {
    let value = match result {
        Ok(v) => json!({ "ok": true, "result": v }),
        Err(e) => json!({
            "ok": false,
            "error": { "code": e.code as i32, "kind": kind_of(e.code), "message": e.message },
        }),
    };
    value.to_string()
}

fn call_with_raw_args(c: &Commerce, method: &str, args_json: Option<&str>) -> CallResult {
    let args = match args_json {
        None => Value::Null,
        Some(s) if s.trim().is_empty() => Value::Null,
        Some(s) => serde_json::from_str::<Value>(s)
            .map_err(|e| CallError::invalid(format!("arguments are not valid JSON: {e}")))?,
    };
    if !matches!(args, Value::Null | Value::Object(_)) {
        return Err(CallError::invalid("arguments must be a JSON object"));
    }
    call(c, method, args)
}

// ---------------------------------------------------------------------------
// C ABI
// ---------------------------------------------------------------------------

/// Open (or create) a store at `db_path` (`":memory:"` for an ephemeral one)
/// and write its handle to `*out_handle`.
///
/// Returns `0` (`Ok`) on success, otherwise an [`FfiErrorCode`] value with the
/// message available from `stateset_last_error_message`. Release the handle
/// with `stateset_destroy`.
///
/// # Safety
///
/// `db_path` must be a valid NUL-terminated string; `out_handle` must be a
/// valid, writable pointer.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_json_open(
    db_path: *const c_char,
    out_handle: *mut CommerceHandle,
) -> i32 {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        clear_last_error();
        if out_handle.is_null() {
            set_last_error("null out_handle pointer");
            return FfiErrorCode::NullPointer;
        }
        // SAFETY: caller guarantees `db_path` is NUL-terminated or null (checked).
        let path = match unsafe { c_string_to_rust(db_path) } {
            Ok(p) => p,
            Err(code) => return code,
        };
        match init_engine(path).and_then(register_new_engine_handle) {
            Ok(handle) => {
                // SAFETY: `out_handle` is non-null and writable per the contract.
                unsafe { *out_handle = handle };
                FfiErrorCode::Ok
            }
            Err(code) => code,
        }
    }));
    match outcome {
        Ok(code) => code as i32,
        Err(_) => {
            set_last_error("panic across FFI boundary in stateset_json_open");
            FfiErrorCode::InternalError as i32
        }
    }
}

/// Invoke `method` (e.g. `"orders.create"`) with `args_json` (a JSON object,
/// or null/empty for none) and return a JSON envelope.
///
/// The return value is never null under normal operation; free it with
/// `stateset_string_free`. Errors (unknown method, invalid arguments, engine
/// failures, stale handles, panics) are reported inside the envelope.
///
/// # Safety
///
/// `engine` must be a handle from `stateset_json_open` (or `stateset_init`)
/// that has not been destroyed; `method` must be a NUL-terminated string;
/// `args_json` must be null or NUL-terminated.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub unsafe extern "C" fn stateset_json_call(
    engine: CommerceHandle,
    method: *const c_char,
    args_json: *const c_char,
) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();
        let result = (|| {
            // SAFETY: contract above; null is checked inside.
            let method = unsafe { c_string_to_rust(method) }
                .map_err(|code| CallError { code, message: "invalid method string".into() })?;
            let args = if args_json.is_null() {
                None
            } else {
                // SAFETY: non-null, NUL-terminated per the contract.
                Some(unsafe { c_string_to_rust(args_json) }.map_err(|code| CallError {
                    code,
                    message: "arguments are not valid UTF-8".into(),
                })?)
            };
            let lease = begin_engine_use(engine).map_err(|code| CallError {
                code,
                message: "invalid, stale or destroyed engine handle".into(),
            })?;
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                call_with_raw_args(lease.engine(), method, args)
            }));
            caught.unwrap_or_else(|payload| {
                let detail = payload
                    .downcast_ref::<&str>()
                    .map(|s| (*s).to_owned())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic payload".into());
                Err(CallError {
                    code: FfiErrorCode::InternalError,
                    message: format!("engine panicked in {method}: {detail}"),
                })
            })
        })();
        if let Err(e) = &result {
            set_last_error(&e.message);
        }
        rust_to_c_string(&envelope(&result))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> Commerce {
        Commerce::new(":memory:").expect("in-memory store")
    }

    fn ok(c: &Commerce, method: &str, args: Value) -> Value {
        call(c, method, args).unwrap_or_else(|e| panic!("{method}: {e:?}"))
    }

    /// The Swift binding's hand-written header and the .NET P/Invoke table
    /// must name every export the bindings rely on.
    #[test]
    fn swift_header_declares_every_json_export() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bindings");
        let header = root.join("swift/Sources/StateSetC/stateset.h");
        let pinvoke = root.join("dotnet/dotnet/StateSet/NativeMethods.cs");
        if !header.exists() || !pinvoke.exists() {
            return; // packaged crate without the repo's bindings directory
        }
        let header = std::fs::read_to_string(header).expect("read header");
        let pinvoke = std::fs::read_to_string(pinvoke).expect("read NativeMethods.cs");
        for symbol in [
            "stateset_abi_version",
            "stateset_json_open",
            "stateset_json_call",
            "stateset_destroy",
            "stateset_string_free",
            "stateset_last_error_message",
            "stateset_crypto_jcs_canonicalize",
            "stateset_crypto_free_buffer",
            "stateset_crypto_payload_plain_hash",
            "stateset_crypto_merkle_root",
        ] {
            assert!(header.contains(&format!("{symbol}(")), "stateset.h lacks {symbol}");
            assert!(pinvoke.contains(&format!("{symbol}(")), "NativeMethods.cs lacks {symbol}");
        }
    }

    #[test]
    fn every_listed_method_is_dispatched() {
        let c = engine();
        for m in METHODS {
            // Calling with `{}` either succeeds or fails on arguments -- never
            // as "unknown method".
            if let Err(e) = call(&c, m, json!({})) {
                assert!(!e.message.starts_with("unknown method"), "{m} is listed but not routed");
            }
        }
        assert!(call(&c, "customers.nope", json!({})).unwrap_err().message.contains("unknown"));
    }

    #[test]
    fn money_round_trips_as_exact_strings_and_floats_are_refused() {
        let c = engine();
        let cust = ok(
            &c,
            "customers.create",
            json!({"email": "m@example.com", "first_name": "M", "last_name": "N"}),
        );
        let order = ok(
            &c,
            "orders.create",
            json!({
                "customer_id": cust["id"],
                "items": [{"product_id": cust["id"], "sku": "S", "name": "W", "quantity": 3, "unit_price": "0.10"}],
            }),
        );
        assert_eq!(order["total_amount"], json!("0.30"));
        let err = call(
            &c,
            "orders.create",
            json!({
                "customer_id": cust["id"],
                "items": [{"product_id": cust["id"], "sku": "S", "name": "W", "quantity": 1, "unit_price": 0.1}],
            }),
        )
        .unwrap_err();
        assert_eq!(err.code, FfiErrorCode::InvalidArgument);
        assert!(err.message.contains("floating-point"), "{}", err.message);
    }

    #[test]
    fn not_found_and_bad_ids_are_typed() {
        let c = engine();
        assert_eq!(ok(&c, "orders.get", json!({"id": Uuid::new_v4()})), Value::Null);
        let err = call(&c, "orders.get", json!({"id": "not-a-uuid"})).unwrap_err();
        assert_eq!(err.code, FfiErrorCode::InvalidArgument);
        let err = call(&c, "orders.cancel", json!({"id": Uuid::new_v4()})).unwrap_err();
        assert_eq!(err.code, FfiErrorCode::NotFound, "{}", err.message);
    }

    #[test]
    fn c_abi_round_trip_and_persistence() {
        use std::ffi::{CStr, CString};
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("store.db");
        let cpath = CString::new(path.to_str().expect("utf8 path")).expect("cstring");

        let call_c = |h: CommerceHandle, m: &str, a: &str| -> Value {
            let m = CString::new(m).expect("method");
            let a = CString::new(a).expect("args");
            let raw = unsafe { stateset_json_call(h, m.as_ptr(), a.as_ptr()) };
            assert!(!raw.is_null());
            let s = unsafe { CStr::from_ptr(raw) }.to_str().expect("utf8").to_owned();
            unsafe { crate::strings::stateset_string_free(raw) };
            serde_json::from_str(&s).expect("envelope json")
        };

        let mut h: CommerceHandle = std::ptr::null_mut();
        assert_eq!(unsafe { stateset_json_open(cpath.as_ptr(), &raw mut h) }, 0);
        let env = call_c(
            h,
            "customers.create",
            r#"{"email":"p@example.com","first_name":"P","last_name":"Q"}"#,
        );
        assert_eq!(env["ok"], json!(true), "{env}");
        let id = env["result"]["id"].as_str().expect("id").to_owned();
        unsafe { crate::api::stateset_destroy(h) };

        // A destroyed handle is refused inside the envelope, not by crashing.
        let stale = call_c(h, "customers.list", "{}");
        assert_eq!(stale["ok"], json!(false));

        let mut h2: CommerceHandle = std::ptr::null_mut();
        assert_eq!(unsafe { stateset_json_open(cpath.as_ptr(), &raw mut h2) }, 0);
        let env = call_c(h2, "customers.get", &json!({"id": id}).to_string());
        assert_eq!(env["result"]["email"], json!("p@example.com"));
        let bad = call_c(h2, "customers.create", "not json");
        assert_eq!(bad["error"]["kind"], json!("invalid_argument"));
        unsafe { crate::api::stateset_destroy(h2) };
    }
}
