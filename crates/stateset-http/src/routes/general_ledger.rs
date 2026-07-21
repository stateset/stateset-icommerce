//! General ledger endpoints (chart of accounts, journal entries, periods, reports).

use crate::error::{ErrorBody, HttpError};
use crate::state::{AppState, tenant_id_from_headers};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Default actor recorded on post/close/lock operations when the caller does
/// not provide one.
const DEFAULT_ACTOR: &str = "api";

// ============================================================================
// Request / response schemas
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateGlAccountRequest {
    /// Structured account code (e.g. `"1010"`).
    pub account_number: String,
    pub name: String,
    pub description: Option<String>,
    /// One of `asset`, `liability`, `equity`, `revenue`, `expense`.
    pub account_type: String,
    /// Optional sub-type (e.g. `cash`, `accounts_receivable`, `sales_revenue`).
    pub account_sub_type: Option<String>,
    pub parent_account_id: Option<String>,
    pub is_header: Option<bool>,
    pub is_posting: Option<bool>,
    /// ISO-4217 currency code (e.g. `USD`).
    pub currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct GlAccountResponse {
    pub id: String,
    pub account_number: String,
    pub name: String,
    pub description: Option<String>,
    pub account_type: String,
    pub account_sub_type: Option<String>,
    pub parent_account_id: Option<String>,
    pub is_header: bool,
    pub is_posting: bool,
    pub normal_balance: String,
    pub currency: String,
    pub status: String,
    /// Decimal balance as a string.
    pub current_balance: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct GlAccountListResponse {
    pub accounts: Vec<GlAccountResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct GlAccountFilterParams {
    /// One of `asset`, `liability`, `equity`, `revenue`, `expense`.
    pub account_type: Option<String>,
    /// One of `active`, `inactive`, `archived`.
    pub status: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateJournalEntryLineRequest {
    pub account_id: String,
    pub description: Option<String>,
    /// Decimal debit amount as a string. Defaults to `"0"`.
    pub debit_amount: Option<String>,
    /// Decimal credit amount as a string. Defaults to `"0"`.
    pub credit_amount: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateJournalEntryRequest {
    /// Entry date in `YYYY-MM-DD` format. Must fall in an open period.
    pub entry_date: String,
    /// One of `standard`, `adjusting`, `closing`, `reversing`, `opening`.
    pub entry_type: Option<String>,
    pub description: String,
    pub lines: Vec<CreateJournalEntryLineRequest>,
    /// Post immediately after creation when balanced.
    pub auto_post: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, ToSchema)]
pub(crate) struct PostJournalEntryRequest {
    /// Actor recorded as the poster. Defaults to `api`.
    pub posted_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, ToSchema)]
pub(crate) struct ReverseJournalEntryRequest {
    /// Reversal date in `YYYY-MM-DD` format. Defaults to today (UTC).
    pub reversal_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, ToSchema)]
pub(crate) struct RevalueRequest {
    /// Effective date in `YYYY-MM-DD` format; must fall in an open period.
    /// Defaults to today (UTC).
    pub as_of_date: Option<String>,
    /// ISO-4217 base currency (e.g. `USD`). Defaults to the store's
    /// configured base currency.
    pub base_currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct RevaluationLineResponse {
    pub account_id: String,
    pub account_number: String,
    pub account_name: String,
    /// Account currency (differs from the base currency).
    pub currency: String,
    /// Outstanding balance in the account's own currency.
    pub foreign_balance: String,
    /// Value currently carried on the books (base currency).
    pub carrying_value: String,
    /// Exchange rate used (1 account-currency unit in base units).
    pub rate: String,
    /// `foreign_balance * rate` rounded to base-currency precision.
    pub revalued_value: String,
    /// `revalued_value - carrying_value`.
    pub adjustment: String,
    /// Unrealized FX gain (positive) or loss (negative) in base currency.
    pub unrealized_gain_loss: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct RevaluationResponse {
    pub as_of_date: String,
    pub base_currency: String,
    pub total_unrealized_gain_loss: String,
    pub lines: Vec<RevaluationLineResponse>,
    /// Balanced posted adjusting entry; absent when no adjustment was needed.
    pub journal_entry: Option<JournalEntryResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct JournalEntryLineResponse {
    pub id: String,
    pub line_number: i32,
    pub account_id: String,
    pub account_number: Option<String>,
    pub account_name: Option<String>,
    pub description: Option<String>,
    /// Decimal debit amount as a string.
    pub debit_amount: String,
    /// Decimal credit amount as a string.
    pub credit_amount: String,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct JournalEntryResponse {
    pub id: String,
    pub entry_number: String,
    pub entry_date: String,
    pub period_id: String,
    pub entry_type: String,
    pub source: String,
    pub description: String,
    /// Decimal total debits as a string.
    pub total_debits: String,
    /// Decimal total credits as a string.
    pub total_credits: String,
    pub is_balanced: bool,
    pub status: String,
    pub posted_at: Option<String>,
    pub posted_by: Option<String>,
    pub reversed_entry_id: Option<String>,
    pub reversing_entry_id: Option<String>,
    pub lines: Vec<JournalEntryLineResponse>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct JournalEntryListResponse {
    pub journal_entries: Vec<JournalEntryResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct JournalEntryFilterParams {
    pub period_id: Option<String>,
    /// One of `draft`, `pending`, `posted`, `voided`, `reversed`.
    pub status: Option<String>,
    /// One of `standard`, `adjusting`, `closing`, `reversing`, `opening`.
    pub entry_type: Option<String>,
    pub account_id: Option<String>,
    /// Inclusive start date in `YYYY-MM-DD` format.
    pub from_date: Option<String>,
    /// Inclusive end date in `YYYY-MM-DD` format.
    pub to_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub(crate) struct CreateGlPeriodRequest {
    pub period_name: String,
    pub fiscal_year: i32,
    pub period_number: i32,
    /// First date of the period (inclusive), `YYYY-MM-DD`.
    pub start_date: String,
    /// Last date of the period (inclusive), `YYYY-MM-DD`.
    pub end_date: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, ToSchema)]
pub(crate) struct ClosePeriodRequest {
    /// Actor recorded as the closer. Defaults to `api`.
    pub closed_by: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default, ToSchema)]
pub(crate) struct LockPeriodRequest {
    /// Actor recorded as the locker. Defaults to `api`.
    pub locked_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct GlPeriodResponse {
    pub id: String,
    pub period_name: String,
    pub fiscal_year: i32,
    pub period_number: i32,
    pub start_date: String,
    pub end_date: String,
    pub status: String,
    pub closed_at: Option<String>,
    pub closed_by: Option<String>,
    pub locked_at: Option<String>,
    pub locked_by: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct GlPeriodListResponse {
    pub periods: Vec<GlPeriodResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct GlPeriodFilterParams {
    pub fiscal_year: Option<i32>,
    /// One of `future`, `open`, `closed`, `locked`.
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Default, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct AsOfDateParams {
    /// Report date in `YYYY-MM-DD` format. Defaults to today (UTC).
    pub as_of_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct IncomeStatementParams {
    /// Inclusive period start date in `YYYY-MM-DD` format.
    pub start_date: String,
    /// Inclusive period end date in `YYYY-MM-DD` format.
    pub end_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct TrialBalanceLineResponse {
    pub account_id: String,
    pub account_number: String,
    pub account_name: String,
    pub account_type: String,
    /// Decimal debit balance as a string.
    pub debit_balance: String,
    /// Decimal credit balance as a string.
    pub credit_balance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct TrialBalanceResponse {
    pub as_of_date: String,
    /// Decimal total debits as a string.
    pub total_debits: String,
    /// Decimal total credits as a string.
    pub total_credits: String,
    pub is_balanced: bool,
    pub lines: Vec<TrialBalanceLineResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct BalanceSheetLineResponse {
    pub account_id: String,
    pub account_number: String,
    pub account_name: String,
    /// Decimal balance as a string.
    pub balance: String,
    pub indent_level: i32,
    pub is_total: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct BalanceSheetResponse {
    pub as_of_date: String,
    /// Decimal total assets as a string.
    pub total_assets: String,
    /// Decimal total liabilities as a string.
    pub total_liabilities: String,
    /// Decimal total equity as a string.
    pub total_equity: String,
    pub assets: Vec<BalanceSheetLineResponse>,
    pub liabilities: Vec<BalanceSheetLineResponse>,
    pub equity: Vec<BalanceSheetLineResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct IncomeStatementLineResponse {
    pub account_id: String,
    pub account_number: String,
    pub account_name: String,
    /// Decimal amount as a string.
    pub amount: String,
    pub indent_level: i32,
    pub is_total: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub(crate) struct IncomeStatementResponse {
    pub period_start: String,
    pub period_end: String,
    /// Decimal total revenue as a string.
    pub total_revenue: String,
    /// Decimal total expenses as a string.
    pub total_expenses: String,
    /// Decimal net income as a string.
    pub net_income: String,
    pub revenue_lines: Vec<IncomeStatementLineResponse>,
    pub expense_lines: Vec<IncomeStatementLineResponse>,
}

// ============================================================================
// Conversions and parsing helpers
// ============================================================================

fn to_account_resp(a: &stateset_core::GlAccount) -> GlAccountResponse {
    GlAccountResponse {
        id: a.id.to_string(),
        account_number: a.account_number.clone(),
        name: a.name.clone(),
        description: a.description.clone(),
        account_type: a.account_type.to_string(),
        account_sub_type: a.account_sub_type.map(|s| s.to_string()),
        parent_account_id: a.parent_account_id.map(|id| id.to_string()),
        is_header: a.is_header,
        is_posting: a.is_posting,
        normal_balance: a.normal_balance.to_string(),
        currency: a.currency.to_string(),
        status: a.status.to_string(),
        current_balance: a.current_balance.to_string(),
        created_at: a.created_at.to_rfc3339(),
    }
}

fn to_line_resp(l: &stateset_core::JournalEntryLine) -> JournalEntryLineResponse {
    JournalEntryLineResponse {
        id: l.id.to_string(),
        line_number: l.line_number,
        account_id: l.account_id.to_string(),
        account_number: l.account_number.clone(),
        account_name: l.account_name.clone(),
        description: l.description.clone(),
        debit_amount: l.debit_amount.to_string(),
        credit_amount: l.credit_amount.to_string(),
        currency: l.currency.to_string(),
    }
}

fn to_entry_resp(e: &stateset_core::JournalEntry) -> JournalEntryResponse {
    JournalEntryResponse {
        id: e.id.to_string(),
        entry_number: e.entry_number.clone(),
        entry_date: e.entry_date.to_string(),
        period_id: e.period_id.to_string(),
        entry_type: e.entry_type.to_string(),
        source: e.source.to_string(),
        description: e.description.clone(),
        total_debits: e.total_debits.to_string(),
        total_credits: e.total_credits.to_string(),
        is_balanced: e.is_balanced,
        status: e.status.to_string(),
        posted_at: e.posted_at.map(|t| t.to_rfc3339()),
        posted_by: e.posted_by.clone(),
        reversed_entry_id: e.reversed_entry_id.map(|id| id.to_string()),
        reversing_entry_id: e.reversing_entry_id.map(|id| id.to_string()),
        lines: e.lines.iter().map(to_line_resp).collect(),
        created_at: e.created_at.to_rfc3339(),
    }
}

fn to_period_resp(p: &stateset_core::GlPeriod) -> GlPeriodResponse {
    GlPeriodResponse {
        id: p.id.to_string(),
        period_name: p.period_name.clone(),
        fiscal_year: p.fiscal_year,
        period_number: p.period_number,
        start_date: p.start_date.to_string(),
        end_date: p.end_date.to_string(),
        status: p.status.to_string(),
        closed_at: p.closed_at.map(|t| t.to_rfc3339()),
        closed_by: p.closed_by.clone(),
        locked_at: p.locked_at.map(|t| t.to_rfc3339()),
        locked_by: p.locked_by.clone(),
        created_at: p.created_at.to_rfc3339(),
    }
}

fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_decimal(s: &str, what: &str) -> Result<Decimal, HttpError> {
    s.parse().map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s}")))
}

fn parse_date(s: &str, what: &str) -> Result<NaiveDate, HttpError> {
    s.parse()
        .map_err(|_| HttpError::BadRequest(format!("invalid {what}: {s} (expected YYYY-MM-DD)")))
}

fn actor_or_default(actor: Option<String>) -> String {
    actor.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| DEFAULT_ACTOR.to_string())
}

// ============================================================================
// Router
// ============================================================================

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/gl/accounts", post(create_account).get(list_accounts))
        .route("/gl/accounts/{id}", get(get_account))
        .route("/gl/journal-entries", post(create_journal_entry).get(list_journal_entries))
        .route("/gl/journal-entries/{id}", get(get_journal_entry))
        .route("/gl/journal-entries/{id}/post", post(post_journal_entry))
        .route("/gl/journal-entries/{id}/void", post(void_journal_entry))
        .route("/gl/journal-entries/{id}/reverse", post(reverse_journal_entry))
        .route("/gl/revalue", post(revalue))
        .route("/gl/trial-balance", get(trial_balance))
        .route("/gl/balance-sheet", get(balance_sheet))
        .route("/gl/income-statement", get(income_statement))
        .route("/gl/periods", post(create_period).get(list_periods))
        .route("/gl/periods/{id}/open", post(open_period))
        .route("/gl/periods/{id}/close", post(close_period))
        .route("/gl/periods/{id}/lock", post(lock_period))
        .route("/gl/periods/{id}/reopen", post(reopen_period))
}

// ============================================================================
// Chart of accounts handlers
// ============================================================================

#[utoipa::path(post, operation_id = "general_ledger_create_account", path = "/api/v1/gl/accounts", tag = "general_ledger",
    request_body = CreateGlAccountRequest,
    responses((status = 201, body = GlAccountResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateGlAccountRequest>,
) -> Result<(StatusCode, Json<GlAccountResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let account_sub_type = match req.account_sub_type.as_deref() {
        Some(s) => Some(parse_id(s, "account_sub_type")?),
        None => None,
    };
    let parent_account_id = match req.parent_account_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "parent_account_id")?),
        None => None,
    };
    let currency = match req.currency.as_deref() {
        Some(s) => Some(parse_id(s, "currency")?),
        None => None,
    };
    let input = stateset_core::CreateGlAccount {
        account_number: req.account_number,
        name: req.name,
        description: req.description,
        account_type: parse_id(&req.account_type, "account_type")?,
        account_sub_type,
        parent_account_id,
        is_header: req.is_header,
        is_posting: req.is_posting,
        currency,
    };
    let account = c.general_ledger().create_account(input)?;
    Ok((StatusCode::CREATED, Json(to_account_resp(&account))))
}

#[utoipa::path(get, operation_id = "general_ledger_list_accounts", path = "/api/v1/gl/accounts", tag = "general_ledger",
    params(GlAccountFilterParams),
    responses((status = 200, body = GlAccountListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_accounts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<GlAccountFilterParams>,
) -> Result<Json<GlAccountListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let account_type = match params.account_type.as_deref() {
        Some(s) => Some(parse_id(s, "account_type")?),
        None => None,
    };
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let base = stateset_core::GlAccountFilter {
        account_type,
        status,
        search: params.search.clone(),
        ..Default::default()
    };
    let total = c.general_ledger().list_accounts(base.clone())?.len();
    let filter = stateset_core::GlAccountFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let accounts = c.general_ledger().list_accounts(filter)?;
    Ok(Json(GlAccountListResponse {
        accounts: accounts.iter().map(to_account_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, operation_id = "general_ledger_get_account", path = "/api/v1/gl/accounts/{id}", tag = "general_ledger",
    params(("id" = String, Path, description = "GL account ID")),
    responses((status = 200, body = GlAccountResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<GlAccountResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let account = c
        .general_ledger()
        .get_account(id)?
        .ok_or_else(|| HttpError::NotFound(format!("GL account {id} not found")))?;
    Ok(Json(to_account_resp(&account)))
}

// ============================================================================
// Journal entry handlers
// ============================================================================

#[utoipa::path(post, operation_id = "general_ledger_create_journal_entry", path = "/api/v1/gl/journal-entries", tag = "general_ledger",
    request_body = CreateJournalEntryRequest,
    responses((status = 201, body = JournalEntryResponse), (status = 400, body = ErrorBody), (status = 422, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_journal_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateJournalEntryRequest>,
) -> Result<(StatusCode, Json<JournalEntryResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let entry_type = match req.entry_type.as_deref() {
        Some(s) => Some(parse_id(s, "entry_type")?),
        None => None,
    };
    let mut lines = Vec::with_capacity(req.lines.len());
    for line in req.lines {
        lines.push(stateset_core::CreateJournalEntryLine {
            account_id: parse_id::<Uuid>(&line.account_id, "account_id")?,
            description: line.description,
            debit_amount: match line.debit_amount.as_deref() {
                Some(s) => parse_decimal(s, "debit_amount")?,
                None => Decimal::ZERO,
            },
            credit_amount: match line.credit_amount.as_deref() {
                Some(s) => parse_decimal(s, "credit_amount")?,
                None => Decimal::ZERO,
            },
            reference_type: None,
            reference_id: None,
        });
    }
    let input = stateset_core::CreateJournalEntry {
        entry_date: parse_date(&req.entry_date, "entry_date")?,
        entry_type,
        description: req.description,
        lines,
        source_document_type: None,
        source_document_id: None,
        auto_post: req.auto_post,
    };
    let entry = c.general_ledger().create_journal_entry(input)?;
    Ok((StatusCode::CREATED, Json(to_entry_resp(&entry))))
}

#[utoipa::path(get, operation_id = "general_ledger_list_journal_entries", path = "/api/v1/gl/journal-entries", tag = "general_ledger",
    params(JournalEntryFilterParams),
    responses((status = 200, body = JournalEntryListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_journal_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<JournalEntryFilterParams>,
) -> Result<Json<JournalEntryListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let period_id = match params.period_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "period_id")?),
        None => None,
    };
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let entry_type = match params.entry_type.as_deref() {
        Some(s) => Some(parse_id(s, "entry_type")?),
        None => None,
    };
    let account_id = match params.account_id.as_deref() {
        Some(s) => Some(parse_id::<Uuid>(s, "account_id")?),
        None => None,
    };
    let from_date = match params.from_date.as_deref() {
        Some(s) => Some(parse_date(s, "from_date")?),
        None => None,
    };
    let to_date = match params.to_date.as_deref() {
        Some(s) => Some(parse_date(s, "to_date")?),
        None => None,
    };
    let base = stateset_core::JournalEntryFilter {
        period_id,
        status,
        entry_type,
        account_id,
        from_date,
        to_date,
        ..Default::default()
    };
    let total = c.general_ledger().list_journal_entries(base.clone())?.len();
    let filter = stateset_core::JournalEntryFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let entries = c.general_ledger().list_journal_entries(filter)?;
    Ok(Json(JournalEntryListResponse {
        journal_entries: entries.iter().map(to_entry_resp).collect(),
        total,
    }))
}

#[utoipa::path(get, operation_id = "general_ledger_get_journal_entry", path = "/api/v1/gl/journal-entries/{id}", tag = "general_ledger",
    params(("id" = String, Path, description = "Journal entry ID")),
    responses((status = 200, body = JournalEntryResponse), (status = 404, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn get_journal_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<JournalEntryResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let entry = c
        .general_ledger()
        .get_journal_entry(id)?
        .ok_or_else(|| HttpError::NotFound(format!("Journal entry {id} not found")))?;
    Ok(Json(to_entry_resp(&entry)))
}

#[utoipa::path(post, operation_id = "general_ledger_post_journal_entry", path = "/api/v1/gl/journal-entries/{id}/post", tag = "general_ledger",
    request_body = PostJournalEntryRequest,
    params(("id" = String, Path, description = "Journal entry ID")),
    responses((status = 200, body = JournalEntryResponse), (status = 422, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn post_journal_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<PostJournalEntryRequest>,
) -> Result<Json<JournalEntryResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let posted_by = actor_or_default(req.posted_by);
    Ok(Json(to_entry_resp(&c.general_ledger().post_journal_entry(id, &posted_by)?)))
}

#[utoipa::path(post, operation_id = "general_ledger_void_journal_entry", path = "/api/v1/gl/journal-entries/{id}/void", tag = "general_ledger",
    params(("id" = String, Path, description = "Journal entry ID")),
    responses((status = 200, body = JournalEntryResponse), (status = 422, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn void_journal_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<JournalEntryResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_entry_resp(&c.general_ledger().void_journal_entry(id)?)))
}

#[utoipa::path(post, operation_id = "general_ledger_reverse_journal_entry", path = "/api/v1/gl/journal-entries/{id}/reverse", tag = "general_ledger",
    request_body = ReverseJournalEntryRequest,
    params(("id" = String, Path, description = "Journal entry ID")),
    responses((status = 200, body = JournalEntryResponse), (status = 422, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn reverse_journal_entry(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ReverseJournalEntryRequest>,
) -> Result<Json<JournalEntryResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let reversal_date = match req.reversal_date.as_deref() {
        Some(s) => parse_date(s, "reversal_date")?,
        None => chrono::Utc::now().date_naive(),
    };
    Ok(Json(to_entry_resp(&c.general_ledger().reverse_journal_entry(id, reversal_date)?)))
}

// ============================================================================
// Report handlers
// ============================================================================

fn as_of_or_today(params: &AsOfDateParams) -> Result<NaiveDate, HttpError> {
    match params.as_of_date.as_deref() {
        Some(s) => parse_date(s, "as_of_date"),
        None => Ok(chrono::Utc::now().date_naive()),
    }
}

#[utoipa::path(post, operation_id = "general_ledger_revalue", path = "/api/v1/gl/revalue", tag = "general_ledger",
    request_body = RevalueRequest,
    responses((status = 200, body = RevaluationResponse), (status = 400, body = ErrorBody), (status = 422, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn revalue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RevalueRequest>,
) -> Result<Json<RevaluationResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let as_of_date = match req.as_of_date.as_deref() {
        Some(s) => parse_date(s, "as_of_date")?,
        None => chrono::Utc::now().date_naive(),
    };
    let base_currency = match req.base_currency.as_deref() {
        Some(s) => Some(parse_id::<stateset_core::Currency>(s, "base_currency")?),
        None => None,
    };
    let result = c.general_ledger().revalue(as_of_date, base_currency)?;
    Ok(Json(RevaluationResponse {
        as_of_date: result.as_of_date.to_string(),
        base_currency: result.base_currency.to_string(),
        total_unrealized_gain_loss: result.total_unrealized_gain_loss.to_string(),
        lines: result
            .lines
            .iter()
            .map(|l| RevaluationLineResponse {
                account_id: l.account_id.to_string(),
                account_number: l.account_number.clone(),
                account_name: l.account_name.clone(),
                currency: l.currency.to_string(),
                foreign_balance: l.foreign_balance.to_string(),
                carrying_value: l.carrying_value.to_string(),
                rate: l.rate.to_string(),
                revalued_value: l.revalued_value.to_string(),
                adjustment: l.adjustment.to_string(),
                unrealized_gain_loss: l.unrealized_gain_loss.to_string(),
            })
            .collect(),
        journal_entry: result.journal_entry.as_ref().map(to_entry_resp),
    }))
}

#[utoipa::path(get, operation_id = "general_ledger_trial_balance", path = "/api/v1/gl/trial-balance", tag = "general_ledger",
    params(AsOfDateParams),
    responses((status = 200, body = TrialBalanceResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn trial_balance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<AsOfDateParams>,
) -> Result<Json<TrialBalanceResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let report = c.general_ledger().get_trial_balance(as_of_or_today(&params)?)?;
    Ok(Json(TrialBalanceResponse {
        as_of_date: report.as_of_date.to_string(),
        total_debits: report.total_debits.to_string(),
        total_credits: report.total_credits.to_string(),
        is_balanced: report.is_balanced,
        lines: report
            .lines
            .iter()
            .map(|l| TrialBalanceLineResponse {
                account_id: l.account_id.to_string(),
                account_number: l.account_number.clone(),
                account_name: l.account_name.clone(),
                account_type: l.account_type.to_string(),
                debit_balance: l.debit_balance.to_string(),
                credit_balance: l.credit_balance.to_string(),
            })
            .collect(),
    }))
}

fn to_balance_sheet_line(l: &stateset_core::BalanceSheetLine) -> BalanceSheetLineResponse {
    BalanceSheetLineResponse {
        account_id: l.account_id.to_string(),
        account_number: l.account_number.clone(),
        account_name: l.account_name.clone(),
        balance: l.balance.to_string(),
        indent_level: l.indent_level,
        is_total: l.is_total,
    }
}

#[utoipa::path(get, operation_id = "general_ledger_balance_sheet", path = "/api/v1/gl/balance-sheet", tag = "general_ledger",
    params(AsOfDateParams),
    responses((status = 200, body = BalanceSheetResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn balance_sheet(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<AsOfDateParams>,
) -> Result<Json<BalanceSheetResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let report = c.general_ledger().get_balance_sheet(as_of_or_today(&params)?)?;
    Ok(Json(BalanceSheetResponse {
        as_of_date: report.as_of_date.to_string(),
        total_assets: report.total_assets.to_string(),
        total_liabilities: report.total_liabilities.to_string(),
        total_equity: report.total_equity.to_string(),
        assets: report.assets.iter().map(to_balance_sheet_line).collect(),
        liabilities: report.liabilities.iter().map(to_balance_sheet_line).collect(),
        equity: report.equity.iter().map(to_balance_sheet_line).collect(),
    }))
}

fn to_income_statement_line(l: &stateset_core::IncomeStatementLine) -> IncomeStatementLineResponse {
    IncomeStatementLineResponse {
        account_id: l.account_id.to_string(),
        account_number: l.account_number.clone(),
        account_name: l.account_name.clone(),
        amount: l.amount.to_string(),
        indent_level: l.indent_level,
        is_total: l.is_total,
    }
}

#[utoipa::path(get, operation_id = "general_ledger_income_statement", path = "/api/v1/gl/income-statement", tag = "general_ledger",
    params(IncomeStatementParams),
    responses((status = 200, body = IncomeStatementResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn income_statement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<IncomeStatementParams>,
) -> Result<Json<IncomeStatementResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let start_date = parse_date(&params.start_date, "start_date")?;
    let end_date = parse_date(&params.end_date, "end_date")?;
    if end_date < start_date {
        return Err(HttpError::BadRequest("end_date must not precede start_date".to_string()));
    }
    let report = c.general_ledger().get_income_statement(start_date, end_date)?;
    Ok(Json(IncomeStatementResponse {
        period_start: report.period_start.to_string(),
        period_end: report.period_end.to_string(),
        total_revenue: report.total_revenue.to_string(),
        total_expenses: report.total_expenses.to_string(),
        net_income: report.net_income.to_string(),
        revenue_lines: report.revenue_lines.iter().map(to_income_statement_line).collect(),
        expense_lines: report.expense_lines.iter().map(to_income_statement_line).collect(),
    }))
}

// ============================================================================
// Period handlers
// ============================================================================

#[utoipa::path(post, operation_id = "general_ledger_create_period", path = "/api/v1/gl/periods", tag = "general_ledger",
    request_body = CreateGlPeriodRequest,
    responses((status = 201, body = GlPeriodResponse), (status = 400, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn create_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateGlPeriodRequest>,
) -> Result<(StatusCode, Json<GlPeriodResponse>), HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let start_date = parse_date(&req.start_date, "start_date")?;
    let end_date = parse_date(&req.end_date, "end_date")?;
    if end_date < start_date {
        return Err(HttpError::BadRequest("end_date must not precede start_date".to_string()));
    }
    let input = stateset_core::CreateGlPeriod {
        period_name: req.period_name,
        fiscal_year: req.fiscal_year,
        period_number: req.period_number,
        start_date,
        end_date,
    };
    let period = c.general_ledger().create_period(input)?;
    Ok((StatusCode::CREATED, Json(to_period_resp(&period))))
}

#[utoipa::path(get, operation_id = "general_ledger_list_periods", path = "/api/v1/gl/periods", tag = "general_ledger",
    params(GlPeriodFilterParams),
    responses((status = 200, body = GlPeriodListResponse)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn list_periods(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<GlPeriodFilterParams>,
) -> Result<Json<GlPeriodListResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let status = match params.status.as_deref() {
        Some(s) => Some(parse_id(s, "status")?),
        None => None,
    };
    let base = stateset_core::GlPeriodFilter {
        fiscal_year: params.fiscal_year,
        status,
        ..Default::default()
    };
    let total = c.general_ledger().list_periods(base.clone())?.len();
    let filter = stateset_core::GlPeriodFilter {
        limit: Some(params.limit.unwrap_or(50).clamp(1, 200)),
        offset: Some(params.offset.unwrap_or(0)),
        ..base
    };
    let periods = c.general_ledger().list_periods(filter)?;
    Ok(Json(GlPeriodListResponse { periods: periods.iter().map(to_period_resp).collect(), total }))
}

#[utoipa::path(post, operation_id = "general_ledger_open_period", path = "/api/v1/gl/periods/{id}/open", tag = "general_ledger",
    params(("id" = String, Path, description = "GL period ID")),
    responses((status = 200, body = GlPeriodResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn open_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<GlPeriodResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_period_resp(&c.general_ledger().open_period(id)?)))
}

#[utoipa::path(post, operation_id = "general_ledger_close_period", path = "/api/v1/gl/periods/{id}/close", tag = "general_ledger",
    request_body = ClosePeriodRequest,
    params(("id" = String, Path, description = "GL period ID")),
    responses((status = 200, body = GlPeriodResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn close_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<ClosePeriodRequest>,
) -> Result<Json<GlPeriodResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let closed_by = actor_or_default(req.closed_by);
    Ok(Json(to_period_resp(&c.general_ledger().close_period(id, &closed_by)?)))
}

#[utoipa::path(post, operation_id = "general_ledger_lock_period", path = "/api/v1/gl/periods/{id}/lock", tag = "general_ledger",
    request_body = LockPeriodRequest,
    params(("id" = String, Path, description = "GL period ID")),
    responses((status = 200, body = GlPeriodResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers, req))]
pub(crate) async fn lock_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<LockPeriodRequest>,
) -> Result<Json<GlPeriodResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    let locked_by = actor_or_default(req.locked_by);
    Ok(Json(to_period_resp(&c.general_ledger().lock_period(id, &locked_by)?)))
}

#[utoipa::path(post, operation_id = "general_ledger_reopen_period", path = "/api/v1/gl/periods/{id}/reopen", tag = "general_ledger",
    params(("id" = String, Path, description = "GL period ID")),
    responses((status = 200, body = GlPeriodResponse), (status = 409, body = ErrorBody)))]
#[tracing::instrument(skip(state, headers))]
pub(crate) async fn reopen_period(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<GlPeriodResponse>, HttpError> {
    let tid = tenant_id_from_headers(&headers);
    let c = state.commerce_for_tenant(tid.as_deref())?;
    Ok(Json(to_period_resp(&c.general_ledger().reopen_period(id)?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use stateset_embedded::Commerce;
    use tower::ServiceExt;

    fn app() -> Router {
        let state = AppState::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        router().with_state(state)
    }

    async fn send(
        app: &Router,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let builder = Request::builder().method(method).uri(uri);
        let request = match body {
            Some(json) => builder
                .header("content-type", "application/json")
                .body(Body::from(json.to_string()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        };
        let resp = app.clone().oneshot(request).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, json)
    }

    async fn create_open_period(app: &Router) -> String {
        let (status, period) = send(
            app,
            "POST",
            "/gl/periods",
            Some(serde_json::json!({
                "period_name": "2025-01",
                "fiscal_year": 2025,
                "period_number": 1,
                "start_date": "2025-01-01",
                "end_date": "2025-01-31"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(period["status"], "future");
        let id = period["id"].as_str().unwrap().to_string();

        let (status, opened) = send(app, "POST", &format!("/gl/periods/{id}/open"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(opened["status"], "open");
        id
    }

    async fn create_account(app: &Router, number: &str, name: &str, kind: &str) -> String {
        let (status, account) = send(
            app,
            "POST",
            "/gl/accounts",
            Some(serde_json::json!({
                "account_number": number,
                "name": name,
                "account_type": kind,
                "is_posting": true
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        account["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn create_post_and_trial_balance_flow() {
        let app = app();
        create_open_period(&app).await;
        let cash = create_account(&app, "1010", "Cash", "asset").await;
        let sales = create_account(&app, "4010", "Sales Revenue", "revenue").await;

        let (status, entry) = send(
            &app,
            "POST",
            "/gl/journal-entries",
            Some(serde_json::json!({
                "entry_date": "2025-01-15",
                "description": "Cash sale",
                "lines": [
                    {"account_id": cash, "debit_amount": "100.00"},
                    {"account_id": sales, "credit_amount": "100.00"}
                ]
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(entry["is_balanced"], true);
        assert_eq!(entry["status"], "draft");
        assert_eq!(entry["total_debits"], "100.00");
        let entry_id = entry["id"].as_str().unwrap().to_string();

        let (status, posted) = send(
            &app,
            "POST",
            &format!("/gl/journal-entries/{entry_id}/post"),
            Some(serde_json::json!({"posted_by": "tester"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(posted["status"], "posted");
        assert_eq!(posted["posted_by"], "tester");

        let (status, fetched) =
            send(&app, "GET", &format!("/gl/journal-entries/{entry_id}"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(fetched["status"], "posted");

        let (status, tb) = send(&app, "GET", "/gl/trial-balance?as_of_date=2025-01-31", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(tb["is_balanced"], true);
        assert_eq!(tb["total_debits"], tb["total_credits"]);
        assert_eq!(tb["total_debits"], "100.00");
    }

    #[tokio::test]
    async fn unbalanced_entry_cannot_be_posted() {
        let app = app();
        create_open_period(&app).await;
        let cash = create_account(&app, "1010", "Cash", "asset").await;
        let sales = create_account(&app, "4010", "Sales Revenue", "revenue").await;

        let (status, entry) = send(
            &app,
            "POST",
            "/gl/journal-entries",
            Some(serde_json::json!({
                "entry_date": "2025-01-15",
                "description": "Unbalanced entry",
                "lines": [
                    {"account_id": cash, "debit_amount": "100.00"},
                    {"account_id": sales, "credit_amount": "40.00"}
                ]
            })),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(entry["is_balanced"], false);
        let entry_id = entry["id"].as_str().unwrap().to_string();

        let (status, body) = send(
            &app,
            "POST",
            &format!("/gl/journal-entries/{entry_id}/post"),
            Some(serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["code"], "validation_error");
    }

    #[tokio::test]
    async fn period_close_lock_reopen_lifecycle() {
        let app = app();
        let period_id = create_open_period(&app).await;

        let (status, closed) = send(
            &app,
            "POST",
            &format!("/gl/periods/{period_id}/close"),
            Some(serde_json::json!({"closed_by": "controller"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(closed["status"], "closed");
        assert_eq!(closed["closed_by"], "controller");

        let (status, reopened) =
            send(&app, "POST", &format!("/gl/periods/{period_id}/reopen"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(reopened["status"], "open");

        let (status, _) = send(
            &app,
            "POST",
            &format!("/gl/periods/{period_id}/close"),
            Some(serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, locked) = send(
            &app,
            "POST",
            &format!("/gl/periods/{period_id}/lock"),
            Some(serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(locked["status"], "locked");

        let (status, listed) = send(&app, "GET", "/gl/periods?status=locked", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(listed["total"], 1);
    }

    #[tokio::test]
    async fn get_missing_account_returns_not_found() {
        let app = app();
        let (status, body) =
            send(&app, "GET", &format!("/gl/accounts/{}", Uuid::new_v4()), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "not_found");
    }
}
