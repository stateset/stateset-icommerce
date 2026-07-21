//! PostgreSQL implementation of fraud detection repository

use super::map_db_error;
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{
    CommerceError, CreateFraudAssessment, CreateFraudRule, FraudAssessment, FraudAssessmentFilter,
    FraudDecision, FraudRepository, FraudRule, FraudRuleFilter, FraudRuleId, FraudSignal,
    FraudSignalType, OrderId, Result, UpdateFraudRule,
};
use uuid::Uuid;

/// PostgreSQL fraud repository
#[derive(Debug, Clone)]
pub struct PgFraudRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct FraudAssessmentRow {
    order_id: Uuid,
    risk_score: f64,
    signals: serde_json::Value,
    decision: String,
    reviewed_by: Option<String>,
    review_notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct FraudRuleRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    signal_type: String,
    threshold: f64,
    action: String,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgFraudRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_assessment(row: FraudAssessmentRow) -> Result<FraudAssessment> {
        let FraudAssessmentRow {
            order_id,
            risk_score,
            signals,
            decision,
            reviewed_by,
            review_notes,
            created_at,
            updated_at,
        } = row;

        let signals: Vec<FraudSignal> = serde_json::from_value(signals).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid fraud_assessment.signals JSON: {}", e))
        })?;

        let decision: FraudDecision = decision.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid fraud_assessment.decision '{}': {}",
                decision, e
            ))
        })?;

        Ok(FraudAssessment {
            order_id: OrderId::from(order_id),
            risk_score,
            signals,
            decision,
            reviewed_by,
            review_notes,
            created_at,
            updated_at,
        })
    }

    fn row_to_rule(row: FraudRuleRow) -> Result<FraudRule> {
        let FraudRuleRow {
            id,
            name,
            description,
            signal_type,
            threshold,
            action,
            enabled,
            created_at,
            updated_at,
        } = row;

        let signal_type: FraudSignalType = signal_type.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid fraud_rule.signal_type '{}': {}",
                signal_type, e
            ))
        })?;

        let action: FraudDecision = action.parse().map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid fraud_rule.action '{}': {}", action, e))
        })?;

        Ok(FraudRule {
            id: FraudRuleId::from(id),
            name,
            description,
            signal_type,
            threshold,
            action,
            enabled,
            created_at,
            updated_at,
        })
    }

    // ---- async methods ----

    /// Create a fraud assessment (async)
    pub async fn create_assessment_async(
        &self,
        input: CreateFraudAssessment,
    ) -> Result<FraudAssessment> {
        let now = Utc::now();
        let order_id = input.order_id.into_uuid();

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

        let signals_json = serde_json::to_value(&signals)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            "INSERT INTO fraud_assessments (order_id, risk_score, signals, decision,
             reviewed_by, review_notes, created_at, updated_at)
             VALUES ($1, $2, $3, $4, NULL, NULL, $5, $6)",
        )
        .bind(order_id)
        .bind(risk_score)
        .bind(&signals_json)
        .bind(decision.to_string())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_assessment_async(input.order_id).await?.ok_or(CommerceError::NotFound)
    }

    /// Get fraud assessment by order ID (async)
    pub async fn get_assessment_async(&self, order_id: OrderId) -> Result<Option<FraudAssessment>> {
        let row = sqlx::query_as::<_, FraudAssessmentRow>(
            "SELECT order_id, risk_score, signals, decision, reviewed_by, review_notes,
             created_at, updated_at
             FROM fraud_assessments WHERE order_id = $1",
        )
        .bind(order_id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_assessment).transpose()
    }

    /// List fraud assessments with filter (async)
    pub async fn list_assessments_async(
        &self,
        filter: FraudAssessmentFilter,
    ) -> Result<Vec<FraudAssessment>> {
        let mut sql = String::from(
            "SELECT order_id, risk_score, signals, decision, reviewed_by, review_notes,
             created_at, updated_at
             FROM fraud_assessments WHERE 1=1",
        );
        let mut param_idx: u32 = 1;

        if filter.decision.is_some() {
            sql.push_str(&format!(" AND decision = ${param_idx}"));
            param_idx += 1;
        }
        if filter.min_risk_score.is_some() {
            sql.push_str(&format!(" AND risk_score >= ${param_idx}"));
            param_idx += 1;
        }
        if filter.unreviewed_only == Some(true) {
            sql.push_str(" AND reviewed_by IS NULL");
        }

        sql.push_str(" ORDER BY created_at DESC");

        sql.push_str(&format!(" LIMIT ${param_idx}"));
        param_idx += 1;
        if filter.offset.is_some() {
            sql.push_str(&format!(" OFFSET ${param_idx}"));
            let _ = param_idx;
        }

        let mut query = sqlx::query_as::<_, FraudAssessmentRow>(&sql);

        if let Some(decision) = &filter.decision {
            query = query.bind(decision.to_string());
        }
        if let Some(min_risk_score) = filter.min_risk_score {
            query = query.bind(min_risk_score);
        }
        query = query.bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            query = query.bind(offset as i64);
        }

        let rows = query.fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_assessment).collect()
    }

    /// Review a fraud assessment (async)
    pub async fn review_assessment_async(
        &self,
        order_id: OrderId,
        decision: FraudDecision,
        reviewer: String,
        notes: Option<String>,
    ) -> Result<FraudAssessment> {
        let now = Utc::now();

        sqlx::query(
            "UPDATE fraud_assessments SET decision = $1, reviewed_by = $2,
             review_notes = $3, updated_at = $4 WHERE order_id = $5",
        )
        .bind(decision.to_string())
        .bind(&reviewer)
        .bind(&notes)
        .bind(now)
        .bind(order_id.into_uuid())
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_assessment_async(order_id).await?.ok_or(CommerceError::NotFound)
    }

    /// Create a fraud rule (async)
    pub async fn create_rule_async(&self, input: CreateFraudRule) -> Result<FraudRule> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO fraud_rules (id, name, description, signal_type, threshold, action,
             enabled, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, TRUE, $7, $8)",
        )
        .bind(id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(input.signal_type.to_string())
        .bind(input.threshold)
        .bind(input.action.to_string())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.get_rule_async(FraudRuleId::from(id)).await?.ok_or(CommerceError::NotFound)
    }

    /// Get a fraud rule by ID (async)
    pub async fn get_rule_async(&self, id: FraudRuleId) -> Result<Option<FraudRule>> {
        let row = sqlx::query_as::<_, FraudRuleRow>(
            "SELECT id, name, description, signal_type, threshold, action, enabled,
             created_at, updated_at
             FROM fraud_rules WHERE id = $1",
        )
        .bind(id.into_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)?;

        row.map(Self::row_to_rule).transpose()
    }

    /// Update a fraud rule (async)
    pub async fn update_rule_async(
        &self,
        id: FraudRuleId,
        input: UpdateFraudRule,
    ) -> Result<FraudRule> {
        let now = Utc::now();
        let mut sets = vec!["updated_at = $1".to_string()];
        let mut param_idx: u32 = 2;

        let has_name = input.name.is_some();
        let has_description = input.description.is_some();
        let has_threshold = input.threshold.is_some();
        let has_action = input.action.is_some();
        let has_enabled = input.enabled.is_some();

        if has_name {
            sets.push(format!("name = ${param_idx}"));
            param_idx += 1;
        }
        if has_description {
            sets.push(format!("description = ${param_idx}"));
            param_idx += 1;
        }
        if has_threshold {
            sets.push(format!("threshold = ${param_idx}"));
            param_idx += 1;
        }
        if has_action {
            sets.push(format!("action = ${param_idx}"));
            param_idx += 1;
        }
        if has_enabled {
            sets.push(format!("enabled = ${param_idx}"));
            param_idx += 1;
        }

        let sql = format!("UPDATE fraud_rules SET {} WHERE id = ${param_idx}", sets.join(", "));

        let mut query = sqlx::query(&sql).bind(now);

        if let Some(ref name) = input.name {
            query = query.bind(name.clone());
        }
        if let Some(ref description) = input.description {
            query = query.bind(description.clone());
        }
        if let Some(threshold) = input.threshold {
            query = query.bind(threshold);
        }
        if let Some(action) = input.action {
            query = query.bind(action.to_string());
        }
        if let Some(enabled) = input.enabled {
            query = query.bind(enabled);
        }

        query = query.bind(id.into_uuid());

        query.execute(&self.pool).await.map_err(map_db_error)?;

        self.get_rule_async(id).await?.ok_or(CommerceError::NotFound)
    }

    /// List fraud rules with filter (async)
    pub async fn list_rules_async(&self, filter: FraudRuleFilter) -> Result<Vec<FraudRule>> {
        let mut sql = String::from(
            "SELECT id, name, description, signal_type, threshold, action, enabled,
             created_at, updated_at
             FROM fraud_rules WHERE 1=1",
        );
        let mut param_idx: u32 = 1;

        if filter.signal_type.is_some() {
            sql.push_str(&format!(" AND signal_type = ${param_idx}"));
            param_idx += 1;
        }
        if filter.action.is_some() {
            sql.push_str(&format!(" AND action = ${param_idx}"));
            param_idx += 1;
        }
        if filter.enabled.is_some() {
            sql.push_str(&format!(" AND enabled = ${param_idx}"));
            param_idx += 1;
        }

        sql.push_str(" ORDER BY created_at DESC");

        sql.push_str(&format!(" LIMIT ${param_idx}"));
        param_idx += 1;
        if filter.offset.is_some() {
            sql.push_str(&format!(" OFFSET ${param_idx}"));
            let _ = param_idx;
        }

        let mut query = sqlx::query_as::<_, FraudRuleRow>(&sql);

        if let Some(signal_type) = &filter.signal_type {
            query = query.bind(signal_type.to_string());
        }
        if let Some(action) = &filter.action {
            query = query.bind(action.to_string());
        }
        if let Some(enabled) = filter.enabled {
            query = query.bind(enabled);
        }
        query = query.bind(super::effective_limit(filter.limit));
        if let Some(offset) = filter.offset {
            query = query.bind(offset as i64);
        }

        let rows = query.fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_rule).collect()
    }

    /// Delete a fraud rule (async)
    pub async fn delete_rule_async(&self, id: FraudRuleId) -> Result<()> {
        sqlx::query("DELETE FROM fraud_rules WHERE id = $1")
            .bind(id.into_uuid())
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        Ok(())
    }

    /// Get all active (enabled) fraud rules (async)
    pub async fn get_active_rules_async(&self) -> Result<Vec<FraudRule>> {
        let rows = sqlx::query_as::<_, FraudRuleRow>(
            "SELECT id, name, description, signal_type, threshold, action, enabled,
             created_at, updated_at
             FROM fraud_rules WHERE enabled = TRUE ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;

        rows.into_iter().map(Self::row_to_rule).collect()
    }
}

