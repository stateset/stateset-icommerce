#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::threadsafe_function::{ErrorStrategy, ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi_derive::napi;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use serde::{Deserialize, Serialize};
use stateset_core::{CartId, CustomerId, OrderId, ProductId, PromotionId, SubscriptionId};
use stateset_embedded::Commerce as RustCommerce;
use stateset_embedded::CurrencyCode;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

mod errors;

use errors::{ErrCode, coded, from_cause, guard, guard_async, wrap};

/// Narrows a value to the lossy `f64` view without ever inventing a number.
///
/// The predecessor of this function, `to_f64_or_nan`, swallowed a failed
/// conversion and handed JavaScript `NaN`. `NaN` then flowed into revenue,
/// average order value and inventory valuation with nothing on the wire to say
/// the number was fabricated: `NaN` compares false against every threshold, so
/// a report reading it silently showed a blank or a zero rather than an error.
/// This returns a coded `INTERNAL` failure naming the field instead.
///
/// Two things can go wrong, and both are covered:
///
/// 1. **The conversion refuses.** For `rust_decimal::Decimal` this arm is an
///    invariant guard rather than a live branch: `Decimal::to_f64` — which is
///    what `TryFrom<Decimal> for f64` delegates to — has no reachable `None`
///    arm. A scale of zero goes through `to_i128`, which is total for a 96-bit
///    mantissa, and every other scale returns `Some`. `Decimal::MAX` included.
///    `tests::decimal_max_is_representable` pins that. The arm is live for the
///    non-`Decimal` `TryInto<f64>` inputs this helper also serves.
/// 2. **The conversion succeeds but produces a non-finite `f64`.** Reachable
///    whenever the source is already an `f64` (a growth percentage divided by a
///    zero baseline, say). That is the `NaN` this function exists to stop, so it
///    is rejected here rather than passed through.
fn to_f64_checked<T>(value: T, field: &str) -> Result<f64>
where
    T: TryInto<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Display,
{
    narrow_to_f64(value, "", field)
}

/// [`to_f64_checked`] for a currency amount, which says so in the failure.
///
/// Split from the plain form because the same narrowing serves hours,
/// percentages, scores and quantities, and calling one of those a "money value"
/// in an error a human reads would be wrong.
fn money_to_f64<T>(value: T, field: &str) -> Result<f64>
where
    T: TryInto<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Display,
{
    narrow_to_f64(value, "money value ", field)
}

/// The shared body. `prefix` and `field` are only ever formatted on the failure
/// path, so the ~111 money fields this runs for on a busy read cost no
/// allocation.
fn narrow_to_f64<T>(value: T, prefix: &str, field: &str) -> Result<f64>
where
    T: TryInto<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Display,
{
    let converted: f64 = match value.try_into() {
        Ok(converted) => converted,
        Err(err) => {
            return Err(coded(
                ErrCode::Internal,
                format!("{prefix}{field} is not representable as f64: {err}"),
            ));
        }
    };
    if converted.is_finite() {
        Ok(converted)
    } else {
        Err(coded(ErrCode::Internal, format!("{prefix}{field} is not representable as f64")))
    }
}

fn optional_to_f64_checked<T>(value: Option<T>, field: &str) -> Result<Option<f64>>
where
    T: TryInto<f64>,
    <T as TryInto<f64>>::Error: std::fmt::Display,
{
    value.map(|inner| to_f64_checked(inner, field)).transpose()
}

/// The two views of one money value: the lossy `f64` the binding has always
/// returned, and the exact base-10 rendering that never touched a float.
///
/// Every money field on an output struct is a pair — `total` and
/// `total_exact` — and both halves come out of this one call on one `Decimal`,
/// so a struct literal cannot accidentally pair one amount's `f64` with another
/// amount's exact string. `Decimal::to_string` is the exact base-10 form, scale
/// and all: `Decimal::new(30, 2).to_string()` is `"0.30"`, not `"0.3"` and not
/// the `0.30000000000000004` a float sum would show.
fn money_pair(value: Decimal, field: &str) -> Result<(f64, String)> {
    let exact = value.to_string();
    let approx = money_to_f64(value, field)?;
    Ok((approx, exact))
}

/// [`money_pair`] for an optional amount: absent stays absent on both halves.
fn optional_money_pair(
    value: Option<Decimal>,
    field: &str,
) -> Result<(Option<f64>, Option<String>)> {
    match value {
        Some(value) => {
            let (approx, exact) = money_pair(value, field)?;
            Ok((Some(approx), Some(exact)))
        }
        None => Ok((None, None)),
    }
}

fn convert_output<T, U>(value: T) -> Result<U>
where
    U: TryFrom<T, Error = Error>,
{
    U::try_from(value)
}

fn convert_optional_output<T, U>(value: Option<T>) -> Result<Option<U>>
where
    U: TryFrom<T, Error = Error>,
{
    value.map(convert_output).transpose()
}

fn convert_outputs<T, U>(values: Vec<T>) -> Result<Vec<U>>
where
    U: TryFrom<T, Error = Error>,
{
    values.into_iter().map(convert_output).collect()
}

