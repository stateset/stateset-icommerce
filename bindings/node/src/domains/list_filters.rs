//! List filters for purchase orders, work orders and quality records.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// =============================================================================
// List filters for purchase orders, work orders and quality records
// =============================================================================

pub(crate) fn parse_rfc3339_field(
    value: Option<&String>,
    field: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    value
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&chrono::Utc)).map_err(
                |_| coded(ErrCode::Validation, format!("Invalid RFC 3339 timestamp for {field}")),
            )
        })
        .transpose()
}

pub(crate) fn parse_uuid_field(value: Option<&String>, field: &str) -> Result<Option<uuid::Uuid>> {
    value
        .map(|s| {
            uuid::Uuid::parse_str(s)
                .map_err(|_| coded(ErrCode::Validation, format!("Invalid UUID for {field}")))
        })
        .transpose()
}

/// Parse an optional enumeration field with the core type's own `FromStr`,
/// refusing an unknown spelling with `VALIDATION` naming the field.
pub(crate) fn parse_enum_field<T: FromStr>(value: Option<&str>, field: &str) -> Result<Option<T>> {
    value
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.parse::<T>()
                .map_err(|_| coded(ErrCode::Validation, format!("Invalid {field}: '{s}'")))
        })
        .transpose()
}

/// Parse an optional exact decimal field (`"12.50"`), refusing a malformed one.
pub(crate) fn parse_decimal_field(value: Option<&str>, field: &str) -> Result<Option<Decimal>> {
    value.filter(|s| !s.trim().is_empty()).map(|s| parse_decimal_str(s, field)).transpose()
}

/// Filter for `purchaseOrders.list()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PurchaseOrderFilterInput {
    pub supplier_id: Option<String>,
    /// Purchase order status (`canceled` is accepted as an alias of `cancelled`).
    #[napi(ts_type = "PurchaseOrderStatus | 'canceled'")]
    pub status: Option<String>,
    /// RFC 3339 timestamp (inclusive lower bound on order date)
    pub from_date: Option<String>,
    /// RFC 3339 timestamp (inclusive upper bound on order date)
    pub to_date: Option<String>,
    /// Exact decimal string
    pub min_total: Option<String>,
    /// Exact decimal string
    pub max_total: Option<String>,
    /// Page size (server default 500, hard cap 1000)
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: `[orderDate, id]`
    pub after_cursor: Option<Vec<String>>,
}

pub(crate) fn parse_after_cursor_input(
    cursor: Option<Vec<String>>,
) -> Result<Option<(String, String)>> {
    match cursor {
        None => Ok(None),
        Some(parts) if parts.len() == 2 => {
            let mut it = parts.into_iter();
            let sort_key = it.next().unwrap_or_default();
            let id = it.next().unwrap_or_default();
            Ok(Some((sort_key, id)))
        }
        Some(_) => Err(coded(ErrCode::Validation, "afterCursor must be [sortKey, id]")),
    }
}

pub(crate) fn purchase_order_filter_from_input(
    filter: Option<PurchaseOrderFilterInput>,
) -> Result<stateset_core::PurchaseOrderFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::PurchaseOrderFilter::default());
    };
    Ok(stateset_core::PurchaseOrderFilter {
        supplier_id: parse_uuid_field(f.supplier_id.as_ref(), "supplierId")?,
        status: f
            .status
            .as_deref()
            .map(|s| {
                s.parse::<stateset_core::PurchaseOrderStatus>().map_err(|_| {
                    coded(ErrCode::Validation, format!("Invalid purchase order status: {s}"))
                })
            })
            .transpose()?,
        from_date: parse_rfc3339_field(f.from_date.as_ref(), "fromDate")?,
        to_date: parse_rfc3339_field(f.to_date.as_ref(), "toDate")?,
        min_total: f.min_total.as_deref().map(|s| parse_decimal_str(s, "minTotal")).transpose()?,
        max_total: f.max_total.as_deref().map(|s| parse_decimal_str(s, "maxTotal")).transpose()?,
        limit: f.limit,
        offset: f.offset,
        after_cursor: parse_after_cursor_input(f.after_cursor)?,
    })
}

