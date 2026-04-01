//! SQLite implementation of fraud detection repository

use super::{
    map_db_error, parse_datetime_row, parse_enum_row, parse_json_row, parse_uuid_row,
    with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateFraudAssessment, CreateFraudRule, FraudAssessment, FraudAssessmentFilter,
    FraudDecision, FraudRepository, FraudRule, FraudRuleFilter, FraudRuleId, FraudSignal, OrderId,
    Result, UpdateFraudRule,
};

#[derive(Debug)]
pub struct SqliteFraudRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteFraudRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    fn row_to_assessment(row: &rusqlite::Row<'_>) -> rusqlite::Result<FraudAssessment> {
        let signals_json: String = row.get("signals")?;
        let signals: Vec<FraudSignal> =
            parse_json_row(&signals_json, "fraud_assessment", "signals")?;

        Ok(FraudAssessment {
            order_id: parse_uuid_row(
                &row.get::<_, String>("order_id")?,
                "fraud_assessment",
                "order_id",
            )?
            .into(),
            risk_score: row.get("risk_score")?,
            signals,
            decision: parse_enum_row(
                &row.get::<_, String>("decision")?,
                "fraud_assessment",
                "decision",
            )?,
            reviewed_by: row.get("reviewed_by")?,
            review_notes: row.get("review_notes")?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "fraud_assessment",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "fraud_assessment",
                "updated_at",
            )?,
        })
    }

    fn row_to_rule(row: &rusqlite::Row<'_>) -> rusqlite::Result<FraudRule> {
        Ok(FraudRule {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "fraud_rule", "id")?.into(),
            name: row.get("name")?,
            description: row.get("description")?,
            signal_type: parse_enum_row(
                &row.get::<_, String>("signal_type")?,
                "fraud_rule",
                "signal_type",
            )?,
            threshold: row.get("threshold")?,
            action: parse_enum_row(&row.get::<_, String>("action")?, "fraud_rule", "action")?,
            enabled: row.get::<_, i32>("enabled")? != 0,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "fraud_rule",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "fraud_rule",
                "updated_at",
            )?,
        })
    }
}

impl FraudRepository for SqliteFraudRepository {
    fn create_assessment(&self, input: CreateFraudAssessment) -> Result<FraudAssessment> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let order_id_str = input.order_id.to_string();

        let signals: Vec<FraudSignal> = input
            .signals
            .into_iter()
            .map(|s| FraudSignal {
                order_id: input.order_id,
                signal_type: s.signal_type,
                score: s.score,
                details: s.details,
                detected_at: now,
            })
            .collect();

        let risk_score = FraudAssessment::calculate_risk_score(&signals);
        let decision =
            if risk_score >= 0.8 { FraudDecision::Review } else { FraudDecision::Accept };

