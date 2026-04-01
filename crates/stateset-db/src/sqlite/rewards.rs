//! SQLite implementation of reward catalog repository

use super::{
    map_db_error, parse_datetime_row, parse_decimal_opt_row, parse_enum_row, parse_uuid_row,
    with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateReward, Result, Reward, RewardFilter, RewardId, RewardRepository,
};

#[derive(Debug)]
pub struct SqliteRewardRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteRewardRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_reward(row: &rusqlite::Row<'_>) -> rusqlite::Result<Reward> {
        Ok(Reward {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "reward", "id")?.into(),
            program_id: parse_uuid_row(
                &row.get::<_, String>("program_id")?,
                "reward",
                "program_id",
            )?
            .into(),
            name: row.get("name")?,
            description: row.get("description")?,
            points_cost: row.get::<_, i64>("points_cost")? as u64,
            reward_type: parse_enum_row(
                &row.get::<_, String>("reward_type")?,
                "reward",
                "reward_type",
            )?,
            value: parse_decimal_opt_row(row.get("value")?, "reward", "value")?,
            is_active: row.get::<_, i32>("is_active")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "reward",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "reward",
                "updated_at",
            )?,
        })
    }
}

impl RewardRepository for SqliteRewardRepository {
    fn create(&self, input: CreateReward) -> Result<Reward> {
        let id = RewardId::new();
        let now = Utc::now();
        let id_str = id.to_string();
        let now_str = now.to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO rewards (id, program_id, name, description, points_cost, reward_type, value, is_active, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?, ?)",
                rusqlite::params![
                    &id_str,
                    input.program_id.to_string(),
                    &input.name,
                    &input.description,
                    input.points_cost as i64,
                    input.reward_type.to_string(),
                    input.value.map(|v| v.to_string()),
                    &now_str,
                    &now_str,
                ],
            )?;

            tx.query_row("SELECT * FROM rewards WHERE id = ?", [&id_str], Self::row_to_reward)
        })
    }

    fn get(&self, id: RewardId) -> Result<Option<Reward>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM rewards WHERE id = ?",
            [id.to_string()],
            Self::row_to_reward,
        ) {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list(&self, filter: RewardFilter) -> Result<Vec<Reward>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM rewards WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(program_id) = filter.program_id {
            sql.push_str(" AND program_id = ?");
            params.push(Box::new(program_id.to_string()));
        }
        if let Some(reward_type) = filter.reward_type {
            sql.push_str(" AND reward_type = ?");
            params.push(Box::new(reward_type.to_string()));
        }
        if let Some(is_active) = filter.is_active {
            sql.push_str(" AND is_active = ?");
            params.push(Box::new(is_active as i32));
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
            .query_map(param_refs.as_slice(), Self::row_to_reward)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete(&self, id: RewardId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM rewards WHERE id = ?", [id.to_string()]).map_err(map_db_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use rust_decimal_macros::dec;
    use stateset_core::{LoyaltyProgramId, RewardType};

    fn test_repo() -> SqliteRewardRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        let conn = db.conn().expect("conn");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS rewards (
                id TEXT PRIMARY KEY,
                program_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                points_cost INTEGER NOT NULL DEFAULT 0,
                reward_type TEXT NOT NULL DEFAULT 'discount',
                value TEXT,
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .expect("create table");
        SqliteRewardRepository::new(db.pool().clone())
    }

    #[test]
    fn create_and_get_reward() {
        let repo = test_repo();
        let program_id = LoyaltyProgramId::new();
        let reward = repo
            .create(CreateReward {
                program_id,
                name: "10% Off Coupon".into(),
                description: Some("Get 10% off your next order".into()),
                points_cost: 500,
                reward_type: RewardType::Discount,
                value: Some(dec!(10.00)),
            })
            .expect("create");

        assert_eq!(reward.name, "10% Off Coupon");
        assert_eq!(reward.points_cost, 500);
        assert_eq!(reward.value, Some(dec!(10.00)));
        assert!(reward.is_active);

        let fetched = repo.get(reward.id).expect("get").expect("found");
        assert_eq!(fetched.id, reward.id);
        assert_eq!(fetched.program_id, program_id);
    }

    #[test]
    fn list_and_delete_rewards() {
        let repo = test_repo();
        let program_id = LoyaltyProgramId::new();

        for i in 0..3 {
            repo.create(CreateReward {
                program_id,
                name: format!("Reward {i}"),
                description: None,
                points_cost: (i + 1) * 100,
                reward_type: RewardType::Discount,
                value: None,
            })
            .expect("create");
        }

        let all = repo.list(RewardFilter::default()).expect("list");
        assert_eq!(all.len(), 3);

        repo.delete(all[0].id).expect("delete");
        let remaining = repo.list(RewardFilter::default()).expect("list after delete");
        assert_eq!(remaining.len(), 2);
    }
}
