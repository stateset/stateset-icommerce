//! Fraud detection operations for order risk assessment and rule management
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateFraudAssessment, OrderId};
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! let assessment = commerce.fraud().create_assessment(CreateFraudAssessment {
//!     order_id: OrderId::new(),
//!     signals: vec![],
//!     ..Default::default()
//! })?;
//!
//! println!("Risk score: {}", assessment.risk_score);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{
    CreateFraudAssessment, CreateFraudRule, FraudAssessment, FraudAssessmentFilter, FraudDecision,
    FraudRule, FraudRuleFilter, FraudRuleId, OrderId, Result, UpdateFraudRule,
};
use stateset_db::Database;
use std::sync::Arc;

/// Fraud detection operations for risk assessment and rules.
pub struct Fraud {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for Fraud {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fraud").finish_non_exhaustive()
    }
}

impl Fraud {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Assessment Operations
    // ========================================================================

    /// Create a fraud assessment for an order.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateFraudAssessment, OrderId};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let assessment = commerce.fraud().create_assessment(CreateFraudAssessment {
    ///     order_id: OrderId::new(),
    ///     signals: vec![],
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_assessment(&self, input: CreateFraudAssessment) -> Result<FraudAssessment> {
        self.db.fraud().create_assessment(input)
    }

    /// Get a fraud assessment for an order.
    pub fn get_assessment(&self, order_id: OrderId) -> Result<Option<FraudAssessment>> {
        self.db.fraud().get_assessment(order_id)
    }

    /// List fraud assessments with optional filtering.
    pub fn list_assessments(&self, filter: FraudAssessmentFilter) -> Result<Vec<FraudAssessment>> {
        self.db.fraud().list_assessments(filter)
    }

    /// Update an assessment after manual review.
    pub fn review_assessment(
        &self,
        order_id: OrderId,
        decision: FraudDecision,
        reviewer: String,
        notes: Option<String>,
    ) -> Result<FraudAssessment> {
        self.db.fraud().review_assessment(order_id, decision, reviewer, notes)
    }

    // ========================================================================
    // Rule Operations
    // ========================================================================

    /// Create a fraud detection rule.
    pub fn create_rule(&self, input: CreateFraudRule) -> Result<FraudRule> {
        self.db.fraud().create_rule(input)
    }

    /// Get a fraud rule by ID.
    pub fn get_rule(&self, id: FraudRuleId) -> Result<Option<FraudRule>> {
        self.db.fraud().get_rule(id)
    }

    /// Update a fraud rule.
    pub fn update_rule(&self, id: FraudRuleId, input: UpdateFraudRule) -> Result<FraudRule> {
        self.db.fraud().update_rule(id, input)
    }

    /// List fraud rules with optional filtering.
    pub fn list_rules(&self, filter: FraudRuleFilter) -> Result<Vec<FraudRule>> {
        self.db.fraud().list_rules(filter)
    }

    /// Delete a fraud rule.
    pub fn delete_rule(&self, id: FraudRuleId) -> Result<()> {
        self.db.fraud().delete_rule(id)
    }

    /// Get all enabled (active) fraud rules.
    pub fn get_active_rules(&self) -> Result<Vec<FraudRule>> {
        self.db.fraud().get_active_rules()
    }
}
