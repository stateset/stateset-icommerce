//! Prepayments  (advance payments to suppliers).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Prepayments  (advance payments to suppliers)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePrepaymentInput {
    pub supplier_id: String,
    /// Exact decimal string, e.g. "1000.00"
    pub amount: String,
    /// Currency code, e.g. "USD"
    pub currency: Option<String>,
    /// Payment method (e.g. "wire", "ach")
    pub method: Option<String>,
    pub reference: Option<String>,
    pub memo: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ApplyPrepaymentInput {
    /// bill or payment_obligation
    #[napi(ts_type = "PrepaymentTargetType")]
    pub target_type: String,
    pub target_id: String,
    /// Exact decimal string
    pub amount: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PrepaymentFilterInput {
    pub supplier_id: Option<String>,
    /// open, applied, refunded, cancelled
    #[napi(ts_type = "PrepaymentStatus")]
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PrepaymentOutput {
    pub id: String,
    pub number: String,
    pub supplier_id: String,
    /// Exact decimal string
    pub amount: String,
    /// Exact decimal string
    pub remaining: String,
    pub currency: String,
    /// open, applied, refunded, cancelled
    #[napi(ts_type = "PrepaymentStatus")]
    pub status: String,
    pub method: Option<String>,
    pub reference: Option<String>,
    pub memo: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Prepayment> for PrepaymentOutput {
    fn from(p: stateset_core::Prepayment) -> Self {
        Self {
            id: p.id.to_string(),
            number: p.number,
            supplier_id: p.supplier_id.to_string(),
            amount: p.amount.to_string(),
            remaining: p.remaining.to_string(),
            currency: p.currency.to_string(),
            status: format!("{}", p.status),
            method: p.method,
            reference: p.reference,
            memo: p.memo,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PrepaymentApplicationOutput {
    pub id: String,
    pub prepayment_id: String,
    /// bill or payment_obligation
    #[napi(ts_type = "PrepaymentTargetType")]
    pub target_type: String,
    pub target_id: String,
    /// Exact decimal string
    pub amount: String,
    pub reversed: bool,
    pub created_at: String,
}

impl From<stateset_core::PrepaymentApplication> for PrepaymentApplicationOutput {
    fn from(a: stateset_core::PrepaymentApplication) -> Self {
        Self {
            id: a.id.to_string(),
            prepayment_id: a.prepayment_id.to_string(),
            target_type: format!("{}", a.target_type),
            target_id: a.target_id.to_string(),
            amount: a.amount.to_string(),
            reversed: a.reversed,
            created_at: a.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct Prepayments {
    pub(crate) commerce: Handle,
}

#[napi]
impl Prepayments {
    /// Whether the prepayments backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.prepayments().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreatePrepaymentInput) -> Result<PrepaymentOutput> {
        let commerce = self.commerce.get()?;
        let prepayment = commerce
            .prepayments()
            .create(stateset_core::CreatePrepayment {
                supplier_id: parse_uuid_str(&input.supplier_id, "supplier_id")?,
                amount: parse_decimal_str(&input.amount, "amount")?,
                currency: parse_currency_opt(input.currency)?,
                method: input.method,
                reference: input.reference,
                memo: input.memo,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create prepayment", e))?;
        Ok(prepayment.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<PrepaymentOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "prepayment")?;
        let prepayment = commerce
            .prepayments()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get prepayment", e))?;
        Ok(prepayment.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<PrepaymentFilterInput>,
    ) -> Result<Vec<PrepaymentOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::PrepaymentFilter::default()),
            |f| -> Result<stateset_core::PrepaymentFilter> {
                Ok(stateset_core::PrepaymentFilter {
                    supplier_id: parse_optional_uuid(f.supplier_id, "supplier_id")?,
                    status: f
                        .status
                        .map(|s| {
                            s.parse::<stateset_core::PrepaymentStatus>().map_err(|_| {
                                coded(ErrCode::Validation, "Invalid prepayment status")
                            })
                        })
                        .transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let prepayments = commerce
            .prepayments()
            .list(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list prepayments", e))?;
        Ok(prepayments.into_iter().map(Into::into).collect())
    }

    /// Apply a prepayment against a bill or payment obligation.
    #[napi]
    pub async fn apply(&self, id: String, input: ApplyPrepaymentInput) -> Result<PrepaymentOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "prepayment")?;
        let target_type = input
            .target_type
            .parse::<stateset_core::PrepaymentTargetType>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid prepayment target type"))?;
        let prepayment = commerce
            .prepayments()
            .apply(
                uuid.into(),
                stateset_core::ApplyPrepayment {
                    target_type,
                    target_id: parse_uuid_str(&input.target_id, "target_id")?,
                    amount: parse_decimal_str(&input.amount, "amount")?,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to apply prepayment", e))?;
        Ok(prepayment.into())
    }

    /// List applications for a prepayment.
    #[napi]
    pub async fn list_applications(&self, id: String) -> Result<Vec<PrepaymentApplicationOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "prepayment")?;
        let applications = commerce
            .prepayments()
            .list_applications(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list prepayment applications", e))?;
        Ok(applications.into_iter().map(Into::into).collect())
    }

    /// Reverse a previously-recorded application.
    #[napi]
    pub async fn reverse_application(
        &self,
        id: String,
        application_id: String,
    ) -> Result<PrepaymentOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "prepayment")?;
        let app_uuid = parse_uuid_str(&application_id, "application")?;
        let prepayment =
            commerce.prepayments().reverse_application(uuid.into(), app_uuid.into()).map_err(
                |e| wrap(ErrCode::Internal, "Failed to reverse prepayment application", e),
            )?;
        Ok(prepayment.into())
    }

    /// Refund the remaining balance, closing the prepayment.
    #[napi]
    pub async fn refund(&self, id: String) -> Result<PrepaymentOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "prepayment")?;
        let prepayment = commerce
            .prepayments()
            .refund(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to refund prepayment", e))?;
        Ok(prepayment.into())
    }
}
