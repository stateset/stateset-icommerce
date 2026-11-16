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

/// Filter for `Inventory.list`. Every field is optional.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct InventoryListFilter {
    /// Lists SKUs containing this text (ASCII case-insensitive).
    pub sku: Option<String>,
    /// `true` lists only active items, `false` only inactive ones; omit for both.
    pub is_active: Option<bool>,
    /// `true` lists only items below their reorder threshold (reorder point
    /// plus safety stock) at some location, the rule the engine's reorder
    /// report applies; `false` lists only items that are not.
    pub low_stock: Option<bool>,
    /// Page size: default 100, capped at 1000.
    pub limit: Option<u32>,
    /// Items to skip, in SKU order.
    pub offset: Option<u32>,
}

/// One inventory item with its stock summed over every location.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct InventoryListItemOutput {
    pub id: i64,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub unit_of_measure: String,
    pub is_active: bool,
    /// Exact decimal string summed over locations (`"0"` with no balance).
    pub on_hand: String,
    /// Exact decimal string: reserved or held units.
    pub allocated: String,
    /// Exact decimal string: on hand minus allocated.
    pub available: String,
    /// Below its reorder threshold at some location.
    pub low_stock: bool,
}

/// Default `Inventory.list` page size.
const INVENTORY_LIST_DEFAULT_LIMIT: u32 = 100;
/// Largest `Inventory.list` page (the repositories' pagination ceiling).
const INVENTORY_LIST_MAX_LIMIT: u32 = 1000;

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

    /// List inventory items in SKU order, each with its stock summed over
    /// every location, so items created with `createItem` are listed even
    /// when no product variant carries their SKU.
    ///
    /// `lowStock` uses the engine's reorder rule (below reorder point plus
    /// safety stock at some location, as `get_reorder_needed` reports it).
    /// Pages hold at most 1000 items (default 100).
    #[napi]
    pub async fn list(
        &self,
        filter: Option<InventoryListFilter>,
    ) -> Result<Vec<InventoryListItemOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();
        let limit =
            filter.limit.unwrap_or(INVENTORY_LIST_DEFAULT_LIMIT).min(INVENTORY_LIST_MAX_LIMIT);
        let offset = filter.offset.unwrap_or(0);
        let inventory = commerce.inventory();
        let low: std::collections::HashSet<String> = inventory
            .get_reorder_needed()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list low-stock items", e))?
            .into_iter()
            .map(|level| level.sku)
            .collect();
        let base = stateset_core::InventoryFilter {
            sku: filter.sku.clone(),
            is_active: filter.is_active,
            ..Default::default()
        };
        let items = match filter.low_stock {
            // The repository pages the listing without a low-stock filter, so
            // the filter runs over every page and `limit`/`offset` count
            // matching items only.
            Some(wanted) => {
                let mut matching = Vec::new();
                let mut page_offset = 0_u32;
                loop {
                    let page = inventory
                        .list(stateset_core::InventoryFilter {
                            limit: Some(INVENTORY_LIST_MAX_LIMIT),
                            offset: Some(page_offset),
                            ..base.clone()
                        })
                        .map_err(|e| wrap(ErrCode::Internal, "Failed to list inventory", e))?;
                    let full = page.len() == INVENTORY_LIST_MAX_LIMIT as usize;
                    matching
                        .extend(page.into_iter().filter(|item| low.contains(&item.sku) == wanted));
                    if !full {
                        break;
                    }
                    page_offset += INVENTORY_LIST_MAX_LIMIT;
                }
                matching.into_iter().skip(offset as usize).take(limit as usize).collect()
            }
            None => inventory
                .list(stateset_core::InventoryFilter {
                    limit: Some(limit),
                    offset: Some(offset),
                    ..base
                })
                .map_err(|e| wrap(ErrCode::Internal, "Failed to list inventory", e))?,
        };
        items
            .into_iter()
            .map(|item| {
                let stock = inventory
                    .get_stock(&item.sku)
                    .map_err(|e| wrap(ErrCode::Internal, "Failed to get stock", e))?;
                let (on_hand, allocated, available) = stock.map_or_else(
                    || ("0".to_owned(), "0".to_owned(), "0".to_owned()),
                    |level| {
                        (
                            level.total_on_hand.to_string(),
                            level.total_allocated.to_string(),
                            level.total_available.to_string(),
                        )
                    },
                );
                Ok(InventoryListItemOutput {
                    low_stock: low.contains(&item.sku),
                    id: item.id,
                    sku: item.sku,
                    name: item.name,
                    description: item.description,
                    unit_of_measure: item.unit_of_measure,
                    is_active: item.is_active,
                    on_hand,
                    allocated,
                    available,
                })
            })
            .collect()
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

    /// Atomically consume confirmed holds after full shipment. Safe to retry;
    /// shipping status alone does not post the physical stock transaction.
    #[napi]
    pub async fn fulfill_order_stock(&self, order_id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let id = order_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid order UUID"))?;
        commerce
            .inventory()
            .fulfill_order_stock(id)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to post order stock", e))
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