/// The engine behind every JavaScript handle.
///
/// The engine is `Sync` and pools its own connections (the HTTP server shares a
/// bare `Arc<Commerce>` across threads), so nothing here serialises calls. Each
/// call clones the inner `Arc` under a brief read lock and releases the lock
/// before touching the database, so readers and writers proceed concurrently
/// and `close` never waits on in-flight work: it empties the slot, and the
/// engine drops when the last in-flight call finishes.
pub(crate) struct EngineHandle {
    slot: std::sync::RwLock<Option<Arc<Engine>>>,
}

/// What every sub-API holds. Cheap to clone; identity is the engine.
pub(crate) type Handle = Arc<EngineHandle>;

/// The engine, dropped somewhere it is allowed to block.
///
/// `stateset_embedded::Commerce` owns a tokio runtime of its own (webhooks,
/// the event store), and tokio refuses to drop a runtime from inside another
/// runtime's worker — which is exactly where the last `Arc` clone usually
/// goes out of scope: at the end of an in-flight napi async call, or in
/// `close()`. Hand the drop to a plain thread in that case.
pub(crate) struct Engine(Option<RustCommerce>);

impl std::ops::Deref for Engine {
    type Target = RustCommerce;
    fn deref(&self) -> &RustCommerce {
        // `None` only ever exists inside `drop`.
        self.0.as_ref().unwrap_or_else(|| unreachable!("engine taken outside drop"))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let Some(commerce) = self.0.take() else { return };
        if tokio::runtime::Handle::try_current().is_ok() {
            std::thread::spawn(move || drop(commerce));
        } else {
            drop(commerce);
        }
    }
}

impl EngineHandle {
    fn new(commerce: RustCommerce) -> Handle {
        Arc::new(Self { slot: std::sync::RwLock::new(Some(Arc::new(Engine(Some(commerce))))) })
    }

    /// The live engine, or `PRECONDITION_FAILED` once `close` has run.
    fn get(&self) -> Result<Arc<Engine>> {
        let slot = self.slot.read().unwrap_or_else(std::sync::PoisonError::into_inner);
        slot.clone()
            .ok_or_else(|| coded(ErrCode::PreconditionFailed, "Commerce instance is closed"))
    }

    /// Take the engine out. Idempotent; returns what was there so the caller
    /// chooses where the drop runs.
    fn close(&self) -> Option<Arc<Engine>> {
        self.slot.write().unwrap_or_else(std::sync::PoisonError::into_inner).take()
    }

    fn is_closed(&self) -> bool {
        self.slot.read().unwrap_or_else(std::sync::PoisonError::into_inner).is_none()
    }
}

/// Parse an optional identifier, refusing a malformed one with `VALIDATION`.
///
/// The predecessor pattern, `value.and_then(|s| s.parse().ok())`, turned a
/// typo into "not sent", and one call site went on to `unwrap_or_default()`
/// that into the nil UUID — an order line whose product id was quietly
/// discarded. A caller who sent an id meant it.
fn parse_optional_id<T: FromStr>(value: Option<String>, what: &str) -> Result<Option<T>> {
    value
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.parse::<T>()
                .map_err(|_| coded(ErrCode::Validation, format!("Invalid {what} UUID '{s}'")))
        })
        .transpose()
}

/// Parse an optional list of identifiers; one malformed entry refuses the
/// whole list. The predecessor `filter_map(.. .ok())` dropped the bad entry,
/// so a promotion "for these products" silently applied to fewer of them.
fn parse_id_list<T: From<uuid::Uuid>>(
    values: Option<Vec<String>>,
    what: &str,
) -> Result<Option<Vec<T>>> {
    values
        .map(|ids| {
            ids.into_iter()
                .map(|s| {
                    uuid::Uuid::parse_str(&s).map(T::from).map_err(|_| {
                        coded(ErrCode::Validation, format!("Invalid {what} UUID '{s}'"))
                    })
                })
                .collect::<Result<Vec<T>>>()
        })
        .transpose()
}

/// Parse an optional ISO-4217 currency, refusing an unknown code with
/// `VALIDATION` rather than silently pricing the record in the store default.
fn parse_optional_currency(value: Option<String>) -> Result<Option<CurrencyCode>> {
    value
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.parse::<CurrencyCode>()
                .map_err(|_| coded(ErrCode::Validation, format!("Invalid currency code '{s}'")))
        })
        .transpose()
}

/// Parse an optional RFC 3339 timestamp, refusing a malformed one.
fn parse_optional_datetime(
    value: Option<String>,
    what: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    value
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(&s).map(|d| d.with_timezone(&chrono::Utc)).map_err(
                |_| {
                    coded(
                        ErrCode::Validation,
                        format!("Invalid {what}: expected an RFC 3339 timestamp, got '{s}'"),
                    )
                },
            )
        })
        .transpose()
}

/// Parse an optional `YYYY-MM-DD` date, refusing a malformed one.
fn parse_optional_date(value: Option<String>, what: &str) -> Result<Option<chrono::NaiveDate>> {
    value
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").map_err(|_| {
                coded(
                    ErrCode::Validation,
                    format!("Invalid {what}: expected YYYY-MM-DD, got '{s}'"),
                )
            })
        })
        .transpose()
}

