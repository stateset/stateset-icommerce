//! Price Schedules  (time-bounded pricing).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Price Schedules  (time-bounded pricing)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePriceScheduleInput {
    pub name: String,
    pub code: Option<String>,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
    /// RFC 3339 timestamp
    pub starts_at: Option<String>,
    /// RFC 3339 timestamp
    pub ends_at: Option<String>,
    /// Priority used to break ties (higher wins); default 0
    pub priority: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdatePriceScheduleInput {
    pub name: Option<String>,
    pub code: Option<String>,
    /// RFC 3339 timestamp
    pub starts_at: Option<String>,
    /// RFC 3339 timestamp
    pub ends_at: Option<String>,
    pub is_active: Option<bool>,
    pub priority: Option<i32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PriceScheduleFilterInput {
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PriceScheduleOutput {
    pub id: String,
    pub name: String,
    pub code: Option<String>,
    pub currency: String,
    /// RFC 3339 timestamp
    pub starts_at: Option<String>,
    /// RFC 3339 timestamp
    pub ends_at: Option<String>,
    pub is_active: bool,
    pub priority: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PriceSchedule> for PriceScheduleOutput {
    fn from(s: stateset_core::PriceSchedule) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            code: s.code,
            currency: s.currency.to_string(),
            starts_at: s.starts_at.map(|d| d.to_rfc3339()),
            ends_at: s.ends_at.map(|d| d.to_rfc3339()),
            is_active: s.is_active,
            priority: s.priority,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PriceScheduleEntryOutput {
    pub price_schedule_id: String,
    pub product_id: String,
    /// Exact decimal string
    pub price: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PriceScheduleEntry> for PriceScheduleEntryOutput {
    fn from(e: stateset_core::PriceScheduleEntry) -> Self {
        Self {
            price_schedule_id: e.price_schedule_id.to_string(),
            product_id: e.product_id.to_string(),
            price: e.price.to_string(),
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct PriceSchedules {
    pub(crate) commerce: Handle,
}

#[napi]
impl PriceSchedules {
    /// Whether the price-schedules backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.price_schedules().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreatePriceScheduleInput) -> Result<PriceScheduleOutput> {
        let commerce = self.commerce.get()?;
        let schedule = commerce
            .price_schedules()
            .create(stateset_core::CreatePriceSchedule {
                name: input.name,
                code: input.code,
                currency: parse_currency_opt(input.currency)?,
                starts_at: parse_rfc3339_opt(input.starts_at, "starts_at")?,
                ends_at: parse_rfc3339_opt(input.ends_at, "ends_at")?,
                priority: input.priority.unwrap_or(0),
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create price schedule", e))?;
        Ok(schedule.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PriceScheduleOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "price schedule")?;
        let schedule = commerce
            .price_schedules()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get price schedule", e))?;
        Ok(schedule.map(Into::into))
    }

    #[napi]
    pub async fn update(
        &self,
        id: String,
        input: UpdatePriceScheduleInput,
    ) -> Result<PriceScheduleOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "price schedule")?;
        let schedule = commerce
            .price_schedules()
            .update(
                uuid.into(),
                stateset_core::UpdatePriceSchedule {
                    name: input.name,
                    code: input.code,
                    starts_at: parse_rfc3339_opt(input.starts_at, "starts_at")?,
                    ends_at: parse_rfc3339_opt(input.ends_at, "ends_at")?,
                    is_active: input.is_active,
                    priority: input.priority,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update price schedule", e))?;
        Ok(schedule.into())
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<PriceScheduleFilterInput>,
    ) -> Result<Vec<PriceScheduleOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(stateset_core::PriceScheduleFilter::default, |f| {
            stateset_core::PriceScheduleFilter {
                is_active: f.is_active,
                limit: f.limit,
                offset: f.offset,
            }
        });
        let schedules = commerce
            .price_schedules()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list price schedules", e))?;
        Ok(schedules.into_iter().map(Into::into).collect())
    }

    /// Delete a price schedule and its entries.
    #[napi]
    pub async fn delete(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "price schedule")?;
        commerce
            .price_schedules()
            .delete(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete price schedule", e))?;
        Ok(())
    }

    /// Upsert a per-product scheduled price (exact decimal string).
    #[napi]
    pub async fn set_entry(
        &self,
        id: String,
        product_id: String,
        price: String,
    ) -> Result<PriceScheduleEntryOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "price schedule")?;
        let product_uuid = parse_uuid_str(&product_id, "product")?;
        let entry = commerce
            .price_schedules()
            .set_entry(uuid.into(), product_uuid.into(), parse_decimal_str(&price, "price")?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to set price schedule entry", e))?;
        Ok(entry.into())
    }

    /// Remove a per-product entry.
    #[napi]
    pub async fn delete_entry(&self, id: String, product_id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "price schedule")?;
        let product_uuid = parse_uuid_str(&product_id, "product")?;
        commerce
            .price_schedules()
            .delete_entry(uuid.into(), product_uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete price schedule entry", e))?;
        Ok(())
    }

    /// List per-product entries for a schedule.
    #[napi]
    pub async fn list_entries(&self, id: String) -> Result<Vec<PriceScheduleEntryOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "price schedule")?;
        let entries = commerce
            .price_schedules()
            .list_entries(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list price schedule entries", e))?;
        Ok(entries.into_iter().map(Into::into).collect())
    }

    /// Resolve the effective scheduled price for a product at an instant
    /// (`at` is an RFC 3339 timestamp; defaults to now). Returns an exact
    /// decimal string, or null when no schedule applies.
    #[napi]
    pub async fn resolve_price(
        &self,
        product_id: String,
        at: Option<String>,
    ) -> Result<Option<String>> {
        let commerce = self.commerce.get()?;
        let product_uuid = parse_uuid_str(&product_id, "product")?;
        let at = parse_rfc3339_opt(at, "at")?.unwrap_or_else(chrono::Utc::now);
        let price = commerce
            .price_schedules()
            .resolve_price(product_uuid.into(), at)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to resolve scheduled price", e))?;
        Ok(price.map(|p| p.to_string()))
    }
}
