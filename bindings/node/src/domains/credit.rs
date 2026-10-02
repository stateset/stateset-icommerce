//! Credit Management API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Credit Management API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateCreditAccountInput {
    pub customer_id: String,
    pub credit_limit: f64,
    pub payment_terms: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreditAccountOutput {
    pub id: String,
    pub customer_id: String,
    /// @deprecated Use the `creditLimitExact` twin; float money will be removed in 2.0.
    pub credit_limit: f64,
    /// Exact base-10 credit limit, straight from the engine's `Decimal`. Prefer this field for money.
    pub credit_limit_exact: String,
    /// @deprecated Use the `creditUsedExact` twin; float money will be removed in 2.0.
    pub credit_used: f64,
    /// Exact base-10 credit used, straight from the engine's `Decimal`. Prefer this field for money.
    pub credit_used_exact: String,
    /// @deprecated Use the `creditAvailableExact` twin; float money will be removed in 2.0.
    pub credit_available: f64,
    /// Exact base-10 credit available, straight from the engine's `Decimal`. Prefer this field for money.
    pub credit_available_exact: String,
    #[napi(ts_type = "CreditAccountStatus")]
    pub status: String,
    pub payment_terms: Option<String>,
}

impl TryFrom<stateset_core::CreditAccount> for CreditAccountOutput {
    type Error = Error;

    fn try_from(c: stateset_core::CreditAccount) -> Result<Self> {
        let (credit_limit, credit_limit_exact) = money_pair(c.credit_limit, "credit limit")?;
        let (credit_used, credit_used_exact) = money_pair(c.current_balance, "credit used")?;
        let (credit_available, credit_available_exact) =
            money_pair(c.available_credit, "credit available")?;
        Ok(Self {
            id: c.id.to_string(),
            customer_id: c.customer_id.to_string(),
            credit_limit,
            credit_limit_exact,
            credit_used,
            credit_used_exact,
            credit_available,
            credit_available_exact,
            status: format!("{:?}", c.status),
            payment_terms: c.payment_terms,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreditCheckOutput {
    pub approved: bool,
    pub reason: Option<String>,
    /// @deprecated Use the `availableCreditExact` twin; float money will be removed in 2.0.
    pub available_credit: f64,
    /// Exact base-10 available credit, straight from the engine's `Decimal`. Prefer this field for money.
    pub available_credit_exact: String,
    pub requires_approval: bool,
}

impl TryFrom<stateset_core::CreditCheckResult> for CreditCheckOutput {
    type Error = Error;

    fn try_from(c: stateset_core::CreditCheckResult) -> Result<Self> {
        let (available_credit, available_credit_exact) =
            money_pair(c.available_credit, "available credit")?;
        Ok(Self {
            approved: c.approved,
            reason: c.reason,
            available_credit,
            available_credit_exact,
            requires_approval: c.requires_approval,
        })
    }
}

/// Optional filters for `Credit.listCreditAccounts`. No argument lists all.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CreditAccountFilterInput {
    pub customer_id: Option<String>,
    /// The rendered form (`OnHold`) or the engine's snake_case (`on_hold`).
    #[napi(ts_type = "CreditAccountStatusInput")]
    pub status: Option<String>,
    /// Only accounts whose balance exceeds their limit.
    pub over_limit: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<CreditAccountFilterInput> for stateset_core::CreditAccountFilter {
    type Error = Error;

    fn try_from(f: CreditAccountFilterInput) -> Result<Self> {
        Ok(Self {
            customer_id: parse_optional_id::<uuid::Uuid>(f.customer_id, "customer")?
                .map(CustomerId::from),
            status: parse_optional_enum(f.status, "credit account status")?,
            over_limit: f.over_limit,
            limit: f.limit,
            offset: f.offset,
            ..Default::default()
        })
    }
}

#[napi]
pub struct Credit {
    pub(crate) commerce: Handle,
}

#[napi]
impl Credit {
    /// Create a credit account
    #[napi]
    pub async fn create_credit_account(
        &self,
        input: CreateCreditAccountInput,
    ) -> Result<CreditAccountOutput> {
        let commerce = self.commerce.get()?;
        let customer_id = input
            .customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let account = commerce
            .credit()
            .create_credit_account(stateset_core::CreateCreditAccount {
                customer_id,
                credit_limit: decimal_from_f64(input.credit_limit, "credit limit")?,
                payment_terms: input.payment_terms,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create credit account", e))?;
        convert_output(account)
    }

    /// Get a credit account by ID
    #[napi]
    pub async fn get_credit_account(&self, id: String) -> Result<Option<CreditAccountOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let account = commerce
            .credit()
            .get_credit_account(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get account", e))?;
        convert_optional_output(account)
    }

    /// Get credit account by customer
    #[napi]
    pub async fn get_credit_account_by_customer(
        &self,
        customer_id: String,
    ) -> Result<Option<CreditAccountOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let account = commerce
            .credit()
            .get_credit_account_by_customer(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get account", e))?;
        convert_optional_output(account)
    }

    /// List credit accounts, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list_credit_accounts(
        &self,
        filter: Option<CreditAccountFilterInput>,
    ) -> Result<Vec<CreditAccountOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::CreditAccountFilter = filter.unwrap_or_default().try_into()?;
        let accounts = commerce
            .credit()
            .list_credit_accounts(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list accounts", e))?;
        convert_outputs(accounts)
    }

    /// Check credit
    #[napi]
    pub async fn check_credit(
        &self,
        customer_id: String,
        order_amount: f64,
    ) -> Result<CreditCheckOutput> {
        let commerce = self.commerce.get()?;
        let uuid = customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let result = commerce
            .credit()
            .check_credit(uuid, decimal_from_f64(order_amount, "order amount")?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to check credit", e))?;
        convert_output(result)
    }

    /// Adjust credit limit
    #[napi]
    pub async fn adjust_credit_limit(
        &self,
        customer_id: String,
        new_limit: f64,
        reason: String,
    ) -> Result<CreditAccountOutput> {
        let commerce = self.commerce.get()?;
        let uuid = customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let account = commerce
            .credit()
            .adjust_credit_limit(uuid, decimal_from_f64(new_limit, "new credit limit")?, &reason)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to adjust limit", e))?;
        convert_output(account)
    }

    /// Suspend credit account
    #[napi]
    pub async fn suspend_credit_account(
        &self,
        customer_id: String,
        reason: String,
    ) -> Result<CreditAccountOutput> {
        let commerce = self.commerce.get()?;
        let uuid = customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let account = commerce
            .credit()
            .suspend_credit_account(uuid, &reason)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to suspend account", e))?;
        convert_output(account)
    }

    /// Reactivate credit account
    #[napi]
    pub async fn reactivate_credit_account(
        &self,
        customer_id: String,
    ) -> Result<CreditAccountOutput> {
        let commerce = self.commerce.get()?;
        let uuid = customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let account = commerce
            .credit()
            .reactivate_credit_account(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to reactivate account", e))?;
        convert_output(account)
    }

    /// Get over-limit customers
    #[napi]
    pub async fn get_over_limit_customers(&self) -> Result<Vec<CreditAccountOutput>> {
        let commerce = self.commerce.get()?;
        let accounts = commerce
            .credit()
            .get_over_limit_customers()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get accounts", e))?;
        convert_outputs(accounts)
    }
}