/// Parse an optional JSON document, refusing a malformed one.
fn parse_optional_json<T: serde::de::DeserializeOwned>(
    value: Option<String>,
    what: &str,
) -> Result<Option<T>> {
    value
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            serde_json::from_str::<T>(&s)
                .map_err(|e| coded(ErrCode::Validation, format!("Invalid {what} JSON: {e}")))
        })
        .transpose()
}

fn decimal_from_f64(value: f64, field: &str) -> Result<Decimal> {
    Decimal::from_f64(value).ok_or_else(|| coded(ErrCode::Validation, format!("Invalid {field}")))
}

fn optional_decimal_from_f64(value: Option<f64>, field: &str) -> Result<Option<Decimal>> {
    value.map(|value| decimal_from_f64(value, field)).transpose()
}

/// Resolves a money input that accepts either an exact base-10 string or an
/// `f64`, preferring the string.
///
/// The string wins when both arrive, for two reasons: it is what a caller who
/// bothered to send one meant, and it is the only one of the two that can carry
/// a value an `f64` cannot hold exactly. `19.99` typed as a JavaScript number
/// is already `19.989999999999998` before the binding sees it; typed as
/// `"19.99"` it survives.
fn money_input(exact: Option<&str>, value: Option<f64>, field: &str) -> Result<Decimal> {
    match (exact, value) {
        (Some(text), _) => parse_decimal_str(text, field),
        (None, Some(value)) => decimal_from_f64(value, field),
        (None, None) => Err(coded(
            ErrCode::Validation,
            format!("Missing {field}: send it as an exact base-10 string (the `...Exact` field)"),
        )),
    }
}

/// [`money_input`] where the `f64` half is itself optional. The exact string
/// still wins; absent on both halves stays absent.
fn optional_money_input(
    exact: Option<&str>,
    value: Option<f64>,
    field: &str,
) -> Result<Option<Decimal>> {
    match exact {
        Some(text) => parse_decimal_str(text, field).map(Some),
        None => optional_decimal_from_f64(value, field),
    }
}

/// Options for `Commerce.open`.
#[napi(object)]
#[derive(Clone, Default)]
pub struct OpenOptions {
    /// Size of the connection pool. Defaults to the engine's own default.
    pub max_connections: Option<u32>,
}

fn open_engine(db_path: &str, options: Option<OpenOptions>) -> Result<RustCommerce> {
    let mut builder = RustCommerce::builder().database(db_path);
    if let Some(max) = options.and_then(|o| o.max_connections) {
        builder = builder.max_connections(max);
    }
    builder.build().map_err(|e| wrap(ErrCode::Internal, "Failed to open commerce database", e))
}

/// The worker-thread task behind `Commerce.open`.
pub struct OpenTask {
    db_path: String,
    options: Option<OpenOptions>,
}

impl Task for OpenTask {
    type Output = RustCommerce;
    type JsValue = Commerce;

    fn compute(&mut self) -> Result<Self::Output> {
        let options = self.options.take();
        guard(|| open_engine(&self.db_path, options))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(Commerce { inner: EngineHandle::new(output) })
    }
}

/// JavaScript-friendly Commerce instance
#[napi]
pub struct Commerce {
    inner: Handle,
}

#[napi]
impl Commerce {
    /// Execution features implemented by this native binary. Hosts must check
    /// this before relying on optional safety fields that old binaries ignore.
    #[napi]
    pub fn kernel_features(&self) -> Result<Vec<String>> {
        guard(|| {
            Ok(vec![
                "checkout.stock_policy.v1".to_owned(),
                "checkout.cart_fingerprint.v1".to_owned(),
            ])
        })
    }

    /// Read exact quote terms and their fingerprint from the same cart snapshot.
    /// Keep this result with the issued quote; never recalculate it at acceptance.
    ///
    /// The declared shape (`CheckoutSnapshot`, `scripts/types/kernel.d.ts`)
    /// mirrors `stateset_core::Cart` as serde writes it — snake_case, exact
    /// decimal strings — not the camelCase `CartOutput` of the `carts` API.
    #[napi(ts_return_type = "Promise<CheckoutSnapshot>")]
    pub async fn checkout_snapshot(&self, cart_id: String) -> Result<serde_json::Value> {
        guard_async(async move {
            let id: CartId =
                cart_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid cart UUID"))?;
            let commerce = self.inner.get()?;
            let cart = commerce
                .carts()
                .get(id)
                .map_err(|error| from_cause(ErrCode::Internal, error))?
                .ok_or_else(|| coded(ErrCode::NotFound, "Cart not found"))?;
            let fingerprint = cart
                .checkout_fingerprint()
                .map_err(|error| from_cause(ErrCode::Internal, error))?;
            Ok(serde_json::json!({ "cart": cart, "fingerprint": fingerprint }))
        })
        .await
    }

    /// Create a new Commerce instance with a database path
    /// Use ":memory:" for an in-memory database
    #[napi(constructor)]
    pub fn new(db_path: String) -> Result<Self> {
        guard(|| {
            let commerce = RustCommerce::new(&db_path)
                .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize commerce", e))?;

            Ok(Self { inner: EngineHandle::new(commerce) })
        })
    }

