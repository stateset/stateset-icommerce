//! Inventory API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Inventory API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateInventoryItemInput {
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub initial_quantity: Option<f64>,
    pub reorder_point: Option<f64>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InventoryItemOutput {
    pub id: i64,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub unit_of_measure: String,
    pub is_active: bool,
}

impl From<stateset_core::InventoryItem> for InventoryItemOutput {
    fn from(i: stateset_core::InventoryItem) -> Self {
        Self {
            id: i.id,
            sku: i.sku,
            name: i.name,
            description: i.description,
            unit_of_measure: i.unit_of_measure,
            is_active: i.is_active,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StockLevelOutput {
    pub sku: String,
    pub name: String,
    pub total_on_hand: String,
    pub total_allocated: String,
    pub total_available: String,
}

impl TryFrom<stateset_core::StockLevel> for StockLevelOutput {
    type Error = Error;

    fn try_from(s: stateset_core::StockLevel) -> Result<Self> {
        Ok(Self {
            sku: s.sku,
            name: s.name,
            total_on_hand: s.total_on_hand.to_string(),
            total_allocated: s.total_allocated.to_string(),
            total_available: s.total_available.to_string(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ReservationOutput {
    pub id: String,
    pub item_id: i64,
    pub quantity: String,
    #[napi(ts_type = "ReservationStatus")]
    pub status: String,
}

impl TryFrom<stateset_core::InventoryReservation> for ReservationOutput {
    type Error = Error;

    fn try_from(r: stateset_core::InventoryReservation) -> Result<Self> {
        Ok(Self {
            id: r.id.to_string(),
            item_id: r.item_id,
            quantity: r.quantity.to_string(),
            status: format!("{}", r.status),
        })
    }
}

#[napi]
pub struct Inventory {
    pub(crate) commerce: Handle,
}

#[napi]
impl Inventory {
    #[napi]
    pub async fn create_item(
        &self,
        input: CreateInventoryItemInput,
    ) -> Result<InventoryItemOutput> {
        let commerce = self.commerce.get()?;

        let item = commerce
            .inventory()
            .create_item(stateset_core::CreateInventoryItem {
                sku: input.sku,
                name: input.name,
                description: input.description,
                initial_quantity: optional_decimal_from_f64(
                    input.initial_quantity,
                    "inventory initial quantity",
                )?,
                reorder_point: optional_decimal_from_f64(
                    input.reorder_point,
                    "inventory reorder point",
                )?,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create inventory item", e))?;

        Ok(item.into())
    }

    #[napi]
    pub async fn get_stock(&self, sku: String) -> Result<Option<StockLevelOutput>> {
        let commerce = self.commerce.get()?;
        let stock = commerce
            .inventory()
            .get_stock(&sku)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get stock", e))?;

        convert_optional_output(stock)
    }

    #[napi]
    pub async fn adjust(&self, sku: String, quantity: f64, reason: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let qty = Decimal::from_f64(quantity)
            .ok_or_else(|| coded(ErrCode::Validation, "Invalid quantity"))?;

        commerce
            .inventory()
            .adjust(&sku, qty, &reason)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to adjust inventory", e))?;

        Ok(())
    }

    #[napi]
    pub async fn reserve(
        &self,
        sku: String,
        quantity: f64,
        reference_type: String,
        reference_id: String,
        expires_in_seconds: Option<i64>,
    ) -> Result<ReservationOutput> {
        let commerce = self.commerce.get()?;
        let qty = Decimal::from_f64(quantity)
            .ok_or_else(|| coded(ErrCode::Validation, "Invalid quantity"))?;

        let reservation = commerce
            .inventory()
            .reserve(&sku, qty, &reference_type, &reference_id, expires_in_seconds)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to reserve inventory", e))?;

        convert_output(reservation)
    }

    #[napi]
    pub async fn confirm_reservation(&self, reservation_id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid =
            reservation_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        commerce
            .inventory()
            .confirm_reservation(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to confirm reservation", e))?;

        Ok(())
    }

    #[napi]
    pub async fn release_reservation(&self, reservation_id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid =
            reservation_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;

        commerce
            .inventory()
            .release_reservation(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to release reservation", e))?;

        Ok(())
    }
}