/// Filter for `workOrders.list()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct WorkOrderFilterInput {
    pub product_id: Option<String>,
    pub bom_id: Option<String>,
    /// Work order status; the unseparated spellings (`inprogress`, `onhold`, ...) and `canceled` are accepted too.
    #[napi(
        ts_type = "WorkOrderStatus | 'inprogress' | 'partiallycompleted' | 'canceled' | 'onhold'"
    )]
    pub status: Option<String>,
    /// Work order priority.
    #[napi(ts_type = "WorkOrderPriority")]
    pub priority: Option<String>,
    pub assigned_to: Option<String>,
    pub work_center_id: Option<String>,
    pub overdue_only: Option<bool>,
    /// Page size (server default 500, hard cap 1000)
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: `[createdAt, id]`
    pub after_cursor: Option<Vec<String>>,
}

pub(crate) fn work_order_filter_from_input(
    filter: Option<WorkOrderFilterInput>,
) -> Result<stateset_core::WorkOrderFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::WorkOrderFilter::default());
    };
    Ok(stateset_core::WorkOrderFilter {
        product_id: parse_uuid_field(f.product_id.as_ref(), "productId")?
            .map(stateset_core::ProductId::from),
        bom_id: parse_uuid_field(f.bom_id.as_ref(), "bomId")?,
        status: f
            .status
            .as_deref()
            .map(|s| {
                s.parse::<stateset_core::WorkOrderStatus>().map_err(|_| {
                    coded(ErrCode::Validation, format!("Invalid work order status: {s}"))
                })
            })
            .transpose()?,
        priority: f
            .priority
            .as_deref()
            .map(|s| {
                s.parse::<stateset_core::WorkOrderPriority>().map_err(|_| {
                    coded(ErrCode::Validation, format!("Invalid work order priority: {s}"))
                })
            })
            .transpose()?,
        assigned_to: parse_uuid_field(f.assigned_to.as_ref(), "assignedTo")?,
        work_center_id: f.work_center_id,
        overdue_only: f.overdue_only,
        limit: f.limit,
        offset: f.offset,
        after_cursor: parse_after_cursor_input(f.after_cursor)?,
    })
}

/// Filter for `quality.listInspections()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct InspectionFilterInput {
    /// Inspection type.
    #[napi(ts_type = "'incoming' | 'receiving' | 'in_process' | 'final' | 'random' | 'return'")]
    pub inspection_type: Option<String>,
    /// Inspection status.
    #[napi(
        ts_type = "'pending' | 'scheduled' | 'in_progress' | 'passed' | 'failed' | 'partial_pass' | 'on_hold' | 'cancelled'"
    )]
    pub status: Option<String>,
    pub reference_type: Option<String>,
    pub reference_id: Option<String>,
    pub inspector_id: Option<String>,
    /// RFC 3339 timestamp (inclusive lower bound on created_at)
    pub from_date: Option<String>,
    /// RFC 3339 timestamp (inclusive upper bound on created_at)
    pub to_date: Option<String>,
    /// Page size (server default 500, hard cap 1000)
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: `[createdAt, id]`
    pub after_cursor: Option<Vec<String>>,
}

pub(crate) fn inspection_filter_from_input(
    filter: Option<InspectionFilterInput>,
) -> Result<stateset_core::InspectionFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::InspectionFilter::default());
    };
    Ok(stateset_core::InspectionFilter {
        inspection_type: f
            .inspection_type
            .as_deref()
            .map(|s| {
                s.parse::<stateset_core::InspectionType>().map_err(|_| {
                    coded(ErrCode::Validation, format!("Invalid inspection type: {s}"))
                })
            })
            .transpose()?,
        status: f
            .status
            .as_deref()
            .map(|s| {
                s.parse::<stateset_core::InspectionStatus>().map_err(|_| {
                    coded(ErrCode::Validation, format!("Invalid inspection status: {s}"))
                })
            })
            .transpose()?,
        reference_type: f.reference_type,
        reference_id: parse_uuid_field(f.reference_id.as_ref(), "referenceId")?,
        inspector_id: f.inspector_id,
        from_date: parse_rfc3339_field(f.from_date.as_ref(), "fromDate")?,
        to_date: parse_rfc3339_field(f.to_date.as_ref(), "toDate")?,
        limit: f.limit,
        offset: f.offset,
        after_cursor: parse_after_cursor_input(f.after_cursor)?,
    })
}

