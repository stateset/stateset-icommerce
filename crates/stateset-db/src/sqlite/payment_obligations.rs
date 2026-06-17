//! SQLite implementation of the payment obligation repository

use super::{
    map_db_error, parse_date_row, parse_datetime_row, parse_decimal_row, parse_enum_row,
    parse_json_row, parse_uuid_opt_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::{NaiveDate, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    CommerceError, CreatePaymentObligation, CurrencyCode, PaymentObligation,
    PaymentObligationDashboard, PaymentObligationFilter, PaymentObligationId,
    PaymentObligationRepository, PaymentObligationStatus, Result,
};

#[derive(Debug)]
pub struct SqlitePaymentObligationRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePaymentObligationRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_obligation(row: &rusqlite::Row<'_>) -> rusqlite::Result<PaymentObligation> {
        let linked_json: String = row.get("linked_bill_ids")?;
        Ok(PaymentObligation {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "payment_obligation", "id")?.into(),
            number: row.get("number")?,
            supplier_id: parse_uuid_row(
                &row.get::<_, String>("supplier_id")?,
                "payment_obligation",
                "supplier_id",
            )?,
            purchase_order_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("purchase_order_id")?,
                "payment_obligation",
                "purchase_order_id",
            )?,
            amount: parse_decimal_row(
                &row.get::<_, String>("amount")?,
                "payment_obligation",
                "amount",
            )?,
            amount_paid: parse_decimal_row(
                &row.get::<_, String>("amount_paid")?,
                "payment_obligation",
                "amount_paid",
            )?,
            currency: parse_enum_row::<CurrencyCode>(
                &row.get::<_, String>("currency")?,
                "payment_obligation",
                "currency",
            )?,
            due_date: parse_date_row(
                &row.get::<_, String>("due_date")?,
                "payment_obligation",
                "due_date",
            )?,
            status: parse_enum_row::<PaymentObligationStatus>(
                &row.get::<_, String>("status")?,
                "payment_obligation",
                "status",
            )?,
            linked_bill_ids: parse_json_row(&linked_json, "payment_obligation", "linked_bill_ids")?,
            notes: row.get("notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "payment_obligation",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "payment_obligation",
                "updated_at",
            )?,
        })
    }

    fn fetch(tx: &rusqlite::Connection, id: &str) -> rusqlite::Result<PaymentObligation> {
        tx.query_row(
            "SELECT * FROM payment_obligations WHERE id = ?",
            [id],
            Self::row_to_obligation,
        )
    }

    fn json_err(e: serde_json::Error) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::DatabaseError(
            e.to_string(),
        )))
    }
}

