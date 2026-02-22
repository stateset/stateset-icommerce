//! SQLite implementation of fraud detection repository

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use stateset_core::{
    CommerceError, CreateFraudAssessment, CreateFraudRule, FraudAssessment, FraudAssessmentFilter,
    FraudDecision, FraudRepository, FraudRule, FraudRuleFilter, FraudRuleId, OrderId, Result,
    UpdateFraudRule,
};

#[derive(Debug)]
pub struct SqliteFraudRepository {
    #[allow(dead_code)]
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteFraudRepository {
    pub fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl FraudRepository for SqliteFraudRepository {
    fn create_assessment(&self, _input: CreateFraudAssessment) -> Result<FraudAssessment> {
        todo!("SQLite fraud create_assessment")
    }

    fn get_assessment(&self, _order_id: OrderId) -> Result<Option<FraudAssessment>> {
        todo!("SQLite fraud get_assessment")
    }

    fn list_assessments(
        &self,
        _filter: FraudAssessmentFilter,
    ) -> Result<Vec<FraudAssessment>> {
        todo!("SQLite fraud list_assessments")
    }

    fn review_assessment(
        &self,
        _order_id: OrderId,
        _decision: FraudDecision,
        _reviewer: String,
        _notes: Option<String>,
    ) -> Result<FraudAssessment> {
        todo!("SQLite fraud review_assessment")
    }

    fn create_rule(&self, _input: CreateFraudRule) -> Result<FraudRule> {
        todo!("SQLite fraud create_rule")
    }

    fn get_rule(&self, _id: FraudRuleId) -> Result<Option<FraudRule>> {
        todo!("SQLite fraud get_rule")
    }

    fn update_rule(&self, _id: FraudRuleId, _input: UpdateFraudRule) -> Result<FraudRule> {
        todo!("SQLite fraud update_rule")
    }

    fn list_rules(&self, _filter: FraudRuleFilter) -> Result<Vec<FraudRule>> {
        todo!("SQLite fraud list_rules")
    }

    fn delete_rule(&self, _id: FraudRuleId) -> Result<()> {
        todo!("SQLite fraud delete_rule")
    }

    fn get_active_rules(&self) -> Result<Vec<FraudRule>> {
        todo!("SQLite fraud get_active_rules")
    }
}