    /// Open a database off the event loop.
    ///
    /// The constructor runs every pending migration synchronously on the
    /// JavaScript thread; this does the same work on a worker and resolves to
    /// the ready instance.
    #[napi(ts_return_type = "Promise<Commerce>")]
    pub fn open(db_path: String, options: Option<OpenOptions>) -> AsyncTask<OpenTask> {
        AsyncTask::new(OpenTask { db_path, options })
    }

    /// Release the engine. Calls already in flight complete; every later call
    /// rejects with `PRECONDITION_FAILED`. Idempotent.
    #[napi]
    pub async fn close(&self) -> Result<()> {
        // The drop closes the connection pool; it runs here on the worker
        // rather than on the JavaScript thread.
        drop(self.inner.close());
        Ok(())
    }

    /// Whether `close` has run.
    #[napi(getter)]
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    /// Execute a versioned commerce kernel command under host-supplied policy.
    ///
    /// `policy` must come from trusted application configuration. It must not
    /// be copied from model-generated tool arguments.
    ///
    /// `KernelCommand`, `KernelPolicy` and `KernelReceipt` are declared in
    /// `scripts/types/kernel.d.ts` and mirror `stateset_core::kernel`'s serde
    /// contracts; the JSON passes through unchanged and serde validates it.
    #[napi(
        ts_args_type = "command: KernelCommand, policy: KernelPolicy",
        ts_return_type = "Promise<KernelReceipt>"
    )]
    pub async fn execute_kernel_command(
        &self,
        command: serde_json::Value,
        policy: serde_json::Value,
    ) -> Result<serde_json::Value> {
        guard_async(async move {
            let policy: stateset_core::KernelPolicy = serde_json::from_value(policy)
                .map_err(|error| wrap(ErrCode::Validation, "Invalid kernel policy", error))?;
            let commerce = self.inner.get()?;
            commerce
                .execute_kernel_command(command, policy)
                .map_err(|error| wrap(ErrCode::Internal, "Kernel execution failed", error))
        })
        .await
    }

    /// Provision immutable, durable monetary authority for governed commands.
    /// This is an operator API and should not be exposed as a model tool.
    #[napi(
        ts_args_type = "budget: EconomicBudget",
        ts_return_type = "Promise<EconomicBudgetStatus>"
    )]
    pub async fn provision_economic_budget(
        &self,
        budget: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let budget: stateset_core::EconomicBudget = serde_json::from_value(budget)
            .map_err(|error| wrap(ErrCode::Validation, "Invalid economic budget", error))?;
        let commerce = self.inner.get()?;
        let status = commerce
            .provision_economic_budget(&budget)
            .map_err(|error| wrap(ErrCode::Internal, "Budget provisioning failed", error))?;
        serde_json::to_value(status)
            .map_err(|error| wrap(ErrCode::Internal, "Budget serialization failed", error))
    }

    /// Read exact committed and available balances for a durable budget.
    /// Resolves to `null` when no budget carries `budget_id`.
    #[napi(ts_return_type = "Promise<EconomicBudgetStatus | null>")]
    pub async fn economic_budget_status(&self, budget_id: String) -> Result<serde_json::Value> {
        let commerce = self.inner.get()?;
        let status = commerce
            .economic_budget_status(&budget_id)
            .map_err(|error| wrap(ErrCode::Internal, "Budget lookup failed", error))?;
        serde_json::to_value(status)
            .map_err(|error| wrap(ErrCode::Internal, "Budget serialization failed", error))
    }

    /// Get the customers API
    #[napi(getter)]
    pub fn customers(&self) -> Customers {
        Customers { commerce: self.inner.clone() }
    }

    /// Get the orders API
    #[napi(getter)]
    pub fn orders(&self) -> Orders {
        Orders { commerce: self.inner.clone() }
    }

    /// Get the products API
    #[napi(getter)]
    pub fn products(&self) -> Products {
        Products { commerce: self.inner.clone() }
    }

    /// Get the custom objects API (custom states / metaobjects)
    #[napi(getter)]
    pub fn custom_objects(&self) -> CustomObjects {
        CustomObjects { commerce: self.inner.clone() }
    }

    /// Alias for `custom_objects` (for users who prefer the "custom states" name)
    #[napi(getter)]
    pub fn custom_states(&self) -> CustomObjects {
        self.custom_objects()
    }

    /// Get the inventory API
    #[napi(getter)]
    pub fn inventory(&self) -> Inventory {
        Inventory { commerce: self.inner.clone() }
    }

    /// Get the returns API
    #[napi(getter)]
    pub fn returns(&self) -> Returns {
        Returns { commerce: self.inner.clone() }
    }

    /// Get the gift cards API
    #[napi(getter)]
    pub fn gift_cards(&self) -> GiftCards {
        GiftCards { commerce: self.inner.clone() }
    }

    /// Get the loyalty API
    #[napi(getter)]
    pub fn loyalty(&self) -> Loyalty {
        Loyalty { commerce: self.inner.clone() }
    }

    /// Get the store credits API
    #[napi(getter)]
    pub fn store_credits(&self) -> StoreCredits {
        StoreCredits { commerce: self.inner.clone() }
    }

    /// Get the product reviews API
    #[napi(getter)]
    pub fn reviews(&self) -> Reviews {
        Reviews { commerce: self.inner.clone() }
    }

    /// Get the wishlists API
    #[napi(getter)]
    pub fn wishlists(&self) -> Wishlists {
        Wishlists { commerce: self.inner.clone() }
    }

    /// Get the customer segments API
    #[napi(getter)]
    pub fn segments(&self) -> Segments {
        Segments { commerce: self.inner.clone() }
    }

    /// Get the payments API
    #[napi(getter)]
    pub fn payments(&self) -> Payments {
        Payments { commerce: self.inner.clone() }
    }

    /// Get the x402 payment protocol API
    #[napi(getter)]
    pub fn x402(&self) -> X402 {
        X402 { commerce: self.inner.clone() }
    }

    /// Get the shipments API
    #[napi(getter)]
    pub fn shipments(&self) -> Shipments {
        Shipments { commerce: self.inner.clone() }
    }

    /// Get the warranties API
    #[napi(getter)]
    pub fn warranties(&self) -> Warranties {
        Warranties { commerce: self.inner.clone() }
    }

    /// Get the purchase orders API
    #[napi(getter)]
    pub fn purchase_orders(&self) -> PurchaseOrders {
        PurchaseOrders { commerce: self.inner.clone() }
    }

    /// Get the invoices API
    #[napi(getter)]
    pub fn invoices(&self) -> Invoices {
        Invoices { commerce: self.inner.clone() }
    }

    /// Get the bill of materials API
    #[napi(getter)]
    pub fn bom(&self) -> Bom {
        Bom { commerce: self.inner.clone() }
    }

    /// Get the work orders API
    #[napi(getter)]
    pub fn work_orders(&self) -> WorkOrders {
        WorkOrders { commerce: self.inner.clone() }
    }

    /// Get the carts/checkout API
    #[napi(getter)]
    pub fn carts(&self) -> Carts {
        Carts { commerce: self.inner.clone() }
    }

    /// Get the analytics API
    #[napi(getter)]
    pub fn analytics(&self) -> Analytics {
        Analytics { commerce: self.inner.clone() }
    }

    /// Get the currency API
    #[napi(getter)]
    pub fn currency(&self) -> CurrencyOperations {
        CurrencyOperations { commerce: self.inner.clone() }
    }

    /// Get the subscriptions API
    #[napi(getter)]
    pub fn subscriptions(&self) -> Subscriptions {
        Subscriptions { commerce: self.inner.clone() }
    }

    /// Get the promotions API
    #[napi(getter)]
    pub fn promotions(&self) -> Promotions {
        Promotions { commerce: self.inner.clone() }
    }

    /// Get the tax API
    #[napi(getter)]
    pub fn tax(&self) -> Tax {
        Tax { commerce: self.inner.clone() }
    }

    /// Get the quality control API
    #[napi(getter)]
    pub fn quality(&self) -> Quality {
        Quality { commerce: self.inner.clone() }
    }

    /// Get the lot/batch tracking API
    #[napi(getter)]
    pub fn lots(&self) -> Lots {
        Lots { commerce: self.inner.clone() }
    }

    /// Get the serial number API
    #[napi(getter)]
    pub fn serials(&self) -> Serials {
        Serials { commerce: self.inner.clone() }
    }

    /// Get the warehouse API
    #[napi(getter)]
    pub fn warehouse(&self) -> Warehouse {
        Warehouse { commerce: self.inner.clone() }
    }

    /// Get the receiving API
    #[napi(getter)]
    pub fn receiving(&self) -> Receiving {
        Receiving { commerce: self.inner.clone() }
    }

    /// Get the fulfillment API
    #[napi(getter)]
    pub fn fulfillment(&self) -> Fulfillment {
        Fulfillment { commerce: self.inner.clone() }
    }

    /// Get the accounts payable API
    #[napi(getter)]
    pub fn accounts_payable(&self) -> AccountsPayable {
        AccountsPayable { commerce: self.inner.clone() }
    }

    /// Get the accounts receivable API
    #[napi(getter)]
    pub fn accounts_receivable(&self) -> AccountsReceivable {
        AccountsReceivable { commerce: self.inner.clone() }
    }

    /// Get the cost accounting API
    #[napi(getter)]
    pub fn cost_accounting(&self) -> CostAccounting {
        CostAccounting { commerce: self.inner.clone() }
    }

    /// Get the credit management API
    #[napi(getter)]
    pub fn credit(&self) -> Credit {
        Credit { commerce: self.inner.clone() }
    }

    /// Get the backorder management API
    #[napi(getter)]
    pub fn backorder(&self) -> Backorders {
        Backorders { commerce: self.inner.clone() }
    }

    /// Alias for `backorder`, matching the class name and the other plural
    /// sub-API names (`orders`, `returns`, `shipments`).
    #[napi(getter)]
    pub fn backorders(&self) -> Backorders {
        Backorders { commerce: self.inner.clone() }
    }

    /// Get the general ledger API
    #[napi(getter)]
    pub fn general_ledger(&self) -> GeneralLedger {
        GeneralLedger { commerce: self.inner.clone() }
    }

    /// Get the fixed assets API
    #[napi(getter)]
    pub fn fixed_assets(&self) -> FixedAssets {
        FixedAssets { commerce: self.inner.clone() }
    }

    /// Get the revenue recognition (ASC 606) API
    #[napi(getter)]
    pub fn revenue_recognition(&self) -> RevenueRecognition {
        RevenueRecognition { commerce: self.inner.clone() }
    }

    /// Get the cycle counts API
    #[napi(getter)]
    pub fn cycle_counts(&self) -> CycleCounts {
        CycleCounts { commerce: self.inner.clone() }
    }

    /// Get the EDI documents API (trading-partner document tracking)
    #[napi(getter)]
    pub fn edi_documents(&self) -> EdiDocuments {
        EdiDocuments { commerce: self.inner.clone() }
    }

    /// Get the activity logs API (append-only subject history)
    #[napi(getter)]
    pub fn activity_logs(&self) -> ActivityLogs {
        ActivityLogs { commerce: self.inner.clone() }
    }

    /// Get the channels API (sales / fulfillment channels)
    #[napi(getter)]
    pub fn channels(&self) -> Channels {
        Channels { commerce: self.inner.clone() }
    }

    /// Get the companies API (B2B accounts and contacts)
    #[napi(getter)]
    pub fn companies(&self) -> Companies {
        Companies { commerce: self.inner.clone() }
    }

    /// Get the units of measure API (unit classes, UOMs, conversion rules)
    #[napi(getter)]
    pub fn units_of_measure(&self) -> UnitsOfMeasure {
        UnitsOfMeasure { commerce: self.inner.clone() }
    }

    /// Get the shipping zones API (geographic zones, methods, rates)
    #[napi(getter)]
    pub fn shipping_zones(&self) -> ShippingZones {
        ShippingZones { commerce: self.inner.clone() }
    }

    /// Get the stock snapshots API (point-in-time inventory)
    #[napi(getter)]
    pub fn stock_snapshots(&self) -> StockSnapshots {
        StockSnapshots { commerce: self.inner.clone() }
    }

    /// Get the print stations API (paired agents + print job queue)
    #[napi(getter)]
    pub fn print_stations(&self) -> PrintStations {
        PrintStations { commerce: self.inner.clone() }
    }

    /// Get the integration mappings API (external↔internal value translation)
    #[napi(getter)]
    pub fn integration_mappings(&self) -> IntegrationMappings {
        IntegrationMappings { commerce: self.inner.clone() }
    }

    /// Get the integration field mappings API (field-path mappings)
    #[napi(getter)]
    pub fn integration_field_mappings(&self) -> IntegrationFieldMappings {
        IntegrationFieldMappings { commerce: self.inner.clone() }
    }

    /// Get the payment obligations API (scheduled AP payments)
    #[napi(getter)]
    pub fn payment_obligations(&self) -> PaymentObligations {
        PaymentObligations { commerce: self.inner.clone() }
    }

    /// Get the maintenance API (backup, restore, export, import)
    #[napi(getter)]
    pub fn maintenance(&self) -> Maintenance {
        Maintenance { commerce: self.inner.clone() }
    }

    /// Get the purgatory API (order ingestion staging)
    #[napi(getter)]
    pub fn purgatory(&self) -> Purgatory {
        Purgatory { commerce: self.inner.clone() }
    }

    /// Get the topology snapshots API (operational topology health)
    #[napi(getter)]
    pub fn topology_snapshots(&self) -> TopologySnapshots {
        TopologySnapshots { commerce: self.inner.clone() }
    }

    /// Get the fraud API (risk assessments and detection rules)
    #[napi(getter)]
    pub fn fraud(&self) -> Fraud {
        Fraud { commerce: self.inner.clone() }
    }

    /// Get the search configuration API (search tuning profiles)
    #[napi(getter)]
    pub fn search_config(&self) -> SearchConfigs {
        SearchConfigs { commerce: self.inner.clone() }
    }

    /// Get the ERC-8004 API (trustless agent identity, reputation, validation)
    #[napi(getter)]
    pub fn erc8004(&self) -> Erc8004 {
        Erc8004 { commerce: self.inner.clone() }
    }

    /// Get the vendor returns API (return-to-supplier)
    #[napi(getter)]
    pub fn vendor_returns(&self) -> VendorReturns {
        VendorReturns { commerce: self.inner.clone() }
    }

    /// Get the prepayments API (advance payments to suppliers)
    #[napi(getter)]
    pub fn prepayments(&self) -> Prepayments {
        Prepayments { commerce: self.inner.clone() }
    }

    /// Get the vendor credits API (supplier-owed credits)
    #[napi(getter)]
    pub fn vendor_credits(&self) -> VendorCredits {
        VendorCredits { commerce: self.inner.clone() }
    }

    /// Get the price schedules API (time-bounded pricing)
    #[napi(getter)]
    pub fn price_schedules(&self) -> PriceSchedules {
        PriceSchedules { commerce: self.inner.clone() }
    }

    /// Get the price levels API (B2B pricing tiers)
    #[napi(getter)]
    pub fn price_levels(&self) -> PriceLevels {
        PriceLevels { commerce: self.inner.clone() }
    }

    /// Get the transfer orders API (inter-warehouse stock movement)
    #[napi(getter)]
    pub fn transfer_orders(&self) -> TransferOrders {
        TransferOrders { commerce: self.inner.clone() }
    }

    /// Get the production batches API (grouping manufacturing work orders)
    #[napi(getter)]
    pub fn production_batches(&self) -> ProductionBatches {
        ProductionBatches { commerce: self.inner.clone() }
    }

    /// Get the supplier SKUs API (per-supplier SKU / unit-cost overrides)
    #[napi(getter)]
    pub fn supplier_skus(&self) -> SupplierSkus {
        SupplierSkus { commerce: self.inner.clone() }
    }

    /// Get the inbound shipments API (advance ship notices)
    #[napi(getter)]
    pub fn inbound_shipments(&self) -> InboundShipments {
        InboundShipments { commerce: self.inner.clone() }
    }

    /// Get the events API (pub/sub and webhook management)
    #[napi(getter)]
    pub fn events(&self) -> Events {
        Events { commerce: self.inner.clone() }
    }

    /// Create a vector search instance with the given OpenAI API key
    ///
    /// Vector search enables semantic similarity search across products,
    /// customers, orders, and inventory items using OpenAI embeddings.
    #[napi]
    pub fn vector(&self, api_key: String) -> Result<VectorSearch> {
        guard(|| Ok(VectorSearch { commerce: self.inner.clone(), api_key }))
    }
}