        let signals_json = serde_json::to_string(&signals)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO fraud_assessments (order_id, risk_score, signals, decision, reviewed_by, review_notes, created_at, updated_at)
                 VALUES (?, ?, ?, ?, NULL, NULL, ?, ?)",
                rusqlite::params![
                    &order_id_str,
                    risk_score,
                    &signals_json,
                    decision.to_string(),
                    &now_str,
                    &now_str,
                ],
            )?;

            tx.query_row(
                "SELECT * FROM fraud_assessments WHERE order_id = ?",
                [&order_id_str],
                Self::row_to_assessment,
            )
        })
    }

    fn get_assessment(&self, order_id: OrderId) -> Result<Option<FraudAssessment>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM fraud_assessments WHERE order_id = ?",
            [order_id.to_string()],
            Self::row_to_assessment,
        ) {
            Ok(a) => Ok(Some(a)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn list_assessments(&self, filter: FraudAssessmentFilter) -> Result<Vec<FraudAssessment>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM fraud_assessments WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(decision) = filter.decision {
            sql.push_str(" AND decision = ?");
            params.push(Box::new(decision.to_string()));
        }
        if let Some(min_score) = filter.min_risk_score {
            sql.push_str(" AND risk_score >= ?");
            params.push(Box::new(min_score));
        }
        if filter.unreviewed_only == Some(true) {
            sql.push_str(" AND reviewed_by IS NULL");
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
            .query_map(param_refs.as_slice(), Self::row_to_assessment)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn review_assessment(
        &self,
        order_id: OrderId,
        decision: FraudDecision,
        reviewer: String,
        notes: Option<String>,
    ) -> Result<FraudAssessment> {
        let order_id_str = order_id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "UPDATE fraud_assessments SET decision = ?, reviewed_by = ?, review_notes = ?, updated_at = ? WHERE order_id = ?",
                rusqlite::params![
                    decision.to_string(),
                    &reviewer,
                    &notes,
                    &now_str,
                    &order_id_str,
                ],
            )?;

            tx.query_row(
                "SELECT * FROM fraud_assessments WHERE order_id = ?",
                [&order_id_str],
                Self::row_to_assessment,
            )
        })
    }

    fn create_rule(&self, input: CreateFraudRule) -> Result<FraudRule> {
        let id = FraudRuleId::new();
        let now = Utc::now();
        let id_str = id.to_string();
        let now_str = now.to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO fraud_rules (id, name, description, signal_type, threshold, action, enabled, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)",
                rusqlite::params![
                    &id_str,
                    &input.name,
                    &input.description,
                    input.signal_type.to_string(),
                    input.threshold,
                    input.action.to_string(),
                    &now_str,
                    &now_str,
                ],
            )?;

            tx.query_row("SELECT * FROM fraud_rules WHERE id = ?", [&id_str], Self::row_to_rule)
        })
    }

    fn get_rule(&self, id: FraudRuleId) -> Result<Option<FraudRule>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM fraud_rules WHERE id = ?",
            [id.to_string()],
            Self::row_to_rule,
        ) {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn update_rule(&self, id: FraudRuleId, input: UpdateFraudRule) -> Result<FraudRule> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();

        with_immediate_transaction(&self.pool, |tx| {
            let mut sets = vec!["updated_at = ?".to_string()];
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(now_str.clone())];

            if let Some(ref name) = input.name {
                sets.push("name = ?".into());
                params.push(Box::new(name.clone()));
            }
            if let Some(ref desc) = input.description {
                sets.push("description = ?".into());
                params.push(Box::new(desc.clone()));
            }
            if let Some(threshold) = input.threshold {
                sets.push("threshold = ?".into());
                params.push(Box::new(threshold));
            }
            if let Some(action) = input.action {
                sets.push("action = ?".into());
                params.push(Box::new(action.to_string()));
            }
            if let Some(enabled) = input.enabled {
                sets.push("enabled = ?".into());
                params.push(Box::new(enabled as i32));
            }

            let sql = format!("UPDATE fraud_rules SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(id_str.clone()));

            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            tx.execute(&sql, param_refs.as_slice())?;

            tx.query_row("SELECT * FROM fraud_rules WHERE id = ?", [&id_str], Self::row_to_rule)
        })
    }

    fn list_rules(&self, filter: FraudRuleFilter) -> Result<Vec<FraudRule>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM fraud_rules WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = vec![];

        if let Some(signal_type) = filter.signal_type {
            sql.push_str(" AND signal_type = ?");
            params.push(Box::new(signal_type.to_string()));
        }
        if let Some(action) = filter.action {
            sql.push_str(" AND action = ?");
            params.push(Box::new(action.to_string()));
        }
        if let Some(enabled) = filter.enabled {
            sql.push_str(" AND enabled = ?");
            params.push(Box::new(enabled as i32));
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
            .query_map(param_refs.as_slice(), Self::row_to_rule)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn delete_rule(&self, id: FraudRuleId) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM fraud_rules WHERE id = ?", [id.to_string()])
            .map_err(map_db_error)?;
        Ok(())
    }

    fn get_active_rules(&self) -> Result<Vec<FraudRule>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM fraud_rules WHERE enabled = 1 ORDER BY created_at DESC")
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([], Self::row_to_rule)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;
    use stateset_core::{CreateFraudSignal, FraudSignalType};

    fn test_repo() -> SqliteFraudRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        let conn = db.conn().expect("conn");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS fraud_assessments (
                order_id TEXT PRIMARY KEY,
                risk_score REAL NOT NULL DEFAULT 0.0,
                signals TEXT NOT NULL DEFAULT '[]',
                decision TEXT NOT NULL DEFAULT 'accept',
                reviewed_by TEXT,
                review_notes TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS fraud_rules (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                signal_type TEXT NOT NULL,
                threshold REAL NOT NULL DEFAULT 0.5,
                action TEXT NOT NULL DEFAULT 'review',
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .expect("create tables");
        SqliteFraudRepository::new(db.pool().clone())
    }

    #[test]
    fn create_and_get_assessment() {
        let repo = test_repo();
        let order_id = OrderId::new();
        let assessment = repo
            .create_assessment(CreateFraudAssessment {
                order_id,
                signals: vec![CreateFraudSignal {
                    signal_type: FraudSignalType::VelocitySpike,
                    score: 0.6,
                    details: "High order velocity".into(),
                }],
            })
            .expect("create assessment");

        assert_eq!(assessment.order_id, order_id);
        assert!((assessment.risk_score - 0.6).abs() < f64::EPSILON);
        assert_eq!(assessment.signals.len(), 1);

        let fetched = repo.get_assessment(order_id).expect("get").expect("found");
        assert_eq!(fetched.order_id, order_id);
    }

    #[test]
    fn create_and_delete_rule() {
        let repo = test_repo();
        let rule = repo
            .create_rule(CreateFraudRule {
                name: "Velocity check".into(),
                description: Some("Block fast orders".into()),
                signal_type: FraudSignalType::VelocitySpike,
                threshold: 0.8,
                action: FraudDecision::Reject,
            })
            .expect("create rule");

        assert_eq!(rule.name, "Velocity check");
        assert!(rule.enabled);

        let fetched = repo.get_rule(rule.id).expect("get").expect("found");
        assert_eq!(fetched.id, rule.id);

        repo.delete_rule(rule.id).expect("delete");
        assert!(repo.get_rule(rule.id).expect("get after delete").is_none());
    }
}
