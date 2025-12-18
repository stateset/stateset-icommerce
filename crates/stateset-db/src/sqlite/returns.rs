//! SQLite return repository implementation

use super::{map_db_error, parse_decimal};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreateReturn, ItemCondition, Result, Return, ReturnFilter, ReturnItem,
    ReturnReason, ReturnRepository, ReturnStatus, UpdateReturn,
};
use uuid::Uuid;

/// SQLite implementation of ReturnRepository
pub struct SqliteReturnRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteReturnRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_return(row: &rusqlite::Row) -> rusqlite::Result<Return> {
        Ok(Return {
            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
            order_id: row.get::<_, String>("order_id")?.parse().unwrap_or_default(),
            customer_id: row.get::<_, String>("customer_id")?.parse().unwrap_or_default(),
            status: parse_return_status(&row.get::<_, String>("status")?),
            reason: parse_return_reason(&row.get::<_, String>("reason")?),
            reason_details: row.get("reason_details")?,
            refund_amount: row.get::<_, Option<String>>("refund_amount")?.map(|s| parse_decimal(&s)),
            refund_method: row.get("refund_method")?,
            tracking_number: row.get("tracking_number")?,
            items: vec![], // Loaded separately
            notes: row.get("notes")?,
            version: row.get::<_, Option<i32>>("version")?.unwrap_or(1),
            created_at: row.get::<_, String>("created_at")?.parse().unwrap_or_else(|_| Utc::now()),
            updated_at: row.get::<_, String>("updated_at")?.parse().unwrap_or_else(|_| Utc::now()),
        })
    }

    fn load_return_items(&self, return_id: Uuid) -> Result<Vec<ReturnItem>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, return_id, order_item_id, sku, name, quantity, condition, refund_amount
                 FROM return_items WHERE return_id = ?",
            )
            .map_err(map_db_error)?;

        let items = stmt
            .query_map([return_id.to_string()], |row| {
                Ok(ReturnItem {
                    id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
                    return_id: row.get::<_, String>("return_id")?.parse().unwrap_or_default(),
                    order_item_id: row.get::<_, String>("order_item_id")?.parse().unwrap_or_default(),
                    sku: row.get("sku")?,
                    name: row.get("name")?,
                    quantity: row.get("quantity")?,
                    condition: parse_item_condition(&row.get::<_, String>("condition")?),
                    refund_amount: parse_decimal(&row.get::<_, String>("refund_amount")?),
                })
            })
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        Ok(items)
    }
}

