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
    pub credit_limit: String,
    pub payment_terms: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreditAccountOutput {
    pub id: String,
    pub customer_id: String,
    /// Exact decimal string.
    pub credit_limit: String,
    /// Exact base-10 credit limit, straight from the engine's `Decimal`. Prefer this field for money.
    pub credit_limit_exact: String,
    /// Exact decimal string.
    pub credit_used: String,
    /// Exact base-10 credit used, straight from the engine's `Decimal`. Prefer this field for money.
    pub credit_used_exact: String,
    /// Exact decimal string.
    pub credit_available: String,
    /// Exact base-10 credit available, straight from the engine's `Decimal`. Prefer this field for money.
    pub credit_available_exact: String,
    #[napi(ts_type = "CreditAccountStatus")]
    pub status: String,
    pub payment_terms: Option<String>,
}

impl TryFrom<stateset_core::CreditAccount> for CreditAccountOutput {
    type Error = Error;

    fn try_from(c: stateset_core::CreditAccount) -> Result<Self> {
        let credit_limit = c.credit_limit.to_string();
        let credit_limit_exact = credit_limit.clone();
        let credit_used = c.current_balance.to_string();
        let credit_used_exact = credit_used.clone();
        let credit_available = c.available_credit.to_string();
        let credit_available_exact = credit_available.clone();
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
    /// Exact decimal string.
    pub available_credit: String,
    /// Exact base-10 available credit, straight from the engine's `Decimal`. Prefer this field for money.
    pub available_credit_exact: String,
    pub requires_approval: bool,
}

impl TryFrom<stateset_core::CreditCheckResult> for CreditCheckOutput {
    type Error = Error;

    fn try_from(c: stateset_core::CreditCheckResult) -> Result<Self> {
        let available_credit = c.available_credit.to_string();
        let available_credit_exact = available_credit.clone();
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
                credit_limit: parse_decimal_str(&input.credit_limit, "credit limit")?,
                payment_terms: input.payment_terms,
                notes: input.notes,
                ..Default::default()
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create credit account", e))?;
        effective_credit_output(&commerce, account)
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
        account.map(|account| effective_credit_output(&commerce, account)).transpose()
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
        account.map(|account| effective_credit_output(&commerce, account)).transpose()
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
        accounts.into_iter().map(|account| effective_credit_output(&commerce, account)).collect()
    }

    /// Check credit
    #[napi]
    pub async fn check_credit(
        &self,
        customer_id: String,
        order_amount: String,
    ) -> Result<CreditCheckOutput> {
        let commerce = self.commerce.get()?;
        let uuid = customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let result = commerce
            .credit()
            .check_credit(uuid, parse_decimal_str(&order_amount, "order amount")?)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to check credit", e))?;
        convert_output(result)
    }

    /// Adjust credit limit
    #[napi]
    pub async fn adjust_credit_limit(
        &self,
        customer_id: String,
        new_limit: String,
        reason: String,
    ) -> Result<CreditAccountOutput> {
        let commerce = self.commerce.get()?;
        let uuid = customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let account = commerce
            .credit()
            .adjust_credit_limit(uuid, parse_decimal_str(&new_limit, "new credit limit")?, &reason)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to adjust limit", e))?;
        effective_credit_output(&commerce, account)
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
        effective_credit_output(&commerce, account)
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
        effective_credit_output(&commerce, account)
    }

    /// Get over-limit customers
    #[napi]
    pub async fn get_over_limit_customers(&self) -> Result<Vec<CreditAccountOutput>> {
        let commerce = self.commerce.get()?;
        let accounts = commerce
            .credit()
            .get_over_limit_customers()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get accounts", e))?;
        accounts.into_iter().map(|account| effective_credit_output(&commerce, account)).collect()
    }
}

fn effective_credit_output(
    commerce: &stateset_embedded::Commerce,
    mut account: stateset_core::CreditAccount,
) -> Result<CreditAccountOutput> {
    account.available_credit = commerce
        .credit()
        .check_credit(account.customer_id, Decimal::ZERO)
        .map_err(|e| wrap(ErrCode::Internal, "Failed to calculate credit exposure", e))?
        .available_credit;
    convert_output(account)
}
