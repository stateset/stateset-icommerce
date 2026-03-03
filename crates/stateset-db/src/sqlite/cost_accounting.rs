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
};

#[derive(Debug)]
pub struct SqliteCostAccountingRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCostAccountingRepository {
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

        // Check if exists
        let existing = self.get_item_cost(&sku)?;

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            if existing.is_some() {
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
                        cost_method.as_ref().map(|m| m.to_string()),
                        standard_cost.as_ref().map(|c| c.to_string()),
                        material_cost.as_ref().map(|c| c.to_string()),
                        labor_cost.as_ref().map(|c| c.to_string()),
                        overhead_cost.as_ref().map(|c| c.to_string()),
                        currency,
                        now.to_rfc3339(),
                        now.to_rfc3339(),
                        &sku,
                    ],
                )
                .map_err(map_db_error)?;
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
                ).map_err(map_db_error)?;
            }
        }

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
            params.push(Box::new(format!("%{}%", sku)));
        }
        if let Some(ref method) = filter.cost_method {
            sql.push_str(" AND cost_method = ?");
            params.push(Box::new(method.to_string()));
        }

        sql.push_str(" ORDER BY sku");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
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

        {
            let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
            // Calculate new weighted average
            // Get current quantity from inventory
            let sku_param = sku.to_string();
            let sku_params: [&dyn rusqlite::ToSql; 1] = [&sku_param];
            let current_qty = sum_decimal_query(
                &conn,
                "SELECT quantity_on_hand FROM inventory_items WHERE sku = ?",
                &sku_params,
                "inventory_items",
                "quantity_on_hand",
            )?;
            let avg_str: String = conn
                .query_row(
                    "SELECT COALESCE(average_cost, '0') FROM item_costs WHERE sku = ?",
                    [sku],
                    |row| row.get(0),
                )
                .map_err(map_db_error)?;
            let current_avg = parse_decimal_strict(&avg_str, "item_cost", "average_cost")?;

            let total_qty = current_qty + quantity;
            let new_avg = if total_qty > Decimal::ZERO {
                ((current_avg * current_qty) + (unit_cost * quantity)) / total_qty
            } else {
                unit_cost
            };

            conn.execute(
                "UPDATE item_costs SET average_cost = ?, last_cost = ?, updated_at = ? WHERE sku = ?",
                rusqlite::params![
                    new_avg.to_string(),
                    unit_cost.to_string(),
                    now.to_rfc3339(),
                    sku,
                ],
            ).map_err(map_db_error)?;
        }

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
        if filter.has_remaining == Some(true) {
            sql.push_str(" AND CAST(remaining_quantity AS REAL) > 0");
        }

        sql.push_str(" ORDER BY layer_date ASC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), |row| self.row_to_cost_layer(row))
            .map_err(map_db_error)?;

        let mut layers = Vec::new();
        for row in rows {
            layers.push(row.map_err(map_db_error)?);
        }
        Ok(layers)
    }

    fn issue_fifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        let mut conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut remaining = input.quantity;
        let mut transactions = Vec::new();

        // Get layers in FIFO order (oldest first) from the same transaction snapshot.
        let layers: Vec<CostLayer> = {
            let mut stmt = tx
                .prepare(
                    "SELECT id, sku, layer_date, quantity, remaining_quantity, unit_cost, total_cost,
                            source_type, source_id, lot_id, location_id, created_at
                     FROM cost_layers
                     WHERE sku = ? AND CAST(remaining_quantity AS REAL) > 0
                     ORDER BY layer_date ASC",
                )
                .map_err(map_db_error)?;
            let rows = stmt
                .query_map([&input.sku], |row| self.row_to_cost_layer(row))
                .map_err(map_db_error)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)?
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
        let tx = conn.transaction().map_err(map_db_error)?;
        let mut remaining = input.quantity;
        let mut transactions = Vec::new();

        // Get layers in LIFO order (newest first) from the same transaction snapshot.
        let layers: Vec<CostLayer> = {
            let mut stmt = tx
                .prepare(
                    "SELECT id, sku, layer_date, quantity, remaining_quantity, unit_cost, total_cost,
                            source_type, source_id, lot_id, location_id, created_at
                     FROM cost_layers
                     WHERE sku = ? AND CAST(remaining_quantity AS REAL) > 0
                     ORDER BY layer_date DESC",
                )
                .map_err(map_db_error)?;
            let rows = stmt
                .query_map([&input.sku], |row| self.row_to_cost_layer(row))
                .map_err(map_db_error)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_db_error)?
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
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
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
        let variance_percent = if input.standard_cost != Decimal::ZERO {
            (variance_amount / input.standard_cost) * Decimal::from(100)
        } else {
            Decimal::ZERO
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
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
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
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
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

        conn.execute(
            "UPDATE cost_adjustments SET status = ?, approved_by = ?, approved_at = ? WHERE id = ?",
            rusqlite::params![
                CostAdjustmentStatus::Approved.to_string(),
                approved_by,
                now.to_rfc3339(),
                id.to_string(),
            ],
        )
        .map_err(map_db_error)?;

        self.get_adjustment(id)?.ok_or(CommerceError::NotFound)
    }

    fn apply_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let adjustment = self.get_adjustment(id)?.ok_or(CommerceError::NotFound)?;

        if adjustment.status != CostAdjustmentStatus::Approved {
            return Err(CommerceError::ValidationError(
                "Adjustment must be approved before applying".into(),
            ));
        }

        // Update item cost
        self.set_item_cost(SetItemCost {
            sku: adjustment.sku.clone(),
            standard_cost: Some(adjustment.new_cost),
            ..Default::default()
        })?;

        // Update status
        conn.execute(
            "UPDATE cost_adjustments SET status = ? WHERE id = ?",
            [CostAdjustmentStatus::Applied.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        self.get_adjustment(id)?.ok_or(CommerceError::NotFound)
    }

    fn reject_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE cost_adjustments SET status = ? WHERE id = ?",
            [CostAdjustmentStatus::Rejected.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

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

        let mut stmt = conn
            .prepare(
                "SELECT ii.quantity_on_hand, ic.standard_cost, ic.average_cost, ic.last_cost
                 FROM inventory_items ii
                 LEFT JOIN item_costs ic ON ii.sku = ic.sku",
            )
            .map_err(map_db_error)?;
        let mut rows = stmt.query([]).map_err(map_db_error)?;

        let mut total_quantity = Decimal::ZERO;
        let mut total_value = Decimal::ZERO;

        while let Some(row) = rows.next().map_err(map_db_error)? {
            let qty_raw: Option<String> = row.get(0).map_err(map_db_error)?;
            let quantity = match qty_raw {
                Some(value) if !value.is_empty() => {
                    parse_decimal_strict(&value, "inventory_items", "quantity_on_hand")?
                }
                _ => Decimal::ZERO,
            };

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

        let result = conn.query_row(
            "SELECT
                ii.sku,
                ii.quantity_on_hand,
                ic.standard_cost,
                ic.average_cost
             FROM inventory_items ii
             LEFT JOIN item_costs ic ON ii.sku = ic.sku
             WHERE ii.sku = ?",
            [sku],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        );

        let (sku_value, qty_raw, standard_raw, average_raw) = match result {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(map_db_error(e)),
        };

        let quantity_on_hand = match qty_raw {
            Some(value) if !value.is_empty() => {
                parse_decimal_strict(&value, "sku_cost_summary", "quantity_on_hand")?
            }
            _ => Decimal::ZERO,
        };
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
