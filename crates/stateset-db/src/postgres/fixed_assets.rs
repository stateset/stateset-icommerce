//! PostgreSQL fixed asset repository implementation

use super::map_db_error;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    AssetDisposal, CommerceError, CreateFixedAsset, CurrencyCode, DepreciationEntry,
    DepreciationEntryStatus, DepreciationMethod, DepreciationSchedule, FixedAsset,
    FixedAssetCategory, FixedAssetFilter, FixedAssetRepository, FixedAssetStatus, Result,
    UpdateFixedAsset, Validate, generate_asset_number, generate_depreciation_schedule,
};
use uuid::Uuid;

/// PostgreSQL implementation of `FixedAssetRepository`
#[derive(Debug, Clone)]
pub struct PgFixedAssetRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct FixedAssetRow {
    id: Uuid,
    asset_number: String,
    name: String,
    description: Option<String>,
    category: String,
    acquisition_date: NaiveDate,
    acquisition_cost: Decimal,
    salvage_value: Decimal,
    useful_life_months: i32,
    depreciation_method: String,
    status: String,
    in_service_date: Option<NaiveDate>,
    location_id: Option<Uuid>,
    asset_account_id: Option<Uuid>,
    accumulated_depreciation_account_id: Option<Uuid>,
    depreciation_expense_account_id: Option<Uuid>,
    accumulated_depreciation: Decimal,
    currency: String,
    disposal_date: Option<NaiveDate>,
    disposal_proceeds: Option<Decimal>,
    disposal_book_value: Option<Decimal>,
    disposal_gain_loss: Option<Decimal>,
    disposal_notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct DepreciationEntryRow {
    period: i32,
    amount: Decimal,
    accumulated: Decimal,
    book_value: Decimal,
    status: String,
}

impl PgFixedAssetRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn parse_field<T: std::str::FromStr>(raw: &str, field: &str) -> Result<T>
    where
        T::Err: std::fmt::Display,
    {
        raw.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid fixed_asset.{field} '{raw}': {e}"))
        })
    }

    fn row_to_asset(row: FixedAssetRow) -> Result<FixedAsset> {
        let disposal = match row.disposal_date {
            Some(disposal_date) => Some(AssetDisposal {
                disposal_date,
                proceeds: row.disposal_proceeds.unwrap_or(Decimal::ZERO),
                book_value_at_disposal: row.disposal_book_value.unwrap_or(Decimal::ZERO),
                gain_loss: row.disposal_gain_loss.unwrap_or(Decimal::ZERO),
                notes: row.disposal_notes,
            }),
            None => None,
        };
        Ok(FixedAsset {
            id: row.id,
            asset_number: row.asset_number,
            name: row.name,
            description: row.description,
            category: Self::parse_field::<FixedAssetCategory>(&row.category, "category")?,
            acquisition_date: row.acquisition_date,
            acquisition_cost: row.acquisition_cost,
            salvage_value: row.salvage_value,
            useful_life_months: u32::try_from(row.useful_life_months).unwrap_or(0),
            depreciation_method: serde_json::from_str::<DepreciationMethod>(
                &row.depreciation_method,
            )
            .map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "Invalid fixed_asset.depreciation_method: {e}"
                ))
            })?,
            status: Self::parse_field::<FixedAssetStatus>(&row.status, "status")?,
            in_service_date: row.in_service_date,
            location_id: row.location_id,
            asset_account_id: row.asset_account_id,
            accumulated_depreciation_account_id: row.accumulated_depreciation_account_id,
            depreciation_expense_account_id: row.depreciation_expense_account_id,
            accumulated_depreciation: row.accumulated_depreciation,
            currency: Self::parse_field::<CurrencyCode>(&row.currency, "currency")?,
            disposal,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_entry(row: DepreciationEntryRow) -> Result<DepreciationEntry> {
        Ok(DepreciationEntry {
            period: u32::try_from(row.period).unwrap_or(0),
            amount: row.amount,
            accumulated: row.accumulated,
            book_value: row.book_value,
            status: Self::parse_field::<DepreciationEntryStatus>(
                &row.status,
                "depreciation_entry.status",
            )?,
        })
    }

    async fn load_asset_async(&self, id: Uuid) -> Result<Option<FixedAsset>> {
        let row = sqlx::query_as::<_, FixedAssetRow>("SELECT * FROM fixed_assets WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_asset).transpose()
    }

    async fn require_asset_locked(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        id: Uuid,
    ) -> Result<FixedAsset> {
        let row = sqlx::query_as::<_, FixedAssetRow>(
            "SELECT * FROM fixed_assets WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        row.map(Self::row_to_asset).transpose()?.ok_or(CommerceError::NotFound)
    }

    async fn load_entries_async(&self, id: Uuid) -> Result<Vec<DepreciationEntry>> {
        let rows = sqlx::query_as::<_, DepreciationEntryRow>(
            "SELECT * FROM fixed_asset_depreciation_entries WHERE asset_id = $1 ORDER BY period",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_entry).collect()
    }

    /// Create a fixed asset (async)
    pub async fn create_async(&self, input: CreateFixedAsset) -> Result<FixedAsset> {
        input.validate()?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let asset_number = input.asset_number.clone().unwrap_or_else(generate_asset_number);
        let currency = input.currency.unwrap_or_default();
        let status = if input.in_service_date.is_some() {
            FixedAssetStatus::InService
        } else {
            FixedAssetStatus::Draft
        };
        let method_json = serde_json::to_string(&input.depreciation_method)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        sqlx::query(
            "INSERT INTO fixed_assets (id, asset_number, name, description, category, acquisition_date, acquisition_cost, salvage_value, useful_life_months, depreciation_method, status, in_service_date, location_id, asset_account_id, accumulated_depreciation_account_id, depreciation_expense_account_id, accumulated_depreciation, currency, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, 0, $17, $18, $18)",
        )
        .bind(id)
        .bind(&asset_number)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.category.to_string())
        .bind(input.acquisition_date)
        .bind(input.acquisition_cost)
        .bind(input.salvage_value)
        .bind(i32::try_from(input.useful_life_months).unwrap_or(i32::MAX))
        .bind(&method_json)
        .bind(status.to_string())
        .bind(input.in_service_date)
        .bind(input.location_id)
        .bind(input.asset_account_id)
        .bind(input.accumulated_depreciation_account_id)
        .bind(input.depreciation_expense_account_id)
        .bind(currency.to_string())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;
        self.load_asset_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a fixed asset (async)
    pub async fn get_async(&self, id: Uuid) -> Result<Option<FixedAsset>> {
        self.load_asset_async(id).await
    }

    /// List fixed assets (async)
    pub async fn list_async(&self, filter: FixedAssetFilter) -> Result<Vec<FixedAsset>> {
        let limit = i64::from(filter.limit.unwrap_or(100));
        let offset = i64::from(filter.offset.unwrap_or(0));
        let mut query = String::from("SELECT * FROM fixed_assets WHERE 1=1");
        let mut idx = 1;
        if filter.category.is_some() {
            query.push_str(&format!(" AND category = ${idx}"));
            idx += 1;
        }
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${idx}"));
            idx += 1;
        }
        if filter.location_id.is_some() {
            query.push_str(&format!(" AND location_id = ${idx}"));
            idx += 1;
        }
        if filter.acquired_from.is_some() {
            query.push_str(&format!(" AND acquisition_date >= ${idx}"));
            idx += 1;
        }
        if filter.acquired_to.is_some() {
            query.push_str(&format!(" AND acquisition_date <= ${idx}"));
            idx += 1;
        }
        if filter.search.is_some() {
            query.push_str(&format!(" AND (name ILIKE ${idx} OR asset_number ILIKE ${idx})"));
            idx += 1;
        }
        query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET ${}", idx, idx + 1));

        let mut q = sqlx::query_as::<_, FixedAssetRow>(&query);
        if let Some(category) = filter.category {
            q = q.bind(category.to_string());
        }
        if let Some(status) = filter.status {
            q = q.bind(status.to_string());
        }
        if let Some(location) = filter.location_id {
            q = q.bind(location);
        }
        if let Some(from) = filter.acquired_from {
            q = q.bind(from);
        }
        if let Some(to) = filter.acquired_to {
            q = q.bind(to);
        }
        if let Some(search) = filter.search {
            q = q.bind(format!("%{search}%"));
        }
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_asset).collect()
    }

    /// Update a fixed asset (async)
    pub async fn update_async(&self, id: Uuid, input: UpdateFixedAsset) -> Result<FixedAsset> {
        input.validate()?;
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let asset = Self::require_asset_locked(&mut tx, id).await?;
        if asset.status.is_terminal() {
            return Err(CommerceError::Conflict(
                "disposed or written-off assets cannot be updated".into(),
            ));
        }
        sqlx::query(
            "UPDATE fixed_assets SET name = $1, description = $2, category = $3, salvage_value = $4, useful_life_months = $5, in_service_date = $6, location_id = $7, asset_account_id = $8, accumulated_depreciation_account_id = $9, depreciation_expense_account_id = $10, updated_at = $11 WHERE id = $12",
        )
        .bind(input.name.unwrap_or(asset.name))
        .bind(input.description.or(asset.description))
        .bind(input.category.unwrap_or(asset.category).to_string())
        .bind(input.salvage_value.unwrap_or(asset.salvage_value))
        .bind(
            i32::try_from(input.useful_life_months.unwrap_or(asset.useful_life_months))
                .unwrap_or(i32::MAX),
        )
        .bind(input.in_service_date.or(asset.in_service_date))
        .bind(input.location_id.or(asset.location_id))
        .bind(input.asset_account_id.or(asset.asset_account_id))
        .bind(
            input
                .accumulated_depreciation_account_id
                .or(asset.accumulated_depreciation_account_id),
        )
        .bind(input.depreciation_expense_account_id.or(asset.depreciation_expense_account_id))
        .bind(Utc::now())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        self.load_asset_async(id).await?.ok_or(CommerceError::NotFound)
    }

    async fn transition_async(
        &self,
        id: Uuid,
        next: FixedAssetStatus,
        date: NaiveDate,
        proceeds: Option<Decimal>,
        notes: Option<String>,
    ) -> Result<FixedAsset> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let asset = Self::require_asset_locked(&mut tx, id).await?;
        if !asset.status.can_transition_to(next) {
            return Err(CommerceError::Conflict(format!(
                "fixed asset cannot transition from {} to {next}",
                asset.status
            )));
        }
        let now = Utc::now();
        match next {
            FixedAssetStatus::InService => {
                sqlx::query(
                    "UPDATE fixed_assets SET status = 'in_service', in_service_date = $1, updated_at = $2 WHERE id = $3",
                )
                .bind(date)
                .bind(now)
                .bind(id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            }
            FixedAssetStatus::Disposed | FixedAssetStatus::WrittenOff => {
                let disposal = AssetDisposal::new(
                    date,
                    proceeds.unwrap_or(Decimal::ZERO),
                    asset.book_value(),
                    notes,
                );
                sqlx::query(
                    "UPDATE fixed_assets SET status = $1, disposal_date = $2, disposal_proceeds = $3, disposal_book_value = $4, disposal_gain_loss = $5, disposal_notes = $6, updated_at = $7 WHERE id = $8",
                )
                .bind(next.to_string())
                .bind(disposal.disposal_date)
                .bind(disposal.proceeds)
                .bind(disposal.book_value_at_disposal)
                .bind(disposal.gain_loss)
                .bind(&disposal.notes)
                .bind(now)
                .bind(id)
                .execute(tx.as_mut())
                .await
                .map_err(map_db_error)?;
            }
            _ => {
                sqlx::query("UPDATE fixed_assets SET status = $1, updated_at = $2 WHERE id = $3")
                    .bind(next.to_string())
                    .bind(now)
                    .bind(id)
                    .execute(tx.as_mut())
                    .await
                    .map_err(map_db_error)?;
            }
        }
        tx.commit().await.map_err(map_db_error)?;
        self.load_asset_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Place a draft asset in service (async).
    pub async fn place_in_service_async(&self, id: Uuid, date: NaiveDate) -> Result<FixedAsset> {
        self.transition_async(id, FixedAssetStatus::InService, date, None, None).await
    }

    /// Dispose of an asset for the given proceeds, recording gain/loss (async).
    pub async fn dispose_async(
        &self,
        id: Uuid,
        date: NaiveDate,
        proceeds: Decimal,
        notes: Option<String>,
    ) -> Result<FixedAsset> {
        if proceeds < Decimal::ZERO {
            return Err(CommerceError::ValidationError("proceeds must be non-negative".into()));
        }
        self.transition_async(id, FixedAssetStatus::Disposed, date, Some(proceeds), notes).await
    }

    /// Write off an asset (disposal with zero proceeds) (async).
    pub async fn write_off_async(
        &self,
        id: Uuid,
        date: NaiveDate,
        notes: Option<String>,
    ) -> Result<FixedAsset> {
        self.transition_async(id, FixedAssetStatus::WrittenOff, date, Some(Decimal::ZERO), notes)
            .await
    }

    /// Generate and persist the depreciation schedule (async)
    pub async fn generate_schedule_async(&self, id: Uuid) -> Result<DepreciationSchedule> {
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let asset = Self::require_asset_locked(&mut tx, id).await?;
        let posted: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM fixed_asset_depreciation_entries WHERE asset_id = $1 AND status = 'posted'",
        )
        .bind(id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        if posted > 0 {
            return Err(CommerceError::Conflict(
                "cannot regenerate a schedule with posted depreciation entries".into(),
            ));
        }
        let entries = generate_depreciation_schedule(
            asset.depreciation_method,
            asset.acquisition_cost,
            asset.salvage_value,
            asset.useful_life_months,
        );
        if entries.is_empty() {
            return Err(CommerceError::ValidationError(
                "cannot generate a time-based depreciation schedule for this asset".into(),
            ));
        }
        sqlx::query("DELETE FROM fixed_asset_depreciation_entries WHERE asset_id = $1")
            .bind(id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        for e in &entries {
            sqlx::query(
                "INSERT INTO fixed_asset_depreciation_entries (asset_id, period, amount, accumulated, book_value, status)
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(id)
            .bind(i32::try_from(e.period).unwrap_or(i32::MAX))
            .bind(e.amount)
            .bind(e.accumulated)
            .bind(e.book_value)
            .bind(e.status.to_string())
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }
        tx.commit().await.map_err(map_db_error)?;
        Ok(DepreciationSchedule {
            asset_id: id,
            method: asset.depreciation_method,
            total_depreciation: asset.depreciable_base(),
            entries,
        })
    }

    /// Get the persisted depreciation schedule (async)
    pub async fn get_schedule_async(&self, id: Uuid) -> Result<Option<DepreciationSchedule>> {
        let Some(asset) = self.load_asset_async(id).await? else {
            return Ok(None);
        };
        let entries = self.load_entries_async(id).await?;
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

    /// Post the next `periods` scheduled depreciation entries (async)
    pub async fn post_depreciation_async(&self, id: Uuid, periods: u32) -> Result<FixedAsset> {
        if periods == 0 {
            return Err(CommerceError::ValidationError("periods must be positive".into()));
        }
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;
        let asset = Self::require_asset_locked(&mut tx, id).await?;
        if asset.status != FixedAssetStatus::InService {
            return Err(CommerceError::Conflict(
                "depreciation can only be posted for in-service assets".into(),
            ));
        }
        let rows = sqlx::query_as::<_, DepreciationEntryRow>(
            "SELECT * FROM fixed_asset_depreciation_entries WHERE asset_id = $1 AND status = 'scheduled' ORDER BY period LIMIT $2",
        )
        .bind(id)
        .bind(i64::from(periods))
        .fetch_all(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        let pending: Vec<DepreciationEntry> =
            rows.into_iter().map(Self::row_to_entry).collect::<Result<_>>()?;
        if pending.is_empty() {
            return Err(CommerceError::Conflict(
                "no scheduled depreciation entries remain to post".into(),
            ));
        }
        for e in &pending {
            sqlx::query(
                "UPDATE fixed_asset_depreciation_entries SET status = 'posted' WHERE asset_id = $1 AND period = $2",
            )
            .bind(id)
            .bind(i32::try_from(e.period).unwrap_or(i32::MAX))
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }
        let accumulated =
            pending.last().map(|e| e.accumulated).unwrap_or(asset.accumulated_depreciation);
        let status = if accumulated >= asset.depreciable_base() {
            FixedAssetStatus::FullyDepreciated
        } else {
            FixedAssetStatus::InService
        };
        sqlx::query(
            "UPDATE fixed_assets SET accumulated_depreciation = $1, status = $2, updated_at = $3 WHERE id = $4",
        )
        .bind(accumulated)
        .bind(status.to_string())
        .bind(Utc::now())
        .bind(id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;
        tx.commit().await.map_err(map_db_error)?;
        let asset = self.load_asset_async(id).await?.ok_or(CommerceError::NotFound)?;
        let posted_amount: Decimal = pending.iter().map(|e| e.amount).sum();
        let first_period = pending.first().map_or(0, |e| e.period);
        let last_period = pending.last().map_or(0, |e| e.period);
        self.auto_post_depreciation_entry(&asset, posted_amount, first_period, last_period).await?;
        Ok(asset)
    }

    /// Create and post a balanced depreciation journal entry
    /// (debit depreciation expense, credit accumulated depreciation) when the
    /// active GL auto-posting config has `auto_post_depreciation` enabled.
    ///
    /// Mirrors the invoice auto-posting pattern: config-gated, posted via the
    /// existing general-ledger repository. Accounts come from the asset itself,
    /// falling back to the first active posting account with the matching
    /// account sub-type.
    async fn auto_post_depreciation_entry(
        &self,
        asset: &FixedAsset,
        amount: Decimal,
        first_period: u32,
        last_period: u32,
    ) -> Result<()> {
        let gl = super::general_ledger::PgGeneralLedgerRepository::new(self.pool.clone());
        let Some(config) = gl.get_auto_posting_config_async().await? else { return Ok(()) };
        if !config.auto_post_depreciation || amount <= Decimal::ZERO {
            return Ok(());
        }
        let expense_account_id = Self::resolve_depreciation_account(
            &gl,
            asset.depreciation_expense_account_id,
            stateset_core::AccountSubType::DepreciationExpense,
        )
        .await?;
        let accumulated_account_id = Self::resolve_depreciation_account(
            &gl,
            asset.accumulated_depreciation_account_id,
            stateset_core::AccountSubType::AccumulatedDepreciation,
        )
        .await?;
        let periods = if first_period == last_period {
            format!("period {first_period}")
        } else {
            format!("periods {first_period}-{last_period}")
        };
        gl.create_journal_entry_async(stateset_core::CreateJournalEntry {
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
        })
        .await?;
        Ok(())
    }

    /// Resolve a GL account for depreciation posting: prefer the account
    /// configured on the asset, then fall back to the first active posting
    /// account with the given sub-type in the chart of accounts.
    async fn resolve_depreciation_account(
        gl: &super::general_ledger::PgGeneralLedgerRepository,
        preferred: Option<Uuid>,
        sub_type: stateset_core::AccountSubType,
    ) -> Result<Uuid> {
        if let Some(id) = preferred {
            return Ok(id);
        }
        gl.list_accounts_async(stateset_core::GlAccountFilter {
            account_sub_type: Some(sub_type),
            status: Some(stateset_core::AccountStatus::Active),
            is_posting: Some(true),
            limit: Some(1),
            ..Default::default()
        })
        .await?
        .first()
        .map(|a| a.id)
        .ok_or_else(|| {
            CommerceError::ValidationError(format!(
                "auto-post depreciation requires a {sub_type} account on the asset or in the chart of accounts"
            ))
        })
    }
}

impl FixedAssetRepository for PgFixedAssetRepository {
    fn create(&self, input: CreateFixedAsset) -> Result<FixedAsset> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: Uuid) -> Result<Option<FixedAsset>> {
        super::block_on(self.get_async(id))
    }

    fn list(&self, filter: FixedAssetFilter) -> Result<Vec<FixedAsset>> {
        super::block_on(self.list_async(filter))
    }

    fn update(&self, id: Uuid, input: UpdateFixedAsset) -> Result<FixedAsset> {
        super::block_on(self.update_async(id, input))
    }

    fn place_in_service(&self, id: Uuid, date: NaiveDate) -> Result<FixedAsset> {
        super::block_on(self.place_in_service_async(id, date))
    }

    fn dispose(
        &self,
        id: Uuid,
        date: NaiveDate,
        proceeds: Decimal,
        notes: Option<String>,
    ) -> Result<FixedAsset> {
        super::block_on(self.dispose_async(id, date, proceeds, notes))
    }

    fn write_off(&self, id: Uuid, date: NaiveDate, notes: Option<String>) -> Result<FixedAsset> {
        super::block_on(self.write_off_async(id, date, notes))
    }

    fn generate_schedule(&self, id: Uuid) -> Result<DepreciationSchedule> {
        super::block_on(self.generate_schedule_async(id))
    }

    fn get_schedule(&self, id: Uuid) -> Result<Option<DepreciationSchedule>> {
        super::block_on(self.get_schedule_async(id))
    }

    fn post_depreciation(&self, id: Uuid, periods: u32) -> Result<FixedAsset> {
        super::block_on(self.post_depreciation_async(id, periods))
    }
}