/// Filter for `quality.listNcrs()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct NcrFilterInput {
    /// Where the non-conformance was found.
    #[napi(
        ts_type = "'inspection' | 'customer_complaint' | 'internal_audit' | 'supplier_issue' | 'production_defect' | 'shipping_damage'"
    )]
    pub source: Option<String>,
    /// Severity.
    #[napi(ts_type = "'critical' | 'major' | 'minor' | 'observation'")]
    pub severity: Option<String>,
    /// NCR status.
    #[napi(
        ts_type = "'open' | 'under_review' | 'pending_disposition' | 'corrective_action' | 'preventive_action' | 'verification' | 'closed' | 'cancelled'"
    )]
    pub status: Option<String>,
    pub sku: Option<String>,
    pub lot_number: Option<String>,
    pub assigned_to: Option<String>,
    /// RFC 3339 timestamp (inclusive lower bound on created_at)
    pub from_date: Option<String>,
    /// RFC 3339 timestamp (inclusive upper bound on created_at)
    pub to_date: Option<String>,
    /// Page size (server default 500, hard cap 1000)
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: `[createdAt, id]`
    pub after_cursor: Option<Vec<String>>,
}

pub(crate) fn ncr_filter_from_input(
    filter: Option<NcrFilterInput>,
) -> Result<stateset_core::NonConformanceFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::NonConformanceFilter::default());
    };
    Ok(stateset_core::NonConformanceFilter {
        source: f
            .source
            .as_deref()
            .map(|s| {
                s.parse::<stateset_core::NonConformanceSource>()
                    .map_err(|_| coded(ErrCode::Validation, format!("Invalid NCR source: {s}")))
            })
            .transpose()?,
        severity: f
            .severity
            .as_deref()
            .map(|s| {
                s.parse::<stateset_core::Severity>()
                    .map_err(|_| coded(ErrCode::Validation, format!("Invalid NCR severity: {s}")))
            })
            .transpose()?,
        status: f
            .status
            .as_deref()
            .map(|s| {
                s.parse::<stateset_core::NcrStatus>()
                    .map_err(|_| coded(ErrCode::Validation, format!("Invalid NCR status: {s}")))
            })
            .transpose()?,
        sku: f.sku,
        lot_number: f.lot_number,
        assigned_to: f.assigned_to,
        from_date: parse_rfc3339_field(f.from_date.as_ref(), "fromDate")?,
        to_date: parse_rfc3339_field(f.to_date.as_ref(), "toDate")?,
        limit: f.limit,
        offset: f.offset,
        after_cursor: parse_after_cursor_input(f.after_cursor)?,
    })
}

// =============================================================================
// List filters for the core commerce lists that used to take no arguments.
//
// Every `*_filter_from_input` maps `None` to the engine's default filter, so
// a bare `list()` keeps its previous behaviour. Ids, dates and enumerations
// are parsed strictly: a malformed value is `VALIDATION`, never silently
// dropped.
// =============================================================================

/// Filter for `customers.list()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CustomerFilterInput {
    /// Exact e-mail match
    pub email: Option<String>,
    /// Account status
    #[napi(ts_type = "CustomerStatus")]
    pub status: Option<String>,
    /// Customers carrying this tag
    pub tag: Option<String>,
    pub accepts_marketing: Option<bool>,
    /// Page size
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: `[createdAt, id]`
    pub after_cursor: Option<Vec<String>>,
}

pub(crate) fn customer_filter_from_input(
    filter: Option<CustomerFilterInput>,
) -> Result<stateset_core::CustomerFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::CustomerFilter::default());
    };
    Ok(stateset_core::CustomerFilter {
        email: f.email,
        status: parse_enum_field(f.status.as_deref(), "customer status")?,
        tag: f.tag,
        accepts_marketing: f.accepts_marketing,
        limit: f.limit,
        offset: f.offset,
        after_cursor: parse_after_cursor_input(f.after_cursor)?,
    })
}

/// Filter for `orders.list()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct OrderFilterInput {
    pub customer_id: Option<String>,
    /// Order status (`canceled` and the unseparated `partiallyshipped` are accepted too)
    #[napi(ts_type = "OrderStatus | 'partiallyshipped' | 'canceled'")]
    pub status: Option<String>,
    /// Order-level payment status
    #[napi(ts_type = "PaymentStatus | 'partiallypaid' | 'partiallyrefunded'")]
    pub payment_status: Option<String>,
    /// Order-level fulfillment status
    #[napi(ts_type = "FulfillmentStatus | 'partiallyfulfilled'")]
    pub fulfillment_status: Option<String>,
    /// RFC 3339 timestamp (inclusive lower bound on order date)
    pub from_date: Option<String>,
    /// RFC 3339 timestamp (inclusive upper bound on order date)
    pub to_date: Option<String>,
    /// Page size
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: `[orderDate, id]`
    pub after_cursor: Option<Vec<String>>,
}