impl FraudRepository for PgFraudRepository {
    fn create_assessment(&self, input: CreateFraudAssessment) -> Result<FraudAssessment> {
        super::block_on(self.create_assessment_async(input))
    }

    fn get_assessment(&self, order_id: OrderId) -> Result<Option<FraudAssessment>> {
        super::block_on(self.get_assessment_async(order_id))
    }

    fn list_assessments(&self, filter: FraudAssessmentFilter) -> Result<Vec<FraudAssessment>> {
        super::block_on(self.list_assessments_async(filter))
    }

    fn review_assessment(
        &self,
        order_id: OrderId,
        decision: FraudDecision,
        reviewer: String,
        notes: Option<String>,
    ) -> Result<FraudAssessment> {
        super::block_on(self.review_assessment_async(order_id, decision, reviewer, notes))
    }

    fn create_rule(&self, input: CreateFraudRule) -> Result<FraudRule> {
        super::block_on(self.create_rule_async(input))
    }

    fn get_rule(&self, id: FraudRuleId) -> Result<Option<FraudRule>> {
        super::block_on(self.get_rule_async(id))
    }

    fn update_rule(&self, id: FraudRuleId, input: UpdateFraudRule) -> Result<FraudRule> {
        super::block_on(self.update_rule_async(id, input))
    }

    fn list_rules(&self, filter: FraudRuleFilter) -> Result<Vec<FraudRule>> {
        super::block_on(self.list_rules_async(filter))
    }

    fn delete_rule(&self, id: FraudRuleId) -> Result<()> {
        super::block_on(self.delete_rule_async(id))
    }

    fn get_active_rules(&self) -> Result<Vec<FraudRule>> {
        super::block_on(self.get_active_rules_async())
    }
}
