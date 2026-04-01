//! PostgreSQL implementation of reward catalog repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateReward, Result, Reward, RewardFilter, RewardId, RewardRepository,
    RewardType,
};
use uuid::Uuid;

/// PostgreSQL reward repository
#[derive(Debug, Clone)]
pub struct PgRewardRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct RewardRow {
    id: Uuid,
    program_id: Uuid,
    name: String,
    description: Option<String>,
    points_cost: i64,
    reward_type: String,
    value: Option<Decimal>,
    is_active: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgRewardRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_reward(row: RewardRow) -> Result<Reward> {
        let RewardRow {
            id,
            program_id,
            name,
            description,
            points_cost,
            reward_type,
            value,
            is_active,
            created_at,
            updated_at,
        } = row;

        let reward_type: RewardType = reward_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid reward.reward_type '{}': {}",
                reward_type, e
            ))
        })?;

        Ok(Reward {
            id: RewardId::from(id),
            program_id: program_id.into(),
            name,
            description,
            #[allow(clippy::cast_sign_loss)]
            points_cost: points_cost as u64,
            reward_type,
            value,
            is_active,
            created_at,
            updated_at,
        })
    }

    // ---- async methods ----

    /// Create a reward (async)
    pub async fn create_async(&self, input: CreateReward) -> Result<Reward> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO rewards (id, program_id, name, description, points_cost, reward_type, value, is_active, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, true, $8, $9)",
        )
        .bind(id)
        .bind(input.program_id.into_uuid())
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.points_cost as i64)
        .bind(input.reward_type.to_string())
        .bind(input.value)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_async(RewardId::from(id)).await?.ok_or(CommerceError::NotFound)
    }

    /// Get reward by ID (async)
    pub async fn get_async(&self, id: RewardId) -> Result<Option<Reward>> {
        let row = sqlx::query_as::<_, RewardRow>(
            "SELECT id, program_id, name, description, points_cost, reward_type,
             value, is_active, created_at, updated_at
             FROM rewards WHERE id = $1",
        )
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_reward).transpose()
    }

    /// List rewards with filter (async)
    pub async fn list_async(&self, filter: RewardFilter) -> Result<Vec<Reward>> {
        let mut sql = String::from(
            "SELECT id, program_id, name, description, points_cost, reward_type,
             value, is_active, created_at, updated_at
             FROM rewards WHERE 1=1",
        );
        let mut param_idx: u32 = 1;

        if filter.program_id.is_some() {
            sql.push_str(&format!(" AND program_id = ${param_idx}"));
            param_idx += 1;
        }
        if filter.reward_type.is_some() {
            sql.push_str(&format!(" AND reward_type = ${param_idx}"));
            param_idx += 1;
        }
        if filter.is_active.is_some() {
            sql.push_str(&format!(" AND is_active = ${param_idx}"));
            param_idx += 1;
        }

        sql.push_str(" ORDER BY created_at DESC");

        if filter.limit.is_some() {
            sql.push_str(&format!(" LIMIT ${param_idx}"));
            param_idx += 1;
        }
        if filter.offset.is_some() {
            sql.push_str(&format!(" OFFSET ${param_idx}"));
            let _ = param_idx;
        }

        let mut query = sqlx::query_as::<_, RewardRow>(&sql);

        if let Some(program_id) = &filter.program_id {
            query = query.bind(program_id.into_uuid());
        }
        if let Some(reward_type) = &filter.reward_type {
            query = query.bind(reward_type.to_string());
        }
        if let Some(is_active) = filter.is_active {
            query = query.bind(is_active);
        }
        if let Some(limit) = filter.limit {
            query = query.bind(limit as i64);
        }
        if let Some(offset) = filter.offset {
            query = query.bind(offset as i64);
        }

        let rows = query.fetch_all(&self.pool).await.map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_reward).collect()
    }

    /// Delete a reward (async)
    pub async fn delete_async(&self, id: RewardId) -> Result<()> {
        sqlx::query("DELETE FROM rewards WHERE id = $1")
            .bind(id.into_uuid())
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }
}

impl RewardRepository for PgRewardRepository {
    fn create(&self, input: CreateReward) -> Result<Reward> {
        super::block_on(self.create_async(input))
    }

    fn get(&self, id: RewardId) -> Result<Option<Reward>> {
        super::block_on(self.get_async(id))
    }

    fn list(&self, filter: RewardFilter) -> Result<Vec<Reward>> {
        super::block_on(self.list_async(filter))
    }

    fn delete(&self, id: RewardId) -> Result<()> {
        super::block_on(self.delete_async(id))
    }
}
