//! SQLite implementation of the fixed asset repository

use super::{
    map_db_error, parse_date_row, parse_datetime_row, parse_decimal_row, parse_enum_row,
    parse_json_row, parse_uuid_opt_row, parse_uuid_row, with_immediate_transaction,
};
use chrono::{NaiveDate, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rust_decimal::Decimal;
use stateset_core::{
    AssetDisposal, CommerceError, CreateFixedAsset, CurrencyCode, DepreciationEntry,
    DepreciationEntryStatus, DepreciationMethod, DepreciationSchedule, FixedAsset,
    FixedAssetCategory, FixedAssetFilter, FixedAssetStatus, Result, UpdateFixedAsset, Validate,
    generate_asset_number, generate_depreciation_schedule,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqliteFixedAssetRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteFixedAssetRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn conflict(msg: &str) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::Conflict(msg.to_string())))
    }

    fn row_to_asset(row: &rusqlite::Row<'_>) -> rusqlite::Result<FixedAsset> {
        let disposal = match row.get::<_, Option<String>>("disposal_date")? {
            Some(date) => Some(AssetDisposal {
                disposal_date: parse_date_row(&date, "fixed_asset", "disposal_date")?,
                proceeds: parse_decimal_row(
                    &row.get::<_, Option<String>>("disposal_proceeds")?.unwrap_or_default(),
                    "fixed_asset",
                    "disposal_proceeds",
                )?,
                book_value_at_disposal: parse_decimal_row(
                    &row.get::<_, Option<String>>("disposal_book_value")?.unwrap_or_default(),
                    "fixed_asset",
                    "disposal_book_value",
                )?,
                gain_loss: parse_decimal_row(
                    &row.get::<_, Option<String>>("disposal_gain_loss")?.unwrap_or_default(),
                    "fixed_asset",
                    "disposal_gain_loss",
                )?,
                notes: row.get("disposal_notes")?,
            }),
            None => None,
        };
        let in_service_date = match row.get::<_, Option<String>>("in_service_date")? {
            Some(s) => Some(parse_date_row(&s, "fixed_asset", "in_service_date")?),
            None => None,
        };
        Ok(FixedAsset {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "fixed_asset", "id")?,
            asset_number: row.get("asset_number")?,
            name: row.get("name")?,
            description: row.get("description")?,
            category: parse_enum_row::<FixedAssetCategory>(
                &row.get::<_, String>("category")?,
                "fixed_asset",
                "category",
            )?,
            acquisition_date: parse_date_row(
                &row.get::<_, String>("acquisition_date")?,
                "fixed_asset",
                "acquisition_date",
            )?,
            acquisition_cost: parse_decimal_row(
                &row.get::<_, String>("acquisition_cost")?,
                "fixed_asset",
                "acquisition_cost",
            )?,
            salvage_value: parse_decimal_row(
                &row.get::<_, String>("salvage_value")?,
                "fixed_asset",
                "salvage_value",
            )?,
            useful_life_months: row.get::<_, i64>("useful_life_months")? as u32,
            depreciation_method: parse_json_row::<DepreciationMethod>(
                &row.get::<_, String>("depreciation_method")?,
                "fixed_asset",
                "depreciation_method",
            )?,
            status: parse_enum_row::<FixedAssetStatus>(
                &row.get::<_, String>("status")?,
                "fixed_asset",
                "status",
            )?,
            in_service_date,
            location_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("location_id")?,
                "fixed_asset",
                "location_id",
            )?,
            asset_account_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("asset_account_id")?,
                "fixed_asset",
                "asset_account_id",
            )?,
            accumulated_depreciation_account_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("accumulated_depreciation_account_id")?,
                "fixed_asset",
                "accumulated_depreciation_account_id",
            )?,
            depreciation_expense_account_id: parse_uuid_opt_row(
                row.get::<_, Option<String>>("depreciation_expense_account_id")?,
                "fixed_asset",
                "depreciation_expense_account_id",
            )?,
            accumulated_depreciation: parse_decimal_row(
                &row.get::<_, String>("accumulated_depreciation")?,
                "fixed_asset",
                "accumulated_depreciation",
            )?,
            currency: parse_enum_row::<CurrencyCode>(
                &row.get::<_, String>("currency")?,
                "fixed_asset",
                "currency",
            )?,
            disposal,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "fixed_asset",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "fixed_asset",
                "updated_at",
            )?,
        })
    }

    fn row_to_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<DepreciationEntry> {
        Ok(DepreciationEntry {
            period: row.get::<_, i64>("period")? as u32,
            amount: parse_decimal_row(
                &row.get::<_, String>("amount")?,
                "depreciation_entry",
                "amount",
            )?,
            accumulated: parse_decimal_row(
                &row.get::<_, String>("accumulated")?,
                "depreciation_entry",
                "accumulated",
            )?,
            book_value: parse_decimal_row(
                &row.get::<_, String>("book_value")?,
                "depreciation_entry",
                "book_value",
            )?,
            status: parse_enum_row::<DepreciationEntryStatus>(
                &row.get::<_, String>("status")?,
                "depreciation_entry",
                "status",
            )?,
        })
    }

    fn load_asset(conn: &rusqlite::Connection, id: &str) -> rusqlite::Result<FixedAsset> {
        conn.query_row("SELECT * FROM fixed_assets WHERE id = ?", [id], Self::row_to_asset)
    }

    fn load_entries(
        conn: &rusqlite::Connection,
        id: &str,
    ) -> rusqlite::Result<Vec<DepreciationEntry>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM fixed_asset_depreciation_entries WHERE asset_id = ? ORDER BY period",
        )?;
        stmt.query_map([id], Self::row_to_entry)?.collect()
    }

    fn transition(
        &self,
        id: Uuid,
        next: FixedAssetStatus,
        date: NaiveDate,
        proceeds: Option<Decimal>,
        notes: Option<String>,
    ) -> Result<FixedAsset> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let asset = Self::load_asset(tx, &id_str)?;
            if !asset.status.can_transition_to(next) {
                return Err(Self::conflict(&format!(
                    "fixed asset cannot transition from {} to {next}",
                    asset.status
                )));
            }
            match next {
                FixedAssetStatus::InService => {
                    tx.execute(
                        "UPDATE fixed_assets SET status = 'in_service', in_service_date = ?, updated_at = ? WHERE id = ?",
                        rusqlite::params![date.to_string(), &now, &id_str],
                    )?;
                }
                FixedAssetStatus::Disposed | FixedAssetStatus::WrittenOff => {
                    let proceeds = proceeds.unwrap_or(Decimal::ZERO);
                    let disposal =
                        AssetDisposal::new(date, proceeds, asset.book_value(), notes.clone());
                    tx.execute(
                        "UPDATE fixed_assets SET status = ?, disposal_date = ?, disposal_proceeds = ?, disposal_book_value = ?, disposal_gain_loss = ?, disposal_notes = ?, updated_at = ? WHERE id = ?",
                        rusqlite::params![
                            next.to_string(),
                            disposal.disposal_date.to_string(),
                            disposal.proceeds.to_string(),
                            disposal.book_value_at_disposal.to_string(),
                            disposal.gain_loss.to_string(),
                            &disposal.notes,
                            &now,
                            &id_str,
                        ],
                    )?;
                }
                _ => {
                    tx.execute(
                        "UPDATE fixed_assets SET status = ?, updated_at = ? WHERE id = ?",
                        rusqlite::params![next.to_string(), &now, &id_str],
                    )?;
                }
            }
            Self::load_asset(tx, &id_str)
        })
    }
}

