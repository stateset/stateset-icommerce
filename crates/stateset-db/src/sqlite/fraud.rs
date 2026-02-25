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
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    #[allow(dead_code)]
    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

impl FraudRepository for SqliteFraudRepository {
    fn create_assessment(&self, _input: CreateFraudAssessment) -> Result<FraudAssessment> {
        Err(CommerceError::DatabaseError(
            "SQLite fraud create_assessment not yet implemented".to_string(),
        ))
    }

    fn get_assessment(&self, _order_id: OrderId) -> Result<Option<FraudAssessment>> {
        Err(CommerceError::DatabaseError(
            "SQLite fraud get_assessment not yet implemented".to_string(),
        ))
    }

    fn list_assessments(&self, _filter: FraudAssessmentFilter) -> Result<Vec<FraudAssessment>> {
        Err(CommerceError::DatabaseError(
            "SQLite fraud list_assessments not yet implemented".to_string(),
        ))
    }

    fn review_assessment(
        &self,
        _order_id: OrderId,
        _decision: FraudDecision,
        _reviewer: String,
        _notes: Option<String>,
    ) -> Result<FraudAssessment> {
        Err(CommerceError::DatabaseError(
            "SQLite fraud review_assessment not yet implemented".to_string(),
        ))
    }

    fn create_rule(&self, _input: CreateFraudRule) -> Result<FraudRule> {
        Err(CommerceError::DatabaseError(
            "SQLite fraud create_rule not yet implemented".to_string(),
        ))
    }

    fn get_rule(&self, _id: FraudRuleId) -> Result<Option<FraudRule>> {
        Err(CommerceError::DatabaseError("SQLite fraud get_rule not yet implemented".to_string()))
    }

    fn update_rule(&self, _id: FraudRuleId, _input: UpdateFraudRule) -> Result<FraudRule> {
        Err(CommerceError::DatabaseError(
            "SQLite fraud update_rule not yet implemented".to_string(),
        ))
    }

    fn list_rules(&self, _filter: FraudRuleFilter) -> Result<Vec<FraudRule>> {
        Err(CommerceError::DatabaseError("SQLite fraud list_rules not yet implemented".to_string()))
    }

    fn delete_rule(&self, _id: FraudRuleId) -> Result<()> {
        Err(CommerceError::DatabaseError(
            "SQLite fraud delete_rule not yet implemented".to_string(),
        ))
    }

    fn get_active_rules(&self) -> Result<Vec<FraudRule>> {
        Err(CommerceError::DatabaseError(
            "SQLite fraud get_active_rules not yet implemented".to_string(),
        ))
    }
}
