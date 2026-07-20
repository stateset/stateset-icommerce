//! SQLite implementation of the vendor credit repository

use super::{
    map_db_error, parse_datetime_row, parse_decimal_row, parse_enum_row, parse_uuid_opt_row,
    parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rust_decimal::Decimal;
use stateset_core::{
    ApplyVendorCredit, CommerceError, CreateVendorCredit, CurrencyCode, Result, VendorCredit,
    VendorCreditApplication, VendorCreditApplicationId, VendorCreditFilter, VendorCreditId,
    VendorCreditRepository, VendorCreditStatus, VendorCreditTargetType,
};

#[derive(Debug)]
pub struct SqliteVendorCreditRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteVendorCreditRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_credit(row: &rusqlite::Row<'_>) -> rusqlite::Result<VendorCredit> {
        Ok(VendorCredit {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "vendor_credit", "id")?.into(),
            number: row.get("number")?,
            supplier_id: parse_uuid_row(
                &row.get::<_, String>("supplier_id")?,
                "vendor_credit",
                "supplier_id",
            )?,
            vendor_return_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("vendor_return_id")?,
                "vendor_credit",
                "vendor_return_id",
            )?,
            amount: parse_decimal_row(&row.get::<_, String>("amount")?, "vendor_credit", "amount")?,
            remaining: parse_decimal_row(
                &row.get::<_, String>("remaining")?,
                "vendor_credit",
                "remaining",
            )?,
            currency: parse_enum_row::<CurrencyCode>(
                &row.get::<_, String>("currency")?,
                "vendor_credit",
                "currency",
            )?,
            status: parse_enum_row::<VendorCreditStatus>(
                &row.get::<_, String>("status")?,
                "vendor_credit",
                "status",
            )?,
            memo: row.get("memo")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "vendor_credit",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "vendor_credit",
                "updated_at",
            )?,
        })
    }

    fn row_to_app(row: &rusqlite::Row<'_>) -> rusqlite::Result<VendorCreditApplication> {
        Ok(VendorCreditApplication {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "vendor_credit_app", "id")?.into(),
            vendor_credit_id: parse_uuid_row(
                &row.get::<_, String>("vendor_credit_id")?,
                "vendor_credit_app",
                "vendor_credit_id",
            )?
            .into(),
            target_type: parse_enum_row::<VendorCreditTargetType>(
                &row.get::<_, String>("target_type")?,
                "vendor_credit_app",
                "target_type",
            )?,
            target_id: parse_uuid_row(
                &row.get::<_, String>("target_id")?,
                "vendor_credit_app",
                "target_id",
            )?,
            amount: parse_decimal_row(
                &row.get::<_, String>("amount")?,
                "vendor_credit_app",
                "amount",
            )?,
            reversed: row.get::<_, i32>("reversed")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "vendor_credit_app",
                "created_at",
            )?,
        })
    }

    fn fetch(tx: &rusqlite::Connection, id: &str) -> rusqlite::Result<VendorCredit> {
        tx.query_row("SELECT * FROM vendor_credits WHERE id = ?", [id], Self::row_to_credit)
    }

    fn conflict(msg: &str) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::Conflict(msg.to_string())))
    }

    fn validation(msg: &str) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::ValidationError(
            msg.to_string(),
        )))
    }

    /// Recompute status from remaining balance (does not touch cancelled).
    fn status_for(remaining: Decimal, current: VendorCreditStatus) -> VendorCreditStatus {
        if current == VendorCreditStatus::Cancelled {
            VendorCreditStatus::Cancelled
        } else if remaining <= Decimal::ZERO {
            VendorCreditStatus::Applied
        } else {
            VendorCreditStatus::Open
        }
    }
}