impl stateset_core::FixedAssetRepository for SqliteFixedAssetRepository {
    fn create(&self, input: CreateFixedAsset) -> Result<FixedAsset> {
        input.validate()?;
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        let asset_number = input.asset_number.clone().unwrap_or_else(generate_asset_number);
        let currency = input.currency.unwrap_or_default();
        let status = if input.in_service_date.is_some() {
            FixedAssetStatus::InService
        } else {
            FixedAssetStatus::Draft
        };
        let method_json = serde_json::to_string(&input.depreciation_method)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO fixed_assets (id, asset_number, name, description, category, acquisition_date, acquisition_cost, salvage_value, useful_life_months, depreciation_method, status, in_service_date, location_id, asset_account_id, accumulated_depreciation_account_id, depreciation_expense_account_id, accumulated_depreciation, currency, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, '0', ?, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &asset_number,
                    &input.name,
                    &input.description,
                    input.category.to_string(),
                    input.acquisition_date.to_string(),
                    input.acquisition_cost.to_string(),
                    input.salvage_value.to_string(),
                    i64::from(input.useful_life_months),
                    &method_json,
                    status.to_string(),
                    input.in_service_date.map(|d| d.to_string()),
                    input.location_id.map(|u| u.to_string()),
                    input.asset_account_id.map(|u| u.to_string()),
                    input.accumulated_depreciation_account_id.map(|u| u.to_string()),
                    input.depreciation_expense_account_id.map(|u| u.to_string()),
                    currency.to_string(),
                    &now,
                    &now,
                ],
            )?;
            Self::load_asset(tx, &id_str)
        })
    }

    fn get(&self, id: Uuid) -> Result<Option<FixedAsset>> {
        let conn = self.conn()?;
        match Self::load_asset(&conn, &id.to_string()) {
            Ok(a) => Ok(Some(a)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: FixedAssetFilter) -> Result<Vec<FixedAsset>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM fixed_assets WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];
        if let Some(category) = filter.category {
            sql.push_str(" AND category = ?");
            params.push(Box::new(category.to_string()));
        }
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        if let Some(location) = filter.location_id {
            sql.push_str(" AND location_id = ?");
            params.push(Box::new(location.to_string()));
        }
        if let Some(from) = filter.acquired_from {
            sql.push_str(" AND acquisition_date >= ?");
            params.push(Box::new(from.to_string()));
        }
        if let Some(to) = filter.acquired_to {
            sql.push_str(" AND acquisition_date <= ?");
            params.push(Box::new(to.to_string()));
        }
        if let Some(search) = filter.search.as_deref() {
            sql.push_str(" AND (name LIKE ? ESCAPE '\\' OR asset_number LIKE ? ESCAPE '\\')");
            let pattern = format!("%{}%", super::escape_like(search));
            params.push(Box::new(pattern.clone()));
            params.push(Box::new(pattern));
        }
        sql.push_str(" ORDER BY created_at DESC");
        crate::sqlite::append_limit_offset(&mut sql, filter.limit, filter.offset);
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        stmt.query_map(param_refs.as_slice(), Self::row_to_asset)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)
    }

    fn update(&self, id: Uuid, input: UpdateFixedAsset) -> Result<FixedAsset> {
        input.validate()?;
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let asset = Self::load_asset(tx, &id_str)?;
            if asset.status.is_terminal() {
                return Err(Self::conflict("disposed or written-off assets cannot be updated"));
            }
            let name = input.name.clone().unwrap_or(asset.name);
            let description = input.description.clone().or(asset.description);
            let category = input.category.unwrap_or(asset.category);
            let salvage_value = input.salvage_value.unwrap_or(asset.salvage_value);
            let useful_life_months = input.useful_life_months.unwrap_or(asset.useful_life_months);
            let in_service_date = input.in_service_date.or(asset.in_service_date);
            let location_id = input.location_id.or(asset.location_id);
            let asset_account_id = input.asset_account_id.or(asset.asset_account_id);
            let accum_account_id = input
                .accumulated_depreciation_account_id
                .or(asset.accumulated_depreciation_account_id);
            let expense_account_id =
                input.depreciation_expense_account_id.or(asset.depreciation_expense_account_id);
            tx.execute(
                "UPDATE fixed_assets SET name = ?, description = ?, category = ?, salvage_value = ?, useful_life_months = ?, in_service_date = ?, location_id = ?, asset_account_id = ?, accumulated_depreciation_account_id = ?, depreciation_expense_account_id = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![
                    &name,
                    &description,
                    category.to_string(),
                    salvage_value.to_string(),
                    i64::from(useful_life_months),
                    in_service_date.map(|d| d.to_string()),
                    location_id.map(|u| u.to_string()),
                    asset_account_id.map(|u| u.to_string()),
                    accum_account_id.map(|u| u.to_string()),
                    expense_account_id.map(|u| u.to_string()),
                    &now,
                    &id_str,
                ],
            )?;
            Self::load_asset(tx, &id_str)
        })
    }

    fn place_in_service(&self, id: Uuid, date: NaiveDate) -> Result<FixedAsset> {
        self.transition(id, FixedAssetStatus::InService, date, None, None)
    }

    fn dispose(
        &self,
        id: Uuid,
        date: NaiveDate,
        proceeds: Decimal,
        notes: Option<String>,
    ) -> Result<FixedAsset> {
        if proceeds < Decimal::ZERO {
            return Err(CommerceError::ValidationError("proceeds must be non-negative".into()));
        }
        self.transition(id, FixedAssetStatus::Disposed, date, Some(proceeds), notes)
    }

    fn write_off(&self, id: Uuid, date: NaiveDate, notes: Option<String>) -> Result<FixedAsset> {
        self.transition(id, FixedAssetStatus::WrittenOff, date, Some(Decimal::ZERO), notes)
    }

    fn generate_schedule(&self, id: Uuid) -> Result<DepreciationSchedule> {
        let id_str = id.to_string();
        with_immediate_transaction(&self.pool, |tx| {
            let asset = Self::load_asset(tx, &id_str)?;
            let posted: i64 = tx.query_row(
                "SELECT COUNT(*) FROM fixed_asset_depreciation_entries WHERE asset_id = ? AND status = 'posted'",
                [&id_str],
                |r| r.get(0),
            )?;
            if posted > 0 {
                return Err(Self::conflict(
                    "cannot regenerate a schedule with posted depreciation entries",
                ));
            }
            let entries = generate_depreciation_schedule(
                asset.depreciation_method,
                asset.acquisition_cost,
                asset.salvage_value,
                asset.useful_life_months,
            );
            if entries.is_empty() {
                return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                    CommerceError::ValidationError(
                        "cannot generate a time-based depreciation schedule for this asset".into(),
                    ),
                )));
            }
            tx.execute(
                "DELETE FROM fixed_asset_depreciation_entries WHERE asset_id = ?",
                [&id_str],
            )?;
            for e in &entries {
                tx.execute(
                    "INSERT INTO fixed_asset_depreciation_entries (asset_id, period, amount, accumulated, book_value, status)
                     VALUES (?, ?, ?, ?, ?, ?)",
                    rusqlite::params![
                        &id_str,
                        i64::from(e.period),
                        e.amount.to_string(),
                        e.accumulated.to_string(),
                        e.book_value.to_string(),
                        e.status.to_string(),
                    ],
                )?;
            }
            Ok(DepreciationSchedule {
                asset_id: id,
                method: asset.depreciation_method,
                total_depreciation: asset.depreciable_base(),
                entries,
            })
        })
    }

    fn get_schedule(&self, id: Uuid) -> Result<Option<DepreciationSchedule>> {
        let conn = self.conn()?;
        let id_str = id.to_string();
        let asset = match Self::load_asset(&conn, &id_str) {
            Ok(a) => a,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(map_db_error(e)),
        };
        let entries = Self::load_entries(&conn, &id_str).map_err(map_db_error)?;
        if entries.is_empty() {
            return Ok(None);
        }
        Ok(Some(DepreciationSchedule {
            asset_id: id,
            method: asset.depreciation_method,
            total_depreciation: entries.iter().map(|e| e.amount).sum(),
            entries,
        }))
    }

    fn post_depreciation(&self, id: Uuid, periods: u32) -> Result<FixedAsset> {
        if periods == 0 {
            return Err(CommerceError::ValidationError("periods must be positive".into()));
        }
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        let (asset, posted_amount, first_period, last_period) = with_immediate_transaction(
            &self.pool,
            |tx| {
                let asset = Self::load_asset(tx, &id_str)?;
                if asset.status != FixedAssetStatus::InService {
                    return Err(Self::conflict(
                        "depreciation can only be posted for in-service assets",
                    ));
                }
                let pending: Vec<DepreciationEntry> = {
                    let mut stmt = tx.prepare(
                    "SELECT * FROM fixed_asset_depreciation_entries WHERE asset_id = ? AND status = 'scheduled' ORDER BY period LIMIT ?",
                )?;
                    stmt.query_map(
                        rusqlite::params![&id_str, i64::from(periods)],
                        Self::row_to_entry,
                    )?
                    .collect::<std::result::Result<Vec<_>, _>>()?
                };
                if pending.is_empty() {
                    return Err(Self::conflict("no scheduled depreciation entries remain to post"));
                }
                for e in &pending {
                    tx.execute(
                    "UPDATE fixed_asset_depreciation_entries SET status = 'posted' WHERE asset_id = ? AND period = ?",
                    rusqlite::params![&id_str, i64::from(e.period)],
                )?;
                }
                // Accumulated depreciation through the last posted entry is exact.
                let accumulated =
                    pending.last().map(|e| e.accumulated).unwrap_or(asset.accumulated_depreciation);
                let fully = accumulated >= asset.depreciable_base();
                let status = if fully {
                    FixedAssetStatus::FullyDepreciated
                } else {
                    FixedAssetStatus::InService
                };
                tx.execute(
                "UPDATE fixed_assets SET accumulated_depreciation = ?, status = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![accumulated.to_string(), status.to_string(), &now, &id_str],
            )?;
                let posted_amount: Decimal = pending.iter().map(|e| e.amount).sum();
                let first_period = pending.first().map_or(0, |e| e.period);
                let last_period = pending.last().map_or(0, |e| e.period);
                Ok((Self::load_asset(tx, &id_str)?, posted_amount, first_period, last_period))
            },
        )?;
        self.auto_post_depreciation_entry(&asset, posted_amount, first_period, last_period)?;
        Ok(asset)
    }
}