// ============================================================================
// Domain modules. Each is `use super::*` over this file's helpers, and every
// module's items are glob-imported here so siblings can reference each other
// (a cart's checkout result carries an `OrderOutput`, and so on).
// ============================================================================

#[allow(unused_imports)] // a few modules (crypto probes, test probes) export nothing siblings use
mod domains {
    use super::*;

    mod accounts_payable;
    mod accounts_receivable;
    mod activity_logs;
    mod analytics;
    mod backorders;
    mod bom;
    mod carts;
    mod channels;
    mod companies;
    mod cost_accounting;
    mod credit;
    mod currency;
    mod custom_objects;
    mod customers;
    mod cycle_counts;
    mod edi_documents;
    mod erc8004;
    mod events;
    mod fixed_assets;
    mod fraud;
    mod fulfillment;
    mod general_ledger;
    mod gift_cards;
    mod inbound_shipments;
    mod integration_field_mappings;
    mod integration_mappings;
    mod inventory;
    mod invoices;
    mod list_filters;
    mod lots;
    mod loyalty;
    mod orders;
    mod payment_obligations;
    mod payments;
    mod pqc_strict;
    mod prepayments;
    mod price_levels;
    mod price_schedules;
    mod print_stations;
    mod procurement_shared;
    mod production_batches;
    mod products;
    mod promotions;
    mod purchase_orders;
    mod purgatory;
    mod quality;
    mod receiving;
    mod returns;
    mod revenue_recognition;
    mod reviews;
    mod search_config;
    mod segments;
    mod serials;
    mod shipments;
    mod shipping_zones;
    mod stock_snapshots;
    mod store_credits;
    mod subscriptions;
    mod supplier_skus;
    mod tax;
    mod test_probes;
    mod topology_snapshots;
    mod transfer_orders;
    mod units_of_measure;
    mod vector_search;
    mod vendor_credits;
    mod vendor_returns;
    mod ves_crypto;
    mod warehouse;
    mod warranties;
    mod wishlists;
    mod work_orders;
    mod x402;

