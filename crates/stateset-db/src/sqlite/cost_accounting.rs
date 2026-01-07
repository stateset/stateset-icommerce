//! SQLite implementation of cost accounting repository

use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CostAccountingRepository, CostAdjustment, CostAdjustmentFilter,
    CostAdjustmentStatus, CostLayer, CostLayerFilter,
    CostMethod, CostRollup, CostTransaction, CostTransactionFilter, CostTransactionType,
    CostVariance, CostVarianceFilter, CreateCostAdjustment, CreateCostLayer, InventoryValuation,
    IssueCostLayers, ItemCost, ItemCostFilter, RecordCostVariance, Result, SetItemCost,
    SkuCostSummary, generate_cost_adjustment_number,
};
use uuid::Uuid;

use super::{map_db_error, parse_decimal};

pub struct SqliteCostAccountingRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteCostAccountingRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn row_to_item_cost(&self, row: &rusqlite::Row) -> rusqlite::Result<ItemCost> {
        Ok(ItemCost {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            sku: row.get(1)?,
            cost_method: row.get::<_, String>(2)?.parse().unwrap_or_default(),
            standard_cost: parse_decimal(&row.get::<_, String>(3)?),
            average_cost: parse_decimal(&row.get::<_, String>(4)?),
            last_cost: parse_decimal(&row.get::<_, String>(5)?),
            material_cost: parse_decimal(&row.get::<_, String>(6)?),
            labor_cost: parse_decimal(&row.get::<_, String>(7)?),
            overhead_cost: parse_decimal(&row.get::<_, String>(8)?),
            currency: row.get(9)?,
            effective_date: row.get::<_, String>(10)?.parse().unwrap_or_else(|_| Utc::now()),
            created_at: row.get::<_, String>(11)?.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: row.get::<_, String>(12)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn row_to_cost_layer(&self, row: &rusqlite::Row) -> rusqlite::Result<CostLayer> {
        Ok(CostLayer {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            sku: row.get(1)?,
            layer_date: row.get::<_, String>(2)?.parse().unwrap_or_else(|_| Utc::now()),
            quantity: parse_decimal(&row.get::<_, String>(3)?),
            remaining_quantity: parse_decimal(&row.get::<_, String>(4)?),
            unit_cost: parse_decimal(&row.get::<_, String>(5)?),
            total_cost: parse_decimal(&row.get::<_, String>(6)?),
            source_type: row.get::<_, String>(7)?.parse().unwrap_or_default(),
            source_id: row.get::<_, Option<String>>(8)?.and_then(|s| s.parse().ok()),
            lot_id: row.get::<_, Option<String>>(9)?.and_then(|s| s.parse().ok()),
            location_id: row.get(10)?,
            created_at: row.get::<_, String>(11)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn row_to_cost_transaction(&self, row: &rusqlite::Row) -> rusqlite::Result<CostTransaction> {
        Ok(CostTransaction {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            sku: row.get(1)?,
            transaction_type: row.get::<_, String>(2)?.parse().unwrap_or_default(),
            quantity: parse_decimal(&row.get::<_, String>(3)?),
            unit_cost: parse_decimal(&row.get::<_, String>(4)?),
            total_cost: parse_decimal(&row.get::<_, String>(5)?),
            layer_id: row.get::<_, Option<String>>(6)?.and_then(|s| s.parse().ok()),
            reference_type: row.get(7)?,
            reference_id: row.get::<_, Option<String>>(8)?.and_then(|s| s.parse().ok()),
            notes: row.get(9)?,
            created_at: row.get::<_, String>(10)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn row_to_cost_variance(&self, row: &rusqlite::Row) -> rusqlite::Result<CostVariance> {
        Ok(CostVariance {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            sku: row.get(1)?,
            variance_type: row.get::<_, String>(2)?.parse().unwrap_or_default(),
            variance_date: row.get::<_, String>(3)?.parse().unwrap_or_else(|_| Utc::now()),
            standard_cost: parse_decimal(&row.get::<_, String>(4)?),
            actual_cost: parse_decimal(&row.get::<_, String>(5)?),
            variance_amount: parse_decimal(&row.get::<_, String>(6)?),
            variance_percent: parse_decimal(&row.get::<_, String>(7)?),
            quantity: parse_decimal(&row.get::<_, String>(8)?),
            total_variance: parse_decimal(&row.get::<_, String>(9)?),
            reference_type: row.get(10)?,
            reference_id: row.get::<_, Option<String>>(11)?.and_then(|s| s.parse().ok()),
            notes: row.get(12)?,
            created_at: row.get::<_, String>(13)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn row_to_cost_adjustment(&self, row: &rusqlite::Row) -> rusqlite::Result<CostAdjustment> {
        Ok(CostAdjustment {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            adjustment_number: row.get(1)?,
            sku: row.get(2)?,
            adjustment_type: row.get::<_, String>(3)?.parse().unwrap_or_default(),
            previous_cost: parse_decimal(&row.get::<_, String>(4)?),
            new_cost: parse_decimal(&row.get::<_, String>(5)?),
            adjustment_amount: parse_decimal(&row.get::<_, String>(6)?),
            reason: row.get(7)?,
            approved_by: row.get(8)?,
            approved_at: row.get::<_, Option<String>>(9)?.and_then(|s| s.parse().ok()),
            status: row.get::<_, String>(10)?.parse().unwrap_or_default(),
            created_by: row.get(11)?,
            created_at: row.get::<_, String>(12)?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn row_to_cost_rollup(&self, row: &rusqlite::Row) -> rusqlite::Result<CostRollup> {
        Ok(CostRollup {
            id: row.get::<_, String>(0)?.parse().unwrap_or_default(),
            sku: row.get(1)?,
            bom_id: row.get::<_, Option<String>>(2)?.and_then(|s| s.parse().ok()),
            rollup_date: row.get::<_, String>(3)?.parse().unwrap_or_else(|_| Utc::now()),
            material_cost: parse_decimal(&row.get::<_, String>(4)?),
            labor_cost: parse_decimal(&row.get::<_, String>(5)?),
            overhead_cost: parse_decimal(&row.get::<_, String>(6)?),
            total_cost: parse_decimal(&row.get::<_, String>(7)?),
            previous_cost: parse_decimal(&row.get::<_, String>(8)?),
            cost_change: parse_decimal(&row.get::<_, String>(9)?),
            created_at: row.get::<_, String>(10)?.parse().unwrap_or_else(|_| Utc::now()),
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
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        // Check if exists
        let existing = self.get_item_cost(&input.sku)?;

        if let Some(_existing) = existing {
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
                    input.cost_method.map(|m| m.to_string()),
                    input.standard_cost.map(|c| c.to_string()),
                    input.material_cost.map(|c| c.to_string()),
                    input.labor_cost.map(|c| c.to_string()),
                    input.overhead_cost.map(|c| c.to_string()),
                    input.currency,
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    &input.sku,
                ],
            ).map_err(map_db_error)?;

            self.get_item_cost(&input.sku)?.ok_or(CommerceError::NotFound)
        } else {
            // Insert new
            let id = Uuid::new_v4();
            let cost_method = input.cost_method.unwrap_or_default();
            let standard_cost = input.standard_cost.unwrap_or_default();
            let material_cost = input.material_cost.unwrap_or_default();
            let labor_cost = input.labor_cost.unwrap_or_default();
            let overhead_cost = input.overhead_cost.unwrap_or_default();
            let currency = input.currency.unwrap_or_else(|| "USD".to_string());

            conn.execute(
                "INSERT INTO item_costs (id, sku, cost_method, standard_cost, average_cost, last_cost,
                    material_cost, labor_cost, overhead_cost, currency, effective_date, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    id.to_string(),
                    &input.sku,
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

            self.get_item_cost(&input.sku)?.ok_or(CommerceError::NotFound)
        }
    }

    fn list_item_costs(&self, filter: ItemCostFilter) -> Result<Vec<ItemCost>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, sku, cost_method, standard_cost, average_cost, last_cost,
                    material_cost, labor_cost, overhead_cost, currency, effective_date,
                    created_at, updated_at
             FROM item_costs WHERE 1=1"
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
        let rows = stmt.query_map(param_refs.as_slice(), |row| self.row_to_item_cost(row))
            .map_err(map_db_error)?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(map_db_error)?);
        }
        Ok(items)
    }

    fn update_average_cost(&self, sku: &str, quantity: Decimal, unit_cost: Decimal) -> Result<ItemCost> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
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

        // Calculate new weighted average
        // Get current quantity from inventory
        let (current_qty, current_avg): (Decimal, Decimal) = conn.query_row(
            "SELECT COALESCE(
                (SELECT SUM(CAST(quantity_on_hand AS REAL)) FROM inventory_items WHERE sku = ?), 0),
             COALESCE(average_cost, '0')
             FROM item_costs WHERE sku = ?",
            [sku, sku],
            |row| {
                let qty_str: String = row.get(0)?;
                let avg_str: String = row.get(1)?;
                Ok((parse_decimal(&qty_str), parse_decimal(&avg_str)))
            },
        ).unwrap_or((Decimal::ZERO, Decimal::ZERO));

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

        self.get_item_cost(sku)?.ok_or(CommerceError::NotFound)
    }

    fn update_last_cost(&self, sku: &str, unit_cost: Decimal) -> Result<ItemCost> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = Utc::now();

        conn.execute(
            "UPDATE item_costs SET last_cost = ?, updated_at = ? WHERE sku = ?",
            rusqlite::params![unit_cost.to_string(), now.to_rfc3339(), sku],
        ).map_err(map_db_error)?;

        self.get_item_cost(sku)?.ok_or(CommerceError::NotFound)
    }

    fn create_cost_layer(&self, input: CreateCostLayer) -> Result<CostLayer> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let total_cost = input.quantity * input.unit_cost;

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
        ).map_err(map_db_error)?;

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
             FROM cost_layers WHERE 1=1"
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
        let rows = stmt.query_map(param_refs.as_slice(), |row| self.row_to_cost_layer(row))
            .map_err(map_db_error)?;

        let mut layers = Vec::new();
        for row in rows {
            layers.push(row.map_err(map_db_error)?);
        }
        Ok(layers)
    }

    fn issue_fifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut remaining = input.quantity;
        let mut transactions = Vec::new();

        // Get layers in FIFO order (oldest first)
        let layers = self.list_cost_layers(CostLayerFilter {
            sku: Some(input.sku.clone()),
            has_remaining: Some(true),
            ..Default::default()
        })?;

        for layer in layers {
            if remaining <= Decimal::ZERO {
                break;
            }

            let consume_qty = remaining.min(layer.remaining_quantity);
            let new_remaining = layer.remaining_quantity - consume_qty;

            // Update layer
            conn.execute(
                "UPDATE cost_layers SET remaining_quantity = ? WHERE id = ?",
                [&new_remaining.to_string(), &layer.id.to_string()],
            ).map_err(map_db_error)?;

            // Record transaction
            let tx = self.record_cost_transaction(
                &input.sku,
                CostTransactionType::Issue,
                consume_qty,
                layer.unit_cost,
                Some(layer.id),
                input.reference_type.as_deref(),
                input.reference_id,
                input.notes.as_deref(),
            )?;
            transactions.push(tx);

            remaining -= consume_qty;
        }

        Ok(transactions)
    }

    fn issue_lifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut remaining = input.quantity;
        let mut transactions = Vec::new();

        // Get layers in LIFO order (newest first)
        let mut layers = self.list_cost_layers(CostLayerFilter {
            sku: Some(input.sku.clone()),
            has_remaining: Some(true),
            ..Default::default()
        })?;
        layers.reverse();

        for layer in layers {
            if remaining <= Decimal::ZERO {
                break;
            }

            let consume_qty = remaining.min(layer.remaining_quantity);
            let new_remaining = layer.remaining_quantity - consume_qty;

            // Update layer
            conn.execute(
                "UPDATE cost_layers SET remaining_quantity = ? WHERE id = ?",
                [&new_remaining.to_string(), &layer.id.to_string()],
            ).map_err(map_db_error)?;

            // Record transaction
            let tx = self.record_cost_transaction(
                &input.sku,
                CostTransactionType::Issue,
                consume_qty,
                layer.unit_cost,
                Some(layer.id),
                input.reference_type.as_deref(),
                input.reference_id,
                input.notes.as_deref(),
            )?;
            transactions.push(tx);

            remaining -= consume_qty;
        }

        Ok(transactions)
    }

    fn get_layers_remaining(&self, sku: &str) -> Result<Decimal> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result: String = conn.query_row(
            "SELECT COALESCE(SUM(CAST(remaining_quantity AS REAL)), '0') FROM cost_layers WHERE sku = ?",
            [sku],
            |row| row.get(0),
        ).map_err(map_db_error)?;

        Ok(parse_decimal(&result))
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
        ).map_err(map_db_error)?;

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

    fn list_cost_transactions(&self, filter: CostTransactionFilter) -> Result<Vec<CostTransaction>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut sql = String::from(
            "SELECT id, sku, transaction_type, quantity, unit_cost, total_cost,
                    layer_id, reference_type, reference_id, notes, created_at
             FROM cost_transactions WHERE 1=1"
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
        let rows = stmt.query_map(param_refs.as_slice(), |row| self.row_to_cost_transaction(row))
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
        ).map_err(map_db_error)?;

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
             FROM cost_variances WHERE 1=1"
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
        let rows = stmt.query_map(param_refs.as_slice(), |row| self.row_to_cost_variance(row))
            .map_err(map_db_error)?;

        let mut variances = Vec::new();
        for row in rows {
            variances.push(row.map_err(map_db_error)?);
        }
        Ok(variances)
    }

    fn get_variance_summary(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Decimal> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let result: String = conn.query_row(
            "SELECT COALESCE(SUM(CAST(total_variance AS REAL)), '0')
             FROM cost_variances WHERE variance_date BETWEEN ? AND ?",
            [from.to_rfc3339(), to.to_rfc3339()],
            |row| row.get(0),
        ).map_err(map_db_error)?;

        Ok(parse_decimal(&result))
    }

    fn create_adjustment(&self, input: CreateCostAdjustment) -> Result<CostAdjustment> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let adjustment_number = generate_cost_adjustment_number();

        // Get current cost
        let current_cost = self.get_item_cost(&input.sku)?
            .map(|c| c.standard_cost)
            .unwrap_or_default();
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
        ).map_err(map_db_error)?;

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
        let rows = stmt.query_map(param_refs.as_slice(), |row| self.row_to_cost_adjustment(row))
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
        ).map_err(map_db_error)?;

        self.get_adjustment(id)?.ok_or(CommerceError::NotFound)
    }