pub(crate) fn order_filter_from_input(
    filter: Option<OrderFilterInput>,
) -> Result<stateset_core::OrderFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::OrderFilter::default());
    };
    Ok(stateset_core::OrderFilter {
        customer_id: parse_optional_id(f.customer_id, "customer")?,
        status: parse_enum_field(f.status.as_deref(), "order status")?,
        payment_status: parse_enum_field(f.payment_status.as_deref(), "payment status")?,
        fulfillment_status: parse_enum_field(
            f.fulfillment_status.as_deref(),
            "fulfillment status",
        )?,
        from_date: parse_optional_datetime(f.from_date, "fromDate")?,
        to_date: parse_optional_datetime(f.to_date, "toDate")?,
        limit: f.limit,
        offset: f.offset,
        after_cursor: parse_after_cursor_input(f.after_cursor)?,
    })
}

/// Filter for `products.list()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ProductFilterInput {
    /// Catalogue status
    #[napi(ts_type = "ProductStatus")]
    pub status: Option<String>,
    /// Free-text search over name and description
    pub search: Option<String>,
    /// Matches the product's `category` attribute
    pub category: Option<String>,
    /// Exact decimal string (inclusive lower bound on variant price)
    pub min_price: Option<String>,
    /// Exact decimal string (inclusive upper bound on variant price)
    pub max_price: Option<String>,
    pub in_stock: Option<bool>,
    /// Page size
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: `[name, id]`
    pub after_cursor: Option<Vec<String>>,
}

pub(crate) fn product_filter_from_input(
    filter: Option<ProductFilterInput>,
) -> Result<stateset_core::ProductFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::ProductFilter::default());
    };
    Ok(stateset_core::ProductFilter {
        status: parse_enum_field(f.status.as_deref(), "product status")?,
        product_type: None,
        search: f.search,
        category: f.category,
        min_price: parse_decimal_field(f.min_price.as_deref(), "minPrice")?,
        max_price: parse_decimal_field(f.max_price.as_deref(), "maxPrice")?,
        in_stock: f.in_stock,
        limit: f.limit,
        offset: f.offset,
        after_cursor: parse_after_cursor_input(f.after_cursor)?,
    })
}

/// Filter for `returns.list()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ReturnFilterInput {
    pub order_id: Option<String>,
    pub customer_id: Option<String>,
    /// Return status (`canceled` and `intransit` are accepted too)
    #[napi(ts_type = "ReturnStatus | 'intransit' | 'canceled'")]
    pub status: Option<String>,
    /// Return reason (the unseparated spellings such as `wrongitem` are accepted too)
    #[napi(
        ts_type = "ReturnReason | 'wrongitem' | 'notasdescribed' | 'changedmind' | 'betterpricefound' | 'nolongerneeded'"
    )]
    pub reason: Option<String>,
    /// RFC 3339 timestamp (inclusive lower bound on created_at)
    pub from_date: Option<String>,
    /// RFC 3339 timestamp (inclusive upper bound on created_at)
    pub to_date: Option<String>,
    /// Page size
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: `[createdAt, id]`
    pub after_cursor: Option<Vec<String>>,
}

pub(crate) fn return_filter_from_input(
    filter: Option<ReturnFilterInput>,
) -> Result<stateset_core::ReturnFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::ReturnFilter::default());
    };
    Ok(stateset_core::ReturnFilter {
        order_id: parse_optional_id(f.order_id, "order")?,
        customer_id: parse_optional_id(f.customer_id, "customer")?,
        status: parse_enum_field(f.status.as_deref(), "return status")?,
        reason: parse_enum_field(f.reason.as_deref(), "return reason")?,
        from_date: parse_optional_datetime(f.from_date, "fromDate")?,
        to_date: parse_optional_datetime(f.to_date, "toDate")?,
        limit: f.limit,
        offset: f.offset,
        after_cursor: parse_after_cursor_input(f.after_cursor)?,
    })
}

