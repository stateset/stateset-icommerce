//! SQLite implementation of the prepayment repository

use super::{
    map_db_error, parse_datetime_row, parse_decimal_row, parse_enum_row, parse_uuid_row,
    with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    ApplyPrepayment, CommerceError, CreatePrepayment, CurrencyCode, Prepayment,
    PrepaymentApplication, PrepaymentApplicationId, PrepaymentFilter, PrepaymentId,
    PrepaymentRepository, PrepaymentStatus, PrepaymentTargetType, Result,
};

#[derive(Debug)]
pub struct SqlitePrepaymentRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePrepaymentRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_prepayment(row: &rusqlite::Row<'_>) -> rusqlite::Result<Prepayment> {
        Ok(Prepayment {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "prepayment", "id")?.into(),
            number: row.get("number")?,
            supplier_id: parse_uuid_row(
                &row.get::<_, String>("supplier_id")?,
                "prepayment",
                "supplier_id",
            )?,
            amount: parse_decimal_row(&row.get::<_, String>("amount")?, "prepayment", "amount")?,
            remaining: parse_decimal_row(
                &row.get::<_, String>("remaining")?,
                "prepayment",
                "remaining",
            )?,
            currency: parse_enum_row::<CurrencyCode>(
                &row.get::<_, String>("currency")?,
                "prepayment",
                "currency",
            )?,
            status: parse_enum_row::<PrepaymentStatus>(
                &row.get::<_, String>("status")?,
                "prepayment",
                "status",
            )?,
            method: row.get("method")?,
            reference: row.get("reference")?,
            memo: row.get("memo")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "prepayment",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "prepayment",
                "updated_at",
            )?,
        })
    }

    fn row_to_app(row: &rusqlite::Row<'_>) -> rusqlite::Result<PrepaymentApplication> {
        Ok(PrepaymentApplication {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "prepayment_app", "id")?.into(),
            prepayment_id: parse_uuid_row(
                &row.get::<_, String>("prepayment_id")?,
                "prepayment_app",
                "prepayment_id",
            )?
            .into(),
            target_type: parse_enum_row::<PrepaymentTargetType>(
                &row.get::<_, String>("target_type")?,
                "prepayment_app",
                "target_type",
            )?,
            target_id: parse_uuid_row(
                &row.get::<_, String>("target_id")?,
                "prepayment_app",
                "target_id",
            )?,
            amount: parse_decimal_row(
                &row.get::<_, String>("amount")?,
                "prepayment_app",
                "amount",
            )?,
            reversed: row.get::<_, i32>("reversed")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "prepayment_app",
                "created_at",
            )?,
        })
    }

    fn fetch(tx: &rusqlite::Connection, id: &str) -> rusqlite::Result<Prepayment> {
        tx.query_row("SELECT * FROM prepayments WHERE id = ?", [id], Self::row_to_prepayment)
    }

    fn conflict(msg: &str) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::Conflict(msg.to_string())))
    }

    fn validation(msg: &str) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::ValidationError(
            msg.to_string(),
        )))
    }

    fn status_for(remaining: Decimal, current: PrepaymentStatus) -> PrepaymentStatus {
        match current {
            PrepaymentStatus::Cancelled | PrepaymentStatus::Refunded => current,
            _ if remaining <= Decimal::ZERO => PrepaymentStatus::Applied,
            _ => PrepaymentStatus::Open,
        }
    }
}

