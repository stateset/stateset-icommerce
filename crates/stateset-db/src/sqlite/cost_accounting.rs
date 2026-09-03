//! SQLite implementation of cost accounting repository

use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CostAccountingRepository, CostAdjustment, CostAdjustmentFilter,
    CostAdjustmentStatus, CostLayer, CostLayerFilter, CostMethod, CostRollup, CostTransaction,
    CostTransactionFilter, CostTransactionType, CostVariance, CostVarianceFilter,
    CreateCostAdjustment, CreateCostLayer, InventoryValuation, IssueCostLayers, ItemCost,
    ItemCostFilter, RecordCostVariance, Result, SetItemCost, SkuCostSummary,
    generate_cost_adjustment_number,
};
use uuid::Uuid;

use super::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_decimal_row,
    parse_decimal_strict, parse_enum_row, parse_uuid_opt_row, parse_uuid_row, sum_decimal_query,
    with_immediate_transaction,
};

/// Explain why a cost-adjustment transition was refused: report the status the
/// row is actually in, or `NotFound` when it does not exist. Used by the
/// guarded approve/apply/reject transitions.
fn adjustment_conflict(conn: &rusqlite::Connection, id: Uuid, attempted: &str) -> CommerceError {
    match conn.query_row(
        "SELECT status FROM cost_adjustments WHERE id = ?",
        [id.to_string()],
        |row| row.get::<_, String>(0),
    ) {
        Ok(status) => CommerceError::Conflict(format!(
            "Cost adjustment {id} cannot be {attempted}: it is already {status}"
        )),
        Err(rusqlite::Error::QueryReturnedNoRows) => CommerceError::NotFound,
        Err(e) => CommerceError::DatabaseError(e.to_string()),
    }
}