impl VendorCreditRepository for SqliteVendorCreditRepository {
    fn create(&self, input: CreateVendorCredit) -> Result<VendorCredit> {
        if input.amount <= Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "vendor credit amount must be positive".into(),
            ));
        }
        let id = VendorCreditId::new();
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let number = format!("VC-{}", &id_str[..8]);
        let currency = input.currency.unwrap_or(CurrencyCode::USD);
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO vendor_credits (id, number, supplier_id, vendor_return_id, amount, remaining, currency, status, memo, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, 'open', ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &number,
                    input.supplier_id.to_string(),
                    input.vendor_return_id.map(|v| v.to_string()),
                    input.amount.to_string(),
                    input.amount.to_string(),
                    currency.to_string(),
                    &input.memo,
                    &now_str,
                    &now_str,
                ],
            )?;
            Self::fetch(tx, &id_str)
        })
    }

    fn get(&self, id: VendorCreditId) -> Result<Option<VendorCredit>> {
        let conn = self.conn()?;
        match Self::fetch(&conn, &id.to_string()) {
            Ok(c) => Ok(Some(c)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: VendorCreditFilter) -> Result<Vec<VendorCredit>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM vendor_credits WHERE 1=1".to_string();
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
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_credit)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn apply(&self, id: VendorCreditId, input: ApplyVendorCredit) -> Result<VendorCredit> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let credit =
                Self::fetch(tx, &id_str).optional()?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            if credit.status == VendorCreditStatus::Cancelled {
                return Err(Self::conflict("cannot apply a cancelled vendor credit"));
            }
            if input.amount <= Decimal::ZERO {
                return Err(Self::validation("application amount must be positive"));
            }
            if input.amount > credit.remaining {
                return Err(Self::validation("application exceeds remaining balance"));
            }
            let new_remaining = credit.remaining - input.amount;
            let new_status = Self::status_for(new_remaining, credit.status);
            tx.execute(
                "INSERT INTO vendor_credit_applications (id, vendor_credit_id, target_type, target_id, amount, reversed, created_at)
                 VALUES (?, ?, ?, ?, ?, 0, ?)",
                rusqlite::params![
                    VendorCreditApplicationId::new().to_string(),
                    &id_str,
                    input.target_type.to_string(),
                    input.target_id.to_string(),
                    input.amount.to_string(),
                    &now,
                ],
            )?;
            tx.execute(
                "UPDATE vendor_credits SET remaining = ?, status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![new_remaining.to_string(), new_status.to_string(), &now, &id_str],
            )?;
            Self::fetch(tx, &id_str)
        })
    }

    fn list_applications(&self, id: VendorCreditId) -> Result<Vec<VendorCreditApplication>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM vendor_credit_applications WHERE vendor_credit_id = ? ORDER BY created_at")
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
        id: VendorCreditId,
        application_id: VendorCreditApplicationId,
    ) -> Result<VendorCredit> {
        let id_str = id.to_string();
        let app_str = application_id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let row: Option<(String, i32)> = tx
                .query_row(
                    "SELECT amount, reversed FROM vendor_credit_applications WHERE id = ? AND vendor_credit_id = ?",
                    rusqlite::params![&app_str, &id_str],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let (amount_str, reversed) = row.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
            if reversed != 0 {
                return Err(Self::conflict("application already reversed"));
            }
            let amount: Decimal = amount_str.parse().unwrap_or(Decimal::ZERO);
            let credit = Self::fetch(tx, &id_str)?;
            let new_remaining = credit.remaining + amount;
            let new_status = Self::status_for(new_remaining, credit.status);
            tx.execute(
                "UPDATE vendor_credit_applications SET reversed = 1 WHERE id = ?",
                [&app_str],
            )?;
            tx.execute(
                "UPDATE vendor_credits SET remaining = ?, status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![new_remaining.to_string(), new_status.to_string(), &now, &id_str],
            )?;
            Self::fetch(tx, &id_str)
        })
    }

    fn cancel(&self, id: VendorCreditId) -> Result<VendorCredit> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let active: i64 = tx.query_row(
                "SELECT COUNT(*) FROM vendor_credit_applications WHERE vendor_credit_id = ? AND reversed = 0",
                [&id_str],
                |r| r.get(0),
            )?;
            if active > 0 {
                return Err(Self::conflict(
                    "cannot cancel a vendor credit with active applications",
                ));
            }
            tx.execute(
                "UPDATE vendor_credits SET status = 'cancelled', updated_at = ? WHERE id = ?",
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

    fn test_repo() -> SqliteVendorCreditRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteVendorCreditRepository::new(db.pool().clone())
    }

    fn new_credit(repo: &SqliteVendorCreditRepository, amount: Decimal) -> VendorCredit {
        repo.create(CreateVendorCredit {
            supplier_id: Uuid::new_v4(),
            vendor_return_id: None,
            amount,
            currency: Some(CurrencyCode::USD),
            memo: Some("RTV credit".into()),
        })
        .expect("create vendor credit")
    }

    fn apply(target: VendorCreditTargetType, amount: Decimal) -> ApplyVendorCredit {
        ApplyVendorCredit { target_type: target, target_id: Uuid::new_v4(), amount }
    }

    #[test]
    fn create_rejects_non_positive() {
        let repo = test_repo();
        assert!(
            repo.create(CreateVendorCredit {
                supplier_id: Uuid::new_v4(),
                vendor_return_id: None,
                amount: dec!(0),
                currency: None,
                memo: None,
            })
            .is_err()
        );
    }

    #[test]
    fn apply_decrements_and_marks_applied() {
        let repo = test_repo();
        let c = new_credit(&repo, dec!(100));
        let after = repo.apply(c.id, apply(VendorCreditTargetType::Bill, dec!(40))).expect("apply");
        assert_eq!(after.remaining, dec!(60));
        assert_eq!(after.status, VendorCreditStatus::Open);

        let after = repo
            .apply(c.id, apply(VendorCreditTargetType::PaymentObligation, dec!(60)))
            .expect("apply rest");
        assert_eq!(after.remaining, dec!(0));
        assert_eq!(after.status, VendorCreditStatus::Applied);
        assert_eq!(repo.list_applications(c.id).expect("apps").len(), 2);
    }

    #[test]
    fn apply_over_balance_rejected() {
        let repo = test_repo();
        let c = new_credit(&repo, dec!(50));
        assert!(repo.apply(c.id, apply(VendorCreditTargetType::Bill, dec!(60))).is_err());
    }

    #[test]
    fn reverse_restores_balance() {
        let repo = test_repo();
        let c = new_credit(&repo, dec!(100));
        repo.apply(c.id, apply(VendorCreditTargetType::Bill, dec!(100))).expect("apply");
        let apps = repo.list_applications(c.id).expect("apps");
        let reversed = repo.reverse_application(c.id, apps[0].id).expect("reverse");
        assert_eq!(reversed.remaining, dec!(100));
        assert_eq!(reversed.status, VendorCreditStatus::Open);
        // double reverse fails
        assert!(repo.reverse_application(c.id, apps[0].id).is_err());
    }

    #[test]
    fn cancel_blocked_with_active_applications() {
        let repo = test_repo();
        let c = new_credit(&repo, dec!(100));
        repo.apply(c.id, apply(VendorCreditTargetType::Bill, dec!(10))).expect("apply");
        assert!(repo.cancel(c.id).is_err());

        let apps = repo.list_applications(c.id).expect("apps");
        repo.reverse_application(c.id, apps[0].id).expect("reverse");
        let cancelled = repo.cancel(c.id).expect("cancel");
        assert_eq!(cancelled.status, VendorCreditStatus::Cancelled);
    }
}
