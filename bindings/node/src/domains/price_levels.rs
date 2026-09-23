//! Price Levels  (B2B pricing tiers).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Price Levels  (B2B pricing tiers)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePriceLevelInput {
    pub name: String,
    pub code: String,
    pub description: Option<String>,
    /// none, percentage_discount, percentage_markup (default none)
    #[napi(ts_type = "PriceAdjustmentType")]
    pub adjustment_type: Option<String>,
    /// Percentage as exact decimal string (e.g. "10" for 10%); default "0"
    pub adjustment_value: Option<String>,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdatePriceLevelInput {
    pub name: Option<String>,
    pub description: Option<String>,
    /// none, percentage_discount, percentage_markup
    #[napi(ts_type = "PriceAdjustmentType")]
    pub adjustment_type: Option<String>,
    /// Percentage as exact decimal string
    pub adjustment_value: Option<String>,
    pub is_active: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PriceLevelFilterInput {
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PriceLevelOutput {
    pub id: String,
    pub name: String,
    pub code: String,
    pub description: Option<String>,
    /// none, percentage_discount, percentage_markup
    #[napi(ts_type = "PriceAdjustmentType")]
    pub adjustment_type: String,
    /// Percentage as exact decimal string
    pub adjustment_value: String,
    pub currency: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PriceLevel> for PriceLevelOutput {
    fn from(l: stateset_core::PriceLevel) -> Self {
        Self {
            id: l.id.to_string(),
            name: l.name,
            code: l.code,
            description: l.description,
            adjustment_type: format!("{}", l.adjustment_type),
            adjustment_value: l.adjustment_value.to_string(),
            currency: l.currency.to_string(),
            is_active: l.is_active,
            created_at: l.created_at.to_rfc3339(),
            updated_at: l.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PriceLevelEntryOutput {
    pub price_level_id: String,
    pub product_id: String,
    /// Exact decimal string
    pub price: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PriceLevelEntry> for PriceLevelEntryOutput {
    fn from(e: stateset_core::PriceLevelEntry) -> Self {
        Self {
            price_level_id: e.price_level_id.to_string(),
            product_id: e.product_id.to_string(),
            price: e.price.to_string(),
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct PriceLevels {
    pub(crate) commerce: Handle,
}

#[napi]
impl PriceLevels {
    /// Whether the price-levels backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.price_levels().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreatePriceLevelInput) -> Result<PriceLevelOutput> {
        let commerce = self.commerce.get()?;
        let adjustment_type = input
            .adjustment_type
            .map(|s| {
                s.parse::<stateset_core::PriceAdjustmentType>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid price adjustment type"))
            })
            .transpose()?
            .unwrap_or_default();
        let adjustment_value = input
            .adjustment_value
            .as_deref()
            .map(|s| parse_decimal_str(s, "adjustment_value"))
            .transpose()?
            .unwrap_or(Decimal::ZERO);
        let level = commerce
            .price_levels()
            .create(stateset_core::CreatePriceLevel {
                name: input.name,
                code: input.code,
                description: input.description,
                adjustment_type,
                adjustment_value,
                currency: parse_currency_opt(input.currency)?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create price level", e))?;
        Ok(level.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PriceLevelOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "price level")?;
        let level = commerce
            .price_levels()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get price level", e))?;
        Ok(level.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdatePriceLevelInput,
    ) -> Result<PriceLevelOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "price level")?;
        let adjustment_type = input
            .adjustment_type
            .map(|s| {
                s.parse::<stateset_core::PriceAdjustmentType>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid price adjustment type"))
            })
            .transpose()?;
        let level = commerce
            .price_levels()
            .update(
                uuid.into(),
                stateset_core::UpdatePriceLevel {
                    name: input.name,
                    description: input.description,
                    adjustment_type,
                    adjustment_value: parse_optional_decimal_str(
                        input.adjustment_value,
                        "adjustment_value",
                    )?,
                    is_active: input.is_active,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update price level", e))?;
        Ok(level.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<PriceLevelFilterInput>,
    ) -> Result<Vec<PriceLevelOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(stateset_core::PriceLevelFilter::default, |f| {
            stateset_core::PriceLevelFilter {
                is_active: f.is_active,
                limit: f.limit,
                offset: f.offset,
            }
        });
        let levels = commerce
            .price_levels()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list price levels", e))?;
        Ok(levels.into_iter().map(Into::into).collect())
    }

    /// Delete a price level and its entries.
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "price level")?;
        commerce
            .price_levels()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete price level", e))?;
        Ok(())
    }

    /// Upsert a per-product fixed price entry (exact decimal string).
    #[napi]
    pub async fn set_entry(
        &self,
        id: String,
        product_id: String,
        price: String,
    ) -> Result<PriceLevelEntryOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "price level")?;
        let product_uuid = parse_uuid_str(&product_id, "product")?;
        let entry = commerce
            .price_levels()
            .set_entry(uuid.into(), product_uuid.into(), parse_decimal_str(&price, "price")?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set price level entry", e))?;
        Ok(entry.into())
    }

    /// Remove a per-product entry.
    #[napi]
    pub async fn delete_entry(&self, id: String, product_id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "price level")?;
        let product_uuid = parse_uuid_str(&product_id, "product")?;
        commerce
            .price_levels()
            .delete_entry(uuid.into(), product_uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete price level entry", e))?;
        Ok(())
    }

    /// List per-product entries for a level.
    #[napi]
    pub async fn list_entries(&self, id: String) -> Result<Vec<PriceLevelEntryOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "price level")?;
        let entries = commerce
            .price_levels()
            .list_entries(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list price level entries", e))?;
        Ok(entries.into_iter().map(Into::into).collect())
    }
}