impl ReturnRepository for SqliteReturnRepository {
    fn create(&self, input: CreateReturn) -> Result<Return> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_db_error)?;
        let id = Uuid::new_v4();
        let now = Utc::now();

        // Get order to get customer_id
        let customer_id: String = tx
            .query_row(
                "SELECT customer_id FROM orders WHERE id = ?",
                [input.order_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|_| CommerceError::OrderNotFound(input.order_id))?;

        tx.execute(
            "INSERT INTO returns (id, order_id, customer_id, status, reason, reason_details, notes, created_at, updated_at)
             VALUES (?, ?, ?, 'requested', ?, ?, ?, ?, ?)",
            rusqlite::params![
                id.to_string(),
                input.order_id.to_string(),
                customer_id,
                input.reason.to_string(),
                input.reason_details,
                input.notes,
                now.to_rfc3339(),
                now.to_rfc3339(),
            ],
        )
        .map_err(map_db_error)?;

        // Insert return items
        for item in &input.items {
            let item_id = Uuid::new_v4();

            // Get order item details
            let (sku, name, unit_price): (String, String, String) = tx
                .query_row(
                    "SELECT sku, name, unit_price FROM order_items WHERE id = ?",
                    [item.order_item_id.to_string()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(map_db_error)?;

            let refund_amount = parse_decimal(&unit_price) * Decimal::from(item.quantity);

            tx.execute(
                "INSERT INTO return_items (id, return_id, order_item_id, sku, name, quantity, condition, refund_amount)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                rusqlite::params![
                    item_id.to_string(),
                    id.to_string(),
                    item.order_item_id.to_string(),
                    sku,
                    name,
                    item.quantity,
                    item.condition.unwrap_or_default().to_string(),
                    refund_amount.to_string(),
                ],
            )
            .map_err(map_db_error)?;
        }

        // Calculate total refund amount
        let total_refund: f64 = tx
            .query_row(
                "SELECT COALESCE(SUM(CAST(refund_amount AS REAL)), 0) FROM return_items WHERE return_id = ?",
                [id.to_string()],
                |row| row.get(0),
            )
            .map_err(map_db_error)?;

        tx.execute(
            "UPDATE returns SET refund_amount = ? WHERE id = ?",
            rusqlite::params![total_refund.to_string(), id.to_string()],
        )
        .map_err(map_db_error)?;

        // Build the return with items using the same transaction.
        let mut ret = tx
            .query_row(
                "SELECT * FROM returns WHERE id = ?",
                [id.to_string()],
                Self::row_to_return,
            )
            .map_err(map_db_error)?;

        {
            let mut stmt = tx
                .prepare(
                    "SELECT id, return_id, order_item_id, sku, name, quantity, condition, refund_amount
                     FROM return_items WHERE return_id = ?",
                )
                .map_err(map_db_error)?;

            ret.items = stmt
                .query_map([id.to_string()], |row| {
                    Ok(ReturnItem {
                        id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
                        return_id: row.get::<_, String>("return_id")?.parse().unwrap_or_default(),
                        order_item_id: row.get::<_, String>("order_item_id")?
                            .parse()
                            .unwrap_or_default(),
                        sku: row.get("sku")?,
                        name: row.get("name")?,
                        quantity: row.get("quantity")?,
                        condition: parse_item_condition(&row.get::<_, String>("condition")?),
                        refund_amount: parse_decimal(&row.get::<_, String>("refund_amount")?),
                    })
                })
                .map_err(map_db_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_db_error)?;
        }

        tx.commit().map_err(map_db_error)?;

        Ok(ret)
    }

    fn get(&self, id: Uuid) -> Result<Option<Return>> {
        let conn = self.conn()?;
        let result = conn.query_row(
            "SELECT * FROM returns WHERE id = ?",
            [id.to_string()],
            Self::row_to_return,
        );

        match result {
            Ok(mut ret) => {
                // Inline load_return_items to use same connection
                let mut stmt = conn
                    .prepare(
                        "SELECT id, return_id, order_item_id, sku, name, quantity, condition, refund_amount
                         FROM return_items WHERE return_id = ?",
                    )
                    .map_err(map_db_error)?;

                ret.items = stmt
                    .query_map([id.to_string()], |row| {
                        Ok(ReturnItem {
                            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
                            return_id: row.get::<_, String>("return_id")?.parse().unwrap_or_default(),
                            order_item_id: row.get::<_, String>("order_item_id")?.parse().unwrap_or_default(),
                            sku: row.get("sku")?,
                            name: row.get("name")?,
                            quantity: row.get("quantity")?,
                            condition: parse_item_condition(&row.get::<_, String>("condition")?),
                            refund_amount: parse_decimal(&row.get::<_, String>("refund_amount")?),
                        })
                    })
                    .map_err(map_db_error)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(map_db_error)?;

                Ok(Some(ret))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update(&self, id: Uuid, input: UpdateReturn) -> Result<Return> {
        let conn = self.conn()?;
        let now = Utc::now();

        let mut updates = vec!["updated_at = ?"];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now.to_rfc3339())];

        if let Some(status) = &input.status {
            updates.push("status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(tracking) = &input.tracking_number {
            updates.push("tracking_number = ?");
            params.push(Box::new(tracking.clone()));
        }
        if let Some(amount) = &input.refund_amount {
            updates.push("refund_amount = ?");
            params.push(Box::new(amount.to_string()));
        }
        if let Some(method) = &input.refund_method {
            updates.push("refund_method = ?");
            params.push(Box::new(method.clone()));
        }
        if let Some(notes) = &input.notes {
            updates.push("notes = ?");
            params.push(Box::new(notes.clone()));
        }

        params.push(Box::new(id.to_string()));

        let sql = format!("UPDATE returns SET {} WHERE id = ?", updates.join(", "));
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        conn.execute(&sql, params_refs.as_slice())
            .map_err(map_db_error)?;

        // Inline the get logic to avoid connection pool deadlock
        let result = conn.query_row(
            "SELECT * FROM returns WHERE id = ?",
            [id.to_string()],
            Self::row_to_return,
        );

        match result {
            Ok(mut ret) => {
                // Inline load_return_items to use same connection
                let mut stmt = conn
                    .prepare(
                        "SELECT id, return_id, order_item_id, sku, name, quantity, condition, refund_amount
                         FROM return_items WHERE return_id = ?",
                    )
                    .map_err(map_db_error)?;

                ret.items = stmt
                    .query_map([id.to_string()], |row| {
                        Ok(ReturnItem {
                            id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
                            return_id: row.get::<_, String>("return_id")?.parse().unwrap_or_default(),
                            order_item_id: row.get::<_, String>("order_item_id")?.parse().unwrap_or_default(),
                            sku: row.get("sku")?,
                            name: row.get("name")?,
                            quantity: row.get("quantity")?,
                            condition: parse_item_condition(&row.get::<_, String>("condition")?),
                            refund_amount: parse_decimal(&row.get::<_, String>("refund_amount")?),
                        })
                    })
                    .map_err(map_db_error)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(map_db_error)?;

                Ok(ret)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Err(CommerceError::ReturnNotFound(id)),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: ReturnFilter) -> Result<Vec<Return>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM returns WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(order_id) = &filter.order_id {
            sql.push_str(" AND order_id = ?");
            params.push(Box::new(order_id.to_string()));
        }
        if let Some(customer_id) = &filter.customer_id {
            sql.push_str(" AND customer_id = ?");
            params.push(Box::new(customer_id.to_string()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(reason) = &filter.reason {
            sql.push_str(" AND reason = ?");
            params.push(Box::new(reason.to_string()));
        }
        if let Some(from) = &filter.from_date {
            sql.push_str(" AND created_at >= ?");
            params.push(Box::new(from.to_rfc3339()));
        }
        if let Some(to) = &filter.to_date {
            sql.push_str(" AND created_at <= ?");
            params.push(Box::new(to.to_rfc3339()));
        }

        sql.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;

        let returns = stmt
            .query_map(params_refs.as_slice(), Self::row_to_return)
            .map_err(map_db_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(map_db_error)?;

        // Load items for each return using same connection
        let mut result = vec![];
        for mut ret in returns {
            let mut item_stmt = conn
                .prepare(
                    "SELECT id, return_id, order_item_id, sku, name, quantity, condition, refund_amount
                     FROM return_items WHERE return_id = ?",
                )
                .map_err(map_db_error)?;

            ret.items = item_stmt
                .query_map([ret.id.to_string()], |row| {
                    Ok(ReturnItem {
                        id: row.get::<_, String>("id")?.parse().unwrap_or_default(),
                        return_id: row.get::<_, String>("return_id")?.parse().unwrap_or_default(),
                        order_item_id: row.get::<_, String>("order_item_id")?.parse().unwrap_or_default(),
                        sku: row.get("sku")?,
                        name: row.get("name")?,
                        quantity: row.get("quantity")?,
                        condition: parse_item_condition(&row.get::<_, String>("condition")?),
                        refund_amount: parse_decimal(&row.get::<_, String>("refund_amount")?),
                    })
                })
                .map_err(map_db_error)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(map_db_error)?;

            result.push(ret);
        }

        Ok(result)
    }

    fn approve(&self, id: Uuid) -> Result<Return> {
        let ret = self.get(id)?.ok_or(CommerceError::ReturnNotFound(id))?;

        if ret.status != ReturnStatus::Requested {
            return Err(CommerceError::ReturnCannotBeApproved(ret.status.to_string()));
        }

        self.update(id, UpdateReturn {
            status: Some(ReturnStatus::Approved),
            ..Default::default()
        })
    }

    fn reject(&self, id: Uuid, reason: &str) -> Result<Return> {
        let ret = self.get(id)?.ok_or(CommerceError::ReturnNotFound(id))?;

        if ret.status != ReturnStatus::Requested {
            return Err(CommerceError::ReturnCannotBeApproved(ret.status.to_string()));
        }

        self.update(id, UpdateReturn {
            status: Some(ReturnStatus::Rejected),
            notes: Some(reason.to_string()),
            ..Default::default()
        })
    }

    fn complete(&self, id: Uuid) -> Result<Return> {
        let ret = self.get(id)?.ok_or(CommerceError::ReturnNotFound(id))?;

        if !ret.can_complete() {
            return Err(CommerceError::NotPermitted(format!(
                "Return cannot be completed in status: {}",
                ret.status
            )));
        }

        self.update(id, UpdateReturn {
            status: Some(ReturnStatus::Completed),
            ..Default::default()
        })
    }

    fn cancel(&self, id: Uuid) -> Result<Return> {
        self.update(id, UpdateReturn {
            status: Some(ReturnStatus::Cancelled),
            ..Default::default()
        })
    }

    fn count(&self, filter: ReturnFilter) -> Result<u64> {
        let conn = self.conn()?;
        let mut sql = "SELECT COUNT(*) FROM returns WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let count: i64 = conn
            .query_row(&sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(map_db_error)?;

        Ok(count as u64)
    }
}

fn parse_return_status(s: &str) -> ReturnStatus {
    match s {
        "requested" => ReturnStatus::Requested,
        "approved" => ReturnStatus::Approved,
        "rejected" => ReturnStatus::Rejected,
        "in_transit" | "intransit" => ReturnStatus::InTransit,
        "received" => ReturnStatus::Received,
        "inspecting" => ReturnStatus::Inspecting,
        "completed" => ReturnStatus::Completed,
        "cancelled" => ReturnStatus::Cancelled,
        _ => ReturnStatus::Requested,
    }
}

fn parse_return_reason(s: &str) -> ReturnReason {
    match s {
        "defective" => ReturnReason::Defective,
        "wrong_item" | "wrongitem" => ReturnReason::WrongItem,
        "not_as_described" | "notasdescribed" => ReturnReason::NotAsDescribed,
        "changed_mind" | "changedmind" => ReturnReason::ChangedMind,
        "better_price_found" | "betterpricefound" => ReturnReason::BetterPriceFound,
        "no_longer_needed" | "nolongerneeded" => ReturnReason::NoLongerNeeded,
        "damaged" => ReturnReason::Damaged,
        "other" => ReturnReason::Other,
        _ => ReturnReason::Other,
    }
}

fn parse_item_condition(s: &str) -> ItemCondition {
    match s {
        "new" => ItemCondition::New,
        "opened" => ItemCondition::Opened,
        "used" => ItemCondition::Used,
        "damaged" => ItemCondition::Damaged,
        "defective" => ItemCondition::Defective,
        _ => ItemCondition::New,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_return_status_snake_case_and_legacy() {
        assert_eq!(parse_return_status("in_transit"), ReturnStatus::InTransit);
        assert_eq!(parse_return_status("intransit"), ReturnStatus::InTransit);
    }

    #[test]
    fn parses_return_reason_snake_case_and_legacy() {
        assert_eq!(parse_return_reason("wrong_item"), ReturnReason::WrongItem);
        assert_eq!(parse_return_reason("wrongitem"), ReturnReason::WrongItem);
        assert_eq!(
            parse_return_reason("not_as_described"),
            ReturnReason::NotAsDescribed
        );
        assert_eq!(parse_return_reason("notasdescribed"), ReturnReason::NotAsDescribed);
        assert_eq!(parse_return_reason("no_longer_needed"), ReturnReason::NoLongerNeeded);
        assert_eq!(parse_return_reason("nolongerneeded"), ReturnReason::NoLongerNeeded);
    }
}
