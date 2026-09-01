//! PostgreSQL implementation of cost accounting repository

use super::{block_on, map_db_error};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use rust_decimal::Decimal;
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    CommerceError, CostAccountingRepository, CostAdjustment, CostAdjustmentFilter,
    CostAdjustmentStatus, CostAdjustmentType, CostLayer, CostLayerFilter, CostLayerSource,
    CostMethod, CostRollup, CostTransaction, CostTransactionFilter, CostTransactionType,
    CostVariance, CostVarianceFilter, CreateCostAdjustment, CreateCostLayer, CurrencyCode,
    InventoryValuation, IssueCostLayers, ItemCost, ItemCostFilter, RecordCostVariance, Result,
    SetItemCost, SkuCostSummary, VarianceType, generate_cost_adjustment_number,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PgCostAccountingRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct ItemCostRow {
    id: Uuid,
    sku: String,
    cost_method: String,
    standard_cost: Decimal,
    average_cost: Decimal,
    last_cost: Decimal,
    material_cost: Decimal,
    labor_cost: Decimal,
    overhead_cost: Decimal,
    currency: CurrencyCode,
    effective_date: NaiveDate,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CostLayerRow {
    id: Uuid,
    sku: String,
    layer_date: NaiveDate,
    quantity: Decimal,
    remaining_quantity: Decimal,
    unit_cost: Decimal,
    total_cost: Decimal,
    source_type: String,
    source_id: Option<Uuid>,
    lot_id: Option<Uuid>,
    location_id: Option<i32>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CostTransactionRow {
    id: Uuid,
    sku: String,
    transaction_type: String,
    quantity: Decimal,
    unit_cost: Decimal,
    total_cost: Decimal,
    layer_id: Option<Uuid>,
    reference_type: Option<String>,
    reference_id: Option<Uuid>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CostVarianceRow {
    id: Uuid,
    sku: String,
    variance_type: String,
    variance_date: NaiveDate,
    standard_cost: Decimal,
    actual_cost: Decimal,
    variance_amount: Decimal,
    variance_percent: Decimal,
    quantity: Decimal,
    total_variance: Decimal,
    reference_type: Option<String>,
    reference_id: Option<Uuid>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CostAdjustmentRow {
    id: Uuid,
    adjustment_number: String,
    sku: String,
    adjustment_type: String,
    previous_cost: Decimal,
    new_cost: Decimal,
    adjustment_amount: Decimal,
    reason: String,
    approved_by: Option<String>,
    approved_at: Option<DateTime<Utc>>,
    status: String,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct CostRollupRow {
    id: Uuid,
    sku: String,
    bom_id: Option<Uuid>,
    rollup_date: NaiveDate,
    material_cost: Decimal,
    labor_cost: Decimal,
    overhead_cost: Decimal,
    total_cost: Decimal,
    previous_cost: Decimal,
    cost_change: Decimal,
    created_at: DateTime<Utc>,
}

impl PgCostAccountingRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_item_cost(row: ItemCostRow) -> Result<ItemCost> {
        let ItemCostRow {
            id,
            sku,
            cost_method,
            standard_cost,
            average_cost,
            last_cost,
            material_cost,
            labor_cost,
            overhead_cost,
            currency,
            effective_date,
            created_at,
            updated_at,
        } = row;

        let cost_method: CostMethod = cost_method.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid item_cost.cost_method '{}': {}",
                cost_method, e
            ))
        })?;

        Ok(ItemCost {
            id,
            sku,
            cost_method,
            standard_cost,
            average_cost,
            last_cost,
            material_cost,
            labor_cost,
            overhead_cost,
            currency,
            effective_date: from_date(effective_date),
            created_at,
            updated_at,
        })
    }

    fn row_to_cost_layer(row: CostLayerRow) -> Result<CostLayer> {
        let CostLayerRow {
            id,
            sku,
            layer_date,
            quantity,
            remaining_quantity,
            unit_cost,
            total_cost,
            source_type,
            source_id,
            lot_id,
            location_id,
            created_at,
        } = row;

        let source_type: CostLayerSource = source_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid cost_layer.source_type '{}': {}",
                source_type, e
            ))
        })?;

        Ok(CostLayer {
            id,
            sku,
            layer_date: from_date(layer_date),
            quantity,
            remaining_quantity,
            unit_cost,
            total_cost,
            source_type,
            source_id,
            lot_id,
            location_id,
            created_at,
        })
    }

    fn row_to_cost_transaction(row: CostTransactionRow) -> Result<CostTransaction> {
        let CostTransactionRow {
            id,
            sku,
            transaction_type,
            quantity,
            unit_cost,
            total_cost,
            layer_id,
            reference_type,
            reference_id,
            notes,
            created_at,
        } = row;

        let transaction_type: CostTransactionType = transaction_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid cost_transaction.transaction_type '{}': {}",
                transaction_type, e
            ))
        })?;

        Ok(CostTransaction {
            id,
            sku,
            transaction_type,
            quantity,
            unit_cost,
            total_cost,
            layer_id,
            reference_type,
            reference_id,
            notes,
            created_at,
        })
    }

    fn row_to_cost_variance(row: CostVarianceRow) -> Result<CostVariance> {
        let CostVarianceRow {
            id,
            sku,
            variance_type,
            variance_date,
            standard_cost,
            actual_cost,
            variance_amount,
            variance_percent,
            quantity,
            total_variance,
            reference_type,
            reference_id,
            notes,
            created_at,
        } = row;

        let variance_type: VarianceType = variance_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid cost_variance.variance_type '{}': {}",
                variance_type, e
            ))
        })?;

        Ok(CostVariance {
            id,
            sku,
            variance_type,
            variance_date: from_date(variance_date),
            standard_cost,
            actual_cost,
            variance_amount,
            variance_percent,
            quantity,
            total_variance,
            reference_type,
            reference_id,
            notes,
            created_at,
        })
    }

    fn row_to_cost_adjustment(row: CostAdjustmentRow) -> Result<CostAdjustment> {
        let CostAdjustmentRow {
            id,
            adjustment_number,
            sku,
            adjustment_type,
            previous_cost,
            new_cost,
            adjustment_amount,
            reason,
            approved_by,
            approved_at,
            status,
            created_by,
            created_at,
        } = row;

        let adjustment_type: CostAdjustmentType = adjustment_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid cost_adjustment.adjustment_type '{}': {}",
                adjustment_type, e
            ))
        })?;
        let status: CostAdjustmentStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid cost_adjustment.status '{}': {}",
                status, e
            ))
        })?;

        Ok(CostAdjustment {
            id,
            adjustment_number,
            sku,
            adjustment_type,
            previous_cost,
            new_cost,
            adjustment_amount,
            reason,
            approved_by,
            approved_at,
            status,
            created_by,
            created_at,
        })
    }

    fn row_to_cost_rollup(row: CostRollupRow) -> CostRollup {
        CostRollup {
            id: row.id,
            sku: row.sku,
            bom_id: row.bom_id,
            rollup_date: from_date(row.rollup_date),
            material_cost: row.material_cost,
            labor_cost: row.labor_cost,
            overhead_cost: row.overhead_cost,
            total_cost: row.total_cost,
            previous_cost: row.previous_cost,
            cost_change: row.cost_change,
            created_at: row.created_at,
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_cost_transaction_tx(
        tx: &mut sqlx::Transaction<'_, Postgres>,
        sku: &str,
        transaction_type: CostTransactionType,
        quantity: Decimal,
        unit_cost: Decimal,
        layer_id: Option<Uuid>,
        reference_type: Option<&str>,
        reference_id: Option<Uuid>,
        notes: Option<&str>,
    ) -> Result<CostTransaction> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let total_cost = quantity * unit_cost;

        sqlx::query(
            "INSERT INTO cost_transactions (id, sku, transaction_type, quantity, unit_cost,
             total_cost, layer_id, reference_type, reference_id, notes, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(id)
        .bind(sku)
        .bind(transaction_type.to_string())
        .bind(quantity)
        .bind(unit_cost)
        .bind(total_cost)
        .bind(layer_id)
        .bind(reference_type.map(|s| s.to_string()))
        .bind(reference_id)
        .bind(notes.map(|s| s.to_string()))
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        Ok(CostTransaction {
            id,
            sku: sku.to_string(),
            transaction_type,
            quantity,
            unit_cost,
            total_cost,
            layer_id,
            reference_type: reference_type.map(|s| s.to_string()),
            reference_id,
            notes: notes.map(|s| s.to_string()),
            created_at: now,
        })
    }

    pub async fn get_item_cost_async(&self, sku: &str) -> Result<Option<ItemCost>> {
        let row = sqlx::query_as::<_, ItemCostRow>(
            "SELECT id, sku, cost_method, standard_cost, average_cost, last_cost,
                    material_cost, labor_cost, overhead_cost, currency, effective_date,
                    created_at, updated_at
             FROM item_costs WHERE sku = $1",
        )
        .bind(sku)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_item_cost).transpose()
    }

    /// Insert-or-update an item cost on the caller's transaction, locking the
    /// row so the existence check and the write cannot interleave.
    ///
    /// Shared by `set_item_cost_async` and `apply_adjustment_async`, which
    /// needs the cost change to commit together with its status claim.
    async fn set_item_cost_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        input: SetItemCost,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let existing: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM item_costs WHERE sku = $1 FOR UPDATE")
                .bind(&input.sku)
                .fetch_optional(tx.as_mut())
                .await
                .map_err(map_db_error)?;

        if existing.is_some() {
            sqlx::query(
                "UPDATE item_costs SET
                    cost_method = COALESCE($1, cost_method),
                    standard_cost = COALESCE($2, standard_cost),
                    material_cost = COALESCE($3, material_cost),
                    labor_cost = COALESCE($4, labor_cost),
                    overhead_cost = COALESCE($5, overhead_cost),
                    currency = COALESCE($6, currency),
                    effective_date = $7,
                    updated_at = $8
                 WHERE sku = $9",
            )
            .bind(input.cost_method.map(|m| m.to_string()))
            .bind(input.standard_cost)
            .bind(input.material_cost)
            .bind(input.labor_cost)
            .bind(input.overhead_cost)
            .bind(input.currency)
            .bind(to_date(now))
            .bind(now)
            .bind(&input.sku)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        } else {
            let id = Uuid::new_v4();
            let cost_method = input.cost_method.unwrap_or_default();
            let standard_cost = input.standard_cost.unwrap_or_default();
            let material_cost = input.material_cost.unwrap_or_default();
            let labor_cost = input.labor_cost.unwrap_or_default();
            let overhead_cost = input.overhead_cost.unwrap_or_default();
            let currency = input.currency.unwrap_or(CurrencyCode::USD);

            sqlx::query(
                "INSERT INTO item_costs (id, sku, cost_method, standard_cost, average_cost, last_cost,
                    material_cost, labor_cost, overhead_cost, currency, effective_date, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
            )
            .bind(id)
            .bind(&input.sku)
            .bind(cost_method.to_string())
            .bind(standard_cost)
            .bind(standard_cost) // average_cost starts as standard (matches SQLite)
            .bind(standard_cost) // last_cost starts as standard (matches SQLite)
            .bind(material_cost)
            .bind(labor_cost)
            .bind(overhead_cost)
            .bind(currency)
            .bind(to_date(now))
            .bind(now)
            .bind(now)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }
        Ok(())
    }

    pub async fn set_item_cost_async(&self, input: SetItemCost) -> Result<ItemCost> {
        let now = Utc::now();
        let sku = input.sku.clone();
        // The existence check and the write share one transaction (the row is
        // locked FOR UPDATE): on separate pool acquisitions two concurrent
        // callers both observed "absent" and raced on the UNIQUE sku INSERT.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        Self::set_item_cost_tx(&mut tx, input, now).await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_item_cost_async(&sku).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn list_item_costs_async(&self, filter: ItemCostFilter) -> Result<Vec<ItemCost>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, sku, cost_method, standard_cost, average_cost, last_cost,
                    material_cost, labor_cost, overhead_cost, currency, effective_date,
                    created_at, updated_at
             FROM item_costs WHERE 1=1",
        );

        if let Some(sku) = filter.sku {
            builder.push(" AND sku ILIKE ").push_bind(format!("%{}%", sku));
        }
        if let Some(method) = filter.cost_method {
            builder.push(" AND cost_method = ").push_bind(method.to_string());
        }

        builder.push(" ORDER BY sku");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<ItemCostRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_item_cost).collect::<Result<Vec<_>>>()
    }

    pub async fn update_average_cost_async(
        &self,
        sku: &str,
        quantity: Decimal,
        unit_cost: Decimal,
    ) -> Result<ItemCost> {
        let now = Utc::now();

        if self.get_item_cost_async(sku).await?.is_none() {
            self.set_item_cost_async(SetItemCost {
                sku: sku.to_string(),
                standard_cost: Some(unit_cost),
                ..Default::default()
            })
            .await?;
        }

        // Lock the item_costs row and compute the new weighted average inside one
        // transaction, so two concurrent receipts for the same SKU serialize
        // instead of both reading the same `average_cost` and one clobbering the
        // other — a lost update that corrupts the weighted-average cost.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        let (current_avg,): (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(average_cost, 0) FROM item_costs WHERE sku = $1 FOR UPDATE",
        )
        .bind(sku)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // On-hand quantity lives in `inventory_balances` (per location), keyed by
        // `item_id`; `inventory_items` has no `quantity_on_hand` column, so the
        // previous query errored on every call. Sum the balances for the SKU.
        let (current_qty,): (Decimal,) = sqlx::query_as(
            "SELECT COALESCE(SUM(b.quantity_on_hand), 0) FROM inventory_balances b
             JOIN inventory_items i ON b.item_id = i.id WHERE i.sku = $1",
        )
        .bind(sku)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let total_qty = current_qty + quantity;
        let new_avg = if total_qty > Decimal::ZERO {
            ((current_avg * current_qty) + (unit_cost * quantity)) / total_qty
        } else {
            unit_cost
        };

        sqlx::query(
            "UPDATE item_costs SET average_cost = $1, last_cost = $2, updated_at = $3 WHERE sku = $4",
        )
        .bind(new_avg)
        .bind(unit_cost)
        .bind(now)
        .bind(sku)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.get_item_cost_async(sku).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn update_last_cost_async(&self, sku: &str, unit_cost: Decimal) -> Result<ItemCost> {
        let now = Utc::now();
        sqlx::query("UPDATE item_costs SET last_cost = $1, updated_at = $2 WHERE sku = $3")
            .bind(unit_cost)
            .bind(now)
            .bind(sku)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

        self.get_item_cost_async(sku).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn create_cost_layer_async(&self, input: CreateCostLayer) -> Result<CostLayer> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let total_cost = input.quantity * input.unit_cost;

        sqlx::query(
            "INSERT INTO cost_layers (id, sku, layer_date, quantity, remaining_quantity,
                unit_cost, total_cost, source_type, source_id, lot_id, location_id, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
        )
        .bind(id)
        .bind(&input.sku)
        .bind(to_date(now))
        .bind(input.quantity)
        .bind(input.quantity)
        .bind(input.unit_cost)
        .bind(total_cost)
        .bind(input.source_type.to_string())
        .bind(input.source_id)
        .bind(input.lot_id)
        .bind(input.location_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_cost_layer_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_cost_layer_async(&self, id: Uuid) -> Result<Option<CostLayer>> {
        let row = sqlx::query_as::<_, CostLayerRow>(
            "SELECT id, sku, layer_date, quantity, remaining_quantity, unit_cost, total_cost,
                    source_type, source_id, lot_id, location_id, created_at
             FROM cost_layers WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_cost_layer).transpose()
    }

    pub async fn list_cost_layers_async(&self, filter: CostLayerFilter) -> Result<Vec<CostLayer>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, sku, layer_date, quantity, remaining_quantity, unit_cost, total_cost,
                    source_type, source_id, lot_id, location_id, created_at
             FROM cost_layers WHERE 1=1",
        );

        if let Some(sku) = filter.sku {
            builder.push(" AND sku = ").push_bind(sku);
        }
        if let Some(source_type) = filter.source_type {
            builder.push(" AND source_type = ").push_bind(source_type.to_string());
        }
        if let Some(has_remaining) = filter.has_remaining {
            if has_remaining {
                builder.push(" AND remaining_quantity > 0");
            } else {
                builder.push(" AND remaining_quantity <= 0");
            }
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND layer_date >= ").push_bind(to_date(from_date));
        }
        if let Some(to_date_val) = filter.to_date {
            builder.push(" AND layer_date <= ").push_bind(to_date(to_date_val));
        }

        builder.push(" ORDER BY layer_date ASC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<CostLayerRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_cost_layer).collect::<Result<Vec<_>>>()
    }

    pub async fn issue_fifo_async(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut remaining = input.quantity;
        let mut transactions = Vec::new();

        let layers = sqlx::query_as::<_, CostLayerRow>(
            "SELECT id, sku, layer_date, quantity, remaining_quantity, unit_cost, total_cost,
                    source_type, source_id, lot_id, location_id, created_at
             FROM cost_layers WHERE sku = $1 AND remaining_quantity > 0
             ORDER BY layer_date ASC
             FOR UPDATE",
        )
        .bind(&input.sku)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for layer in layers {
            if remaining <= Decimal::ZERO {
                break;
            }

            let consume_qty = remaining.min(layer.remaining_quantity);
            let new_remaining = layer.remaining_quantity - consume_qty;

            sqlx::query("UPDATE cost_layers SET remaining_quantity = $1 WHERE id = $2")
                .bind(new_remaining)
                .bind(layer.id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;

            let tx_record = Self::record_cost_transaction_tx(
                &mut tx,
                &input.sku,
                CostTransactionType::Issue,
                consume_qty,
                layer.unit_cost,
                Some(layer.id),
                input.reference_type.as_deref(),
                input.reference_id,
                input.notes.as_deref(),
            )
            .await?;
            transactions.push(tx_record);

            remaining -= consume_qty;
        }

        if remaining > Decimal::ZERO {
            return Err(CommerceError::ValidationError(format!(
                "Insufficient remaining cost layers for sku {} (short by {})",
                input.sku, remaining
            )));
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(transactions)
    }

    pub async fn issue_lifo_async(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let mut remaining = input.quantity;
        let mut transactions = Vec::new();

        let layers = sqlx::query_as::<_, CostLayerRow>(
            "SELECT id, sku, layer_date, quantity, remaining_quantity, unit_cost, total_cost,
                    source_type, source_id, lot_id, location_id, created_at
             FROM cost_layers WHERE sku = $1 AND remaining_quantity > 0
             ORDER BY layer_date DESC
             FOR UPDATE",
        )
        .bind(&input.sku)
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        for layer in layers {
            if remaining <= Decimal::ZERO {
                break;
            }

            let consume_qty = remaining.min(layer.remaining_quantity);
            let new_remaining = layer.remaining_quantity - consume_qty;

            sqlx::query("UPDATE cost_layers SET remaining_quantity = $1 WHERE id = $2")
                .bind(new_remaining)
                .bind(layer.id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;

            let tx_record = Self::record_cost_transaction_tx(
                &mut tx,
                &input.sku,
                CostTransactionType::Issue,
                consume_qty,
                layer.unit_cost,
                Some(layer.id),
                input.reference_type.as_deref(),
                input.reference_id,
                input.notes.as_deref(),
            )
            .await?;
            transactions.push(tx_record);

            remaining -= consume_qty;
        }

        if remaining > Decimal::ZERO {
            return Err(CommerceError::ValidationError(format!(
                "Insufficient remaining cost layers for sku {} (short by {})",
                input.sku, remaining
            )));
        }

        tx.commit().await.map_err(map_db_error)?;
        Ok(transactions)
    }

    pub async fn get_layers_remaining_async(&self, sku: &str) -> Result<Decimal> {
        let total: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(remaining_quantity), 0) FROM cost_layers WHERE sku = $1",
        )
        .bind(sku)
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(total)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_cost_transaction_async(
        &self,
        sku: &str,
        transaction_type: CostTransactionType,
        quantity: Decimal,
        unit_cost: Decimal,
        layer_id: Option<Uuid>,
        reference_type: Option<&str>,
        reference_id: Option<Uuid>,
        notes: Option<&str>,
    ) -> Result<CostTransaction> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let total_cost = quantity * unit_cost;

        sqlx::query(
            "INSERT INTO cost_transactions (id, sku, transaction_type, quantity, unit_cost,
                total_cost, layer_id, reference_type, reference_id, notes, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(id)
        .bind(sku)
        .bind(transaction_type.to_string())
        .bind(quantity)
        .bind(unit_cost)
        .bind(total_cost)
        .bind(layer_id)
        .bind(reference_type.map(|s| s.to_string()))
        .bind(reference_id)
        .bind(notes.map(|s| s.to_string()))
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(CostTransaction {
            id,
            sku: sku.to_string(),
            transaction_type,
            quantity,
            unit_cost,
            total_cost,
            layer_id,
            reference_type: reference_type.map(|s| s.to_string()),
            reference_id,
            notes: notes.map(|s| s.to_string()),
            created_at: now,
        })
    }

    pub async fn list_cost_transactions_async(
        &self,
        filter: CostTransactionFilter,
    ) -> Result<Vec<CostTransaction>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, sku, transaction_type, quantity, unit_cost, total_cost,
                    layer_id, reference_type, reference_id, notes, created_at
             FROM cost_transactions WHERE 1=1",
        );

        if let Some(sku) = filter.sku {
            builder.push(" AND sku = ").push_bind(sku);
        }
        if let Some(tx_type) = filter.transaction_type {
            builder.push(" AND transaction_type = ").push_bind(tx_type.to_string());
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND created_at >= ").push_bind(from_date);
        }
        if let Some(to_date_val) = filter.to_date {
            builder.push(" AND created_at <= ").push_bind(to_date_val);
        }

        builder.push(" ORDER BY created_at DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<CostTransactionRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_cost_transaction).collect::<Result<Vec<_>>>()
    }

    pub async fn record_variance_async(&self, input: RecordCostVariance) -> Result<CostVariance> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let variance_amount = input.actual_cost - input.standard_cost;
        let variance_percent = if input.standard_cost != Decimal::ZERO {
            (variance_amount / input.standard_cost) * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        let total_variance = variance_amount * input.quantity;

        sqlx::query(
            "INSERT INTO cost_variances (id, sku, variance_type, variance_date, standard_cost,
                actual_cost, variance_amount, variance_percent, quantity, total_variance,
                reference_type, reference_id, notes, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
        )
        .bind(id)
        .bind(&input.sku)
        .bind(input.variance_type.to_string())
        .bind(to_date(now))
        .bind(input.standard_cost)
        .bind(input.actual_cost)
        .bind(variance_amount)
        .bind(variance_percent)
        .bind(input.quantity)
        .bind(total_variance)
        .bind(input.reference_type.clone())
        .bind(input.reference_id)
        .bind(input.notes.clone())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(CostVariance {
            id,
            sku: input.sku,
            variance_type: input.variance_type,
            variance_date: now,
            standard_cost: input.standard_cost,
            actual_cost: input.actual_cost,
            variance_amount,
            variance_percent,
            quantity: input.quantity,
            total_variance,
            reference_type: input.reference_type,
            reference_id: input.reference_id,
            notes: input.notes,
            created_at: now,
        })
    }

    pub async fn list_variances_async(
        &self,
        filter: CostVarianceFilter,
    ) -> Result<Vec<CostVariance>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, sku, variance_type, variance_date, standard_cost, actual_cost,
                    variance_amount, variance_percent, quantity, total_variance,
                    reference_type, reference_id, notes, created_at
             FROM cost_variances WHERE 1=1",
        );

        if let Some(sku) = filter.sku {
            builder.push(" AND sku = ").push_bind(sku);
        }
        if let Some(var_type) = filter.variance_type {
            builder.push(" AND variance_type = ").push_bind(var_type.to_string());
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND variance_date >= ").push_bind(to_date(from_date));
        }
        if let Some(to_date_val) = filter.to_date {
            builder.push(" AND variance_date <= ").push_bind(to_date(to_date_val));
        }

        builder.push(" ORDER BY variance_date DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<CostVarianceRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_cost_variance).collect::<Result<Vec<_>>>()
    }

    pub async fn get_variance_summary_async(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Decimal> {
        let total: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(total_variance), 0) FROM cost_variances
             WHERE variance_date BETWEEN $1 AND $2",
        )
        .bind(to_date(from))
        .bind(to_date(to))
        .fetch_one(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(total)
    }

    pub async fn create_adjustment_async(
        &self,
        input: CreateCostAdjustment,
    ) -> Result<CostAdjustment> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let adjustment_number = generate_cost_adjustment_number();

        let current_cost: Decimal =
            sqlx::query_scalar("SELECT standard_cost FROM item_costs WHERE sku = $1")
                .bind(&input.sku)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?
                .unwrap_or_default();

        let adjustment_amount = input.new_cost - current_cost;

        sqlx::query(
            "INSERT INTO cost_adjustments (id, adjustment_number, sku, adjustment_type,
                previous_cost, new_cost, adjustment_amount, reason, status, created_by, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(id)
        .bind(&adjustment_number)
        .bind(&input.sku)
        .bind(input.adjustment_type.to_string())
        .bind(current_cost)
        .bind(input.new_cost)
        .bind(adjustment_amount)
        .bind(&input.reason)
        .bind(CostAdjustmentStatus::Pending.to_string())
        .bind(input.created_by)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_adjustment_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn get_adjustment_async(&self, id: Uuid) -> Result<Option<CostAdjustment>> {
        let row = sqlx::query_as::<_, CostAdjustmentRow>(
            "SELECT id, adjustment_number, sku, adjustment_type, previous_cost, new_cost,
                    adjustment_amount, reason, approved_by, approved_at, status, created_by, created_at
             FROM cost_adjustments WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_cost_adjustment).transpose()
    }

    pub async fn list_adjustments_async(
        &self,
        filter: CostAdjustmentFilter,
    ) -> Result<Vec<CostAdjustment>> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, adjustment_number, sku, adjustment_type, previous_cost, new_cost,
                    adjustment_amount, reason, approved_by, approved_at, status, created_by, created_at
             FROM cost_adjustments WHERE 1=1",
        );

        if let Some(sku) = filter.sku {
            builder.push(" AND sku = ").push_bind(sku);
        }
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        if let Some(adjustment_type) = filter.adjustment_type {
            builder.push(" AND adjustment_type = ").push_bind(adjustment_type.to_string());
        }
        if let Some(from_date) = filter.from_date {
            builder.push(" AND created_at >= ").push_bind(from_date);
        }
        if let Some(to_date_val) = filter.to_date {
            builder.push(" AND created_at <= ").push_bind(to_date_val);
        }

        builder.push(" ORDER BY created_at DESC");

        builder.push(" LIMIT ").push_bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(offset as i64);
        }

        let rows = builder
            .build_query_as::<CostAdjustmentRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_cost_adjustment).collect::<Result<Vec<_>>>()
    }

    pub async fn approve_adjustment_async(
        &self,
        id: Uuid,
        approved_by: &str,
    ) -> Result<CostAdjustment> {
        let now = Utc::now();

        // Only a pending adjustment may be approved. Unguarded, this
        // re-approved an already-applied adjustment, which `apply_adjustment`
        // would then apply to the item cost a second time.
        let rows = sqlx::query(
            "UPDATE cost_adjustments SET status = $1, approved_by = $2, approved_at = $3
             WHERE id = $4 AND status = 'pending'",
        )
        .bind(CostAdjustmentStatus::Approved.to_string())
        .bind(approved_by)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if rows == 0 {
            return Err(self.adjustment_conflict_async(id, "approved").await);
        }

        self.get_adjustment_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn apply_adjustment_async(&self, id: Uuid) -> Result<CostAdjustment> {
        let now = Utc::now();

        // Claim the adjustment and apply the cost change in ONE transaction.
        // Previously the status check, the item-cost write and the status
        // write each ran on their own pool acquisition: two concurrent callers
        // both passed the check and both moved the cost, and a crash between
        // the cost write and the status write left an "approved" adjustment
        // whose cost had already been applied — re-applying doubled it.
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let claimed = sqlx::query(
            "UPDATE cost_adjustments SET status = $1 WHERE id = $2 AND status = 'approved'",
        )
        .bind(CostAdjustmentStatus::Applied.to_string())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if claimed == 0 {
            drop(tx);
            return Err(self.adjustment_conflict_async(id, "applied").await);
        }

        let (sku, new_cost): (String, Decimal) =
            sqlx::query_as("SELECT sku, new_cost FROM cost_adjustments WHERE id = $1")
                .bind(id)
                .fetch_one(tx.as_mut())
                .await
                .map_err(map_db_error)?;

        Self::set_item_cost_tx(
            &mut tx,
            SetItemCost { sku, standard_cost: Some(new_cost), ..Default::default() },
            now,
        )
        .await?;
        tx.commit().await.map_err(map_db_error)?;

        self.get_adjustment_async(id).await?.ok_or(CommerceError::NotFound)
    }

    pub async fn reject_adjustment_async(&self, id: Uuid) -> Result<CostAdjustment> {
        // An applied adjustment is live in the item cost: recording it as
        // "rejected" would make the audit trail contradict the cost record.
        let rows = sqlx::query(
            "UPDATE cost_adjustments SET status = $1
             WHERE id = $2 AND status IN ('pending', 'approved')",
        )
        .bind(CostAdjustmentStatus::Rejected.to_string())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?
        .rows_affected();
        if rows == 0 {
            return Err(self.adjustment_conflict_async(id, "rejected").await);
        }

        self.get_adjustment_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Explain why a cost-adjustment transition was refused: report the status
    /// the row is actually in, or `NotFound` when it does not exist.
    async fn adjustment_conflict_async(&self, id: Uuid, attempted: &str) -> CommerceError {
        match sqlx::query_scalar::<_, String>("SELECT status FROM cost_adjustments WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
        {
            Ok(Some(status)) => CommerceError::Conflict(format!(
                "Cost adjustment {id} cannot be {attempted}: it is already {status}"
            )),
            Ok(None) => CommerceError::NotFound,
            Err(e) => map_db_error(e),
        }
    }

    pub async fn calculate_rollup_async(
        &self,
        sku: &str,
        bom_id: Option<Uuid>,
    ) -> Result<CostRollup> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let previous_cost =
            self.get_rollup_async(sku).await?.map(|r| r.total_cost).unwrap_or_default();

        let (material_cost, labor_cost, overhead_cost) = if let Some(bom_id) = bom_id {
            let material: Decimal = sqlx::query_scalar(
                "SELECT COALESCE(SUM(bc.quantity * COALESCE(ic.standard_cost, 0)), 0)
                 FROM manufacturing_bom_components bc
                 LEFT JOIN item_costs ic ON bc.component_sku = ic.sku
                 WHERE bc.bom_id = $1",
            )
            .bind(bom_id)
            .fetch_one(&self.pool)
            .await
            .map_err(map_db_error)?;
            (material, Decimal::ZERO, Decimal::ZERO)
        } else {
            match self.get_item_cost_async(sku).await? {
                Some(cost) => (cost.material_cost, cost.labor_cost, cost.overhead_cost),
                None => (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO),
            }
        };

        let total_cost = material_cost + labor_cost + overhead_cost;
        let cost_change = total_cost - previous_cost;

        sqlx::query(
            "INSERT INTO cost_rollups (id, sku, bom_id, rollup_date, material_cost, labor_cost,
                overhead_cost, total_cost, previous_cost, cost_change, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(id)
        .bind(sku)
        .bind(bom_id)
        .bind(to_date(now))
        .bind(material_cost)
        .bind(labor_cost)
        .bind(overhead_cost)
        .bind(total_cost)
        .bind(previous_cost)
        .bind(cost_change)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(CostRollup {
            id,
            sku: sku.to_string(),
            bom_id,
            rollup_date: now,
            material_cost,
            labor_cost,
            overhead_cost,
            total_cost,
            previous_cost,
            cost_change,
            created_at: now,
        })
    }

    pub async fn get_rollup_async(&self, sku: &str) -> Result<Option<CostRollup>> {
        let row = sqlx::query_as::<_, CostRollupRow>(
            "SELECT id, sku, bom_id, rollup_date, material_cost, labor_cost, overhead_cost,
                    total_cost, previous_cost, cost_change, created_at
             FROM cost_rollups WHERE sku = $1 ORDER BY rollup_date DESC LIMIT 1",
        )
        .bind(sku)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(Self::row_to_cost_rollup))
    }

    pub async fn get_inventory_valuation_async(
        &self,
        cost_method: CostMethod,
    ) -> Result<InventoryValuation> {
        let now = Utc::now();
        let cost_field = match cost_method {
            CostMethod::Standard => "COALESCE(ic.standard_cost, 0)",
            CostMethod::Average => "COALESCE(ic.average_cost, 0)",
            CostMethod::Fifo | CostMethod::Lifo => "COALESCE(ic.average_cost, 0)",
            CostMethod::Specific => "COALESCE(ic.last_cost, 0)",
            _ => "COALESCE(ic.average_cost, 0)",
        };

        let sql = format!(
            "SELECT
                COALESCE(SUM(ii.quantity_on_hand), 0),
                COALESCE(SUM(ii.quantity_on_hand * {}), 0)
             FROM inventory_items ii
             LEFT JOIN item_costs ic ON ii.sku = ic.sku",
            cost_field
        );

        let (total_qty, total_val): (Decimal, Decimal) =
            sqlx::query_as(&sql).fetch_one(&self.pool).await.map_err(map_db_error)?;

        let average_unit_cost =
            if total_qty > Decimal::ZERO { total_val / total_qty } else { Decimal::ZERO };

        Ok(InventoryValuation {
            total_quantity: total_qty,
            total_value: total_val,
            average_unit_cost,
            valuation_method: cost_method,
            as_of_date: now,
        })
    }

    pub async fn get_sku_cost_summary_async(&self, sku: &str) -> Result<Option<SkuCostSummary>> {
        let row = sqlx::query_as(
            "SELECT
                ii.sku,
                COALESCE(ii.quantity_on_hand, 0),
                COALESCE(ic.standard_cost, 0),
                COALESCE(ic.average_cost, 0),
                COALESCE(ii.quantity_on_hand * COALESCE(ic.average_cost, 0), 0),
                COALESCE((SELECT SUM(total_variance) FROM cost_variances
                          WHERE sku = ii.sku AND EXTRACT(YEAR FROM variance_date) = EXTRACT(YEAR FROM CURRENT_DATE)), 0)
             FROM inventory_items ii
             LEFT JOIN item_costs ic ON ii.sku = ic.sku
             WHERE ii.sku = $1",
        )
        .bind(sku)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        Ok(row.map(|r: (String, Decimal, Decimal, Decimal, Decimal, Decimal)| SkuCostSummary {
            sku: r.0,
            quantity_on_hand: r.1,
            standard_cost: r.2,
            average_cost: r.3,
            total_value: r.4,
            variance_ytd: r.5,
        }))
    }

    pub async fn get_total_inventory_value_async(&self) -> Result<Decimal> {
        let valuation = self.get_inventory_valuation_async(CostMethod::Average).await?;
        Ok(valuation.total_value)
    }
}

impl CostAccountingRepository for PgCostAccountingRepository {
    fn get_item_cost(&self, sku: &str) -> Result<Option<ItemCost>> {
        block_on(self.get_item_cost_async(sku))
    }

    fn set_item_cost(&self, input: SetItemCost) -> Result<ItemCost> {
        block_on(self.set_item_cost_async(input))
    }

    fn list_item_costs(&self, filter: ItemCostFilter) -> Result<Vec<ItemCost>> {
        block_on(self.list_item_costs_async(filter))
    }

    fn update_average_cost(
        &self,
        sku: &str,
        quantity: Decimal,
        unit_cost: Decimal,
    ) -> Result<ItemCost> {
        block_on(self.update_average_cost_async(sku, quantity, unit_cost))
    }

    fn update_last_cost(&self, sku: &str, unit_cost: Decimal) -> Result<ItemCost> {
        block_on(self.update_last_cost_async(sku, unit_cost))
    }

    fn create_cost_layer(&self, input: CreateCostLayer) -> Result<CostLayer> {
        block_on(self.create_cost_layer_async(input))
    }

    fn get_cost_layer(&self, id: Uuid) -> Result<Option<CostLayer>> {
        block_on(self.get_cost_layer_async(id))
    }

    fn list_cost_layers(&self, filter: CostLayerFilter) -> Result<Vec<CostLayer>> {
        block_on(self.list_cost_layers_async(filter))
    }

    fn issue_fifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        block_on(self.issue_fifo_async(input))
    }

    fn issue_lifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        block_on(self.issue_lifo_async(input))
    }

    fn get_layers_remaining(&self, sku: &str) -> Result<Decimal> {
        block_on(self.get_layers_remaining_async(sku))
    }

    fn record_cost_transaction(
        &self,
        sku: &str,
        transaction_type: CostTransactionType,
        quantity: Decimal,
        unit_cost: Decimal,
        layer_id: Option<Uuid>,
        reference_type: Option<&str>,
        reference_id: Option<Uuid>,
        notes: Option<&str>,
    ) -> Result<CostTransaction> {
        block_on(self.record_cost_transaction_async(
            sku,
            transaction_type,
            quantity,
            unit_cost,
            layer_id,
            reference_type,
            reference_id,
            notes,
        ))
    }

    fn list_cost_transactions(
        &self,
        filter: CostTransactionFilter,
    ) -> Result<Vec<CostTransaction>> {
        block_on(self.list_cost_transactions_async(filter))
    }

    fn record_variance(&self, input: RecordCostVariance) -> Result<CostVariance> {
        block_on(self.record_variance_async(input))
    }

    fn list_variances(&self, filter: CostVarianceFilter) -> Result<Vec<CostVariance>> {
        block_on(self.list_variances_async(filter))
    }

    fn get_variance_summary(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Decimal> {
        block_on(self.get_variance_summary_async(from, to))
    }

    fn create_adjustment(&self, input: CreateCostAdjustment) -> Result<CostAdjustment> {
        block_on(self.create_adjustment_async(input))
    }

    fn get_adjustment(&self, id: Uuid) -> Result<Option<CostAdjustment>> {
        block_on(self.get_adjustment_async(id))
    }

    fn list_adjustments(&self, filter: CostAdjustmentFilter) -> Result<Vec<CostAdjustment>> {
        block_on(self.list_adjustments_async(filter))
    }

    fn approve_adjustment(&self, id: Uuid, approved_by: &str) -> Result<CostAdjustment> {
        block_on(self.approve_adjustment_async(id, approved_by))
    }

    fn apply_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        block_on(self.apply_adjustment_async(id))
    }

    fn reject_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        block_on(self.reject_adjustment_async(id))
    }

    fn calculate_rollup(&self, sku: &str, bom_id: Option<Uuid>) -> Result<CostRollup> {
        block_on(self.calculate_rollup_async(sku, bom_id))
    }

    fn get_rollup(&self, sku: &str) -> Result<Option<CostRollup>> {
        block_on(self.get_rollup_async(sku))
    }

    fn get_inventory_valuation(&self, cost_method: CostMethod) -> Result<InventoryValuation> {
        block_on(self.get_inventory_valuation_async(cost_method))
    }

    fn get_sku_cost_summary(&self, sku: &str) -> Result<Option<SkuCostSummary>> {
        block_on(self.get_sku_cost_summary_async(sku))
    }

    fn get_total_inventory_value(&self) -> Result<Decimal> {
        block_on(self.get_total_inventory_value_async())
    }
}

fn to_date(dt: DateTime<Utc>) -> NaiveDate {
    dt.date_naive()
}

const fn from_date(date: NaiveDate) -> DateTime<Utc> {
    DateTime::from_naive_utc_and_offset(date.and_time(NaiveTime::MIN), Utc)
}