/// Filter for `payments.list()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PaymentFilterInput {
    pub order_id: Option<String>,
    pub invoice_id: Option<String>,
    pub customer_id: Option<String>,
    /// Payment processing status (`canceled` is accepted too)
    #[napi(ts_type = "PaymentTransactionStatus | 'canceled'")]
    pub status: Option<String>,
    /// Payment method
    #[napi(ts_type = "PaymentMethodType")]
    pub payment_method: Option<String>,
    /// ISO 4217 currency code
    pub currency: Option<String>,
    /// Exact decimal string (inclusive lower bound on amount)
    pub min_amount: Option<String>,
    /// Exact decimal string (inclusive upper bound on amount)
    pub max_amount: Option<String>,
    /// RFC 3339 timestamp (inclusive lower bound on created_at)
    pub from_date: Option<String>,
    /// RFC 3339 timestamp (inclusive upper bound on created_at)
    pub to_date: Option<String>,
    /// Page size
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub(crate) fn payment_filter_from_input(
    filter: Option<PaymentFilterInput>,
) -> Result<stateset_core::PaymentFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::PaymentFilter::default());
    };
    Ok(stateset_core::PaymentFilter {
        order_id: parse_optional_id(f.order_id, "order")?,
        invoice_id: parse_optional_id(f.invoice_id, "invoice")?,
        customer_id: parse_optional_id(f.customer_id, "customer")?,
        status: parse_enum_field(f.status.as_deref(), "payment status")?,
        payment_method: parse_enum_field(f.payment_method.as_deref(), "payment method")?,
        processor: None,
        currency: parse_optional_currency(f.currency)?,
        min_amount: parse_decimal_field(f.min_amount.as_deref(), "minAmount")?,
        max_amount: parse_decimal_field(f.max_amount.as_deref(), "maxAmount")?,
        from_date: parse_optional_datetime(f.from_date, "fromDate")?,
        to_date: parse_optional_datetime(f.to_date, "toDate")?,
        limit: f.limit,
        offset: f.offset,
    })
}

/// Filter for `shipments.list()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ShipmentFilterInput {
    pub order_id: Option<String>,
    /// Shipment status (`canceled` and the unseparated spellings are accepted too)
    #[napi(
        ts_type = "ShipmentStatus | 'readytoship' | 'intransit' | 'outfordelivery' | 'canceled' | 'onhold'"
    )]
    pub status: Option<String>,
    /// Carrier
    #[napi(ts_type = "ShippingCarrierFilter")]
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    /// Page size
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub(crate) fn shipment_filter_from_input(
    filter: Option<ShipmentFilterInput>,
) -> Result<stateset_core::ShipmentFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::ShipmentFilter::default());
    };
    Ok(stateset_core::ShipmentFilter {
        order_id: parse_optional_id(f.order_id, "order")?,
        status: parse_enum_field(f.status.as_deref(), "shipment status")?,
        carrier: parse_enum_field(f.carrier.as_deref(), "carrier")?,
        tracking_number: f.tracking_number,
        limit: f.limit,
        offset: f.offset,
    })
}

/// Filter for `warranties.list()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct WarrantyFilterInput {
    pub customer_id: Option<String>,
    pub order_id: Option<String>,
    pub product_id: Option<String>,
    pub sku: Option<String>,
    pub serial_number: Option<String>,
    /// Warranty status
    #[napi(ts_type = "WarrantyStatus")]
    pub status: Option<String>,
    /// Warranty tier
    #[napi(ts_type = "WarrantyType")]
    pub warranty_type: Option<String>,
    /// Only warranties that have not expired
    pub active_only: Option<bool>,
    /// Only warranties expiring within this many days
    pub expiring_within_days: Option<i32>,
    /// Page size
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub(crate) fn warranty_filter_from_input(
    filter: Option<WarrantyFilterInput>,
) -> Result<stateset_core::WarrantyFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::WarrantyFilter::default());
    };
    Ok(stateset_core::WarrantyFilter {
        customer_id: parse_optional_id(f.customer_id, "customer")?,
        order_id: parse_optional_id(f.order_id, "order")?,
        product_id: parse_optional_id(f.product_id, "product")?,
        sku: f.sku,
        serial_number: f.serial_number,
        status: parse_enum_field(f.status.as_deref(), "warranty status")?,
        warranty_type: parse_enum_field(f.warranty_type.as_deref(), "warranty type")?,
        active_only: f.active_only,
        expiring_within_days: f.expiring_within_days,
        limit: f.limit,
        offset: f.offset,
    })
}