impl SqliteFixedAssetRepository {
    /// Create and post a balanced depreciation journal entry
    /// (debit depreciation expense, credit accumulated depreciation) when the
    /// active GL auto-posting config has `auto_post_depreciation` enabled.
    ///
    /// Mirrors the invoice auto-posting pattern: config-gated, posted via the
    /// existing general-ledger repository. Accounts come from the asset itself,
    /// falling back to the first active posting account with the matching
    /// account sub-type.
    fn auto_post_depreciation_entry(
        &self,
        asset: &FixedAsset,
        amount: Decimal,
        first_period: u32,
        last_period: u32,
    ) -> Result<()> {
        use stateset_core::GeneralLedgerRepository;
        let gl = super::general_ledger::SqliteGeneralLedgerRepository::new(self.pool.clone());
        let Some(config) = gl.get_auto_posting_config()? else { return Ok(()) };
        if !config.auto_post_depreciation || amount <= Decimal::ZERO {
            return Ok(());
        }
        let expense_account_id = resolve_depreciation_account(
            &gl,
            asset.depreciation_expense_account_id,
            stateset_core::AccountSubType::DepreciationExpense,
        )?;
        let accumulated_account_id = resolve_depreciation_account(
            &gl,
            asset.accumulated_depreciation_account_id,
            stateset_core::AccountSubType::AccumulatedDepreciation,
        )?;
        let periods = if first_period == last_period {
            format!("period {first_period}")
        } else {
            format!("periods {first_period}-{last_period}")
        };
        gl.create_journal_entry(stateset_core::CreateJournalEntry {
            entry_date: Utc::now().date_naive(),
            entry_type: Some(stateset_core::JournalEntryType::Standard),
            description: format!("Depreciation {} {periods}", asset.asset_number),
            lines: vec![
                stateset_core::CreateJournalEntryLine::debit(
                    expense_account_id,
                    amount,
                    Some("Depreciation Expense".to_string()),
                ),
                stateset_core::CreateJournalEntryLine::credit(
                    accumulated_account_id,
                    amount,
                    Some("Accumulated Depreciation".to_string()),
                ),
            ],
            source_document_type: Some("fixed_asset_depreciation".to_string()),
            source_document_id: Some(asset.id),
            auto_post: Some(true),
        })?;
        Ok(())
    }
}