    fn apply_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        let adjustment = self.get_adjustment(id)?.ok_or(CommerceError::NotFound)?;

        if adjustment.status != CostAdjustmentStatus::Approved {
            return Err(CommerceError::ValidationError("Adjustment must be approved before applying".into()));
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
        ).map_err(map_db_error)?;

        self.get_adjustment(id)?.ok_or(CommerceError::NotFound)
    }

    fn reject_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        conn.execute(
            "UPDATE cost_adjustments SET status = ? WHERE id = ?",
            [CostAdjustmentStatus::Rejected.to_string(), id.to_string()],
        ).map_err(map_db_error)?;

        self.get_adjustment(id)?.ok_or(CommerceError::NotFound)
    }

    fn calculate_rollup(&self, sku: &str, bom_id: Option<Uuid>) -> Result<CostRollup> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Get previous cost
        let previous_cost = self.get_rollup(sku)?
            .map(|r| r.total_cost)
            .unwrap_or_default();

        // Calculate from BOM components if bom_id provided
        let (material_cost, labor_cost, overhead_cost) = if let Some(bom_id) = bom_id {
            // Sum component costs
            let material: String = conn.query_row(
                "SELECT COALESCE(SUM(CAST(bc.quantity AS REAL) * COALESCE(CAST(ic.standard_cost AS REAL), 0)), '0')
                 FROM bom_components bc
                 LEFT JOIN item_costs ic ON bc.component_sku = ic.sku
                 WHERE bc.bom_id = ?",
                [bom_id.to_string()],
                |row| row.get(0),
            ).unwrap_or_else(|_| "0".to_string());
            (parse_decimal(&material), Decimal::ZERO, Decimal::ZERO)
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
        ).map_err(map_db_error)?;

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

        let cost_field = match cost_method {
            CostMethod::Standard => "COALESCE(ic.standard_cost, '0')",
            CostMethod::Average => "COALESCE(ic.average_cost, '0')",
            CostMethod::Fifo | CostMethod::Lifo => "COALESCE(ic.average_cost, '0')",
            CostMethod::Specific => "COALESCE(ic.last_cost, '0')",
        };

        let (total_qty, total_val): (String, String) = conn.query_row(
            &format!(
                "SELECT
                    COALESCE(SUM(CAST(ii.quantity_on_hand AS REAL)), '0'),
                    COALESCE(SUM(CAST(ii.quantity_on_hand AS REAL) * CAST({} AS REAL)), '0')
                 FROM inventory_items ii
                 LEFT JOIN item_costs ic ON ii.sku = ic.sku",
                cost_field
            ),
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap_or(("0".to_string(), "0".to_string()));

        let total_quantity = parse_decimal(&total_qty);
        let total_value = parse_decimal(&total_val);
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
                COALESCE(ii.quantity_on_hand, '0'),
                COALESCE(ic.standard_cost, '0'),
                COALESCE(ic.average_cost, '0'),
                COALESCE(CAST(ii.quantity_on_hand AS REAL) * CAST(ic.average_cost AS REAL), '0'),
                COALESCE((SELECT SUM(CAST(total_variance AS REAL)) FROM cost_variances
                          WHERE sku = ii.sku AND strftime('%Y', variance_date) = strftime('%Y', 'now')), '0')
             FROM inventory_items ii
             LEFT JOIN item_costs ic ON ii.sku = ic.sku
             WHERE ii.sku = ?",
            [sku],
            |row| {
                Ok(SkuCostSummary {
                    sku: row.get(0)?,
                    quantity_on_hand: parse_decimal(&row.get::<_, String>(1)?),
                    standard_cost: parse_decimal(&row.get::<_, String>(2)?),
                    average_cost: parse_decimal(&row.get::<_, String>(3)?),
                    total_value: parse_decimal(&row.get::<_, String>(4)?),
                    variance_ytd: parse_decimal(&row.get::<_, String>(5)?),
                })
            },
        );

        match result {
            Ok(summary) => Ok(Some(summary)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn get_total_inventory_value(&self) -> Result<Decimal> {
        let valuation = self.get_inventory_valuation(CostMethod::Average)?;
        Ok(valuation.total_value)
    }
}
