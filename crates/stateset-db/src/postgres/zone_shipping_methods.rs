//! PostgreSQL implementation of zone shipping method repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateZoneShippingMethod, CurrencyCode, Result, ShippingCondition,
    ShippingMethodId, ShippingMethodType, ZoneShippingMethod, ZoneShippingMethodFilter,
    ZoneShippingMethodRepository, ZoneShippingRate, ZoneShippingRateRequest,
};
use uuid::Uuid;

/// PostgreSQL zone shipping method repository
#[derive(Debug, Clone)]
pub struct PgZoneShippingMethodRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ZoneShippingMethodRow {
    id: Uuid,
    zone_id: Uuid,
    name: String,
    carrier: Option<String>,
    method_type: String,
    base_rate: Decimal,
    currency: CurrencyCode,
    min_delivery_days: Option<i32>,
    max_delivery_days: Option<i32>,
    conditions: serde_json::Value,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgZoneShippingMethodRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_method(row: ZoneShippingMethodRow) -> Result<ZoneShippingMethod> {
        let ZoneShippingMethodRow {
            id,
            zone_id,
            name,
            carrier,
            method_type,
            base_rate,
            currency,
            min_delivery_days,
            max_delivery_days,
            conditions,
            is_active,
            created_at,
            updated_at,
        } = row;

        let method_type: ShippingMethodType = method_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid zone_shipping_method.method_type '{}': {}",
                method_type, e
            ))
        })?;

        let conditions: Vec<ShippingCondition> =
            serde_json::from_value(conditions).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid zone_shipping_method.conditions: {}",
                    e
                ))
            })?;

        Ok(ZoneShippingMethod {
            id: ShippingMethodId::from(id),
            zone_id: zone_id.into(),
            name,
            carrier,
            method_type,
            base_rate,
            currency,
            min_delivery_days,
            max_delivery_days,
            conditions,
            is_active,
            created_at,
            updated_at,
        })
    }

    // ---- async methods ----

    /// Create a zone shipping method (async)
    pub async fn create_async(
        &self,
        input: CreateZoneShippingMethod,
    ) -> Result<ZoneShippingMethod> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let conditions_json = serde_json::to_value(&input.conditions)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            "INSERT INTO zone_shipping_methods (id, zone_id, name, carrier, method_type, base_rate, currency,
             min_delivery_days, max_delivery_days, conditions, is_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, true, $11, $12)",
        )
        .bind(id)
        .bind(input.zone_id.into_uuid())
        .bind(&input.name)
        .bind(&input.carrier)
        .bind(input.method_type.to_string())
        .bind(input.base_rate)
        .bind(input.currency)
        .bind(input.min_delivery_days)
        .bind(input.max_delivery_days)
        .bind(&conditions_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(ShippingMethodId::from(id)).await?.ok_or(CommerceError::NotFound)
    }

    /// Get zone shipping method by ID (async)
    pub async fn get_async(&self, id: ShippingMethodId) -> Result<Option<ZoneShippingMethod>> {
        let row = sqlx::query_as::<_, ZoneShippingMethodRow>(
            "SELECT id, zone_id, name, carrier, method_type, base_rate, currency,
             min_delivery_days, max_delivery_days, conditions, is_active, created_at, updated_at
             FROM zone_shipping_methods WHERE id = $1",
        )
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_method).transpose()
    }

    /// List zone shipping methods with filter (async)
    pub async fn list_async(
        &self,
        filter: ZoneShippingMethodFilter,
    ) -> Result<Vec<ZoneShippingMethod>> {
        let mut sql = String::from(
            "SELECT id, zone_id, name, carrier, method_type, base_rate, currency,
             min_delivery_days, max_delivery_days, conditions, is_active, created_at, updated_at
             FROM zone_shipping_methods WHERE 1=1",
        );
        let mut param_idx: u32 = 1;

        if filter.zone_id.is_some() {
            sql.push_str(&format!(" AND zone_id = ${param_idx}"));
            param_idx += 1;
        }
        if filter.carrier.is_some() {
            sql.push_str(&format!(" AND carrier = ${param_idx}"));
            param_idx += 1;
        }
        if filter.method_type.is_some() {
            sql.push_str(&format!(" AND method_type = ${param_idx}"));
            param_idx += 1;
        }
        if filter.is_active.is_some() {
            sql.push_str(&format!(" AND is_active = ${param_idx}"));
            param_idx += 1;
        }

        sql.push_str(" ORDER BY created_at DESC");

        sql.push_str(&format!(" LIMIT ${param_idx}"));
        param_idx += 1;
        if filter.offset.is_some() {
            sql.push_str(&format!(" OFFSET ${param_idx}"));
            let _ = param_idx;
        }

        let mut query = sqlx::query_as::<_, ZoneShippingMethodRow>(&sql);

        if let Some(zone_id) = &filter.zone_id {
            query = query.bind(zone_id.into_uuid());
        }
        if let Some(ref carrier) = filter.carrier {
            query = query.bind(carrier.clone());
        }
        if let Some(method_type) = &filter.method_type {
            query = query.bind(method_type.to_string());
        }
        if let Some(is_active) = filter.is_active {
            query = query.bind(is_active);
        }
        query = query.bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            query = query.bind(offset as i64);
        }

        let rows = query.fetch_all(&self.pool).await.map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_method).collect()
    }

    /// Delete a zone shipping method (async)
    pub async fn delete_async(&self, id: ShippingMethodId) -> Result<()> {
        sqlx::query("DELETE FROM zone_shipping_methods WHERE id = $1")
            .bind(id.into_uuid())
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    /// Calculate shipping rates for a destination (async)
    pub async fn calculate_rates_async(
        &self,
        request: ZoneShippingRateRequest,
    ) -> Result<Vec<ZoneShippingRate>> {
        // Get all active zone shipping methods
        let rows = sqlx::query_as::<_, ZoneShippingMethodRow>(
            "SELECT id, zone_id, name, carrier, method_type, base_rate, currency,
             min_delivery_days, max_delivery_days, conditions, is_active, created_at, updated_at
             FROM zone_shipping_methods WHERE is_active = true",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        let methods: Vec<ZoneShippingMethod> =
            rows.into_iter().map(Self::row_to_method).collect::<Result<Vec<_>>>()?;

        let rates: Vec<ZoneShippingRate> = methods
            .iter()
            .map(|method| {
                let rate = method.calculate_rate(request.weight, request.order_total);
                ZoneShippingRate {
                    method_id: method.id,
                    method_name: method.name.clone(),
                    carrier: method.carrier.clone(),
                    rate,
                    currency: method.currency,
                    min_delivery_days: method.min_delivery_days,
                    max_delivery_days: method.max_delivery_days,
                }
            })
            .collect();

        Ok(rates)
    }
}

impl ZoneShippingMethodRepository for PgZoneShippingMethodRepository {
    fn create(&self, input: CreateZoneShippingMethod) -> Result<ZoneShippingMethod> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: ShippingMethodId) -> Result<Option<ZoneShippingMethod>> {
        super::block_on(self.get_async(id))
    }

    fn list(&self, filter: ZoneShippingMethodFilter) -> Result<Vec<ZoneShippingMethod>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: ShippingMethodId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }

    fn calculate_rates(&self, request: ZoneShippingRateRequest) -> Result<Vec<ZoneShippingRate>> {
        super::block_on(self.calculate_rates_async(request))
    }
}