    pub(crate) use accounts_payable::*;
    pub(crate) use accounts_receivable::*;
    pub(crate) use activity_logs::*;
    pub(crate) use analytics::*;
    pub(crate) use backorders::*;
    pub(crate) use bom::*;
    pub(crate) use carts::*;
    pub(crate) use channels::*;
    pub(crate) use companies::*;
    pub(crate) use cost_accounting::*;
    pub(crate) use credit::*;
    pub(crate) use currency::*;
    pub(crate) use custom_objects::*;
    pub(crate) use customers::*;
    pub(crate) use cycle_counts::*;
    pub(crate) use edi_documents::*;
    pub(crate) use erc8004::*;
    pub(crate) use events::*;
    pub(crate) use fixed_assets::*;
    pub(crate) use fraud::*;
    pub(crate) use fulfillment::*;
    pub(crate) use general_ledger::*;
    pub(crate) use gift_cards::*;
    pub(crate) use inbound_shipments::*;
    pub(crate) use integration_field_mappings::*;
    pub(crate) use integration_mappings::*;
    pub(crate) use inventory::*;
    pub(crate) use invoices::*;
    pub(crate) use list_filters::*;
    pub(crate) use lots::*;
    pub(crate) use loyalty::*;
    pub(crate) use orders::*;
    pub(crate) use payment_obligations::*;
    pub(crate) use payments::*;
    pub(crate) use pqc_strict::*;
    pub(crate) use prepayments::*;
    pub(crate) use price_levels::*;
    pub(crate) use price_schedules::*;
    pub(crate) use print_stations::*;
    pub(crate) use procurement_shared::*;
    pub(crate) use production_batches::*;
    pub(crate) use products::*;
    pub(crate) use promotions::*;
    pub(crate) use purchase_orders::*;
    pub(crate) use purgatory::*;
    pub(crate) use quality::*;
    pub(crate) use receiving::*;
    pub(crate) use returns::*;
    pub(crate) use revenue_recognition::*;
    pub(crate) use reviews::*;
    pub(crate) use search_config::*;
    pub(crate) use segments::*;
    pub(crate) use serials::*;
    pub(crate) use shipments::*;
    pub(crate) use shipping_zones::*;
    pub(crate) use stock_snapshots::*;
    pub(crate) use store_credits::*;
    pub(crate) use subscriptions::*;
    pub(crate) use supplier_skus::*;
    pub(crate) use tax::*;
    pub(crate) use test_probes::*;
    pub(crate) use topology_snapshots::*;
    pub(crate) use transfer_orders::*;
    pub(crate) use units_of_measure::*;
    pub(crate) use vector_search::*;
    pub(crate) use vendor_credits::*;
    pub(crate) use vendor_returns::*;
    pub(crate) use ves_crypto::*;
    pub(crate) use warehouse::*;
    pub(crate) use warranties::*;
    pub(crate) use wishlists::*;
    pub(crate) use work_orders::*;
    pub(crate) use x402::*;
}
use domains::*;