/// Filter for `invoices.list()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct InvoiceFilterInput {
    pub customer_id: Option<String>,
    pub order_id: Option<String>,
    /// Invoice status
    #[napi(ts_type = "InvoiceStatus")]
    pub status: Option<String>,
    /// Only invoices past their due date
    pub overdue_only: Option<bool>,
    /// RFC 3339 timestamp (inclusive lower bound on invoice date)
    pub from_date: Option<String>,
    /// RFC 3339 timestamp (inclusive upper bound on invoice date)
    pub to_date: Option<String>,
    /// RFC 3339 timestamp (inclusive lower bound on due date)
    pub due_from: Option<String>,
    /// RFC 3339 timestamp (inclusive upper bound on due date)
    pub due_to: Option<String>,
    /// Exact decimal string (inclusive lower bound on total)
    pub min_total: Option<String>,
    /// Exact decimal string (inclusive upper bound on total)
    pub max_total: Option<String>,
    /// Exact decimal string (inclusive lower bound on balance due)
    pub min_balance: Option<String>,
    /// Search by invoice number
    pub invoice_number: Option<String>,
    /// Page size
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub(crate) fn invoice_filter_from_input(
    filter: Option<InvoiceFilterInput>,
) -> Result<stateset_core::InvoiceFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::InvoiceFilter::default());
    };
    Ok(stateset_core::InvoiceFilter {
        customer_id: parse_optional_id(f.customer_id, "customer")?,
        order_id: parse_optional_id(f.order_id, "order")?,
        status: parse_enum_field(f.status.as_deref(), "invoice status")?,
        invoice_type: None,
        overdue_only: f.overdue_only,
        from_date: parse_optional_datetime(f.from_date, "fromDate")?,
        to_date: parse_optional_datetime(f.to_date, "toDate")?,
        due_from: parse_optional_datetime(f.due_from, "dueFrom")?,
        due_to: parse_optional_datetime(f.due_to, "dueTo")?,
        min_total: parse_decimal_field(f.min_total.as_deref(), "minTotal")?,
        max_total: parse_decimal_field(f.max_total.as_deref(), "maxTotal")?,
        min_balance: parse_decimal_field(f.min_balance.as_deref(), "minBalance")?,
        invoice_number: f.invoice_number,
        limit: f.limit,
        offset: f.offset,
    })
}

/// Filter for `bom.list()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct BomFilterInput {
    pub product_id: Option<String>,
    /// BOM status
    #[napi(ts_type = "BomStatus")]
    pub status: Option<String>,
    /// Free-text search over name and BOM number
    pub search: Option<String>,
    /// Page size
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub(crate) fn bom_filter_from_input(
    filter: Option<BomFilterInput>,
) -> Result<stateset_core::BomFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::BomFilter::default());
    };
    Ok(stateset_core::BomFilter {
        product_id: parse_optional_id(f.product_id, "product")?,
        status: parse_enum_field(f.status.as_deref(), "BOM status")?,
        search: f.search,
        limit: f.limit,
        offset: f.offset,
    })
}

/// Filter for `carts.list()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CartFilterInput {
    pub customer_id: Option<String>,
    pub customer_email: Option<String>,
    /// Cart status (`canceled` and the unseparated spellings are accepted too)
    #[napi(ts_type = "CartStatus | 'readyforpayment' | 'paymentpending' | 'canceled'")]
    pub status: Option<String>,
    pub has_items: Option<bool>,
    pub is_abandoned: Option<bool>,
    /// RFC 3339 timestamp (inclusive lower bound on created_at)
    pub created_after: Option<String>,
    /// RFC 3339 timestamp (inclusive upper bound on created_at)
    pub created_before: Option<String>,
    /// Page size
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub(crate) fn cart_filter_from_input(
    filter: Option<CartFilterInput>,
) -> Result<stateset_core::CartFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::CartFilter::default());
    };
    Ok(stateset_core::CartFilter {
        customer_id: parse_optional_id(f.customer_id, "customer")?,
        customer_email: f.customer_email,
        status: parse_enum_field(f.status.as_deref(), "cart status")?,
        has_items: f.has_items,
        is_abandoned: f.is_abandoned,
        created_after: parse_optional_datetime(f.created_after, "createdAfter")?,
        created_before: parse_optional_datetime(f.created_before, "createdBefore")?,
        limit: f.limit,
        offset: f.offset,
    })
}

/// Filter for `purchaseOrders.listSuppliers()`
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct SupplierFilterInput {
    /// Search by name
    pub name: Option<String>,
    /// Filter by country
    pub country: Option<String>,
    /// Only active suppliers
    pub active_only: Option<bool>,
    /// Page size (server default 100)
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

pub(crate) fn supplier_filter_from_input(
    filter: Option<SupplierFilterInput>,
) -> Result<stateset_core::SupplierFilter> {
    let Some(f) = filter else {
        return Ok(stateset_core::SupplierFilter::default());
    };
    Ok(stateset_core::SupplierFilter {
        name: f.name,
        country: f.country,
        active_only: f.active_only,
        limit: f.limit,
        offset: f.offset,
    })
}
