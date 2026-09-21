//! Store credits  (all monetary values cross as exact decimal strings).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Store credits  (all monetary values cross as exact decimal strings)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateStoreCreditInput {
    /// Customer UUID that owns the credit
    pub customer_id: String,
    /// Amount to issue as an exact decimal string, e.g. "25.00"
    pub amount: String,
    /// Currency code, e.g. "USD"
    pub currency: String,
    /// Reason: return, loyalty, compensation, promotion, manual, gift_card
    /// (defaults to "return")
    #[napi(ts_type = "StoreCreditReason")]
    pub reason: Option<String>,
    pub reference_id: Option<String>,
    pub note: Option<String>,
    /// RFC 3339 expiry timestamp (None = never expires)
    pub expires_at: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AdjustStoreCreditInput {
    /// Signed adjustment as an exact decimal string ("10.00" adds, "-10.00"
    /// subtracts). The balance may not be driven below zero.
    pub amount: String,
    pub note: Option<String>,
    pub reference_id: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StoreCreditFilterInput {
    pub customer_id: Option<String>,
    #[napi(ts_type = "StoreCreditStatus")]
    pub status: Option<String>,
    #[napi(ts_type = "StoreCreditReason")]
    pub reason: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StoreCreditOutput {
    pub id: String,
    pub customer_id: String,
    /// Exact decimal string
    pub original_balance: String,
    /// Exact decimal string
    pub current_balance: String,
    pub currency: String,
    #[napi(ts_type = "StoreCreditStatus")]
    pub status: String,
    #[napi(ts_type = "StoreCreditReason")]
    pub reason: String,
    pub reference_id: Option<String>,
    pub note: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::StoreCredit> for StoreCreditOutput {
    fn from(c: stateset_core::StoreCredit) -> Self {
        Self {
            id: c.id.to_string(),
            customer_id: c.customer_id.to_string(),
            original_balance: c.original_balance.to_string(),
            current_balance: c.current_balance.to_string(),
            currency: c.currency.to_string(),
            status: format!("{}", c.status),
            reason: format!("{}", c.reason),
            reference_id: c.reference_id,
            note: c.note,
            expires_at: c.expires_at.map(|d| d.to_rfc3339()),
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct StoreCreditTransactionOutput {
    pub id: String,
    pub store_credit_id: String,
    /// Exact decimal string (positive = credit, negative = debit)
    pub amount: String,
    /// Exact decimal string
    pub balance_after: String,
    #[napi(ts_type = "StoreCreditTransactionType")]
    pub transaction_type: String,
    pub reference_id: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::StoreCreditTransaction> for StoreCreditTransactionOutput {
    fn from(t: stateset_core::StoreCreditTransaction) -> Self {
        Self {
            id: t.id.to_string(),
            store_credit_id: t.store_credit_id.to_string(),
            amount: t.amount.to_string(),
            balance_after: t.balance_after.to_string(),
            transaction_type: format!("{}", t.transaction_type),
            reference_id: t.reference_id,
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

#[napi]
pub struct StoreCredits {
    pub(crate) commerce: Handle,
}

#[napi]
impl StoreCredits {
    /// Whether the store-credits backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.store_credits().is_supported())
    }

    #[napi]
    pub async fn create(&self, input: CreateStoreCreditInput) -> Result<StoreCreditOutput> {
        let commerce = self.commerce.get()?;
        let customer_uuid: uuid::Uuid = input
            .customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let amount = input
            .amount
            .parse::<Decimal>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid amount decimal"))?;
        let currency = input
            .currency
            .parse::<CurrencyCode>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid currency code"))?;
        let reason = match input.reason.as_deref() {
            Some(s) => s
                .parse::<stateset_core::StoreCreditReason>()
                .map_err(|_| coded(ErrCode::Validation, "Invalid store credit reason"))?,
            None => stateset_core::StoreCreditReason::default(),
        };
        let expires_at = match input.expires_at.as_deref() {
            Some(s) => Some(
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| {
                        coded(ErrCode::Validation, "Invalid expires_at RFC 3339 timestamp")
                    })?
                    .with_timezone(&chrono::Utc),
            ),
            None => None,
        };
        let credit = commerce
            .store_credits()
            .create(stateset_core::CreateStoreCredit {
                customer_id: customer_uuid.into(),
                amount,
                currency,
                reason,
                reference_id: input.reference_id,
                note: input.note,
                expires_at,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create store credit", e))?;
        Ok(credit.into())
    }

    #[napi]
    pub async fn get(&self, id: String) -> Result<Option<StoreCreditOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let credit = commerce
            .store_credits()
            .get(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get store credit", e))?;
        Ok(credit.map(Into::into))
    }

    #[napi]
    pub async fn list(
        &self,
        filter: Option<StoreCreditFilterInput>,
    ) -> Result<Vec<StoreCreditOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or(StoreCreditFilterInput {
            customer_id: None,
            status: None,
            reason: None,
            limit: None,
            offset: None,
        });
        let customer_id = match filter.customer_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?
                    .into(),
            ),
            None => None,
        };
        let status = match filter.status.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::StoreCreditStatus>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid store credit status"))?,
            ),
            None => None,
        };
        let reason = match filter.reason.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::StoreCreditReason>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid store credit reason"))?,
            ),
            None => None,
        };
        let credits = commerce
            .store_credits()
            .list(stateset_core::StoreCreditFilter {
                customer_id,
                status,
                reason,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list store credits", e))?;
        Ok(credits.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn adjust(
        &self,
        id: String,
        input: AdjustStoreCreditInput,
    ) -> Result<StoreCreditOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let amount = input
            .amount
            .parse::<Decimal>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid amount decimal"))?;
        let credit = commerce
            .store_credits()
            .adjust(
                uuid.into(),
                stateset_core::AdjustStoreCredit {
                    amount,
                    note: input.note,
                    reference_id: input.reference_id,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to adjust store credit", e))?;
        Ok(credit.into())
    }

    /// Apply (redeem) an amount from the credit, returning the ledger transaction.
    #[napi]
    pub async fn apply(
        &self,
        id: String,
        amount: String,
        reference_id: Option<String>,
    ) -> Result<StoreCreditTransactionOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let amount = amount
            .parse::<Decimal>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid amount decimal"))?;
        let txn = commerce
            .store_credits()
            .apply(uuid.into(), amount, reference_id)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to apply store credit", e))?;
        Ok(txn.into())
    }

    #[napi]
    pub async fn get_transactions(
        &self,
        store_credit_id: String,
    ) -> Result<Vec<StoreCreditTransactionOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            store_credit_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let txns = commerce
            .store_credits()
            .get_transactions(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get transactions", e))?;
        Ok(txns.into_iter().map(Into::into).collect())
    }
}