/// Resolve a GL account for depreciation posting: prefer the account
/// configured on the asset, then fall back to the first active posting
/// account with the given sub-type in the chart of accounts.
fn resolve_depreciation_account(
    gl: &dyn stateset_core::GeneralLedgerRepository,
    preferred: Option<Uuid>,
    sub_type: stateset_core::AccountSubType,
) -> Result<Uuid> {
    if let Some(id) = preferred {
        return Ok(id);
    }
    gl.list_accounts(stateset_core::GlAccountFilter {
        account_sub_type: Some(sub_type),
        status: Some(stateset_core::AccountStatus::Active),
        is_posting: Some(true),
        limit: Some(1),
        ..Default::default()
    })?
    .first()
    .map(|a| a.id)
    .ok_or_else(|| {
        CommerceError::ValidationError(format!(
            "auto-post depreciation requires a {sub_type} account on the asset or in the chart of accounts"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::FixedAssetRepository;

    fn test_repo() -> SqliteFixedAssetRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqliteFixedAssetRepository::new(db.pool().clone())
    }

    fn create_input() -> CreateFixedAsset {
        CreateFixedAsset {
            asset_number: None,
            name: "Forklift".into(),
            description: Some("Warehouse forklift".into()),
            category: FixedAssetCategory::Machinery,
            acquisition_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            acquisition_cost: dec!(10000),
            salvage_value: dec!(1000),
            useful_life_months: 36,
            depreciation_method: DepreciationMethod::StraightLine,
            in_service_date: None,
            location_id: None,
            asset_account_id: None,
            accumulated_depreciation_account_id: None,
            depreciation_expense_account_id: None,
            currency: None,
        }
    }

    #[test]
    fn create_get_defaults_to_draft() {
        let repo = test_repo();
        let a = repo.create(create_input()).expect("create");
        assert_eq!(a.status, FixedAssetStatus::Draft);
        assert!(a.asset_number.starts_with("FA-"));
        let fetched = repo.get(a.id).expect("get").expect("found");
        assert_eq!(fetched.acquisition_cost, dec!(10000));
        assert_eq!(fetched.book_value(), dec!(10000));
        assert_eq!(fetched.depreciation_method, DepreciationMethod::StraightLine);
    }

    #[test]
    fn create_with_in_service_date_is_in_service() {
        let repo = test_repo();
        let mut input = create_input();
        input.in_service_date = NaiveDate::from_ymd_opt(2026, 2, 1);
        let a = repo.create(input).expect("create");
        assert_eq!(a.status, FixedAssetStatus::InService);
    }

    #[test]
    fn create_rejects_invalid_input() {
        let repo = test_repo();
        let mut input = create_input();
        input.salvage_value = dec!(20000);
        assert!(repo.create(input).is_err());
    }

    #[test]
    fn full_lifecycle_with_schedule_totals_exact() {
        let repo = test_repo();
        let a = repo.create(create_input()).expect("create");
        let a = repo
            .place_in_service(a.id, NaiveDate::from_ymd_opt(2026, 2, 1).unwrap())
            .expect("place in service");
        assert_eq!(a.status, FixedAssetStatus::InService);

        let schedule = repo.generate_schedule(a.id).expect("generate schedule");
        assert_eq!(schedule.entries.len(), 36);
        assert_eq!(schedule.total_depreciation, dec!(9000));
        let total: Decimal = schedule.entries.iter().map(|e| e.amount).sum();
        assert_eq!(total, dec!(9000));
        assert_eq!(schedule.entries.last().unwrap().book_value, dec!(1000));

        // Post 3 periods of depreciation.
        let a = repo.post_depreciation(a.id, 3).expect("post depreciation");
        assert_eq!(a.accumulated_depreciation, dec!(750));
        assert_eq!(a.book_value(), dec!(9250));
        let persisted = repo.get_schedule(a.id).expect("get schedule").expect("some");
        let posted = persisted
            .entries
            .iter()
            .filter(|e| e.status == DepreciationEntryStatus::Posted)
            .count();
        assert_eq!(posted, 3);

        // Dispose for 9500 -> gain of 250 against book value 9250.
        let a = repo
            .dispose(a.id, NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(), dec!(9500), None)
            .expect("dispose");
        assert_eq!(a.status, FixedAssetStatus::Disposed);
        let d = a.disposal.expect("disposal recorded");
        assert_eq!(d.book_value_at_disposal, dec!(9250));
        assert_eq!(d.gain_loss, dec!(250));
    }

    #[test]
    fn post_all_periods_fully_depreciates() {
        let repo = test_repo();
        let mut input = create_input();
        input.useful_life_months = 3;
        let a = repo.create(input).expect("create");
        repo.place_in_service(a.id, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
            .expect("in service");
        repo.generate_schedule(a.id).expect("schedule");
        let a = repo.post_depreciation(a.id, 3).expect("post");
        assert_eq!(a.status, FixedAssetStatus::FullyDepreciated);
        assert_eq!(a.accumulated_depreciation, dec!(9000));
        assert!(a.is_fully_depreciated());
        assert!(repo.post_depreciation(a.id, 1).is_err());
    }

    #[test]
    fn regenerate_after_posting_conflicts() {
        let repo = test_repo();
        let a = repo.create(create_input()).expect("create");
        repo.place_in_service(a.id, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
            .expect("in service");
        repo.generate_schedule(a.id).expect("schedule");
        repo.post_depreciation(a.id, 1).expect("post");
        assert!(repo.generate_schedule(a.id).is_err());
    }

    #[test]
    fn write_off_records_zero_proceeds_loss() {
        let repo = test_repo();
        let a = repo.create(create_input()).expect("create");
        repo.place_in_service(a.id, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
            .expect("in service");
        let a = repo
            .write_off(a.id, NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(), Some("stolen".into()))
            .expect("write off");
        assert_eq!(a.status, FixedAssetStatus::WrittenOff);
        let d = a.disposal.expect("disposal");
        assert_eq!(d.proceeds, dec!(0));
        assert_eq!(d.gain_loss, dec!(-10000));
        assert_eq!(d.notes.as_deref(), Some("stolen"));
    }

    #[test]
    fn invalid_transitions_conflict() {
        let repo = test_repo();
        let a = repo.create(create_input()).expect("create");
        // Draft cannot be disposed.
        assert!(
            repo.dispose(a.id, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), dec!(1), None)
                .is_err()
        );
        repo.place_in_service(a.id, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
            .expect("in service");
        // Cannot place in service twice.
        assert!(repo.place_in_service(a.id, NaiveDate::from_ymd_opt(2026, 1, 2).unwrap()).is_err());
        // Terminal assets cannot be updated.
        repo.write_off(a.id, NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(), None)
            .expect("write off");
        assert!(repo.update(a.id, UpdateFixedAsset::default()).is_err());
    }

    #[test]
    fn list_filters_by_status_and_search() {
        let repo = test_repo();
        let a = repo.create(create_input()).expect("create");
        let mut other = create_input();
        other.name = "Delivery Van".into();
        other.category = FixedAssetCategory::Vehicle;
        repo.create(other).expect("create other");
        repo.place_in_service(a.id, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
            .expect("in service");

        let in_service = repo
            .list(FixedAssetFilter {
                status: Some(FixedAssetStatus::InService),
                ..Default::default()
            })
            .expect("list");
        assert_eq!(in_service.len(), 1);
        assert_eq!(in_service[0].id, a.id);

        let vans = repo
            .list(FixedAssetFilter { search: Some("van".into()), ..Default::default() })
            .expect("list search");
        assert_eq!(vans.len(), 1);
        assert_eq!(vans[0].name, "Delivery Van");
    }

    mod gl_auto_posting {
        use super::*;
        use crate::sqlite::general_ledger::SqliteGeneralLedgerRepository;
        use stateset_core::{
            AccountSubType, AccountType, CreateAutoPostingConfig, CreateGlAccount, CreateGlPeriod,
            GeneralLedgerRepository, JournalEntryFilter, JournalEntryStatus,
        };

        fn test_repos() -> (SqliteFixedAssetRepository, SqliteGeneralLedgerRepository) {
            let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
            (
                SqliteFixedAssetRepository::new(db.pool().clone()),
                SqliteGeneralLedgerRepository::new(db.pool().clone()),
            )
        }

        fn create_sub_account(
            gl: &SqliteGeneralLedgerRepository,
            number: &str,
            name: &str,
            account_type: AccountType,
            sub_type: AccountSubType,
        ) -> Uuid {
            gl.create_account(CreateGlAccount {
                account_number: number.into(),
                name: name.into(),
                description: None,
                account_type,
                account_sub_type: Some(sub_type),
                parent_account_id: None,
                is_header: Some(false),
                is_posting: Some(true),
                currency: None,
            })
            .expect("create account")
            .id
        }

        fn setup_gl(gl: &SqliteGeneralLedgerRepository, auto_post_depreciation: bool) {
            gl.initialize_chart_of_accounts().expect("init chart");
            let period = gl
                .create_period(CreateGlPeriod {
                    period_name: "All".into(),
                    fiscal_year: 2026,
                    period_number: 1,
                    start_date: NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
                    end_date: NaiveDate::from_ymd_opt(2030, 12, 31).unwrap(),
                })
                .expect("create period");
            gl.open_period(period.id).expect("open period");
            let by_number = |n: &str| {
                gl.get_account_by_number(n).expect("get account").expect("account exists").id
            };
            gl.set_auto_posting_config(CreateAutoPostingConfig {
                config_name: "Test".into(),
                cash_account_id: by_number("1010"),
                accounts_receivable_account_id: by_number("1100"),
                inventory_account_id: by_number("1200"),
                accounts_payable_account_id: by_number("2010"),
                unearned_revenue_account_id: None,
                sales_revenue_account_id: by_number("4010"),
                shipping_revenue_account_id: None,
                cogs_account_id: by_number("5010"),
                bad_debt_expense_account_id: None,
                fx_gain_loss_account_id: None,
                auto_post_depreciation,
                auto_post_revenue_recognition: false,
            })
            .expect("set config");
        }

        fn in_service_asset(repo: &SqliteFixedAssetRepository) -> FixedAsset {
            let a = repo.create(create_input()).expect("create");
            let a = repo
                .place_in_service(a.id, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
                .expect("in service");
            repo.generate_schedule(a.id).expect("schedule");
            a
        }

        #[test]
        fn post_depreciation_auto_posts_balanced_journal_entry_when_enabled() {
            let (repo, gl) = test_repos();
            setup_gl(&gl, true);
            let expense_id = create_sub_account(
                &gl,
                "5300",
                "Depreciation Expense",
                AccountType::Expense,
                AccountSubType::DepreciationExpense,
            );
            let accum_id = create_sub_account(
                &gl,
                "1510",
                "Accumulated Depreciation",
                AccountType::Asset,
                AccountSubType::AccumulatedDepreciation,
            );

            let a = in_service_asset(&repo);
            let a = repo.post_depreciation(a.id, 3).expect("post depreciation");
            assert_eq!(a.accumulated_depreciation, dec!(750));

            let entries = gl
                .list_journal_entries(JournalEntryFilter {
                    source_document_id: Some(a.id),
                    ..Default::default()
                })
                .expect("list entries");
            assert_eq!(entries.len(), 1);
            let entry = &entries[0];
            assert_eq!(entry.status, JournalEntryStatus::Posted);
            assert!(entry.is_balanced);
            assert_eq!(entry.total_debits, dec!(750));
            assert_eq!(entry.total_credits, dec!(750));
            assert_eq!(entry.source_document_type.as_deref(), Some("fixed_asset_depreciation"));
            assert_eq!(entry.lines.len(), 2);
            let debit = entry.lines.iter().find(|l| l.debit_amount > dec!(0)).expect("debit");
            let credit = entry.lines.iter().find(|l| l.credit_amount > dec!(0)).expect("credit");
            assert_eq!(debit.account_id, expense_id);
            assert_eq!(debit.debit_amount, dec!(750));
            assert_eq!(credit.account_id, accum_id);
            assert_eq!(credit.credit_amount, dec!(750));
        }

        #[test]
        fn post_depreciation_prefers_asset_level_accounts() {
            let (repo, gl) = test_repos();
            setup_gl(&gl, true);
            let expense_id = create_sub_account(
                &gl,
                "5301",
                "Machinery Depreciation Expense",
                AccountType::Expense,
                AccountSubType::DepreciationExpense,
            );
            let accum_id = create_sub_account(
                &gl,
                "1511",
                "Machinery Accumulated Depreciation",
                AccountType::Asset,
                AccountSubType::AccumulatedDepreciation,
            );

            let mut input = create_input();
            input.depreciation_expense_account_id = Some(expense_id);
            input.accumulated_depreciation_account_id = Some(accum_id);
            let a = repo.create(input).expect("create");
            let a = repo
                .place_in_service(a.id, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap())
                .expect("in service");
            repo.generate_schedule(a.id).expect("schedule");
            repo.post_depreciation(a.id, 1).expect("post depreciation");

            let entries = gl
                .list_journal_entries(JournalEntryFilter {
                    source_document_id: Some(a.id),
                    ..Default::default()
                })
                .expect("list entries");
            assert_eq!(entries.len(), 1);
            let accounts: Vec<Uuid> = entries[0].lines.iter().map(|l| l.account_id).collect();
            assert!(accounts.contains(&expense_id));
            assert!(accounts.contains(&accum_id));
        }

        #[test]
        fn post_depreciation_creates_no_journal_entry_when_disabled() {
            let (repo, gl) = test_repos();
            setup_gl(&gl, false);
            let a = in_service_asset(&repo);
            let a = repo.post_depreciation(a.id, 3).expect("post depreciation");
            assert_eq!(a.accumulated_depreciation, dec!(750));

            let entries = gl
                .list_journal_entries(JournalEntryFilter {
                    source_document_id: Some(a.id),
                    ..Default::default()
                })
                .expect("list entries");
            assert!(entries.is_empty());
        }

        #[test]
        fn post_depreciation_without_config_creates_no_journal_entry() {
            let (repo, gl) = test_repos();
            let a = in_service_asset(&repo);
            repo.post_depreciation(a.id, 1).expect("post depreciation");
            let entries = gl
                .list_journal_entries(JournalEntryFilter {
                    source_document_id: Some(a.id),
                    ..Default::default()
                })
                .expect("list entries");
            assert!(entries.is_empty());
        }
    }

    #[test]
    fn declining_balance_schedule_round_trips() {
        let repo = test_repo();
        let mut input = create_input();
        input.depreciation_method = DepreciationMethod::DecliningBalance { rate: dec!(0.2) };
        input.useful_life_months = 12;
        input.salvage_value = dec!(500);
        let a = repo.create(input).expect("create");
        assert_eq!(a.depreciation_method, DepreciationMethod::DecliningBalance { rate: dec!(0.2) });
        let schedule = repo.generate_schedule(a.id).expect("schedule");
        let total: Decimal = schedule.entries.iter().map(|e| e.amount).sum();
        assert_eq!(total, dec!(9500));
        assert_eq!(schedule.entries[0].amount, dec!(2000));
    }
}