#[derive(Debug)]
pub struct SqliteCostAccountingRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCostAccountingRepository {
    /// Insert-or-update an item cost on the caller's connection/transaction.
    ///
    /// Shared by `set_item_cost` (which wraps it in an IMMEDIATE transaction)
    /// and `apply_adjustment` (which needs the cost change to commit together
    /// with the adjustment's status claim).
    fn set_item_cost_with_conn(
        conn: &rusqlite::Connection,
        input: SetItemCost,
        now: DateTime<Utc>,
    ) -> rusqlite::Result<()> {
        let SetItemCost {
            sku,
            cost_method,
            standard_cost,
            material_cost,
            labor_cost,
            overhead_cost,
            currency,
            ..
        } = input;

        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM item_costs WHERE sku = ?)",
            [&sku],
            |row| row.get(0),
        )?;

        if exists {
            // Update existing
            conn.execute(
                "UPDATE item_costs SET
                    cost_method = COALESCE(?, cost_method),
                    standard_cost = COALESCE(?, standard_cost),
                    material_cost = COALESCE(?, material_cost),
                    labor_cost = COALESCE(?, labor_cost),
                    overhead_cost = COALESCE(?, overhead_cost),
                    currency = COALESCE(?, currency),
                    effective_date = ?,
                    updated_at = ?
                 WHERE sku = ?",
                rusqlite::params![
                    cost_method.as_ref().map(std::string::ToString::to_string),
                    standard_cost.as_ref().map(std::string::ToString::to_string),
                    material_cost.as_ref().map(std::string::ToString::to_string),
                    labor_cost.as_ref().map(std::string::ToString::to_string),
                    overhead_cost.as_ref().map(std::string::ToString::to_string),
                    currency,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    &sku,
                ],
            )?;
        } else {
            // Insert new
            let id = Uuid::new_v4();
            let cost_method = cost_method.unwrap_or_default();
            let standard_cost = standard_cost.unwrap_or_default();
            let material_cost = material_cost.unwrap_or_default();
            let labor_cost = labor_cost.unwrap_or_default();
            let overhead_cost = overhead_cost.unwrap_or_default();
            let currency = currency.unwrap_or_default();

            conn.execute(
                "INSERT INTO item_costs (id, sku, cost_method, standard_cost, average_cost, last_cost,
                    material_cost, labor_cost, overhead_cost, currency, effective_date, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    &sku,
                    cost_method.to_string(),
                    standard_cost.to_string(),
                    standard_cost.to_string(), // average_cost starts as standard
                    standard_cost.to_string(), // last_cost starts as standard
                    material_cost.to_string(),
                    labor_cost.to_string(),
                    overhead_cost.to_string(),
                    &currency,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                ],
            )?;
        }
        Ok(())
    }

    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn row_to_item_cost(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<ItemCost> {
        Ok(ItemCost {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "item_cost", "id")?,
            sku: row.get(1)?,
            cost_method: parse_enum_row(&row.get::<_, String>(2)?, "item_cost", "cost_method")?,
            standard_cost: parse_decimal_row(
                &row.get::<_, String>(3)?,
                "item_cost",
                "standard_cost",
            )?,
            average_cost: parse_decimal_row(
                &row.get::<_, String>(4)?,
                "item_cost",
                "average_cost",
            )?,
            last_cost: parse_decimal_row(&row.get::<_, String>(5)?, "item_cost", "last_cost")?,
            material_cost: parse_decimal_row(
                &row.get::<_, String>(6)?,
                "item_cost",
                "material_cost",
            )?,
            labor_cost: parse_decimal_row(&row.get::<_, String>(7)?, "item_cost", "labor_cost")?,
            overhead_cost: parse_decimal_row(
                &row.get::<_, String>(8)?,
                "item_cost",
                "overhead_cost",
            )?,
            currency: row.get(9)?,
            effective_date: parse_datetime_row(
                &row.get::<_, String>(10)?,
                "item_cost",
                "effective_date",
            )?,
            created_at: parse_datetime_row(&row.get::<_, String>(11)?, "item_cost", "created_at")?,
            updated_at: parse_datetime_row(&row.get::<_, String>(12)?, "item_cost", "updated_at")?,
        })
    }

    fn row_to_cost_layer(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<CostLayer> {
        Ok(CostLayer {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "cost_layer", "id")?,
            sku: row.get(1)?,
            layer_date: parse_datetime_row(&row.get::<_, String>(2)?, "cost_layer", "layer_date")?,
            quantity: parse_decimal_row(&row.get::<_, String>(3)?, "cost_layer", "quantity")?,
            remaining_quantity: parse_decimal_row(
                &row.get::<_, String>(4)?,
                "cost_layer",
                "remaining_quantity",
            )?,
            unit_cost: parse_decimal_row(&row.get::<_, String>(5)?, "cost_layer", "unit_cost")?,
            total_cost: parse_decimal_row(&row.get::<_, String>(6)?, "cost_layer", "total_cost")?,
            source_type: parse_enum_row(&row.get::<_, String>(7)?, "cost_layer", "source_type")?,
            source_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(8)?,
                "cost_layer",
                "source_id",
            )?,
            lot_id: parse_uuid_opt_row(row.get::<_, Option<String>>(9)?, "cost_layer", "lot_id")?,
            location_id: row.get(10)?,
            created_at: parse_datetime_row(&row.get::<_, String>(11)?, "cost_layer", "created_at")?,
        })
    }

    fn row_to_cost_transaction(
        &self,
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<CostTransaction> {
        Ok(CostTransaction {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "cost_transaction", "id")?,
            sku: row.get(1)?,
            transaction_type: parse_enum_row(
                &row.get::<_, String>(2)?,
                "cost_transaction",
                "transaction_type",
            )?,
            quantity: parse_decimal_row(&row.get::<_, String>(3)?, "cost_transaction", "quantity")?,
            unit_cost: parse_decimal_row(
                &row.get::<_, String>(4)?,
                "cost_transaction",
                "unit_cost",
            )?,
            total_cost: parse_decimal_row(
                &row.get::<_, String>(5)?,
                "cost_transaction",
                "total_cost",
            )?,
            layer_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(6)?,
                "cost_transaction",
                "layer_id",
            )?,
            reference_type: row.get(7)?,
            reference_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(8)?,
                "cost_transaction",
                "reference_id",
            )?,
            notes: row.get(9)?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(10)?,
                "cost_transaction",
                "created_at",
            )?,
        })
    }

    fn row_to_cost_variance(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<CostVariance> {
        Ok(CostVariance {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "cost_variance", "id")?,
            sku: row.get(1)?,
            variance_type: parse_enum_row(
                &row.get::<_, String>(2)?,
                "cost_variance",
                "variance_type",
            )?,
            variance_date: parse_datetime_row(
                &row.get::<_, String>(3)?,
                "cost_variance",
                "variance_date",
            )?,
            standard_cost: parse_decimal_row(
                &row.get::<_, String>(4)?,
                "cost_variance",
                "standard_cost",
            )?,
            actual_cost: parse_decimal_row(
                &row.get::<_, String>(5)?,
                "cost_variance",
                "actual_cost",
            )?,
            variance_amount: parse_decimal_row(
                &row.get::<_, String>(6)?,
                "cost_variance",
                "variance_amount",
            )?,
            variance_percent: parse_decimal_row(
                &row.get::<_, String>(7)?,
                "cost_variance",
                "variance_percent",
            )?,
            quantity: parse_decimal_row(&row.get::<_, String>(8)?, "cost_variance", "quantity")?,
            total_variance: parse_decimal_row(
                &row.get::<_, String>(9)?,
                "cost_variance",
                "total_variance",
            )?,
            reference_type: row.get(10)?,
            reference_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>(11)?,
                "cost_variance",
                "reference_id",
            )?,
            notes: row.get(12)?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(13)?,
                "cost_variance",
                "created_at",
            )?,
        })
    }

    fn row_to_cost_adjustment(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<CostAdjustment> {
        Ok(CostAdjustment {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "cost_adjustment", "id")?,
            adjustment_number: row.get(1)?,
            sku: row.get(2)?,
            adjustment_type: parse_enum_row(
                &row.get::<_, String>(3)?,
                "cost_adjustment",
                "adjustment_type",
            )?,
            previous_cost: parse_decimal_row(
                &row.get::<_, String>(4)?,
                "cost_adjustment",
                "previous_cost",
            )?,
            new_cost: parse_decimal_row(&row.get::<_, String>(5)?, "cost_adjustment", "new_cost")?,
            adjustment_amount: parse_decimal_row(
                &row.get::<_, String>(6)?,
                "cost_adjustment",
                "adjustment_amount",
            )?,
            reason: row.get(7)?,
            approved_by: row.get(8)?,
            approved_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>(9)?,
                "cost_adjustment",
                "approved_at",
            )?,
            status: parse_enum_row(&row.get::<_, String>(10)?, "cost_adjustment", "status")?,
            created_by: row.get(11)?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(12)?,
                "cost_adjustment",
                "created_at",
            )?,
        })
    }

    fn row_to_cost_rollup(&self, row: &rusqlite::Row<'_>) -> rusqlite::Result<CostRollup> {
        Ok(CostRollup {
            id: parse_uuid_row(&row.get::<_, String>(0)?, "cost_rollup", "id")?,
            sku: row.get(1)?,
            bom_id: parse_uuid_opt_row(row.get::<_, Option<String>>(2)?, "cost_rollup", "bom_id")?,
            rollup_date: parse_datetime_row(
                &row.get::<_, String>(3)?,
                "cost_rollup",
                "rollup_date",
            )?,
            material_cost: parse_decimal_row(
                &row.get::<_, String>(4)?,
                "cost_rollup",
                "material_cost",
            )?,
            labor_cost: parse_decimal_row(&row.get::<_, String>(5)?, "cost_rollup", "labor_cost")?,
            overhead_cost: parse_decimal_row(
                &row.get::<_, String>(6)?,
                "cost_rollup",
                "overhead_cost",
            )?,
            total_cost: parse_decimal_row(&row.get::<_, String>(7)?, "cost_rollup", "total_cost")?,
            previous_cost: parse_decimal_row(
                &row.get::<_, String>(8)?,
                "cost_rollup",
                "previous_cost",
            )?,
            cost_change: parse_decimal_row(
                &row.get::<_, String>(9)?,
                "cost_rollup",
                "cost_change",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>(10)?,
                "cost_rollup",
                "created_at",
            )?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn record_cost_transaction_with_conn(
        conn: &rusqlite::Connection,
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

        conn.execute(
            "INSERT INTO cost_transactions (id, sku, transaction_type, quantity, unit_cost,
                total_cost, layer_id, reference_type, reference_id, notes, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                sku,
                transaction_type.to_string(),
                quantity.to_string(),
                unit_cost.to_string(),
                total_cost.to_string(),
                layer_id.map(|id| id.to_string()),
                reference_type,
                reference_id.map(|id| id.to_string()),
                notes,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        Ok(CostTransaction {
            id,
            sku: sku.to_string(),
            transaction_type,
            quantity,
            unit_cost,
            total_cost,
            layer_id,
            reference_type: reference_type.map(String::from),
            reference_id,
            notes: notes.map(String::from),
            created_at: now,
        })
    }
}

impl CostAccountingRepository for SqliteCostAccountingRepository {
    fn get_item_cost(&self, sku: &str) -> Result<Option<ItemCost>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, sku, cost_method, standard_cost, average_cost, last_cost,
                    material_cost, labor_cost, overhead_cost, currency, effective_date,
                    created_at, updated_at
             FROM item_costs WHERE sku = ?",
            [sku],
            |row| self.row_to_item_cost(row),
        );

        match result {
            Ok(item) => Ok(Some(item)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn set_item_cost(&self, input: SetItemCost) -> Result<ItemCost> {
        let now = Utc::now();
        let sku = input.sku.clone();
        // The existence check and the write share one IMMEDIATE transaction:
        // as a read-modify-write on a UNIQUE sku, doing them on separate
        // pooled connections let two concurrent callers both observe "absent"
        // and race on the INSERT.
        with_immediate_transaction(&self.pool, |tx| {
            Self::set_item_cost_with_conn(tx, input.clone(), now)
        })?;

        self.get_item_cost(&sku)?.ok_or(CommerceError::NotFound)
    }
    fn list_item_costs(&self, filter: ItemCostFilter) -> Result<Vec<ItemCost>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, sku, cost_method, standard_cost, average_cost, last_cost,
                    material_cost, labor_cost, overhead_cost, currency, effective_date,
                    created_at, updated_at
             FROM item_costs WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref sku) = filter.sku {
            sql.push_str(" AND sku LIKE ?");
            params.push(Box::new(format!("%{sku}%")));
        }
        if let Some(ref method) = filter.cost_method {
            sql.push_str(" AND cost_method = ?");
            params.push(Box::new(method.to_string()));
        }

        sql.push_str(" ORDER BY sku");

        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_item_cost(row))
            .map_err(map_db_error)?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(map_db_error)?);
        }
        Ok(items)
    }

    fn update_average_cost(
        &self,
        sku: &str,
        quantity: Decimal,
        unit_cost: Decimal,
    ) -> Result<ItemCost> {
        let now = Utc::now();

        // Ensure item cost exists
        let existing = self.get_item_cost(sku)?;
        if existing.is_none() {
            self.set_item_cost(SetItemCost {
                sku: sku.to_string(),
                standard_cost: Some(unit_cost),
                ..Default::default()
            })?;
        }

        // Read the current on-hand quantity and average cost, compute the new
        // weighted average, and write it back inside ONE `IMMEDIATE` transaction,
        // so two concurrent receipts for the same SKU serialize instead of both
        // reading the same `average_cost` and one clobbering the other — a lost
        // update that corrupts the weighted-average cost.
        let sku_param = sku.to_string();
        let now_str = now.to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let sku_params: [&dyn rusqlite::ToSql; 1] = [&sku_param];
            // On-hand quantity lives in `inventory_balances` (per location), keyed
            // by `item_id`; `inventory_items` has no `quantity_on_hand` column, so
            // the previous query errored on every call. Sum the balances for the
            // SKU across locations.
            let current_qty = sum_decimal_query(
                tx,
                "SELECT b.quantity_on_hand FROM inventory_balances b \
                 JOIN inventory_items i ON b.item_id = i.id WHERE i.sku = ?",
                &sku_params,
                "inventory_balance",
                "quantity_on_hand",
            )
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let avg_str: String = tx.query_row(
                "SELECT COALESCE(average_cost, '0') FROM item_costs WHERE sku = ?",
                [&sku_param],
                |row| row.get(0),
            )?;
            let current_avg = parse_decimal_strict(&avg_str, "item_cost", "average_cost")
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            let total_qty = current_qty + quantity;
            let new_avg = if total_qty > Decimal::ZERO {
                ((current_avg * current_qty) + (unit_cost * quantity)) / total_qty
            } else {
                unit_cost
            };

            tx.execute(
                "UPDATE item_costs SET average_cost = ?, last_cost = ?, updated_at = ? WHERE sku = ?",
                rusqlite::params![
                    new_avg.to_string(),
                    unit_cost.to_string(),
                    now_str,
                    sku_param,
                ],
            )?;
            Ok(())
        })?;

        self.get_item_cost(sku)?.ok_or(CommerceError::NotFound)
    }

    fn update_last_cost(&self, sku: &str, unit_cost: Decimal) -> Result<ItemCost> {
        let now = Utc::now();

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "UPDATE item_costs SET last_cost = ?, updated_at = ? WHERE sku = ?",
                rusqlite::params![unit_cost.to_string(), now.to_rfc3339(), sku],
            )
            .map_err(map_db_error)?;
        }

        self.get_item_cost(sku)?.ok_or(CommerceError::NotFound)
    }

    fn create_cost_layer(&self, input: CreateCostLayer) -> Result<CostLayer> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let total_cost = input.quantity * input.unit_cost;

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            conn.execute(
                "INSERT INTO cost_layers (id, sku, layer_date, quantity, remaining_quantity,
                    unit_cost, total_cost, source_type, source_id, lot_id, location_id, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    &input.sku,
                    now.to_rfc3339(),
                    input.quantity.to_string(),
                    input.quantity.to_string(),
                    input.unit_cost.to_string(),
                    total_cost.to_string(),
                    input.source_type.to_string(),
                    input.source_id.map(|id| id.to_string()),
                    input.lot_id.map(|id| id.to_string()),
                    input.location_id,
                    now.to_rfc3339(),
                ],
            )
            .map_err(map_db_error)?;
        }

        self.get_cost_layer(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_cost_layer(&self, id: Uuid) -> Result<Option<CostLayer>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, sku, layer_date, quantity, remaining_quantity, unit_cost, total_cost,
                    source_type, source_id, lot_id, location_id, created_at
             FROM cost_layers WHERE id = ?",
            [id.to_string()],
            |row| self.row_to_cost_layer(row),
        );

        match result {
            Ok(layer) => Ok(Some(layer)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_cost_layers(&self, filter: CostLayerFilter) -> Result<Vec<CostLayer>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, sku, layer_date, quantity, remaining_quantity, unit_cost, total_cost,
                    source_type, source_id, lot_id, location_id, created_at
             FROM cost_layers WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref sku) = filter.sku {
            sql.push_str(" AND sku = ?");
            params.push(Box::new(sku.clone()));
        }
        if let Some(ref source) = filter.source_type {
            sql.push_str(" AND source_type = ?");
            params.push(Box::new(source.to_string()));
        }
        // `remaining_quantity` is a TEXT decimal; filtering it in SQL with
        // CAST(... AS REAL) coerces to IEEE-754 floats, so the filter is
        // applied below on the exact parsed `Decimal` values instead (and the
        // LIMIT after it, so filtering never eats into the page).
        let has_remaining = filter.has_remaining == Some(true);

        sql.push_str(" ORDER BY layer_date ASC");

        if !has_remaining {
            if let Some(limit) = filter.limit {
                sql.push_str(&format!(" LIMIT {limit}"));
            }
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_cost_layer(row))
            .map_err(map_db_error)?;

        let mut layers = Vec::new();
        for row in rows {
            layers.push(row.map_err(map_db_error)?);
        }
        if has_remaining {
            layers.retain(|layer| layer.remaining_quantity > Decimal::ZERO);
            if let Some(limit) = filter.limit {
                layers.truncate(limit as usize);
            }
        }
        Ok(layers)
    }

    fn issue_fifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut remaining = input.quantity;
        let mut transactions = Vec::new();

        // Get layers in FIFO order (oldest first) from the same transaction
        // snapshot. Depleted layers are skipped in Rust on the exact parsed
        // `Decimal`: `remaining_quantity` is a TEXT decimal, and filtering it
        // in SQL via CAST(... AS REAL) would coerce to IEEE-754 floats.
        let layers: Vec<CostLayer> = {
            let mut stmt = tx
                .prepare(
                    "SELECT id, sku, layer_date, quantity, remaining_quantity, unit_cost, total_cost,
                            source_type, source_id, lot_id, location_id, created_at
                     FROM cost_layers
                     WHERE sku = ?
                     ORDER BY layer_date ASC",
                )
                .map_err(map_db_error)?;
            let rows = stmt
                .query_map([&input.sku], |row| self.row_to_cost_layer(row))
                .map_err(map_db_error)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_db_error)?
                .into_iter()
                .filter(|layer| layer.remaining_quantity > Decimal::ZERO)
                .collect()
        };

        for layer in layers {
            if remaining <= Decimal::ZERO {
                break;
            }

            let consume_qty = remaining.min(layer.remaining_quantity);
            let new_remaining = layer.remaining_quantity - consume_qty;

            // Update layer
            tx.execute(
                "UPDATE cost_layers SET remaining_quantity = ? WHERE id = ?",
                [&new_remaining.to_string(), &layer.id.to_string()],
            )
            .map_err(map_db_error)?;

            // Record transaction
            let tx_record = Self::record_cost_transaction_with_conn(
                &tx,
                &input.sku,
                CostTransactionType::Issue,
                consume_qty,
                layer.unit_cost,
                Some(layer.id),
                input.reference_type.as_deref(),
                input.reference_id,
                input.notes.as_deref(),
            )?;
            transactions.push(tx_record);

            remaining -= consume_qty;
        }

        if remaining > Decimal::ZERO {
            return Err(CommerceError::ValidationError(format!(
                "Insufficient remaining cost layers for sku {} (short by {})",
                input.sku, remaining
            )));
        }

        tx.commit().map_err(map_db_error)?;
        Ok(transactions)
    }

    fn issue_lifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = super::begin_immediate(&mut conn).map_err(map_db_error)?;
        let mut remaining = input.quantity;
        let mut transactions = Vec::new();

        // Get layers in LIFO order (newest first) from the same transaction
        // snapshot. Depleted layers are skipped in Rust on the exact parsed
        // `Decimal`: `remaining_quantity` is a TEXT decimal, and filtering it
        // in SQL via CAST(... AS REAL) would coerce to IEEE-754 floats.
        let layers: Vec<CostLayer> = {
            let mut stmt = tx
                .prepare(
                    "SELECT id, sku, layer_date, quantity, remaining_quantity, unit_cost, total_cost,
                            source_type, source_id, lot_id, location_id, created_at
                     FROM cost_layers
                     WHERE sku = ?
                     ORDER BY layer_date DESC",
                )
                .map_err(map_db_error)?;
            let rows = stmt
                .query_map([&input.sku], |row| self.row_to_cost_layer(row))
                .map_err(map_db_error)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_db_error)?
                .into_iter()
                .filter(|layer| layer.remaining_quantity > Decimal::ZERO)
                .collect()
        };

        for layer in layers {
            if remaining <= Decimal::ZERO {
                break;
            }

            let consume_qty = remaining.min(layer.remaining_quantity);
            let new_remaining = layer.remaining_quantity - consume_qty;

            // Update layer
            tx.execute(
                "UPDATE cost_layers SET remaining_quantity = ? WHERE id = ?",
                [&new_remaining.to_string(), &layer.id.to_string()],
            )
            .map_err(map_db_error)?;

            // Record transaction
            let tx_record = Self::record_cost_transaction_with_conn(
                &tx,
                &input.sku,
                CostTransactionType::Issue,
                consume_qty,
                layer.unit_cost,
                Some(layer.id),
                input.reference_type.as_deref(),
                input.reference_id,
                input.notes.as_deref(),
            )?;
            transactions.push(tx_record);

            remaining -= consume_qty;
        }

        if remaining > Decimal::ZERO {
            return Err(CommerceError::ValidationError(format!(
                "Insufficient remaining cost layers for sku {} (short by {})",
                input.sku, remaining
            )));
        }

        tx.commit().map_err(map_db_error)?;
        Ok(transactions)
    }

    fn get_layers_remaining(&self, sku: &str) -> Result<Decimal> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let sku_param = sku.to_string();
        let sku_params: [&dyn rusqlite::ToSql; 1] = [&sku_param];
        let result = sum_decimal_query(
            &conn,
            "SELECT remaining_quantity FROM cost_layers WHERE sku = ?",
            &sku_params,
            "cost_layers",
            "remaining_quantity",
        )?;

        Ok(result)
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
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Self::record_cost_transaction_with_conn(
            &conn,
            sku,
            transaction_type,
            quantity,
            unit_cost,
            layer_id,
            reference_type,
            reference_id,
            notes,
        )
    }

    fn list_cost_transactions(
        &self,
        filter: CostTransactionFilter,
    ) -> Result<Vec<CostTransaction>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, sku, transaction_type, quantity, unit_cost, total_cost,
                    layer_id, reference_type, reference_id, notes, created_at
             FROM cost_transactions WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref sku) = filter.sku {
            sql.push_str(" AND sku = ?");
            params.push(Box::new(sku.clone()));
        }
        if let Some(ref tx_type) = filter.transaction_type {
            sql.push_str(" AND transaction_type = ?");
            params.push(Box::new(tx_type.to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_cost_transaction(row))
            .map_err(map_db_error)?;

        let mut txns = Vec::new();
        for row in rows {
            txns.push(row.map_err(map_db_error)?);
        }
        Ok(txns)
    }

    fn record_variance(&self, input: RecordCostVariance) -> Result<CostVariance> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        let variance_amount = input.actual_cost - input.standard_cost;
        let variance_percent = if input.standard_cost == Decimal::ZERO {
            Decimal::ZERO
        } else {
            (variance_amount / input.standard_cost) * Decimal::from(100)
        };
        let total_variance = variance_amount * input.quantity;

        conn.execute(
            "INSERT INTO cost_variances (id, sku, variance_type, variance_date, standard_cost,
                actual_cost, variance_amount, variance_percent, quantity, total_variance,
                reference_type, reference_id, notes, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                &input.sku,
                input.variance_type.to_string(),
                now.to_rfc3339(),
                input.standard_cost.to_string(),
                input.actual_cost.to_string(),
                variance_amount.to_string(),
                variance_percent.to_string(),
                input.quantity.to_string(),
                total_variance.to_string(),
                input.reference_type,
                input.reference_id.map(|id| id.to_string()),
                input.notes,
                now.to_rfc3339(),
            ],
        )
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

    fn list_variances(&self, filter: CostVarianceFilter) -> Result<Vec<CostVariance>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, sku, variance_type, variance_date, standard_cost, actual_cost,
                    variance_amount, variance_percent, quantity, total_variance,
                    reference_type, reference_id, notes, created_at
             FROM cost_variances WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref sku) = filter.sku {
            sql.push_str(" AND sku = ?");
            params.push(Box::new(sku.clone()));
        }
        if let Some(ref var_type) = filter.variance_type {
            sql.push_str(" AND variance_type = ?");
            params.push(Box::new(var_type.to_string()));
        }

        sql.push_str(" ORDER BY variance_date DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_cost_variance(row))
            .map_err(map_db_error)?;

        let mut variances = Vec::new();
        for row in rows {
            variances.push(row.map_err(map_db_error)?);
        }
        Ok(variances)
    }

    fn get_variance_summary(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Decimal> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let from_param = from.to_rfc3339();
        let to_param = to.to_rfc3339();
        let params: [&dyn rusqlite::ToSql; 2] = [&from_param, &to_param];
        let result = sum_decimal_query(
            &conn,
            "SELECT total_variance FROM cost_variances WHERE variance_date BETWEEN ? AND ?",
            &params,
            "cost_variances",
            "total_variance",
        )?;

        Ok(result)
    }

    fn create_adjustment(&self, input: CreateCostAdjustment) -> Result<CostAdjustment> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let adjustment_number = generate_cost_adjustment_number();

        // Get current cost
        let current_cost =
            self.get_item_cost(&input.sku)?.map(|c| c.standard_cost).unwrap_or_default();
        let adjustment_amount = input.new_cost - current_cost;

        conn.execute(
            "INSERT INTO cost_adjustments (id, adjustment_number, sku, adjustment_type,
                previous_cost, new_cost, adjustment_amount, reason, status, created_by, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                &adjustment_number,
                &input.sku,
                input.adjustment_type.to_string(),
                current_cost.to_string(),
                input.new_cost.to_string(),
                adjustment_amount.to_string(),
                &input.reason,
                CostAdjustmentStatus::Pending.to_string(),
                input.created_by,
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_adjustment(id)?.ok_or(CommerceError::NotFound)
    }

    fn get_adjustment(&self, id: Uuid) -> Result<Option<CostAdjustment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, adjustment_number, sku, adjustment_type, previous_cost, new_cost,
                    adjustment_amount, reason, approved_by, approved_at, status, created_by, created_at
             FROM cost_adjustments WHERE id = ?",
            [id.to_string()],
            |row| self.row_to_cost_adjustment(row),
        );

        match result {
            Ok(adj) => Ok(Some(adj)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_adjustments(&self, filter: CostAdjustmentFilter) -> Result<Vec<CostAdjustment>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, adjustment_number, sku, adjustment_type, previous_cost, new_cost,
                    adjustment_amount, reason, approved_by, approved_at, status, created_by, created_at
             FROM cost_adjustments WHERE 1=1"
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref sku) = filter.sku {
            sql.push_str(" AND sku = ?");
            params.push(Box::new(sku.clone()));
        }
        if let Some(ref status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(std::convert::AsRef::as_ref).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_cost_adjustment(row))
            .map_err(map_db_error)?;

        let mut adjustments = Vec::new();
        for row in rows {
            adjustments.push(row.map_err(map_db_error)?);
        }
        Ok(adjustments)
    }

    fn approve_adjustment(&self, id: Uuid, approved_by: &str) -> Result<CostAdjustment> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        // Only a pending adjustment may be approved. Unguarded, this
        // re-approved an already-applied adjustment, which `apply_adjustment`
        // would then apply to the item cost a second time.
        let rows = conn
            .execute(
                "UPDATE cost_adjustments SET status = ?, approved_by = ?, approved_at = ?
                 WHERE id = ? AND status = 'pending'",
                rusqlite::params![
                    CostAdjustmentStatus::Approved.to_string(),
                    approved_by,
                    now.to_rfc3339(),
                    id.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        if rows == 0 {
            return Err(adjustment_conflict(&conn, id, "approved"));
        }

        self.get_adjustment(id)?.ok_or(CommerceError::NotFound)
    }

    fn apply_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        let now = Utc::now();

        // Claim the adjustment and apply the cost change in ONE transaction.
        // Previously the status check, the item-cost write and the status
        // write each ran on their own connection: two concurrent callers both
        // passed the check and both moved the cost, and a crash between the
        // cost write and the status write left an "approved" adjustment whose
        // cost had already been applied — re-applying doubled it.
        with_immediate_transaction(&self.pool, |tx| {
            let claimed = tx.execute(
                "UPDATE cost_adjustments SET status = ? WHERE id = ? AND status = 'approved'",
                [CostAdjustmentStatus::Applied.to_string(), id.to_string()],
            )?;
            if claimed == 0 {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    adjustment_conflict(tx, id, "applied"),
                )));
            }

            let (sku, new_cost): (String, String) = tx.query_row(
                "SELECT sku, new_cost FROM cost_adjustments WHERE id = ?",
                [id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let new_cost = parse_decimal_row(&new_cost, "cost_adjustment", "new_cost")?;

            Self::set_item_cost_with_conn(
                tx,
                SetItemCost { sku, standard_cost: Some(new_cost), ..Default::default() },
                now,
            )
        })?;

        self.get_adjustment(id)?.ok_or(CommerceError::NotFound)
    }

    fn reject_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // An applied adjustment is live in the item cost: recording it as
        // "rejected" would make the audit trail contradict the cost record.
        let rows = conn
            .execute(
                "UPDATE cost_adjustments SET status = ?
                 WHERE id = ? AND status IN ('pending', 'approved')",
                [CostAdjustmentStatus::Rejected.to_string(), id.to_string()],
            )
            .map_err(map_db_error)?;
        if rows == 0 {
            return Err(adjustment_conflict(&conn, id, "rejected"));
        }

        self.get_adjustment(id)?.ok_or(CommerceError::NotFound)
    }

    fn calculate_rollup(&self, sku: &str, bom_id: Option<Uuid>) -> Result<CostRollup> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Get previous cost
        let previous_cost = self.get_rollup(sku)?.map(|r| r.total_cost).unwrap_or_default();

        // Calculate from BOM components if bom_id provided
        let (material_cost, labor_cost, overhead_cost) = if let Some(bom_id) = bom_id {
            // Sum component costs
            let mut stmt = conn
                .prepare(
                    "SELECT bc.quantity, ic.standard_cost
                     FROM bom_components bc
                     LEFT JOIN item_costs ic ON bc.component_sku = ic.sku
                     WHERE bc.bom_id = ?",
                )
                .map_err(map_db_error)?;
            let mut rows = stmt.query([bom_id.to_string()]).map_err(map_db_error)?;
            let mut material_cost = Decimal::ZERO;

            while let Some(row) = rows.next().map_err(map_db_error)? {
                let qty_str: String = row.get(0).map_err(map_db_error)?;
                let quantity = parse_decimal_strict(&qty_str, "bom_components", "quantity")?;
                let cost_str: Option<String> = row.get(1).map_err(map_db_error)?;
                let standard_cost = match cost_str {
                    Some(value) if !value.is_empty() => {
                        parse_decimal_strict(&value, "item_costs", "standard_cost")?
                    }
                    _ => Decimal::ZERO,
                };
                material_cost += quantity * standard_cost;
            }
            (material_cost, Decimal::ZERO, Decimal::ZERO)
        } else {
            // Get from item cost
            let item = self.get_item_cost(sku)?;
            match item {
                Some(c) => (c.material_cost, c.labor_cost, c.overhead_cost),
                None => (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO),
            }
        };

        let total_cost = material_cost + labor_cost + overhead_cost;
        let cost_change = total_cost - previous_cost;

        conn.execute(
            "INSERT INTO cost_rollups (id, sku, bom_id, rollup_date, material_cost, labor_cost,
                overhead_cost, total_cost, previous_cost, cost_change, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                sku,
                bom_id.map(|id| id.to_string()),
                now.to_rfc3339(),
                material_cost.to_string(),
                labor_cost.to_string(),
                overhead_cost.to_string(),
                total_cost.to_string(),
                previous_cost.to_string(),
                cost_change.to_string(),
                now.to_rfc3339(),
            ],
        )
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

    fn get_rollup(&self, sku: &str) -> Result<Option<CostRollup>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result = conn.query_row(
            "SELECT id, sku, bom_id, rollup_date, material_cost, labor_cost, overhead_cost,
                    total_cost, previous_cost, cost_change, created_at
             FROM cost_rollups WHERE sku = ? ORDER BY rollup_date DESC LIMIT 1",
            [sku],
            |row| self.row_to_cost_rollup(row),
        );

        match result {
            Ok(rollup) => Ok(Some(rollup)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_inventory_valuation(&self, cost_method: CostMethod) -> Result<InventoryValuation> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        // Sum on-hand from inventory_balances (per-location) up to per-sku via
        // inventory_items.id. Quantities are TEXT decimals, so use the exact
        // `decimal_sum` aggregate (see money_agg) instead of SUM(CAST(.. AS
        // REAL)), which accumulates IEEE-754 float error in the costing path.
        let mut stmt = conn
            .prepare(
                "SELECT decimal_sum(ib.quantity_on_hand) AS qty,
                        ic.standard_cost, ic.average_cost, ic.last_cost
                 FROM inventory_items ii
                 LEFT JOIN inventory_balances ib ON ib.item_id = ii.id
                 LEFT JOIN item_costs ic ON ii.sku = ic.sku
                 GROUP BY ii.id, ic.standard_cost, ic.average_cost, ic.last_cost",
            )
            .map_err(map_db_error)?;
        let mut rows = stmt.query([]).map_err(map_db_error)?;

        let mut total_quantity = Decimal::ZERO;
        let mut total_value = Decimal::ZERO;

        while let Some(row) = rows.next().map_err(map_db_error)? {
            let qty_text: String = row.get(0).map_err(map_db_error)?;
            let quantity =
                parse_decimal_strict(&qty_text, "inventory_balances", "quantity_on_hand")?;

            let standard_raw: Option<String> = row.get(1).map_err(map_db_error)?;
            let average_raw: Option<String> = row.get(2).map_err(map_db_error)?;
            let last_raw: Option<String> = row.get(3).map_err(map_db_error)?;

            let standard_cost = match standard_raw {
                Some(value) if !value.is_empty() => {
                    parse_decimal_strict(&value, "item_costs", "standard_cost")?
                }
                _ => Decimal::ZERO,
            };
            let average_cost = match average_raw {
                Some(value) if !value.is_empty() => {
                    parse_decimal_strict(&value, "item_costs", "average_cost")?
                }
                _ => Decimal::ZERO,
            };
            let last_cost = match last_raw {
                Some(value) if !value.is_empty() => {
                    parse_decimal_strict(&value, "item_costs", "last_cost")?
                }
                _ => Decimal::ZERO,
            };

            let unit_cost = match cost_method {
                CostMethod::Standard => standard_cost,
                CostMethod::Average => average_cost,
                CostMethod::Fifo | CostMethod::Lifo => average_cost,
                CostMethod::Specific => last_cost,
                _ => average_cost,
            };

            total_quantity += quantity;
            total_value += quantity * unit_cost;
        }

        let average_unit_cost = if total_quantity > Decimal::ZERO {
            total_value / total_quantity
        } else {
            Decimal::ZERO
        };

        Ok(InventoryValuation {
            total_quantity,
            total_value,
            average_unit_cost,
            valuation_method: cost_method,
            as_of_date: now,
        })
    }

    fn get_sku_cost_summary(&self, sku: &str) -> Result<Option<SkuCostSummary>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        // Quantities are TEXT decimals; `decimal_sum` (see money_agg) keeps the
        // aggregation exact instead of round-tripping through f64.
        let result = conn.query_row(
            "SELECT
                ii.sku,
                decimal_sum(ib.quantity_on_hand) AS qty,
                ic.standard_cost,
                ic.average_cost
             FROM inventory_items ii
             LEFT JOIN inventory_balances ib ON ib.item_id = ii.id
             LEFT JOIN item_costs ic ON ii.sku = ic.sku
             WHERE ii.sku = ?
             GROUP BY ii.id, ic.standard_cost, ic.average_cost",
            [sku],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        );

        let (sku_value, qty_text, standard_raw, average_raw) = match result {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(map_db_error(e)),
        };

        let quantity_on_hand =
            parse_decimal_strict(&qty_text, "inventory_balances", "quantity_on_hand")?;
        let standard_cost = match standard_raw {
            Some(value) if !value.is_empty() => {
                parse_decimal_strict(&value, "sku_cost_summary", "standard_cost")?
            }
            _ => Decimal::ZERO,
        };
        let average_cost = match average_raw {
            Some(value) if !value.is_empty() => {
                parse_decimal_strict(&value, "sku_cost_summary", "average_cost")?
            }
            _ => Decimal::ZERO,
        };
        let total_value = quantity_on_hand * average_cost;

        let sku_param = sku.to_string();
        let sku_params: [&dyn rusqlite::ToSql; 1] = [&sku_param];
        let variance_ytd = sum_decimal_query(
            &conn,
            "SELECT total_variance FROM cost_variances
             WHERE sku = ? AND strftime('%Y', variance_date) = strftime('%Y', 'now')",
            &sku_params,
            "cost_variances",
            "total_variance",
        )?;

        Ok(Some(SkuCostSummary {
            sku: sku_value,
            quantity_on_hand,
            standard_cost,
            average_cost,
            total_value,
            variance_ytd,
        }))
    }

    fn get_total_inventory_value(&self) -> Result<Decimal> {
        let valuation = self.get_inventory_valuation(CostMethod::Average)?;
        Ok(valuation.total_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqliteDatabase;
    use chrono::Duration;
    use rust_decimal_macros::dec;
    use stateset_core::{
        CostAccountingRepository, CostAdjustmentFilter, CostAdjustmentType, CostLayerFilter,
        CostLayerSource, CostMethod, CreateCostAdjustment, CreateCostLayer, IssueCostLayers,
        ItemCostFilter, RecordCostVariance, SetItemCost, VarianceType,
    };

    fn fresh_repo() -> SqliteCostAccountingRepository {
        SqliteDatabase::in_memory().expect("in-memory").cost_accounting()
    }

    /// A pending cost adjustment moving `sku` from its current cost to `new_cost`.
    fn make_adjustment(
        repo: &SqliteCostAccountingRepository,
        sku: &str,
        new_cost: Decimal,
    ) -> CostAdjustment {
        repo.create_adjustment(CreateCostAdjustment {
            sku: sku.into(),
            adjustment_type: CostAdjustmentType::Revaluation,
            new_cost,
            reason: "test".into(),
            created_by: Some("tester".into()),
        })
        .expect("create adjustment")
    }

    #[test]
    fn cost_adjustment_lifecycle_transitions_are_guarded() {
        let repo = fresh_repo();
        repo.set_item_cost(SetItemCost {
            sku: "ADJ-SKU".into(),
            standard_cost: Some(dec!(10)),
            ..Default::default()
        })
        .expect("seed cost");

        let adjustment = make_adjustment(&repo, "ADJ-SKU", dec!(15));
        repo.approve_adjustment(adjustment.id, "approver").expect("approve");

        // Approving twice would let the cost change be applied a second time.
        let err = repo
            .approve_adjustment(adjustment.id, "approver")
            .expect_err("re-approval must be refused");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

        repo.apply_adjustment(adjustment.id).expect("apply");
        assert_eq!(
            repo.get_item_cost("ADJ-SKU").expect("get").expect("cost").standard_cost,
            dec!(15)
        );

        // Applying twice must not move the cost again, and rejecting an
        // applied adjustment would make the audit trail contradict the cost.
        let err = repo.apply_adjustment(adjustment.id).expect_err("re-apply must be refused");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
        let err = repo
            .reject_adjustment(adjustment.id)
            .expect_err("rejecting an applied adjustment must be refused");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

        assert_eq!(
            repo.get_item_cost("ADJ-SKU").expect("get").expect("cost").standard_cost,
            dec!(15),
            "the cost must move exactly once"
        );
    }

    #[test]
    fn applying_an_unapproved_adjustment_is_refused_and_leaves_cost_untouched() {
        let repo = fresh_repo();
        repo.set_item_cost(SetItemCost {
            sku: "ADJ-PENDING".into(),
            standard_cost: Some(dec!(10)),
            ..Default::default()
        })
        .expect("seed cost");

        let adjustment = make_adjustment(&repo, "ADJ-PENDING", dec!(99));
        let err = repo.apply_adjustment(adjustment.id).expect_err("pending must not apply");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
        assert_eq!(
            repo.get_item_cost("ADJ-PENDING").expect("get").expect("cost").standard_cost,
            dec!(10),
            "a refused apply must not move the cost"
        );

        // A rejected adjustment can never be applied either.
        repo.reject_adjustment(adjustment.id).expect("reject");
        let err = repo.apply_adjustment(adjustment.id).expect_err("rejected must not apply");
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    }

    #[test]
    fn concurrent_apply_moves_the_cost_exactly_once() {
        use std::sync::{Arc, Barrier};

        let db = Arc::new(SqliteDatabase::in_memory().expect("in-memory"));
        let repo = db.cost_accounting();
        repo.set_item_cost(SetItemCost {
            sku: "ADJ-RACE".into(),
            standard_cost: Some(dec!(10)),
            ..Default::default()
        })
        .expect("seed cost");
        let adjustment = make_adjustment(&repo, "ADJ-RACE", dec!(25));
        repo.approve_adjustment(adjustment.id, "approver").expect("approve");

        // Two threads apply the same approved adjustment simultaneously. The
        // claim and the cost write share one transaction, so exactly one wins.
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let db = Arc::clone(&db);
                let barrier = Arc::clone(&barrier);
                let id = adjustment.id;
                std::thread::spawn(move || {
                    barrier.wait();
                    db.cost_accounting().apply_adjustment(id)
                })
            })
            .collect();
        let results: Vec<_> =
            handles.into_iter().map(|h| h.join().expect("thread panicked")).collect();

        assert_eq!(
            results.iter().filter(|r| r.is_ok()).count(),
            1,
            "exactly one apply may succeed: {results:?}"
        );
        assert_eq!(
            db.cost_accounting()
                .get_item_cost("ADJ-RACE")
                .expect("get")
                .expect("cost")
                .standard_cost,
            dec!(25),
            "the cost must land on the adjustment value exactly once"
        );
    }

    fn make_layer(
        repo: &SqliteCostAccountingRepository,
        sku: &str,
        qty: Decimal,
        cost: Decimal,
    ) -> CostLayer {
        repo.create_cost_layer(CreateCostLayer {
            sku: sku.into(),
            quantity: qty,
            unit_cost: cost,
            source_type: CostLayerSource::Purchase,
            source_id: None,
            lot_id: None,
            location_id: Some(1),
        })
        .expect("create layer")
    }

    /// Seed an inventory item with one on-hand balance row per quantity
    /// (each in its own location, since balances are unique per location).
    fn seed_on_hand(repo: &SqliteCostAccountingRepository, sku: &str, quantities: &[&str]) {
        let conn = repo.pool.get().expect("conn");
        conn.execute(
            "INSERT INTO inventory_items (sku, name) VALUES (?1, ?2)",
            rusqlite::params![sku, format!("Item {sku}")],
        )
        .expect("insert item");
        let item_id = conn.last_insert_rowid();
        for (i, qty) in quantities.iter().enumerate() {
            let location_id = (i + 1) as i64;
            conn.execute(
                "INSERT OR IGNORE INTO inventory_locations (id, name, code) VALUES (?1, ?2, ?3)",
                rusqlite::params![
                    location_id,
                    format!("Loc {location_id}"),
                    format!("LOC-{location_id}")
                ],
            )
            .expect("insert location");
            conn.execute(
                // quantity_available must satisfy the balance identity
                // (migration 092), so seed it alongside on_hand.
                "INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_available)
                 VALUES (?1, ?2, ?3, ?3)",
                rusqlite::params![item_id, location_id, qty],
            )
            .expect("insert balance");
        }
    }

    #[test]
    fn inventory_valuation_sums_float_hostile_quantities_exactly() {
        let repo = fresh_repo();
        // 0.1 + 0.2 + 0.3 accumulates float error under SUM(CAST(... AS REAL))
        // (0.6000000000000001); the exact Decimal sum is 0.6.
        seed_on_hand(&repo, "VAL-EXACT", &["0.1", "0.2", "0.3"]);
        repo.set_item_cost(SetItemCost {
            sku: "VAL-EXACT".into(),
            cost_method: Some(CostMethod::Standard),
            standard_cost: Some(dec!(0.1)),
            ..Default::default()
        })
        .expect("cost");

        let v = repo.get_inventory_valuation(CostMethod::Standard).expect("valuation");
        assert_eq!(v.total_quantity, dec!(0.6));
        assert_eq!(v.total_value, dec!(0.06));
    }

    #[test]
    fn inventory_valuation_preserves_high_precision_quantities() {
        let repo = fresh_repo();
        // 25 significant digits cannot round-trip through an f64.
        seed_on_hand(&repo, "VAL-HP", &["1234567.123456789012345678"]);
        repo.set_item_cost(SetItemCost {
            sku: "VAL-HP".into(),
            cost_method: Some(CostMethod::Standard),
            standard_cost: Some(dec!(1)),
            ..Default::default()
        })
        .expect("cost");

        let v = repo.get_inventory_valuation(CostMethod::Standard).expect("valuation");
        assert_eq!(v.total_quantity, dec!(1234567.123456789012345678));
        assert_eq!(v.total_value, dec!(1234567.123456789012345678));
    }

    #[test]
    fn sku_cost_summary_quantity_and_value_are_exact() {
        let repo = fresh_repo();
        seed_on_hand(&repo, "SUM-EXACT", &["0.1", "0.2"]);
        // average_cost starts equal to the standard cost on first insert.
        repo.set_item_cost(SetItemCost {
            sku: "SUM-EXACT".into(),
            cost_method: Some(CostMethod::Average),
            standard_cost: Some(dec!(3)),
            ..Default::default()
        })
        .expect("cost");

        let s = repo.get_sku_cost_summary("SUM-EXACT").expect("ok").expect("found");
        assert_eq!(s.quantity_on_hand, dec!(0.3), "0.1 + 0.2 must sum exactly");
        assert_eq!(s.total_value, dec!(0.9), "0.3 * 3 must be exact");
    }

    #[test]
    fn issue_fifo_skips_depleted_layers_and_has_remaining_excludes_them() {
        let repo = fresh_repo();
        let first = make_layer(&repo, "FIFO-D", dec!(0.3), dec!(5));
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = make_layer(&repo, "FIFO-D", dec!(1), dec!(8));

        // Deplete the first layer with three exact 0.1 issues.
        for _ in 0..3 {
            repo.issue_fifo(IssueCostLayers {
                sku: "FIFO-D".into(),
                quantity: dec!(0.1),
                reference_type: None,
                reference_id: None,
                notes: None,
            })
            .expect("issue");
        }
        let first_after = repo.get_cost_layer(first.id).expect("ok").expect("found");
        assert_eq!(first_after.remaining_quantity, dec!(0));

        // The next issue must come entirely from the second layer.
        let txns = repo
            .issue_fifo(IssueCostLayers {
                sku: "FIFO-D".into(),
                quantity: dec!(0.5),
                reference_type: None,
                reference_id: None,
                notes: None,
            })
            .expect("issue rest");
        assert!(!txns.is_empty());
        assert!(txns.iter().all(|t| t.layer_id == Some(second.id)));

        // has_remaining (with a limit) must return only the non-depleted layer.
        let remaining = repo
            .list_cost_layers(CostLayerFilter {
                sku: Some("FIFO-D".into()),
                has_remaining: Some(true),
                limit: Some(10),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, second.id);
    }

    #[test]
    fn set_item_cost_persists_and_round_trips() {
        let repo = fresh_repo();
        let cost = repo
            .set_item_cost(SetItemCost {
                sku: "WIDGET-1".into(),
                cost_method: Some(CostMethod::Standard),
                standard_cost: Some(dec!(12.50)),
                material_cost: Some(dec!(5.00)),
                labor_cost: Some(dec!(3.00)),
                overhead_cost: Some(dec!(4.50)),
                currency: None,
            })
            .expect("set");
        assert_eq!(cost.sku, "WIDGET-1");
        assert_eq!(cost.cost_method, CostMethod::Standard);
        assert_eq!(cost.standard_cost, dec!(12.50));

        let by_sku = repo.get_item_cost("WIDGET-1").expect("ok").expect("found");
        assert_eq!(by_sku.sku, "WIDGET-1");
        assert!(repo.get_item_cost("MISSING").expect("ok").is_none());
    }

    #[test]
    fn set_item_cost_upserts_on_existing_sku() {
        let repo = fresh_repo();
        repo.set_item_cost(SetItemCost {
            sku: "UP-1".into(),
            standard_cost: Some(dec!(10)),
            ..Default::default()
        })
        .expect("first");
        let updated = repo
            .set_item_cost(SetItemCost {
                sku: "UP-1".into(),
                standard_cost: Some(dec!(15)),
                ..Default::default()
            })
            .expect("second");
        assert_eq!(updated.standard_cost, dec!(15));
        let listed = repo
            .list_item_costs(ItemCostFilter { sku: Some("UP-1".into()), ..Default::default() })
            .expect("list");
        assert_eq!(listed.len(), 1, "upsert, not duplicate");
    }

    #[test]
    fn list_item_costs_filters_by_sku() {
        let repo = fresh_repo();
        repo.set_item_cost(SetItemCost {
            sku: "FILTER-A".into(),
            standard_cost: Some(dec!(1)),
            ..Default::default()
        })
        .expect("a");
        repo.set_item_cost(SetItemCost {
            sku: "FILTER-B".into(),
            standard_cost: Some(dec!(2)),
            ..Default::default()
        })
        .expect("b");

        let only_a = repo
            .list_item_costs(ItemCostFilter { sku: Some("FILTER-A".into()), ..Default::default() })
            .expect("list");
        assert_eq!(only_a.len(), 1);
        assert_eq!(only_a[0].sku, "FILTER-A");
    }

    #[test]
    fn create_cost_layer_persists_and_remaining_starts_full() {
        let repo = fresh_repo();
        let layer = make_layer(&repo, "L-1", dec!(10), dec!(7.50));
        assert_eq!(layer.sku, "L-1");
        assert_eq!(layer.quantity, dec!(10));
        assert_eq!(layer.unit_cost, dec!(7.50));
        assert_eq!(layer.remaining_quantity, dec!(10));

        let by_id = repo.get_cost_layer(layer.id).expect("ok").expect("found");
        assert_eq!(by_id.id, layer.id);

        let remaining = repo.get_layers_remaining("L-1").expect("ok");
        assert_eq!(remaining, dec!(10));
    }

    #[test]
    fn list_cost_layers_filters_by_sku_and_has_remaining() {
        let repo = fresh_repo();
        make_layer(&repo, "LL-A", dec!(5), dec!(1));
        make_layer(&repo, "LL-A", dec!(8), dec!(2));
        make_layer(&repo, "LL-B", dec!(3), dec!(3));

        let a = repo
            .list_cost_layers(CostLayerFilter { sku: Some("LL-A".into()), ..Default::default() })
            .expect("a");
        assert_eq!(a.len(), 2);

        let with_remaining = repo
            .list_cost_layers(CostLayerFilter {
                sku: Some("LL-A".into()),
                has_remaining: Some(true),
                ..Default::default()
            })
            .expect("rem");
        assert_eq!(with_remaining.len(), 2);
    }

    #[test]
    fn issue_fifo_consumes_oldest_layer_first() {
        let repo = fresh_repo();
        // First (oldest) layer at $5; second at $8
        let oldest = make_layer(&repo, "FIFO-1", dec!(10), dec!(5));
        std::thread::sleep(std::time::Duration::from_millis(2));
        let _newer = make_layer(&repo, "FIFO-1", dec!(10), dec!(8));

        let txns = repo
            .issue_fifo(IssueCostLayers {
                sku: "FIFO-1".into(),
                quantity: dec!(7),
                reference_type: Some("order".into()),
                reference_id: None,
                notes: None,
            })
            .expect("issue fifo");

        // Should issue 7 from oldest layer at $5
        assert!(!txns.is_empty());
        // Oldest layer should now have 3 remaining
        let layer = repo.get_cost_layer(oldest.id).expect("ok").expect("found");
        assert_eq!(layer.remaining_quantity, dec!(3));
    }

    #[test]
    fn issue_lifo_consumes_newest_layer_first() {
        let repo = fresh_repo();
        let _oldest = make_layer(&repo, "LIFO-1", dec!(10), dec!(5));
        std::thread::sleep(std::time::Duration::from_millis(2));
        let newest = make_layer(&repo, "LIFO-1", dec!(10), dec!(8));

        let txns = repo
            .issue_lifo(IssueCostLayers {
                sku: "LIFO-1".into(),
                quantity: dec!(4),
                reference_type: Some("issue".into()),
                reference_id: None,
                notes: None,
            })
            .expect("issue lifo");

        assert!(!txns.is_empty());
        let layer = repo.get_cost_layer(newest.id).expect("ok").expect("found");
        assert_eq!(layer.remaining_quantity, dec!(6));
    }

    #[test]
    fn record_variance_persists_and_summary_aggregates() {
        let repo = fresh_repo();
        repo.record_variance(RecordCostVariance {
            sku: "V-1".into(),
            variance_type: VarianceType::Purchase,
            standard_cost: dec!(10),
            actual_cost: dec!(12),
            quantity: dec!(5),
            reference_type: None,
            reference_id: None,
            notes: None,
        })
        .expect("record");

        let from = Utc::now() - Duration::days(1);
        let to = Utc::now() + Duration::days(1);
        let summary = repo.get_variance_summary(from, to).expect("ok");
        // (12-10) * 5 = 10 unfavourable
        assert_eq!(summary, dec!(10));
    }

    #[test]
    fn create_adjustment_starts_pending_then_apply_completes() {
        let repo = fresh_repo();
        let adj = repo
            .create_adjustment(CreateCostAdjustment {
                sku: "ADJ-1".into(),
                adjustment_type: CostAdjustmentType::Revaluation,
                new_cost: dec!(20),
                reason: "year-end revaluation".into(),
                created_by: Some("alice".into()),
            })
            .expect("create adj");
        assert_eq!(adj.sku, "ADJ-1");

        let approved = repo.approve_adjustment(adj.id, "manager").expect("approve");
        assert_eq!(approved.id, adj.id);

        let applied = repo.apply_adjustment(adj.id).expect("apply");
        assert_eq!(applied.id, adj.id);
    }

    #[test]
    fn reject_adjustment_marks_rejected() {
        let repo = fresh_repo();
        let adj = repo
            .create_adjustment(CreateCostAdjustment {
                sku: "REJ-1".into(),
                adjustment_type: CostAdjustmentType::Revaluation,
                new_cost: dec!(99),
                reason: "wrong amount".into(),
                created_by: Some("alice".into()),
            })
            .expect("create adj");
        let rejected = repo.reject_adjustment(adj.id).expect("reject");
        assert_eq!(rejected.id, adj.id);
    }

    #[test]
    fn list_adjustments_filters_by_sku() {
        let repo = fresh_repo();
        repo.create_adjustment(CreateCostAdjustment {
            sku: "F-1".into(),
            adjustment_type: CostAdjustmentType::Revaluation,
            new_cost: dec!(5),
            reason: "r".into(),
            created_by: None,
        })
        .expect("a");
        repo.create_adjustment(CreateCostAdjustment {
            sku: "F-2".into(),
            adjustment_type: CostAdjustmentType::Revaluation,
            new_cost: dec!(5),
            reason: "r".into(),
            created_by: None,
        })
        .expect("b");

        let only_f1 = repo
            .list_adjustments(CostAdjustmentFilter {
                sku: Some("F-1".into()),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(only_f1.len(), 1);
    }

    #[test]
    fn get_total_inventory_value_zero_on_empty_db() {
        let repo = fresh_repo();
        assert_eq!(repo.get_total_inventory_value().expect("ok"), dec!(0));
    }

    #[test]
    fn get_inventory_valuation_uses_supplied_method() {
        let repo = fresh_repo();
        let v = repo.get_inventory_valuation(CostMethod::Average).expect("ok");
        assert_eq!(v.valuation_method, CostMethod::Average);
        assert_eq!(v.total_value, dec!(0));
    }

    #[test]
    fn get_sku_cost_summary_for_unknown_sku_is_none() {
        let repo = fresh_repo();
        assert!(repo.get_sku_cost_summary("NOPE").expect("ok").is_none());
    }
}
