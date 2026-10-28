//! General Ledger API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// General Ledger API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AutoPostingConfigInput {
    pub config_name: String,
    pub cash_account_id: String,
    pub accounts_receivable_account_id: String,
    pub inventory_account_id: String,
    pub accounts_payable_account_id: String,
    pub unearned_revenue_account_id: Option<String>,
    pub sales_revenue_account_id: String,
    pub shipping_revenue_account_id: Option<String>,
    pub sales_tax_payable_account_id: Option<String>,
    pub goods_received_not_invoiced_account_id: Option<String>,
    pub inventory_adjustment_account_id: Option<String>,
    pub work_in_progress_account_id: Option<String>,
    pub cogs_account_id: String,
    pub bad_debt_expense_account_id: Option<String>,
    pub fx_gain_loss_account_id: Option<String>,
    pub auto_post_depreciation: Option<bool>,
    pub auto_post_revenue_recognition: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AutoPostingConfigOutput {
    pub id: String,
    pub config_name: String,
    pub cash_account_id: String,
    pub accounts_receivable_account_id: String,
    pub inventory_account_id: String,
    pub accounts_payable_account_id: String,
    pub unearned_revenue_account_id: Option<String>,
    pub sales_revenue_account_id: String,
    pub shipping_revenue_account_id: Option<String>,
    pub sales_tax_payable_account_id: Option<String>,
    pub goods_received_not_invoiced_account_id: Option<String>,
    pub inventory_adjustment_account_id: Option<String>,
    pub work_in_progress_account_id: Option<String>,
    pub cogs_account_id: String,
    pub bad_debt_expense_account_id: Option<String>,
    pub fx_gain_loss_account_id: Option<String>,
    pub auto_post_depreciation: bool,
    pub auto_post_revenue_recognition: bool,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}