impl PrepaymentRepository for SqlitePrepaymentRepository {
    fn create(&self, input: CreatePrepayment) -> Result<Prepayment> {
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "prepayment amount must be positive".into(),
            ));
        }
        let id = PrepaymentId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let number = format!("PRE-{}", &id_str[..8]);
        let currency = input.currency.unwrap_or(CurrencyCode::USD);
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO prepayments (id, number, supplier_id, amount, remaining, currency, status, method, reference, memo, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, 'open', ?, ?, ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &number,
                    input.supplier_id.to_string(),
                    input.amount.to_string(),
                    input.amount.to_string(),
                    currency.to_string(),
                    &input.method,
                    &input.reference,
                    &input.memo,
                    &now_str,
                    &now_str,
                ],
            )?;
            Self::fetch(tx, &id_str)
        })
    }

    fn get(&self, id: PrepaymentId) -> Result<Option<Prepayment>> {
        let conn = self.conn()?;
        match Self::fetch(&conn, &id.to_string()) {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: PrepaymentFilter) -> Result<Vec<Prepayment>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM prepayments WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(supplier) = filter.supplier_id {
            sql.push_str(" AND supplier_id = ?");
            params.push(Box::new(supplier.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        sql.push_str(" ORDER BY created_at DESC");
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
            .query_map(param_refs.as_slice(), Self::row_to_prepayment)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn apply(&self, id: PrepaymentId, input: ApplyPrepayment) -> Result<Prepayment> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let prepayment =
                Self::fetch(tx, &id_str).optional()?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            if prepayment.status != PrepaymentStatus::Open {
                return Err(Self::conflict("prepayment is not open for application"));
            }
            if input.amount <= Decimal::ZERO {
                return Err(Self::validation("application amount must be positive"));
            }
            if input.amount > prepayment.remaining {
                return Err(Self::validation("application exceeds remaining balance"));
            }
            let new_remaining = prepayment.remaining - input.amount;
            let new_status = Self::status_for(new_remaining, prepayment.status);
            tx.execute(
                "INSERT INTO prepayment_applications (id, prepayment_id, target_type, target_id, amount, reversed, created_at)
                 VALUES (?, ?, ?, ?, ?, 0, ?)",
                rusqlite::params![
                    PrepaymentApplicationId::new().to_string(),
                    &id_str,
                    input.target_type.to_string(),
                    input.target_id.to_string(),
                    input.amount.to_string(),
                    &now,
                ],
            )?;
            tx.execute(
                "UPDATE prepayments SET remaining = ?, status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![new_remaining.to_string(), new_status.to_string(), &now, &id_str],
            )?;
            Self::fetch(tx, &id_str)
        })
    }

    fn list_applications(&self, id: PrepaymentId) -> Result<Vec<PrepaymentApplication>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(
                "SELECT * FROM prepayment_applications WHERE prepayment_id = ? ORDER BY created_at",
            )
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([id.to_string()], Self::row_to_app)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn reverse_application(
        &self,
        id: PrepaymentId,
        application_id: PrepaymentApplicationId,
    ) -> Result<Prepayment> {
        let id_str = id.to_string();
        let app_str = application_id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let row: Option<(String, i32)> = tx
                .query_row(
                    "SELECT amount, reversed FROM prepayment_applications WHERE id = ? AND prepayment_id = ?",
                    rusqlite::params![&app_str, &id_str],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let (amount_str, reversed) = row.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            if reversed != 0 {
                return Err(Self::conflict("application already reversed"));
            }
            let amount: Decimal = amount_str.parse().unwrap_or(Decimal::ZERO);
            let prepayment = Self::fetch(tx, &id_str)?;
            if prepayment.status == PrepaymentStatus::Refunded {
                return Err(Self::conflict("cannot reverse against a refunded prepayment"));
            }
            let new_remaining = prepayment.remaining + amount;
            let new_status = Self::status_for(new_remaining, prepayment.status);
            tx.execute("UPDATE prepayment_applications SET reversed = 1 WHERE id = ?", [&app_str])?;
            tx.execute(
                "UPDATE prepayments SET remaining = ?, status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![new_remaining.to_string(), new_status.to_string(), &now, &id_str],
            )?;
            Self::fetch(tx, &id_str)
        })
    }

    fn refund(&self, id: PrepaymentId) -> Result<Prepayment> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let prepayment = Self::fetch(tx, &id_str)?;
            if prepayment.status == PrepaymentStatus::Cancelled {
                return Err(Self::conflict("cannot refund a cancelled prepayment"));
            }
            // Refund zeroes the remaining balance and closes the prepayment.
            tx.execute(
                "UPDATE prepayments SET remaining = '0', status = 'refunded', updated_at = ? WHERE id = ?",
                rusqlite::params![&now, &id_str],
            )?;
            Self::fetch(tx, &id_str)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn test_repo() -> SqlitePrepaymentRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqlitePrepaymentRepository::new(db.pool().clone())
    }

    fn new_prepayment(repo: &SqlitePrepaymentRepository, amount: Decimal) -> Prepayment {
        repo.create(CreatePrepayment {
            supplier_id: Uuid::new_v4(),
            amount,
            currency: Some(CurrencyCode::USD),
            method: Some("wire".into()),
            reference: Some("WIRE-123".into()),
            memo: None,
        })
        .expect("create prepayment")
    }

    fn apply(amount: Decimal) -> ApplyPrepayment {
        ApplyPrepayment {
            target_type: PrepaymentTargetType::Bill,
            target_id: Uuid::new_v4(),
            amount,
        }
    }

    #[test]
    fn create_rejects_non_positive() {
        let repo = test_repo();
        assert!(
            repo.create(CreatePrepayment {
                supplier_id: Uuid::new_v4(),
                amount: dec!(0),
                currency: None,
                method: None,
                reference: None,
                memo: None,
            })
            .is_err()
        );
    }

    #[test]
    fn apply_decrements_and_marks_applied() {
        let repo = test_repo();
        let p = new_prepayment(&repo, dec!(100));
        let after = repo.apply(p.id, apply(dec!(40))).expect("apply");
        assert_eq!(after.remaining, dec!(60));
        assert_eq!(after.status, PrepaymentStatus::Open);
        let after = repo.apply(p.id, apply(dec!(60))).expect("apply rest");
        assert_eq!(after.remaining, dec!(0));
        assert_eq!(after.status, PrepaymentStatus::Applied);
        assert_eq!(repo.list_applications(p.id).expect("apps").len(), 2);
    }

    #[test]
    fn apply_over_balance_rejected() {
        let repo = test_repo();
        let p = new_prepayment(&repo, dec!(50));
        assert!(repo.apply(p.id, apply(dec!(60))).is_err());
    }

    #[test]
    fn reverse_restores_balance() {
        let repo = test_repo();
        let p = new_prepayment(&repo, dec!(100));
        repo.apply(p.id, apply(dec!(100))).expect("apply");
        let apps = repo.list_applications(p.id).expect("apps");
        let reversed = repo.reverse_application(p.id, apps[0].id).expect("reverse");
        assert_eq!(reversed.remaining, dec!(100));
        assert_eq!(reversed.status, PrepaymentStatus::Open);
        assert!(repo.reverse_application(p.id, apps[0].id).is_err());
    }

    #[test]
    fn refund_closes_prepayment() {
        let repo = test_repo();
        let p = new_prepayment(&repo, dec!(100));
        repo.apply(p.id, apply(dec!(30))).expect("apply");
        let refunded = repo.refund(p.id).expect("refund");
        assert_eq!(refunded.status, PrepaymentStatus::Refunded);
        assert_eq!(refunded.remaining, dec!(0));
        // can't apply after refund
        assert!(repo.apply(p.id, apply(dec!(10))).is_err());
    }
}
