//! PostgreSQL implementation of loyalty program repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    AdjustPoints, CommerceError, CreateLoyaltyProgram, CustomerId, EnrollCustomer, LoyaltyAccount,
    LoyaltyAccountFilter, LoyaltyAccountId, LoyaltyProgram, LoyaltyProgramId,
    LoyaltyProgramRepository, LoyaltyProgramStatus, LoyaltyTransaction, LoyaltyTransactionId,
    LoyaltyTransactionType, Result,
};
use uuid::Uuid;

/// PostgreSQL loyalty program repository
#[derive(Debug, Clone)]
pub struct PgLoyaltyProgramRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct LoyaltyProgramRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    points_per_dollar: i32,
    status: String,
    tiers: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LoyaltyAccountRow {
    id: Uuid,
    program_id: Uuid,
    customer_id: Uuid,
    points_balance: i64,
    lifetime_points: i64,
    tier: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct LoyaltyTransactionRow {
    id: Uuid,
    account_id: Uuid,
    points: i64,
    #[sqlx(rename = "type")]
    transaction_type: String,
    reference_id: Option<String>,
    notes: Option<String>,
    created_at: DateTime<Utc>,
}

impl PgLoyaltyProgramRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_program(row: LoyaltyProgramRow) -> Result<LoyaltyProgram> {
        let LoyaltyProgramRow {
            id,
            name,
            description,
            points_per_dollar,
            status,
            tiers,
            created_at,
            updated_at,
        } = row;

        let status: LoyaltyProgramStatus = status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid loyalty_program.status '{}': {}",
                status, e
            ))
        })?;

        // `tiers` is a JSON array column (migration 051); a NULL or parse
        // failure degrades to an empty tier list rather than erroring.
        let tiers = tiers.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();

        Ok(LoyaltyProgram {
            id: LoyaltyProgramId::from(id),
            name,
            description,
            points_per_dollar: points_per_dollar as u32,
            tiers,
            status,
            created_at,
            updated_at,
        })
    }

    fn row_to_account(row: LoyaltyAccountRow) -> Result<LoyaltyAccount> {
        let LoyaltyAccountRow {
            id,
            program_id,
            customer_id,
            points_balance,
            lifetime_points,
            tier,
            created_at,
            updated_at,
        } = row;

        Ok(LoyaltyAccount {
            id: LoyaltyAccountId::from(id),
            program_id: LoyaltyProgramId::from(program_id),
            customer_id: CustomerId::from(customer_id),
            points_balance,
            lifetime_points: lifetime_points.max(0) as u64,
            tier,
            created_at,
            updated_at,
        })
    }

    fn row_to_transaction(row: LoyaltyTransactionRow) -> Result<LoyaltyTransaction> {
        let LoyaltyTransactionRow {
            id,
            account_id,
            points,
            transaction_type,
            reference_id,
            notes,
            created_at,
        } = row;

        let transaction_type: LoyaltyTransactionType = transaction_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid loyalty_transaction.type '{}': {}",
                transaction_type, e
            ))
        })?;

        Ok(LoyaltyTransaction {
            id: LoyaltyTransactionId::from(id),
            account_id: LoyaltyAccountId::from(account_id),
            points,
            transaction_type,
            reference_id,
            description: notes,
            created_at,
        })
    }

    // ---- async methods ----

    /// Create a loyalty program (async)
    pub async fn create_async(&self, input: CreateLoyaltyProgram) -> Result<LoyaltyProgram> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let tiers_json = serde_json::to_string(&input.tiers).unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            "INSERT INTO loyalty_programs (id, name, description, points_per_dollar, status,
             tiers, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.points_per_dollar as i32)
        .bind(LoyaltyProgramStatus::Active.to_string())
        .bind(&tiers_json)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(LoyaltyProgramId::from(id)).await?.ok_or(CommerceError::NotFound)
    }

    /// Get loyalty program by ID (async)
    pub async fn get_async(&self, id: LoyaltyProgramId) -> Result<Option<LoyaltyProgram>> {
        let row = sqlx::query_as::<_, LoyaltyProgramRow>(
            "SELECT id, name, description, points_per_dollar, status, tiers, created_at, updated_at
             FROM loyalty_programs WHERE id = $1",
        )
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_program).transpose()
    }

    /// List all loyalty programs (async)
    pub async fn list_async(&self) -> Result<Vec<LoyaltyProgram>> {
        let rows = sqlx::query_as::<_, LoyaltyProgramRow>(
            "SELECT id, name, description, points_per_dollar, status, tiers, created_at, updated_at
             FROM loyalty_programs ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_program).collect()
    }

    /// Enroll a customer in a program (async)
    pub async fn enroll_async(&self, input: EnrollCustomer) -> Result<LoyaltyAccount> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO loyalty_accounts (id, program_id, customer_id, points_balance,
             lifetime_points, tier, created_at, updated_at)
             VALUES ($1, $2, $3, 0, 0, 'bronze', $4, $5)",
        )
        .bind(id)
        .bind(input.program_id.into_uuid())
        .bind(input.customer_id.into_uuid())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_account_async(LoyaltyAccountId::from(id)).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a loyalty account by ID (async)
    pub async fn get_account_async(&self, id: LoyaltyAccountId) -> Result<Option<LoyaltyAccount>> {
        let row = sqlx::query_as::<_, LoyaltyAccountRow>(
            "SELECT id, program_id, customer_id, points_balance, lifetime_points, tier,
             created_at, updated_at
             FROM loyalty_accounts WHERE id = $1",
        )
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_account).transpose()
    }

    /// Get loyalty account by customer and program (async)
    pub async fn get_account_by_customer_async(
        &self,
        customer_id: CustomerId,
        program_id: LoyaltyProgramId,
    ) -> Result<Option<LoyaltyAccount>> {
        let row = sqlx::query_as::<_, LoyaltyAccountRow>(
            "SELECT id, program_id, customer_id, points_balance, lifetime_points, tier,
             created_at, updated_at
             FROM loyalty_accounts WHERE customer_id = $1 AND program_id = $2",
        )
        .bind(customer_id.into_uuid())
        .bind(program_id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_account).transpose()
    }

    /// List loyalty accounts with filter (async)
    pub async fn list_accounts_async(
        &self,
        filter: LoyaltyAccountFilter,
    ) -> Result<Vec<LoyaltyAccount>> {
        let mut sql = String::from(
            "SELECT id, program_id, customer_id, points_balance, lifetime_points, tier,
             created_at, updated_at
             FROM loyalty_accounts WHERE 1=1",
        );
        let mut param_idx: u32 = 1;

        if filter.customer_id.is_some() {
            sql.push_str(&format!(" AND customer_id = ${param_idx}"));
            param_idx += 1;
        }
        if filter.program_id.is_some() {
            sql.push_str(&format!(" AND program_id = ${param_idx}"));
            param_idx += 1;
        }
        if filter.tier.is_some() {
            sql.push_str(&format!(" AND tier = ${param_idx}"));
            param_idx += 1;
        }

        sql.push_str(" ORDER BY created_at DESC");

        sql.push_str(&format!(" LIMIT ${param_idx}"));
        param_idx += 1;
        if filter.offset.is_some() {
            sql.push_str(&format!(" OFFSET ${param_idx}"));
            let _ = param_idx;
        }

        let mut query = sqlx::query_as::<_, LoyaltyAccountRow>(&sql);

        if let Some(customer_id) = filter.customer_id {
            query = query.bind(customer_id.into_uuid());
        }
        if let Some(program_id) = filter.program_id {
            query = query.bind(program_id.into_uuid());
        }
        if let Some(ref tier) = filter.tier {
            query = query.bind(tier.clone());
        }
        query = query.bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            query = query.bind(offset as i64);
        }

        let rows = query.fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_account).collect()
    }

    /// Adjust points on an account (async)
    pub async fn adjust_points_async(&self, input: AdjustPoints) -> Result<LoyaltyTransaction> {
        let tx_id = Uuid::new_v4();
        let now = Utc::now();
        let account_id = input.account_id.into_uuid();

        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Lock the account row and fetch the current balance
        let current_balance: i64 = sqlx::query_scalar(
            "SELECT points_balance FROM loyalty_accounts WHERE id = $1 FOR UPDATE",
        )
        .bind(account_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?
        .ok_or(CommerceError::NotFound)?;

        let new_balance = current_balance.checked_add(input.points).ok_or_else(|| {
            CommerceError::ValidationError("Points adjustment overflows".to_string())
        })?;
        if new_balance < 0 {
            return Err(CommerceError::ValidationError("Insufficient points balance".to_string()));
        }

        // Update the account balance
        sqlx::query(
            "UPDATE loyalty_accounts SET points_balance = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(new_balance)
        .bind(now)
        .bind(account_id)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        // If earning points, also increment lifetime_points
        if input.points > 0 {
            sqlx::query(
                "UPDATE loyalty_accounts SET lifetime_points = lifetime_points + $1
                 WHERE id = $2",
            )
            .bind(input.points)
            .bind(account_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;
        }

        // Insert the transaction record
        sqlx::query(
            "INSERT INTO loyalty_transactions (id, account_id, points, type, reference_id,
             notes, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(tx_id)
        .bind(account_id)
        .bind(input.points)
        .bind(input.transaction_type.to_string())
        .bind(&input.reference_id)
        .bind(&input.description)
        .bind(now)
        .execute(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        Ok(LoyaltyTransaction {
            id: LoyaltyTransactionId::from(tx_id),
            account_id: input.account_id,
            points: input.points,
            transaction_type: input.transaction_type,
            reference_id: input.reference_id,
            description: input.description,
            created_at: now,
        })
    }

    /// Get transaction history for an account (async)
    pub async fn get_transactions_async(
        &self,
        account_id: LoyaltyAccountId,
        limit: Option<u32>,
    ) -> Result<Vec<LoyaltyTransaction>> {
        let mut sql = String::from(
            "SELECT id, account_id, points, type, reference_id, notes, created_at
             FROM loyalty_transactions WHERE account_id = $1 ORDER BY created_at DESC",
        );

        sql.push_str(" LIMIT $2");

        let query = sqlx::query_as::<_, LoyaltyTransactionRow>(&sql)
            .bind(account_id.into_uuid())
            .bind(super::effective_limit(limit));

        let rows = query.fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_transaction).collect()
    }
}

impl LoyaltyProgramRepository for PgLoyaltyProgramRepository {
    fn create(&self, input: CreateLoyaltyProgram) -> Result<LoyaltyProgram> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: LoyaltyProgramId) -> Result<Option<LoyaltyProgram>> {
        super::block_on(self.get_async(id))
    }

    fn list(&self) -> Result<Vec<LoyaltyProgram>> {
        super::block_on(self.list_async())
    }

    fn enroll(&self, input: EnrollCustomer) -> Result<LoyaltyAccount> {
        super::block_on(self.enroll_async(input))
    }

    fn get_account(&self, id: LoyaltyAccountId) -> Result<Option<LoyaltyAccount>> {
        super::block_on(self.get_account_async(id))
    }

    fn get_account_by_customer(
        &self,
        customer_id: CustomerId,
        program_id: LoyaltyProgramId,
    ) -> Result<Option<LoyaltyAccount>> {
        super::block_on(self.get_account_by_customer_async(customer_id, program_id))
    }

    fn list_accounts(&self, filter: LoyaltyAccountFilter) -> Result<Vec<LoyaltyAccount>> {
        super::block_on(self.list_accounts_async(filter))
    }

    fn adjust_points(&self, input: AdjustPoints) -> Result<LoyaltyTransaction> {
        super::block_on(self.adjust_points_async(input))
    }

    fn get_transactions(
        &self,
        account_id: LoyaltyAccountId,
        limit: Option<u32>,
    ) -> Result<Vec<LoyaltyTransaction>> {
        super::block_on(self.get_transactions_async(account_id, limit))
    }
}