impl From<stateset_core::AutoPostingConfig> for AutoPostingConfigOutput {
    fn from(c: stateset_core::AutoPostingConfig) -> Self {
        Self {
            id: c.id.to_string(),
            config_name: c.config_name,
            cash_account_id: c.cash_account_id.to_string(),
            accounts_receivable_account_id: c.accounts_receivable_account_id.to_string(),
            inventory_account_id: c.inventory_account_id.to_string(),
            accounts_payable_account_id: c.accounts_payable_account_id.to_string(),
            unearned_revenue_account_id: c.unearned_revenue_account_id.map(|id| id.to_string()),
            sales_revenue_account_id: c.sales_revenue_account_id.to_string(),
            shipping_revenue_account_id: c.shipping_revenue_account_id.map(|id| id.to_string()),
            sales_tax_payable_account_id: c.sales_tax_payable_account_id.map(|id| id.to_string()),
            goods_received_not_invoiced_account_id: c
                .goods_received_not_invoiced_account_id
                .map(|id| id.to_string()),
            inventory_adjustment_account_id: c
                .inventory_adjustment_account_id
                .map(|id| id.to_string()),
            work_in_progress_account_id: c.work_in_progress_account_id.map(|id| id.to_string()),
            cogs_account_id: c.cogs_account_id.to_string(),
            bad_debt_expense_account_id: c.bad_debt_expense_account_id.map(|id| id.to_string()),
            fx_gain_loss_account_id: c.fx_gain_loss_account_id.map(|id| id.to_string()),
            auto_post_depreciation: c.auto_post_depreciation,
            auto_post_revenue_recognition: c.auto_post_revenue_recognition,
            is_active: c.is_active,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateGlAccountInput {
    pub account_number: String,
    pub name: String,
    #[napi(ts_type = "GlAccountTypeInput")]
    pub account_type: String,
    pub description: Option<String>,
    pub currency: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct GlAccountOutput {
    pub id: String,
    pub account_number: String,
    pub name: String,
    #[napi(ts_type = "GlAccountType")]
    pub account_type: String,
    /// Exact decimal string.
    pub balance: String,
    /// Exact base-10 balance, straight from the engine's `Decimal`. Prefer this field for money.
    pub balance_exact: String,
    #[napi(ts_type = "GlAccountStatus")]
    pub status: String,
    pub description: Option<String>,
    pub is_header: bool,
    pub is_posting: bool,
    pub currency: String,
}

impl TryFrom<stateset_core::GlAccount> for GlAccountOutput {
    type Error = Error;

    fn try_from(a: stateset_core::GlAccount) -> Result<Self> {
        let balance = a.current_balance.to_string();
        let balance_exact = balance.clone();
        Ok(Self {
            id: a.id.to_string(),
            account_number: a.account_number,
            name: a.name,
            account_type: format!("{:?}", a.account_type),
            balance,
            balance_exact,
            status: format!("{:?}", a.status),
            description: a.description,
            is_header: a.is_header,
            is_posting: a.is_posting,
            currency: a.currency.to_string(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct JournalEntryLineOutput {
    pub id: String,
    pub account_id: String,
    pub debit_amount: String,
    pub credit_amount: String,
    pub currency: String,
    pub transaction_currency: Option<String>,
    pub transaction_debit_amount: Option<String>,
    pub transaction_credit_amount: Option<String>,
    pub exchange_rate: Option<String>,
}
impl From<stateset_core::JournalEntryLine> for JournalEntryLineOutput {
    fn from(l: stateset_core::JournalEntryLine) -> Self {
        Self {
            id: l.id.to_string(),
            account_id: l.account_id.to_string(),
            debit_amount: l.debit_amount.to_string(),
            credit_amount: l.credit_amount.to_string(),
            currency: l.currency.to_string(),
            transaction_currency: l.transaction_currency.map(|v| v.to_string()),
            transaction_debit_amount: l.transaction_debit_amount.map(|v| v.to_string()),
            transaction_credit_amount: l.transaction_credit_amount.map(|v| v.to_string()),
            exchange_rate: l.exchange_rate.map(|v| v.to_string()),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct JournalEntryOutput {
    pub posted_by: Option<String>,
    pub id: String,
    pub entry_number: String,
    pub entry_date: String,
    pub description: String,
    #[napi(ts_type = "GlJournalEntryStatus")]
    pub status: String,
    pub created_at: String,
    pub lines: Vec<JournalEntryLineOutput>,
    pub total_debits: String,
    pub total_credits: String,
}

impl From<stateset_core::JournalEntry> for JournalEntryOutput {
    fn from(e: stateset_core::JournalEntry) -> Self {
        Self {
            posted_by: e.posted_by,
            lines: e.lines.into_iter().map(Into::into).collect(),
            total_debits: e.total_debits.to_string(),
            total_credits: e.total_credits.to_string(),
            id: e.id.to_string(),
            entry_number: e.entry_number,
            entry_date: e.entry_date.to_string(),
            description: e.description,
            status: format!("{:?}", e.status),
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TrialBalanceOutput {
    pub as_of_date: String,
    /// Exact decimal string.
    pub total_debits: String,
    /// Exact base-10 total debits, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_debits_exact: String,
    /// Exact decimal string.
    pub total_credits: String,
    /// Exact base-10 total credits, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_credits_exact: String,
    pub is_balanced: bool,
}

impl TryFrom<stateset_core::TrialBalance> for TrialBalanceOutput {
    type Error = Error;

    fn try_from(t: stateset_core::TrialBalance) -> Result<Self> {
        let total_debits = t.total_debits.to_string();
        let total_debits_exact = total_debits.clone();
        let total_credits = t.total_credits.to_string();
        let total_credits_exact = total_credits.clone();
        Ok(Self {
            as_of_date: t.as_of_date.to_string(),
            total_debits,
            total_debits_exact,
            total_credits,
            total_credits_exact,
            is_balanced: t.is_balanced,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BalanceSheetOutput {
    pub as_of_date: String,
    /// Exact decimal string.
    pub total_assets: String,
    /// Exact base-10 total assets, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_assets_exact: String,
    /// Exact decimal string.
    pub total_liabilities: String,
    /// Exact base-10 total liabilities, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_liabilities_exact: String,
    /// Exact decimal string.
    pub total_equity: String,
    /// Exact base-10 total equity, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_equity_exact: String,
}

impl TryFrom<stateset_core::BalanceSheet> for BalanceSheetOutput {
    type Error = Error;

    fn try_from(b: stateset_core::BalanceSheet) -> Result<Self> {
        let total_assets = b.total_assets.to_string();
        let total_assets_exact = total_assets.clone();
        let total_liabilities = b.total_liabilities.to_string();
        let total_liabilities_exact = total_liabilities.clone();
        let total_equity = b.total_equity.to_string();
        let total_equity_exact = total_equity.clone();
        Ok(Self {
            as_of_date: b.as_of_date.to_string(),
            total_assets,
            total_assets_exact,
            total_liabilities,
            total_liabilities_exact,
            total_equity,
            total_equity_exact,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct IncomeStatementOutput {
    pub period_start: String,
    pub period_end: String,
    /// Exact decimal string.
    pub total_revenue: String,
    /// Exact base-10 total revenue, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_revenue_exact: String,
    /// Exact decimal string.
    pub total_expenses: String,
    /// Exact base-10 total expenses, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_expenses_exact: String,
    /// Exact decimal string.
    pub net_income: String,
    /// Exact base-10 net income, straight from the engine's `Decimal`. Prefer this field for money.
    pub net_income_exact: String,
}

impl TryFrom<stateset_core::IncomeStatement> for IncomeStatementOutput {
    type Error = Error;

    fn try_from(i: stateset_core::IncomeStatement) -> Result<Self> {
        let total_revenue = i.total_revenue.to_string();
        let total_revenue_exact = total_revenue.clone();
        let total_expenses = i.total_expenses.to_string();
        let total_expenses_exact = total_expenses.clone();
        let net_income = i.net_income.to_string();
        let net_income_exact = net_income.clone();
        Ok(Self {
            period_start: i.period_start.to_string(),
            period_end: i.period_end.to_string(),
            total_revenue,
            total_revenue_exact,
            total_expenses,
            total_expenses_exact,
            net_income,
            net_income_exact,
        })
    }
}

pub(crate) fn parse_account_type(s: &str) -> Result<stateset_core::AccountType> {
    Ok(match s.to_lowercase().as_str() {
        "asset" => stateset_core::AccountType::Asset,
        "liability" => stateset_core::AccountType::Liability,
        "equity" => stateset_core::AccountType::Equity,
        "revenue" => stateset_core::AccountType::Revenue,
        "expense" => stateset_core::AccountType::Expense,
        _ => {
            return Err(unknown_variant(
                "account type",
                s,
                &["asset", "liability", "equity", "revenue", "expense"],
            ));
        }
    })
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RevaluationLineOutput {
    pub account_id: String,
    pub account_number: String,
    pub account_name: String,
    pub currency: String,
    /// Side that increases this account: debit or credit
    #[napi(ts_type = "GlBalanceSide")]
    pub normal_balance: String,
    /// Exact decimal string
    pub foreign_balance: String,
    /// Exact decimal string
    pub carrying_value: String,
    /// Exact decimal string
    pub rate: String,
    /// Exact decimal string
    pub revalued_value: String,
    /// Exact decimal string
    pub adjustment: String,
    /// Exact decimal string
    pub unrealized_gain_loss: String,
}

impl From<stateset_core::RevaluationLine> for RevaluationLineOutput {
    fn from(l: stateset_core::RevaluationLine) -> Self {
        Self {
            account_id: l.account_id.to_string(),
            account_number: l.account_number,
            account_name: l.account_name,
            currency: l.currency.to_string(),
            normal_balance: match l.normal_balance {
                stateset_core::BalanceSide::Debit => "debit".to_string(),
                stateset_core::BalanceSide::Credit => "credit".to_string(),
                _ => "unknown".to_string(),
            },
            foreign_balance: l.foreign_balance.to_string(),
            carrying_value: l.carrying_value.to_string(),
            rate: l.rate.to_string(),
            revalued_value: l.revalued_value.to_string(),
            adjustment: l.adjustment.to_string(),
            unrealized_gain_loss: l.unrealized_gain_loss.to_string(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RevaluationOutput {
    /// ISO date (YYYY-MM-DD)
    pub as_of_date: String,
    pub base_currency: String,
    /// Exact decimal string
    pub total_unrealized_gain_loss: String,
    pub lines: Vec<RevaluationLineOutput>,
    /// Balanced adjusting entry; None when no adjustment was required.
    pub journal_entry: Option<JournalEntryOutput>,
}

impl From<stateset_core::RevaluationResult> for RevaluationOutput {
    fn from(r: stateset_core::RevaluationResult) -> Self {
        Self {
            as_of_date: r.as_of_date.to_string(),
            base_currency: r.base_currency.to_string(),
            total_unrealized_gain_loss: r.total_unrealized_gain_loss.to_string(),
            lines: r.lines.into_iter().map(Into::into).collect(),
            journal_entry: r.journal_entry.map(Into::into),
        }
    }
}

/// Optional filters for `GeneralLedger.listAccounts`. No argument lists all.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct GlAccountFilterInput {
    /// The rendered form (`Asset`) or lowercase (`asset`).
    #[napi(ts_type = "GlAccountTypeFilter")]
    pub account_type: Option<String>,
    pub parent_account_id: Option<String>,
    /// The rendered form (`Active`) or lowercase (`active`).
    #[napi(ts_type = "GlAccountStatusInput")]
    pub status: Option<String>,
    pub is_posting: Option<bool>,
    pub is_header: Option<bool>,
    /// Matches account number or name.
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<GlAccountFilterInput> for stateset_core::GlAccountFilter {
    type Error = Error;

    fn try_from(f: GlAccountFilterInput) -> Result<Self> {
        Ok(Self {
            account_type: parse_optional_enum(f.account_type, "account type")?,
            parent_account_id: parse_optional_id(f.parent_account_id, "parent account")?,
            status: parse_optional_enum(f.status, "account status")?,
            is_posting: f.is_posting,
            is_header: f.is_header,
            search: f.search,
            limit: f.limit,
            offset: f.offset,
            ..Default::default()
        })
    }
}

/// Optional filters for `GeneralLedger.listJournalEntries`. No argument lists all.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct JournalEntryFilterInput {
    pub period_id: Option<String>,
    /// The rendered form (`Posted`) or lowercase (`posted`).
    #[napi(ts_type = "GlJournalEntryStatusInput")]
    pub status: Option<String>,
    pub account_id: Option<String>,
    /// `YYYY-MM-DD`.
    pub from_date: Option<String>,
    /// `YYYY-MM-DD`.
    pub to_date: Option<String>,
    pub source_document_type: Option<String>,
    pub source_document_id: Option<String>,
    /// Matches entry number, memo or reference.
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<JournalEntryFilterInput> for stateset_core::JournalEntryFilter {
    type Error = Error;

    fn try_from(f: JournalEntryFilterInput) -> Result<Self> {
        Ok(Self {
            period_id: parse_optional_id(f.period_id, "period")?,
            status: parse_optional_enum(f.status, "journal entry status")?,
            account_id: parse_optional_id(f.account_id, "account")?,
            from_date: parse_optional_date(f.from_date, "from date")?,
            to_date: parse_optional_date(f.to_date, "to date")?,
            source_document_type: f.source_document_type,
            source_document_id: parse_optional_id(f.source_document_id, "source document")?,
            search: f.search,
            limit: f.limit,
            offset: f.offset,
            ..Default::default()
        })
    }
}

#[napi]
pub struct GeneralLedger {
    pub(crate) commerce: Handle,
}

#[napi]
impl GeneralLedger {
    /// Read the active automatic ledger posting settings.
    #[napi]
    pub async fn get_auto_posting_config(&self) -> Result<Option<AutoPostingConfigOutput>> {
        self.commerce
            .get()?
            .general_ledger()
            .get_auto_posting_config()
            .map(|c| c.map(Into::into))
            .map_err(|e| wrap(ErrCode::Internal, "Failed to read posting settings", e))
    }
    /// Replace automatic posting settings atomically.
    #[napi]
    pub async fn set_auto_posting_config(
        &self,
        input: AutoPostingConfigInput,
    ) -> Result<AutoPostingConfigOutput> {
        let parse = |id: String| {
            id.parse::<uuid::Uuid>()
                .map_err(|_| coded(ErrCode::Validation, "Invalid posting account UUID"))
        };
        let config = stateset_core::CreateAutoPostingConfig {
            config_name: input.config_name,
            cash_account_id: parse(input.cash_account_id)?,
            accounts_receivable_account_id: parse(input.accounts_receivable_account_id)?,
            inventory_account_id: parse(input.inventory_account_id)?,
            accounts_payable_account_id: parse(input.accounts_payable_account_id)?,
            unearned_revenue_account_id: input
                .unearned_revenue_account_id
                .map(&parse)
                .transpose()?,
            sales_revenue_account_id: parse(input.sales_revenue_account_id)?,
            shipping_revenue_account_id: input
                .shipping_revenue_account_id
                .map(&parse)
                .transpose()?,
            sales_tax_payable_account_id: input
                .sales_tax_payable_account_id
                .map(&parse)
                .transpose()?,
            goods_received_not_invoiced_account_id: input
                .goods_received_not_invoiced_account_id
                .map(&parse)
                .transpose()?,
            inventory_adjustment_account_id: input
                .inventory_adjustment_account_id
                .map(&parse)
                .transpose()?,
            work_in_progress_account_id: input
                .work_in_progress_account_id
                .map(&parse)
                .transpose()?,
            cogs_account_id: parse(input.cogs_account_id)?,
            bad_debt_expense_account_id: input
                .bad_debt_expense_account_id
                .map(&parse)
                .transpose()?,
            fx_gain_loss_account_id: input.fx_gain_loss_account_id.map(&parse).transpose()?,
            auto_post_depreciation: input.auto_post_depreciation.unwrap_or(false),
            auto_post_revenue_recognition: input.auto_post_revenue_recognition.unwrap_or(false),
        };
        self.commerce
            .get()?
            .general_ledger()
            .set_auto_posting_config(config)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to save posting settings", e))
    }
    /// Create a GL account
    #[napi]
    pub async fn create_account(&self, input: CreateGlAccountInput) -> Result<GlAccountOutput> {
        let commerce = self.commerce.get()?;
        let account = commerce
            .general_ledger()
            .create_account(stateset_core::CreateGlAccount {
                account_number: input.account_number,
                name: input.name,
                description: input.description,
                account_type: parse_account_type(&input.account_type)?,
                account_sub_type: None,
                parent_account_id: None,
                is_header: None,
                is_posting: Some(true),
                currency: parse_optional_currency(input.currency)?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create account", e))?;
        convert_output(account)
    }

    /// Get a GL account by ID
    #[napi]
    pub async fn get_account(&self, id: String) -> Result<Option<GlAccountOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let account = commerce
            .general_ledger()
            .get_account(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get account", e))?;
        convert_optional_output(account)
    }

    /// Get a GL account by account number
    #[napi]
    pub async fn get_account_by_number(
        &self,
        account_number: String,
    ) -> Result<Option<GlAccountOutput>> {
        let commerce = self.commerce.get()?;
        let account = commerce
            .general_ledger()
            .get_account_by_number(&account_number)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get account", e))?;
        convert_optional_output(account)
    }

    /// List GL accounts, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list_accounts(
        &self,
        filter: Option<GlAccountFilterInput>,
    ) -> Result<Vec<GlAccountOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::GlAccountFilter = filter.unwrap_or_default().try_into()?;
        let accounts = commerce
            .general_ledger()
            .list_accounts(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list accounts", e))?;
        convert_outputs(accounts)
    }

    /// Initialize standard chart of accounts
    #[napi]
    pub async fn initialize_chart_of_accounts(&self) -> Result<Vec<GlAccountOutput>> {
        let commerce = self.commerce.get()?;
        let accounts = commerce
            .general_ledger()
            .initialize_chart_of_accounts()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to initialize chart", e))?;
        convert_outputs(accounts)
    }

    /// Create a balanced draft or post it atomically using a trusted host identity.
    #[napi]
    pub async fn create_journal_entry(
        &self,
        i: CreateJournalEntryInput,
        actor: Option<String>,
    ) -> Result<JournalEntryOutput> {
        let c = self.commerce.get()?;
        let lines = i
            .lines
            .into_iter()
            .map(|line| {
                Ok(stateset_core::CreateJournalEntryLine {
                    account_id: gl_workflow_id(&line.account_id)?,
                    description: line.description,
                    debit_amount: optional_decimal_string(line.debit_amount, "debit amount")?
                        .unwrap_or_default(),
                    credit_amount: optional_decimal_string(line.credit_amount, "credit amount")?
                        .unwrap_or_default(),
                    reference_type: None,
                    reference_id: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let fx = match (i.currency, i.exchange_rate) {
            (None, None) => None,
            (Some(currency), Some(rate)) => Some(stateset_core::JournalEntryFx {
                currency: currency
                    .parse()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid currency"))?,
                rate: finance_money_input(None, Some(rate), "exchange rate")?,
            }),
            _ => {
                return Err(coded(
                    ErrCode::Validation,
                    "Currency and exchange rate must be supplied together",
                ));
            }
        };
        let input = stateset_core::CreateJournalEntry {
            entry_date: gl_workflow_date(&i.entry_date)?,
            entry_type: parse_optional_enum(i.entry_type, "entry type")?,
            description: i.description,
            lines,
            source_document_type: None,
            source_document_id: None,
            auto_post: i.auto_post,
            fx,
        };
        let result = match actor {
            Some(actor) => c.general_ledger().create_journal_entry_as(input, &actor),
            None => c.general_ledger().create_journal_entry(input),
        };
        result
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create journal entry", e))
    }
    #[napi]
    pub async fn reverse_journal_entry(
        &self,
        id: String,
        reversal_date: String,
    ) -> Result<JournalEntryOutput> {
        self.commerce
            .get()?
            .general_ledger()
            .reverse_journal_entry(gl_workflow_id(&id)?, gl_workflow_date(&reversal_date)?)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to reverse journal entry", e))
    }
    #[napi]
    pub async fn get_period(&self, id: String) -> Result<Option<GlPeriodOutput>> {
        self.commerce
            .get()?
            .general_ledger()
            .get_period(gl_workflow_id(&id)?)
            .map(|p| p.map(Into::into))
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get period", e))
    }
    #[napi]
    pub async fn close_period(&self, id: String, actor: String) -> Result<GlPeriodOutput> {
        self.commerce
            .get()?
            .general_ledger()
            .close_period(gl_workflow_id(&id)?, &actor)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to close period", e))
    }
    #[napi]
    pub async fn lock_period(&self, id: String, actor: String) -> Result<GlPeriodOutput> {
        self.commerce
            .get()?
            .general_ledger()
            .lock_period(gl_workflow_id(&id)?, &actor)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to lock period", e))
    }
    #[napi]
    pub async fn reopen_period(&self, id: String) -> Result<GlPeriodOutput> {
        self.commerce
            .get()?
            .general_ledger()
            .reopen_period(gl_workflow_id(&id)?)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to reopen period", e))
    }
    #[napi]
    pub async fn reclose_period(&self, id: String, actor: String) -> Result<JournalEntryOutput> {
        self.commerce
            .get()?
            .general_ledger()
            .reclose_period(gl_workflow_id(&id)?, &actor)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to reclose period", e))
    }

    /// Get a journal entry by ID
    #[napi]
    pub async fn get_journal_entry(&self, id: String) -> Result<Option<JournalEntryOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let entry = commerce
            .general_ledger()
            .get_journal_entry(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get entry", e))?;
        Ok(entry.map(|e| e.into()))
    }

    /// List journal entries, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list_journal_entries(
        &self,
        filter: Option<JournalEntryFilterInput>,
    ) -> Result<Vec<JournalEntryOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::JournalEntryFilter = filter.unwrap_or_default().try_into()?;
        let entries = commerce
            .general_ledger()
            .list_journal_entries(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list entries", e))?;
        Ok(entries.into_iter().map(|e| e.into()).collect())
    }

    /// Post a journal entry
    #[napi]
    pub async fn post_journal_entry(
        &self,
        id: String,
        posted_by: String,
    ) -> Result<JournalEntryOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let entry = commerce
            .general_ledger()
            .post_journal_entry(uuid, &posted_by)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to post entry", e))?;
        Ok(entry.into())
    }

    /// Void a journal entry
    #[napi]
    pub async fn void_journal_entry(&self, id: String) -> Result<JournalEntryOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let entry = commerce
            .general_ledger()
            .void_journal_entry(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to void entry", e))?;
        Ok(entry.into())
    }

    /// Get trial balance
    #[napi]
    pub async fn get_trial_balance(&self, as_of_date: String) -> Result<TrialBalanceOutput> {
        let commerce = self.commerce.get()?;
        let date = chrono::NaiveDate::parse_from_str(&as_of_date, "%Y-%m-%d")
            .map_err(|_| coded(ErrCode::Validation, "Invalid date format"))?;
        let balance = commerce
            .general_ledger()
            .get_trial_balance(date)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get trial balance", e))?;
        convert_output(balance)
    }

    /// Get balance sheet
    #[napi]
    pub async fn get_balance_sheet(&self, as_of_date: String) -> Result<BalanceSheetOutput> {
        let commerce = self.commerce.get()?;
        let date = chrono::NaiveDate::parse_from_str(&as_of_date, "%Y-%m-%d")
            .map_err(|_| coded(ErrCode::Validation, "Invalid date format"))?;
        let sheet = commerce
            .general_ledger()
            .get_balance_sheet(date)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get balance sheet", e))?;
        convert_output(sheet)
    }

    /// Get income statement
    #[napi]
    pub async fn get_income_statement(
        &self,
        start_date: String,
        end_date: String,
    ) -> Result<IncomeStatementOutput> {
        let commerce = self.commerce.get()?;
        let start = chrono::NaiveDate::parse_from_str(&start_date, "%Y-%m-%d")
            .map_err(|_| coded(ErrCode::Validation, "Invalid start date format"))?;
        let end = chrono::NaiveDate::parse_from_str(&end_date, "%Y-%m-%d")
            .map_err(|_| coded(ErrCode::Validation, "Invalid end date format"))?;
        let statement = commerce
            .general_ledger()
            .get_income_statement(start, end)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get income statement", e))?;
        convert_output(statement)
    }

    /// Get account balance
    #[napi]
    pub async fn get_account_balance(
        &self,
        account_id: String,
        as_of_date: Option<String>,
    ) -> Result<String> {
        let commerce = self.commerce.get()?;
        let uuid = account_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let date = parse_optional_date(as_of_date, "as of date")?;
        let balance = commerce
            .general_ledger()
            .get_account_balance(uuid, date)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get balance", e))?;
        balance
            .map(|value| value.to_string())
            .ok_or_else(|| coded(ErrCode::NotFound, "Account balance unavailable"))
    }

    /// Revalue foreign-currency account balances at the as-of exchange rate.
    ///
    /// `as_of_date` is an ISO date (YYYY-MM-DD); `base_currency` defaults to
    /// the store's configured base currency.
    #[napi]
    pub async fn revalue(
        &self,
        as_of_date: String,
        base_currency: Option<String>,
    ) -> Result<RevaluationOutput> {
        let commerce = self.commerce.get()?;
        let date = chrono::NaiveDate::parse_from_str(&as_of_date, "%Y-%m-%d")
            .map_err(|_| coded(ErrCode::Validation, "Invalid date format"))?;
        let base = base_currency
            .map(|s| {
                s.parse::<stateset_core::Currency>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid base currency code"))
            })
            .transpose()?;
        let result = commerce
            .general_ledger()
            .revalue(date, base)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to revalue", e))?;
        Ok(result.into())
    }

    /// Create an accounting period.
    #[napi]
    pub async fn create_period(&self, input: CreateGlPeriodInput) -> Result<GlPeriodOutput> {
        let commerce = self.commerce.get()?;
        let period = commerce
            .general_ledger()
            .create_period(stateset_core::CreateGlPeriod {
                period_name: input.period_name,
                fiscal_year: input.fiscal_year,
                period_number: input.period_number,
                start_date: chrono::NaiveDate::parse_from_str(&input.start_date, "%Y-%m-%d")
                    .map_err(|_| coded(ErrCode::Validation, "Invalid start date format"))?,
                end_date: chrono::NaiveDate::parse_from_str(&input.end_date, "%Y-%m-%d")
                    .map_err(|_| coded(ErrCode::Validation, "Invalid end date format"))?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create period", e))?;
        Ok(period.into())
    }

    /// Open a period (transition from future to open).
    #[napi]
    pub async fn open_period(&self, id: String) -> Result<GlPeriodOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let period = commerce
            .general_ledger()
            .open_period(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to open period", e))?;
        Ok(period.into())
    }

    /// List accounting periods with optional filtering.
    #[napi]
    pub async fn list_periods(
        &self,
        filter: Option<GlPeriodFilterInput>,
    ) -> Result<Vec<GlPeriodOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();
        let status = filter
            .status
            .as_deref()
            .map(|s| {
                s.parse::<stateset_core::PeriodStatus>()
                    .map_err(|_| coded(ErrCode::Validation, format!("Invalid period status: {s}")))
            })
            .transpose()?;
        let periods = commerce
            .general_ledger()
            .list_periods(stateset_core::GlPeriodFilter {
                fiscal_year: filter.fiscal_year,
                status,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list periods", e))?;
        Ok(periods.into_iter().map(Into::into).collect())
    }

    /// Close the month: post scheduled depreciation, recognize revenue
    /// through period end, revalue foreign-currency balances, then run the
    /// period close (closing entries + close period).
    ///
    /// Pass `{ dryRun: true }` to compute per-step counts and amounts without
    /// writing anything.
    #[napi]
    pub async fn close_month(
        &self,
        period_id: String,
        options: Option<CloseMonthOptionsInput>,
    ) -> Result<CloseMonthReportOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            period_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let options = options.unwrap_or_default();
        let report = commerce
            .general_ledger()
            .close_month(
                uuid,
                stateset_core::CloseMonthOptions {
                    dry_run: options.dry_run.unwrap_or(false),
                    skip_depreciation: options.skip_depreciation.unwrap_or(false),
                    skip_revenue_recognition: options.skip_revenue_recognition.unwrap_or(false),
                    skip_fx_revaluation: options.skip_fx_revaluation.unwrap_or(false),
                    skip_period_close: options.skip_period_close.unwrap_or(false),
                    fail_on_warnings: options.fail_on_warnings.unwrap_or(false),
                    closed_by: options.closed_by,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to close month", e))?;
        Ok(report.into())
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateGlPeriodInput {
    /// Display name, typically `YYYY-MM`
    pub period_name: String,
    pub fiscal_year: i32,
    /// Sequential number within the fiscal year (1-12 for monthly)
    pub period_number: i32,
    /// First date of the period (inclusive), ISO date (YYYY-MM-DD)
    pub start_date: String,
    /// Last date of the period (inclusive), ISO date (YYYY-MM-DD)
    pub end_date: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct GlPeriodOutput {
    pub id: String,
    pub period_name: String,
    pub fiscal_year: i32,
    pub period_number: i32,
    /// ISO date (YYYY-MM-DD)
    pub start_date: String,
    /// ISO date (YYYY-MM-DD)
    pub end_date: String,
    /// One of `future`, `open`, `closed`, `locked`
    #[napi(ts_type = "GlPeriodStatus")]
    pub status: String,
    pub closed_by: Option<String>,
}

impl From<stateset_core::GlPeriod> for GlPeriodOutput {
    fn from(p: stateset_core::GlPeriod) -> Self {
        Self {
            id: p.id.to_string(),
            period_name: p.period_name,
            fiscal_year: p.fiscal_year,
            period_number: p.period_number,
            start_date: p.start_date.to_string(),
            end_date: p.end_date.to_string(),
            status: p.status.to_string(),
            closed_by: p.closed_by,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct GlPeriodFilterInput {
    /// Filter by fiscal year
    pub fiscal_year: Option<i32>,
    /// Filter by status: one of `future`, `open`, `closed`, `locked`
    #[napi(ts_type = "GlPeriodStatus")]
    pub status: Option<String>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CloseMonthOptionsInput {
    /// Compute per-step counts/amounts without writing anything
    pub dry_run: Option<bool>,
    /// Skip posting scheduled fixed-asset depreciation
    pub skip_depreciation: Option<bool>,
    /// Skip recognizing deferred revenue through period end
    pub skip_revenue_recognition: Option<bool>,
    /// Skip FX revaluation of foreign-currency accounts
    pub skip_fx_revaluation: Option<bool>,
    /// Skip the final period close (closing entries + close period)
    pub skip_period_close: Option<bool>,
    /// Refuse with an error when any step reports warnings (strict mode)
    pub fail_on_warnings: Option<bool>,
    /// Actor recorded as the closer; defaults to `system`
    pub closed_by: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CloseMonthStepOutput {
    /// One of `executed`, `skipped`, `dry_run`
    #[napi(ts_type = "CloseMonthStepStatus")]
    pub status: String,
    /// Entries posted (or that would be posted in a dry run)
    pub entry_count: i64,
    /// Exact decimal string
    pub total_amount: String,
    /// Per-item failures that did not abort the close
    pub warnings: Vec<String>,
    /// Warnings that record a per-item failure (vs informational skip notes)
    pub failed_item_count: i64,
}

impl From<stateset_core::CloseMonthStepReport> for CloseMonthStepOutput {
    fn from(step: stateset_core::CloseMonthStepReport) -> Self {
        Self {
            status: step.status.to_string(),
            entry_count: i64::try_from(step.entry_count).unwrap_or(i64::MAX),
            total_amount: step.total_amount.to_string(),
            warnings: step.warnings,
            failed_item_count: i64::try_from(step.failed_item_count).unwrap_or(i64::MAX),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CloseMonthReportOutput {
    pub period_id: String,
    pub period_name: String,
    pub dry_run: bool,
    /// Step 1: scheduled depreciation due through period end
    pub depreciation: CloseMonthStepOutput,
    /// Step 2: deferred revenue recognized through period end
    pub revenue_recognition: CloseMonthStepOutput,
    /// Step 3: FX revaluation as of period end
    pub fx_revaluation: CloseMonthStepOutput,
    /// Step 4: closing entries + close period
    pub period_close: CloseMonthStepOutput,
    /// Posted closing entry; None for dry runs or skipped closes
    pub closing_entry: Option<JournalEntryOutput>,
    /// Period status after the run (`closed` after a real close)
    #[napi(ts_type = "GlPeriodStatus")]
    pub period_status: String,
}

impl From<stateset_core::CloseMonthReport> for CloseMonthReportOutput {
    fn from(r: stateset_core::CloseMonthReport) -> Self {
        Self {
            period_id: r.period_id.to_string(),
            period_name: r.period_name,
            dry_run: r.dry_run,
            depreciation: r.depreciation.into(),
            revenue_recognition: r.revenue_recognition.into(),
            fx_revaluation: r.fx_revaluation.into(),
            period_close: r.period_close.into(),
            closing_entry: r.closing_entry.map(Into::into),
            period_status: r.period_status.to_string(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateJournalEntryInput {
    pub entry_date: String,
    pub entry_type: Option<String>,
    pub description: String,
    pub lines: Vec<CreateJournalEntryLineInput>,
    pub auto_post: Option<bool>,
    pub currency: Option<String>,
    pub exchange_rate: Option<String>,
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateJournalEntryLineInput {
    pub account_id: String,
    pub description: Option<String>,
    pub debit_amount: Option<String>,
    pub credit_amount: Option<String>,
}
fn gl_workflow_id(value: &str) -> Result<uuid::Uuid> {
    value.parse().map_err(|_| coded(ErrCode::Validation, "Invalid ledger UUID"))
}
fn gl_workflow_date(value: &str) -> Result<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| coded(ErrCode::Validation, "Expected YYYY-MM-DD"))
}