#[cfg(test)]
mod tests {
    use super::{money_input, money_pair, optional_money_input, to_f64_checked};
    use rust_decimal::Decimal;

    /// `Decimal::to_f64` — which `TryFrom<Decimal> for f64` delegates to — has
    /// no reachable `None` arm, so the conversion cannot fail for any money the
    /// engine can hold. This pins that at the extreme, and pins the reason the
    /// `_exact` twin exists: at `Decimal::MAX` the `f64` half has thrown away
    /// eleven significant digits while the exact half still has all 29.
    #[test]
    fn decimal_max_is_representable() {
        let (approx, exact) = money_pair(Decimal::MAX, "test value").expect("MAX converts");
        assert!(approx.is_finite(), "Decimal::MAX must narrow to a finite f64, got {approx}");
        assert_eq!(exact, "79228162514264337593543950335");
        assert_ne!(
            exact,
            format!("{approx:.0}"),
            "the f64 half is expected to be lossy here — that is the point of the twin"
        );
    }

    #[test]
    fn decimal_min_is_representable() {
        let (approx, exact) = money_pair(Decimal::MIN, "test value").expect("MIN converts");
        assert!(approx.is_finite());
        assert_eq!(exact, "-79228162514264337593543950335");
    }

    /// The reachable failure: a value that is already `NaN` or infinite when it
    /// arrives must not be handed to JavaScript as a number.
    #[test]
    fn non_finite_is_rejected_with_the_field_name() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let err = to_f64_checked(bad, "sales total revenue").expect_err("must reject");
            assert!(
                err.reason.contains("sales total revenue"),
                "the error must name the field, got {}",
                err.reason
            );
            assert!(err.reason.contains("INTERNAL"), "coded INTERNAL, got {}", err.reason);
        }
    }

    /// The exact half keeps the engine's scale, which is what makes it exact:
    /// `0.30` stays two decimal places rather than collapsing to `0.3`, and a
    /// sum that a float would render as `0.30000000000000004` renders as
    /// `"0.30"`.
    #[test]
    fn money_pair_renders_the_exact_scale() {
        let sum = Decimal::new(10, 2) + Decimal::new(20, 2);
        let (approx, exact) = money_pair(sum, "test value").expect("converts");
        assert_eq!(exact, "0.30");
        assert!((approx - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn money_input_prefers_the_exact_string() {
        // 19.99 is not representable in binary floating point; the string is.
        let from_float = money_input(None, Some(19.99), "unit price").expect("float parses");
        let from_exact = money_input(Some("19.99"), None, "unit price").expect("string parses");
        assert_eq!(from_exact.to_string(), "19.99");
        // The exact string beats the float even when both are supplied.
        let both = money_input(Some("19.99"), Some(1.0), "unit price").expect("string wins");
        assert_eq!(both, from_exact);
        assert_eq!(both.to_string(), "19.99");
        assert_eq!(from_float.to_string(), "19.99", "sanity: from_f64 rounds to the same value");
    }

    #[test]
    fn money_input_carries_precision_no_f64_can_hold() {
        // 28 significant digits: `Decimal::from_f64` cannot produce this.
        let exact = "1234567890123456789012.3456";
        let parsed = money_input(Some(exact), None, "unit price").expect("parses");
        assert_eq!(parsed.to_string(), exact);
    }

    #[test]
    fn optional_money_input_keeps_absence() {
        let missing = money_input(None, None, "unit price").unwrap_err();
        assert!(missing.reason.contains("Missing unit price"), "{}", missing.reason);
        assert!(missing.reason.contains("VALIDATION"), "{}", missing.reason);
        assert_eq!(optional_money_input(None, None, "unit price").expect("ok"), None);
        assert_eq!(
            optional_money_input(Some("2.50"), None, "unit price").expect("ok"),
            Some(Decimal::new(250, 2))
        );
    }
}
