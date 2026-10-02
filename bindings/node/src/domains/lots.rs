//! Lots/Batch Tracking API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Lots/Batch Tracking API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateLotInput {
    pub lot_number: Option<String>,
    pub sku: String,
    pub quantity_produced: f64,
    pub production_date: Option<String>,
    pub expiration_date: Option<String>,
    pub supplier_lot_number: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LotOutput {
    pub id: String,
    pub lot_number: String,
    pub sku: String,
    pub quantity_produced: f64,
    pub quantity_available: f64,
    pub quantity_reserved: f64,
    #[napi(ts_type = "LotStatus")]
    pub status: String,
    pub production_date: Option<String>,
    pub expiration_date: Option<String>,
    pub created_at: String,
}

impl TryFrom<stateset_core::Lot> for LotOutput {
    type Error = Error;

    fn try_from(l: stateset_core::Lot) -> Result<Self> {
        let qty_available = l.quantity_available();
        Ok(Self {
            id: l.id.to_string(),
            lot_number: l.lot_number,
            sku: l.sku,
            quantity_produced: to_f64_checked(l.quantity_produced, "lot quantity produced")?,
            quantity_available: to_f64_checked(qty_available, "lot quantity available")?,
            quantity_reserved: to_f64_checked(l.quantity_reserved, "lot quantity reserved")?,
            status: format!("{:?}", l.status),
            production_date: Some(l.production_date.to_rfc3339()),
            expiration_date: l.expiration_date.map(|d| d.to_rfc3339()),
            created_at: l.created_at.to_rfc3339(),
        })
    }
}

/// Optional filters for `Lots.list`. Every field is optional; an empty object
/// (or no argument) lists everything, unpaginated, as before.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct LotFilterInput {
    pub sku: Option<String>,
    pub lot_number: Option<String>,
    /// The rendered form (`OnHold`) or the engine's snake_case (`on_hold`).
    #[napi(ts_type = "LotStatusInput")]
    pub status: Option<String>,
    pub supplier_id: Option<String>,
    pub work_order_id: Option<String>,
    pub purchase_order_id: Option<String>,
    /// RFC 3339 timestamp.
    pub expiring_before: Option<String>,
    /// RFC 3339 timestamp.
    pub expiring_after: Option<String>,
    pub has_quantity: Option<bool>,
    pub location_id: Option<i32>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<LotFilterInput> for stateset_core::LotFilter {
    type Error = Error;

    fn try_from(f: LotFilterInput) -> Result<Self> {
        Ok(Self {
            sku: f.sku,
            lot_number: f.lot_number,
            status: parse_optional_serde_enum(f.status, "lot status")?,
            supplier_id: parse_optional_id(f.supplier_id, "supplier")?,
            work_order_id: parse_optional_id(f.work_order_id, "work order")?,
            purchase_order_id: parse_optional_id(f.purchase_order_id, "purchase order")?,
            expiring_before: parse_optional_datetime(f.expiring_before, "expiring before")?,
            expiring_after: parse_optional_datetime(f.expiring_after, "expiring after")?,
            has_quantity: f.has_quantity,
            location_id: f.location_id,
            limit: f.limit,
            offset: f.offset,
        })
    }
}

/// `OnHold` -> `on_hold`, `on-hold` -> `on_hold`, `ON_HOLD` -> `on_hold`.
///
/// Most outputs in these domains render the Rust `Debug` form of an enum
/// (`PartiallyFulfilled`), while the engine parses snake_case
/// (`partially_fulfilled`). A caller who filters by a value it just read
/// back should not have to re-spell it, so filter inputs accept both.
pub(crate) fn snake_case_token(s: &str) -> String {
    let s = s.trim();
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_lower_or_digit = false;
    for ch in s.chars() {
        if ch == '-' || ch == ' ' {
            out.push('_');
            prev_lower_or_digit = false;
        } else if ch.is_ascii_uppercase() {
            if prev_lower_or_digit {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_lower_or_digit = false;
        } else {
            out.push(ch);
            prev_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }
    out
}

/// The `VALIDATION` error for a hand-written enum parser whose input matched
/// no accepted spelling. The message lists every spelling so the caller can
/// correct the call; nothing is ever mapped onto a default variant.
pub(crate) fn unknown_variant(what: &str, value: &str, expected: &[&str]) -> napi::Error {
    coded(
        ErrCode::Validation,
        format!("Invalid {what} '{value}'; expected one of: {}", expected.join(", ")),
    )
}

/// Parse an optional enumeration filter through the engine's `FromStr`,
/// accepting the rendered form as well (see [`snake_case_token`]). A value
/// that matches neither is refused with `VALIDATION`; nothing is dropped.
pub(crate) fn parse_optional_enum<T: FromStr>(
    value: Option<String>,
    what: &str,
) -> Result<Option<T>> {
    value
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            s.trim()
                .parse::<T>()
                .or_else(|_| snake_case_token(&s).parse::<T>())
                .map_err(|_| coded(ErrCode::Validation, format!("Invalid {what} '{s}'")))
        })
        .transpose()
}

