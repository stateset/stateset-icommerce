//! Fraud (risk assessment + rules).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Fraud (risk assessment + rules)
// ============================================================================

pub(crate) fn parse_fraud_decision(s: &str) -> Result<stateset_core::FraudDecision> {
    s.parse::<stateset_core::FraudDecision>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid fraud decision: {s}")))
}

pub(crate) fn parse_fraud_signal_type(s: &str) -> Result<stateset_core::FraudSignalType> {
    s.parse::<stateset_core::FraudSignalType>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid fraud signal type: {s}")))
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateFraudSignalInput {
    /// Snake-case signal type: `velocity_spike`, `address_mismatch`, ...
    #[napi(ts_type = "FraudSignalType")]
    pub signal_type: String,
    /// Confidence score (0.0 - 1.0)
    pub score: f64,
    pub details: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateFraudAssessmentInput {
    pub order_id: String,
    pub signals: Vec<CreateFraudSignalInput>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FraudAssessmentFilterInput {
    /// Snake-case decision: `accept`, `review`, `reject`
    #[napi(ts_type = "FraudDecision")]
    pub decision: Option<String>,
    pub min_risk_score: Option<f64>,
    pub unreviewed_only: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FraudSignalOutput {
    pub order_id: String,
    /// Snake-case signal type
    #[napi(ts_type = "FraudSignalType")]
    pub signal_type: String,
    pub score: f64,
    pub details: String,
    pub detected_at: String,
}

impl From<stateset_core::FraudSignal> for FraudSignalOutput {
    fn from(s: stateset_core::FraudSignal) -> Self {
        Self {
            order_id: s.order_id.to_string(),
            signal_type: s.signal_type.to_string(),
            score: s.score,
            details: s.details,
            detected_at: s.detected_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FraudAssessmentOutput {
    pub order_id: String,
    pub risk_score: f64,
    pub signals: Vec<FraudSignalOutput>,
    /// Snake-case decision
    #[napi(ts_type = "FraudDecision")]
    pub decision: String,
    pub reviewed_by: Option<String>,
    pub review_notes: Option<String>,
    pub needs_review: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::FraudAssessment> for FraudAssessmentOutput {
    fn from(a: stateset_core::FraudAssessment) -> Self {
        let needs_review = a.needs_review();
        Self {
            order_id: a.order_id.to_string(),
            risk_score: a.risk_score,
            signals: a.signals.into_iter().map(Into::into).collect(),
            decision: a.decision.to_string(),
            reviewed_by: a.reviewed_by,
            review_notes: a.review_notes,
            needs_review,
            created_at: a.created_at.to_rfc3339(),
            updated_at: a.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateFraudRuleInput {
    pub name: String,
    pub description: Option<String>,
    /// Snake-case signal type
    #[napi(ts_type = "FraudSignalType")]
    pub signal_type: String,
    pub threshold: f64,
    /// Snake-case decision to apply when the rule triggers
    #[napi(ts_type = "FraudDecision")]
    pub action: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UpdateFraudRuleInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub threshold: Option<f64>,
    /// Snake-case decision
    #[napi(ts_type = "FraudDecision")]
    pub action: Option<String>,
    pub enabled: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FraudRuleFilterInput {
    /// Snake-case signal type
    #[napi(ts_type = "FraudSignalType")]
    pub signal_type: Option<String>,
    /// Snake-case decision
    #[napi(ts_type = "FraudDecision")]
    pub action: Option<String>,
    pub enabled: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct FraudRuleOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Snake-case signal type
    #[napi(ts_type = "FraudSignalType")]
    pub signal_type: String,
    pub threshold: f64,
    /// Snake-case decision
    #[napi(ts_type = "FraudDecision")]
    pub action: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::FraudRule> for FraudRuleOutput {
    fn from(r: stateset_core::FraudRule) -> Self {
        Self {
            id: r.id.to_string(),
            name: r.name,
            description: r.description,
            signal_type: r.signal_type.to_string(),
            threshold: r.threshold,
            action: r.action.to_string(),
            enabled: r.enabled,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Fraud {
    pub(crate) commerce: Handle,
}

#[napi]
impl Fraud {
    /// Whether the fraud backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.fraud().is_supported())
    }

    /// Create a fraud assessment for an order.
    #[napi]
    pub async fn create_assessment(
        &self,
        input: CreateFraudAssessmentInput,
    ) -> Result<FraudAssessmentOutput> {
        let commerce = self.commerce.get()?;
        let signals = input
            .signals
            .into_iter()
            .map(|s| -> Result<stateset_core::CreateFraudSignal> {
                Ok(stateset_core::CreateFraudSignal {
                    signal_type: parse_fraud_signal_type(&s.signal_type)?,
                    score: s.score,
                    details: s.details,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let assessment = commerce
            .fraud()
            .create_assessment(stateset_core::CreateFraudAssessment {
                order_id: parse_uuid_str(&input.order_id, "order_id")?.into(),
                signals,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create fraud assessment", e))?;
        Ok(assessment.into())
    }

    #[napi]
    pub async fn get_assessment(&self, order_id: String) -> Result<Option<FraudAssessmentOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&order_id, "order_id")?;
        let assessment = commerce
            .fraud()
            .get_assessment(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get fraud assessment", e))?;
        Ok(assessment.map(Into::into))
    }

    #[napi]
    pub async fn list_assessments(
        &self,
        filter: Option<FraudAssessmentFilterInput>,
    ) -> Result<Vec<FraudAssessmentOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::FraudAssessmentFilter::default()),
            |f| -> Result<stateset_core::FraudAssessmentFilter> {
                Ok(stateset_core::FraudAssessmentFilter {
                    decision: f.decision.as_deref().map(parse_fraud_decision).transpose()?,
                    min_risk_score: f.min_risk_score,
                    unreviewed_only: f.unreviewed_only,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let assessments = commerce
            .fraud()
            .list_assessments(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list fraud assessments", e))?;
        Ok(assessments.into_iter().map(Into::into).collect())
    }

    /// Record a manual review decision on an assessment.
    #[napi]
    pub async fn review_assessment(
        &self,
        order_id: String,
        #[napi(ts_arg_type = "FraudDecision")] decision: String,
        reviewer: String,
        notes: Option<String>,
    ) -> Result<FraudAssessmentOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&order_id, "order_id")?;
        let assessment = commerce
            .fraud()
            .review_assessment(uuid.into(), parse_fraud_decision(&decision)?, reviewer, notes)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to review fraud assessment", e))?;
        Ok(assessment.into())
    }

    #[napi]
    pub async fn create_rule(&self, input: CreateFraudRuleInput) -> Result<FraudRuleOutput> {
        let commerce = self.commerce.get()?;
        let rule = commerce
            .fraud()
            .create_rule(stateset_core::CreateFraudRule {
                name: input.name,
                description: input.description,
                signal_type: parse_fraud_signal_type(&input.signal_type)?,
                threshold: input.threshold,
                action: parse_fraud_decision(&input.action)?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create fraud rule", e))?;
        Ok(rule.into())
    }

    #[napi]
    pub async fn get_rule(&self, id: String) -> Result<Option<FraudRuleOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "fraud_rule")?;
        let rule = commerce
            .fraud()
            .get_rule(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get fraud rule", e))?;
        Ok(rule.map(Into::into))
    }

    #[napi]
    pub async fn update_rule(
        &self,
        id: String,
        input: UpdateFraudRuleInput,
    ) -> Result<FraudRuleOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "fraud_rule")?;
        let rule = commerce
            .fraud()
            .update_rule(
                uuid.into(),
                stateset_core::UpdateFraudRule {
                    name: input.name,
                    description: input.description.map(Some),
                    threshold: input.threshold,
                    action: input.action.as_deref().map(parse_fraud_decision).transpose()?,
                    enabled: input.enabled,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update fraud rule", e))?;
        Ok(rule.into())
    }

    #[napi]
    pub async fn list_rules(
        &self,
        filter: Option<FraudRuleFilterInput>,
    ) -> Result<Vec<FraudRuleOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::FraudRuleFilter::default()),
            |f| -> Result<stateset_core::FraudRuleFilter> {
                Ok(stateset_core::FraudRuleFilter {
                    signal_type: f
                        .signal_type
                        .as_deref()
                        .map(parse_fraud_signal_type)
                        .transpose()?,
                    action: f.action.as_deref().map(parse_fraud_decision).transpose()?,
                    enabled: f.enabled,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let rules = commerce
            .fraud()
            .list_rules(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list fraud rules", e))?;
        Ok(rules.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete_rule(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "fraud_rule")?;
        commerce
            .fraud()
            .delete_rule(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete fraud rule", e))
    }

    /// All currently enabled fraud rules.
    #[napi]
    pub async fn get_active_rules(&self) -> Result<Vec<FraudRuleOutput>> {
        let commerce = self.commerce.get()?;
        let rules = commerce
            .fraud()
            .get_active_rules()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get active fraud rules", e))?;
        Ok(rules.into_iter().map(Into::into).collect())
    }
}