impl PaymentObligationRepository for SqlitePaymentObligationRepository {
    fn create(&self, input: CreatePaymentObligation) -> Result<PaymentObligation> {
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "payment obligation amount must be positive".into(),
            ));
        }
        let id = PaymentObligationId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let number = format!("OBL-{}", &id_str[..8]);
        let currency = input.currency.unwrap_or(CurrencyCode::USD);
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO payment_obligations (id, number, supplier_id, purchase_order_id, amount, amount_paid, currency, due_date, status, linked_bill_ids, notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, '0', ?, ?, 'pending', '[]', ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &number,
                    input.supplier_id.to_string(),
                    input.purchase_order_id.map(|p| p.to_string()),
                    input.amount.to_string(),
                    currency.to_string(),
                    input.due_date.to_string(),
                    &input.notes,
                    &now_str,
                    &now_str,
                ],
            )?;
            Self::fetch(tx, &id_str)
        })
    }

    fn get(&self, id: PaymentObligationId) -> Result<Option<PaymentObligation>> {
        let conn = self.conn()?;
        match Self::fetch(&conn, &id.to_string()) {
            Ok(o) => Ok(Some(o)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: PaymentObligationFilter) -> Result<Vec<PaymentObligation>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM payment_obligations WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(supplier) = filter.supplier_id {
            sql.push_str(" AND supplier_id = ?");
            params.push(Box::new(supplier.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(due) = filter.due_before {
            sql.push_str(" AND due_date <= ?");
            params.push(Box::new(due.to_string()));
        }
        sql.push_str(" ORDER BY due_date ASC");
        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_obligation)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn record_payment(
        &self,
        id: PaymentObligationId,
        amount: Decimal,
    ) -> Result<PaymentObligation> {
        if amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError("payment amount must be positive".into()));
        }
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let mut obligation = Self::fetch(tx, &id_str)?;
            if obligation.status == PaymentObligationStatus::Cancelled {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::Conflict("cannot pay a cancelled obligation".into()),
                )));
            }
            if amount > obligation.outstanding() {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(format!(
                        "payment {amount} exceeds outstanding balance {}",
                        obligation.outstanding()
                    )),
                )));
            }
            obligation.amount_paid += amount;
            let new_status = obligation.derive_status();
            tx.execute(
                "UPDATE payment_obligations SET amount_paid = ?, status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    obligation.amount_paid.to_string(),
                    new_status.to_string(),
                    &now,
                    &id_str
                ],
            )?;
            Self::fetch(tx, &id_str)
        })
    }

    fn set_status(
        &self,
        id: PaymentObligationId,
        status: PaymentObligationStatus,
    ) -> Result<PaymentObligation> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "UPDATE payment_obligations SET status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![status.to_string(), &now, &id_str],
            )?;
            Self::fetch(tx, &id_str)
        })
    }

    fn link_bill(&self, id: PaymentObligationId, bill_id: uuid::Uuid) -> Result<PaymentObligation> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let current: String = tx.query_row(
                "SELECT linked_bill_ids FROM payment_obligations WHERE id = ?",
                [&id_str],
                |r| r.get(0),
            )?;
            let mut ids: Vec<uuid::Uuid> =
                serde_json::from_str(&current).map_err(Self::json_err)?;
            if !ids.contains(&bill_id) {
                ids.push(bill_id);
            }
            let json = serde_json::to_string(&ids).map_err(Self::json_err)?;
            tx.execute(
                "UPDATE payment_obligations SET linked_bill_ids = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![&json, &now, &id_str],
            )?;
            Self::fetch(tx, &id_str)
        })
    }

    fn dashboard(&self, today: NaiveDate) -> Result<PaymentObligationDashboard> {
        // Open (non-terminal) obligations only.
        let open = self.list(PaymentObligationFilter {
            status: Some(PaymentObligationStatus::Pending),
            ..Default::default()
        })?;
        let scheduled = self.list(PaymentObligationFilter {
            status: Some(PaymentObligationStatus::Scheduled),
            ..Default::default()
        })?;
        let partial = self.list(PaymentObligationFilter {
            status: Some(PaymentObligationStatus::PartiallyPaid),
            ..Default::default()
        })?;

        let mut dash = PaymentObligationDashboard::default();
        for o in open.iter().chain(scheduled.iter()).chain(partial.iter()) {
            dash.open_count += 1;
            dash.total_outstanding += o.outstanding();
            if o.is_overdue(today) {
                dash.overdue_count += 1;
                dash.overdue_amount += o.outstanding();
            }
        }
        Ok(dash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn test_repo() -> SqlitePaymentObligationRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqlitePaymentObligationRepository::new(db.pool().clone())
    }

    fn new_obl(
        repo: &SqlitePaymentObligationRepository,
        amount: Decimal,
        due: NaiveDate,
    ) -> PaymentObligation {
        repo.create(CreatePaymentObligation {
            supplier_id: Uuid::new_v4(),
            purchase_order_id: None,
            amount,
            currency: Some(CurrencyCode::USD),
            due_date: due,
            notes: None,
        })
        .expect("create obligation")
    }

    #[test]
    fn record_payment_progresses_status() {
        let repo = test_repo();
        let o = new_obl(&repo, dec!(100), day(2026, 7, 1));
        let after = repo.record_payment(o.id, dec!(40)).expect("pay");
        assert_eq!(after.status, PaymentObligationStatus::PartiallyPaid);
        assert_eq!(after.outstanding(), dec!(60));
        let after = repo.record_payment(o.id, dec!(60)).expect("pay rest");
        assert_eq!(after.status, PaymentObligationStatus::Paid);
        assert_eq!(after.outstanding(), dec!(0));
    }

    #[test]
    fn rejects_overpayment() {
        let repo = test_repo();
        let o = new_obl(&repo, dec!(100), day(2026, 7, 1));
        // Cannot pay more than the full amount in one shot.
        assert!(repo.record_payment(o.id, dec!(150)).is_err());
        // Partial payment, then an overpayment of the remainder is rejected.
        repo.record_payment(o.id, dec!(60)).expect("partial");
        assert!(repo.record_payment(o.id, dec!(50)).is_err());
        // Exact remaining balance still succeeds.
        let done = repo.record_payment(o.id, dec!(40)).expect("pay rest");
        assert_eq!(done.status, PaymentObligationStatus::Paid);
        assert_eq!(done.outstanding(), dec!(0));
    }

    #[test]
    fn link_bill_idempotent() {
        let repo = test_repo();
        let o = new_obl(&repo, dec!(100), day(2026, 7, 1));
        let bill = Uuid::new_v4();
        let after = repo.link_bill(o.id, bill).expect("link");
        assert_eq!(after.linked_bill_ids, vec![bill]);
        let again = repo.link_bill(o.id, bill).expect("link again");
        assert_eq!(again.linked_bill_ids.len(), 1);
    }

    #[test]
    fn dashboard_counts_outstanding_and_overdue() {
        let repo = test_repo();
        let today = day(2026, 6, 15);
        new_obl(&repo, dec!(100), day(2026, 6, 1)); // overdue
        new_obl(&repo, dec!(50), day(2026, 7, 1)); // future
        let paid = new_obl(&repo, dec!(200), day(2026, 6, 1));
        repo.record_payment(paid.id, dec!(200)).expect("pay"); // paid → excluded

        let dash = repo.dashboard(today).expect("dashboard");
        assert_eq!(dash.open_count, 2);
        assert_eq!(dash.total_outstanding, dec!(150));
        assert_eq!(dash.overdue_count, 1);
        assert_eq!(dash.overdue_amount, dec!(100));
    }

    #[test]
    fn cancel_blocks_payment() {
        let repo = test_repo();
        let o = new_obl(&repo, dec!(100), day(2026, 7, 1));
        repo.set_status(o.id, PaymentObligationStatus::Cancelled).expect("cancel");
        assert!(repo.record_payment(o.id, dec!(10)).is_err());
    }
}