/// [`parse_optional_enum`] for engine enums that only deserialise through
/// serde (`snake_case`), such as `LotStatus` and `SerialStatus`.
pub(crate) fn parse_optional_serde_enum<T: serde::de::DeserializeOwned>(
    value: Option<String>,
    what: &str,
) -> Result<Option<T>> {
    value
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            serde_json::from_value::<T>(serde_json::Value::String(snake_case_token(&s)))
                .map_err(|_| coded(ErrCode::Validation, format!("Invalid {what} '{s}'")))
        })
        .transpose()
}

#[napi]
pub struct Lots {
    pub(crate) commerce: Handle,
}

#[napi]
impl Lots {
    /// Create a new lot
    #[napi]
    pub async fn create(&self, input: CreateLotInput) -> Result<LotOutput> {
        let commerce = self.commerce.get()?;
        let lot = commerce
            .lots()
            .create(stateset_core::CreateLot {
                lot_number: input.lot_number,
                sku: input.sku,
                quantity: decimal_from_f64(input.quantity_produced, "lot quantity produced")?,
                production_date: parse_optional_datetime(input.production_date, "production date")?,
                expiration_date: parse_optional_datetime(input.expiration_date, "expiration date")?,
                supplier_lot: input.supplier_lot_number,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create lot", e))?;
        convert_output(lot)
    }

    /// Get a lot by ID
    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<LotOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let lot = commerce
            .lots()
            .get(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get lot", e))?;
        convert_optional_output(lot)
    }

    /// Get a lot by lot number
    #[napi]
    pub async fn get_by_number(&self, lot_number: String) -> Result<Option<LotOutput>> {
        let commerce = self.commerce.get()?;
        let lot = commerce
            .lots()
            .get_by_number(&lot_number)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get lot", e))?;
        convert_optional_output(lot)
    }

    /// List lots, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list(&self, filter: Option<LotFilterInput>) -> Result<Vec<LotOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::LotFilter = filter.unwrap_or_default().try_into()?;
        let lots = commerce
            .lots()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list lots", e))?;
        convert_outputs(lots)
    }

    /// Get active lots for a SKU
    #[napi]
    pub async fn get_active_lots(&self, sku: String) -> Result<Vec<LotOutput>> {
        let commerce = self.commerce.get()?;
        let lots = commerce
            .lots()
            .get_active_lots(&sku)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get active lots", e))?;
        convert_outputs(lots)
    }

    /// Get available lots for a SKU (FIFO order)
    #[napi]
    pub async fn get_available_lots_for_sku(&self, sku: String) -> Result<Vec<LotOutput>> {
        let commerce = self.commerce.get()?;
        let lots = commerce
            .lots()
            .get_available_lots_for_sku(&sku)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get available lots", e))?;
        convert_outputs(lots)
    }

    /// Quarantine a lot
    #[napi]
    pub async fn quarantine(&self, id: String, reason: String) -> Result<LotOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let lot = commerce
            .lots()
            .quarantine(uuid, &reason)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to quarantine lot", e))?;
        convert_output(lot)
    }

    /// Release a lot from quarantine
    #[napi]
    pub async fn release_quarantine(&self, id: String) -> Result<LotOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let lot = commerce
            .lots()
            .release_quarantine(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to release lot", e))?;
        convert_output(lot)
    }

    /// Get expiring lots within days
    #[napi]
    pub async fn get_expiring_lots(&self, days: i32) -> Result<Vec<LotOutput>> {
        let commerce = self.commerce.get()?;
        let lots = commerce
            .lots()
            .get_expiring_lots(days)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get expiring lots", e))?;
        convert_outputs(lots)
    }

    /// Get expired lots
    #[napi]
    pub async fn get_expired_lots(&self) -> Result<Vec<LotOutput>> {
        let commerce = self.commerce.get()?;
        let lots = commerce
            .lots()
            .get_expired_lots()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get expired lots", e))?;
        convert_outputs(lots)
    }

    /// Get quarantined lots
    #[napi]
    pub async fn get_quarantined(&self) -> Result<Vec<LotOutput>> {
        let commerce = self.commerce.get()?;
        let lots = commerce
            .lots()
            .get_quarantined()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get quarantined lots", e))?;
        convert_outputs(lots)
    }

    /// Count lots
    #[napi]
    pub async fn count(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .lots()
            .count(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count lots", e))?;
        Ok(count as u32)
    }
}
