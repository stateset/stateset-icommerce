//! Accounts Payable API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Accounts Payable API
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateBillInput {
    /// Trusted host identity. Remote adapters must supply this from authentication.
    pub created_by: Option<String>,
    pub currency: Option<String>,
    pub purchase_order_id: Option<String>,
    pub supplier_id: String,
    pub due_date: String,
    pub payment_terms: Option<String>,
    pub reference_number: Option<String>,
    pub notes: Option<String>,
    /// Bill lines. Optional: omitted (or empty) creates a header-only bill,
    /// as before. The engine derives each line amount and the bill totals.
    pub items: Option<Vec<CreateBillItemInput>>,
}

/// A bill line. Money follows the binding convention: send the `...Exact`
/// base-10 string or its legacy Exact alias.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateBillItemInput {
    pub description: String,
    pub account_code: Option<String>,
    /// Exact decimal quantity; `quantity_exact` remains an alias.
    pub quantity: Option<String>,
    /// Exact base-10 quantity. Takes precedence over `quantity` when present.
    pub quantity_exact: Option<String>,
    /// Exact decimal unit price; `unit_price_exact` remains an alias.
    pub unit_price: Option<String>,
    /// Exact base-10 unit price. Takes precedence over `unit_price` when present.
    pub unit_price_exact: Option<String>,
    /// Tax rate as a percentage (`"8"` = 8%).
    pub tax_rate: Option<String>,
    /// Exact base-10 tax rate. Takes precedence over `tax_rate` when present.
    pub tax_rate_exact: Option<String>,
    pub po_line_id: Option<String>,
}

impl TryFrom<CreateBillItemInput> for stateset_core::CreateBillItem {
    type Error = Error;

    fn try_from(i: CreateBillItemInput) -> Result<Self> {
        Ok(Self {
            description: i.description,
            account_code: i.account_code,
            quantity: finance_money_input(
                i.quantity_exact.as_deref(),
                i.quantity,
                "bill item quantity",
            )?,
            unit_price: finance_money_input(
                i.unit_price_exact.as_deref(),
                i.unit_price,
                "bill item unit price",
            )?,
            tax_rate: optional_finance_money_input(
                i.tax_rate_exact.as_deref(),
                i.tax_rate,
                "bill item tax rate",
            )?,
            po_line_id: parse_optional_id(i.po_line_id, "PO line")?,
        })
    }
}

/// A bill line as stored, with engine-derived `amount` and `taxAmount`.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BillItemOutput {
    pub id: String,
    pub bill_id: String,
    pub line_number: i32,
    pub description: String,
    pub account_code: Option<String>,
    /// Exact base-10 quantity.
    pub quantity_exact: String,
    /// Exact decimal string.
    pub unit_price: String,
    /// Exact base-10 unit price.
    pub unit_price_exact: String,
    /// Exact decimal string.
    pub amount: String,
    /// Exact base-10 line amount (quantity x unit price).
    pub amount_exact: String,
    /// Exact base-10 tax rate (a fraction), when set.
    pub tax_rate_exact: Option<String>,
    /// Exact decimal string.
    pub tax_amount: String,
    /// Exact base-10 line tax.
    pub tax_amount_exact: String,
    pub po_line_id: Option<String>,
    pub created_at: String,
}

impl TryFrom<stateset_core::BillItem> for BillItemOutput {
    type Error = Error;

    fn try_from(i: stateset_core::BillItem) -> Result<Self> {
        let unit_price = i.unit_price.to_string();
        let unit_price_exact = unit_price.clone();
        let amount = i.amount.to_string();
        let amount_exact = amount.clone();
        let tax_amount = i.tax_amount.to_string();
        let tax_amount_exact = tax_amount.clone();
        Ok(Self {
            id: i.id.to_string(),
            bill_id: i.bill_id.to_string(),
            line_number: i.line_number,
            description: i.description,
            account_code: i.account_code,
            quantity_exact: i.quantity.to_string(),
            unit_price,
            unit_price_exact,
            amount,
            amount_exact,
            tax_rate_exact: i.tax_rate.map(|r| r.to_string()),
            tax_amount,
            tax_amount_exact,
            po_line_id: i.po_line_id.map(|id| id.to_string()),
            created_at: i.created_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BillOutput {
    pub currency: String,
    pub created_by: Option<String>,
    pub approved_by: Option<String>,
    pub match_override_reason: Option<String>,
    pub id: String,
    pub bill_number: String,
    pub supplier_id: String,
    #[napi(ts_type = "BillStatus")]
    pub status: String,
    /// Exact decimal string.
    pub total_amount: String,
    /// Exact base-10 total amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_amount_exact: String,
    /// Exact decimal string.
    pub amount_paid: String,
    /// Exact base-10 amount paid, straight from the engine's `Decimal`. Prefer this field for money.
    pub amount_paid_exact: String,
    /// Exact decimal string.
    pub amount_due: String,
    /// Exact base-10 amount due, straight from the engine's `Decimal`. Prefer this field for money.
    pub amount_due_exact: String,
    pub due_date: String,
    pub created_at: String,
}

impl TryFrom<stateset_core::Bill> for BillOutput {
    type Error = Error;

    fn try_from(b: stateset_core::Bill) -> Result<Self> {
        let total_amount = b.total_amount.to_string();
        let total_amount_exact = total_amount.clone();
        let amount_paid = b.amount_paid.to_string();
        let amount_paid_exact = amount_paid.clone();
        let amount_due = b.amount_due.to_string();
        let amount_due_exact = amount_due.clone();
        Ok(Self {
            id: b.id.to_string(),
            bill_number: b.bill_number,
            currency: b.currency.to_string(),
            match_override_reason: b.match_override_reason,
            created_by: b.created_by,
            approved_by: b.approved_by,
            supplier_id: b.supplier_id.to_string(),
            status: format!("{:?}", b.status),
            total_amount,
            total_amount_exact,
            amount_paid,
            amount_paid_exact,
            amount_due,
            amount_due_exact,
            due_date: b.due_date.to_rfc3339(),
            created_at: b.created_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ApAgingSummaryOutput {
    /// Exact decimal string.
    pub current: String,
    /// Exact base-10 current, straight from the engine's `Decimal`. Prefer this field for money.
    pub current_exact: String,
    /// Exact decimal string.
    pub days_1_30: String,
    /// Exact base-10 days 1 30, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_1_30_exact: String,
    /// Exact decimal string.
    pub days_31_60: String,
    /// Exact base-10 days 31 60, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_31_60_exact: String,
    /// Exact decimal string.
    pub days_61_90: String,
    /// Exact base-10 days 61 90, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_61_90_exact: String,
    /// Exact decimal string.
    pub days_over_90: String,
    /// Exact base-10 days over 90, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_over_90_exact: String,
    /// Exact decimal string.
    pub total: String,
    /// Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_exact: String,
}

impl TryFrom<stateset_core::ApAgingSummary> for ApAgingSummaryOutput {
    type Error = Error;

    fn try_from(a: stateset_core::ApAgingSummary) -> Result<Self> {
        let current = a.current.to_string();
        let current_exact = current.clone();
        let days_1_30 = a.days_1_30.to_string();
        let days_1_30_exact = days_1_30.clone();
        let days_31_60 = a.days_31_60.to_string();
        let days_31_60_exact = days_31_60.clone();
        let days_61_90 = a.days_61_90.to_string();
        let days_61_90_exact = days_61_90.clone();
        let days_over_90 = a.days_over_90.to_string();
        let days_over_90_exact = days_over_90.clone();
        let total = a.total.to_string();
        let total_exact = total.clone();
        Ok(Self {
            current,
            current_exact,
            days_1_30,
            days_1_30_exact,
            days_31_60,
            days_31_60_exact,
            days_61_90,
            days_61_90_exact,
            days_over_90,
            days_over_90_exact,
            total,
            total_exact,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ThreeWayMatchLineOutput {
    pub po_line_id: Option<String>,
    pub bill_item_id: String,
    pub description: String,
    /// Exact decimal string
    pub ordered_quantity: Option<String>,
    /// Exact decimal string
    pub ordered_unit_cost: Option<String>,
    /// Exact decimal string
    pub received_quantity: String,
    /// Exact decimal string
    pub billed_quantity: String,
    /// Exact decimal string
    pub billed_unit_cost: String,
    /// Exact decimal string: billed_quantity - received_quantity
    pub quantity_variance: String,
    /// Exact decimal string: billed_unit_cost - ordered_unit_cost
    pub price_variance: String,
    pub matched: bool,
    pub issues: Vec<String>,
}

impl From<stateset_core::ThreeWayMatchLine> for ThreeWayMatchLineOutput {
    fn from(l: stateset_core::ThreeWayMatchLine) -> Self {
        Self {
            po_line_id: l.po_line_id.map(|id| id.to_string()),
            bill_item_id: l.bill_item_id.to_string(),
            description: l.description,
            ordered_quantity: l.ordered_quantity.map(|d| d.to_string()),
            ordered_unit_cost: l.ordered_unit_cost.map(|d| d.to_string()),
            received_quantity: l.received_quantity.to_string(),
            billed_quantity: l.billed_quantity.to_string(),
            billed_unit_cost: l.billed_unit_cost.to_string(),
            quantity_variance: l.quantity_variance.to_string(),
            price_variance: l.price_variance.to_string(),
            matched: l.matched,
            issues: l.issues,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ThreeWayMatchOutput {
    /// Overall status: not_required, pending, matched, variance
    #[napi(ts_type = "ThreeWayMatchStatus")]
    pub match_status: String,
    /// Number of variance lines (set when match_status is "variance")
    pub variance_line_count: Option<u32>,
    /// Tolerance applied, as an exact decimal string percentage (e.g. "5")
    pub tolerance_percent: String,
    pub lines: Vec<ThreeWayMatchLineOutput>,
}

impl From<stateset_core::ThreeWayMatchResult> for ThreeWayMatchOutput {
    fn from(r: stateset_core::ThreeWayMatchResult) -> Self {
        let (match_status, variance_line_count) = match r.match_status {
            stateset_core::MatchStatus::NotRequired => ("not_required".to_string(), None),
            stateset_core::MatchStatus::Pending => ("pending".to_string(), None),
            stateset_core::MatchStatus::Matched => ("matched".to_string(), None),
            stateset_core::MatchStatus::Variance { variance_line_count } => {
                ("variance".to_string(), Some(variance_line_count as u32))
            }
            _ => ("unknown".to_string(), None),
        };
        Self {
            match_status,
            variance_line_count,
            tolerance_percent: r.tolerance_percent.to_string(),
            lines: r.lines.into_iter().map(Into::into).collect(),
        }
    }
}

/// Optional filters for `AccountsPayable.listBills`. No argument lists all.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct BillFilterInput {
    pub supplier_id: Option<String>,
    /// The rendered form (`PartiallyPaid`) or the engine's snake_case (`partially_paid`).
    #[napi(ts_type = "BillStatusInput")]
    pub status: Option<String>,
    pub purchase_order_id: Option<String>,
    pub overdue_only: Option<bool>,
    /// RFC 3339 timestamp.
    pub from_date: Option<String>,
    /// RFC 3339 timestamp.
    pub to_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl TryFrom<BillFilterInput> for stateset_core::BillFilter {
    type Error = Error;

    fn try_from(f: BillFilterInput) -> Result<Self> {
        Ok(Self {
            supplier_id: parse_optional_id(f.supplier_id, "supplier")?,
            status: parse_optional_enum(f.status, "bill status")?,
            purchase_order_id: parse_optional_id(f.purchase_order_id, "purchase order")?,
            overdue_only: f.overdue_only,
            from_date: parse_optional_datetime(f.from_date, "from date")?,
            to_date: parse_optional_datetime(f.to_date, "to date")?,
            limit: f.limit,
            offset: f.offset,
            ..Default::default()
        })
    }
}

#[napi]
pub struct AccountsPayable {
    pub(crate) commerce: Handle,
}

#[napi]
impl AccountsPayable {
    #[napi]
    pub async fn create_payment(&self, input: CreateBillPaymentInput) -> Result<BillPaymentOutput> {
        self.commerce
            .get()?
            .accounts_payable()
            .create_payment(ap_workflow_input(input)?)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to pay bills", e))
    }
    #[napi]
    pub async fn get_payment(&self, id: String) -> Result<Option<BillPaymentOutput>> {
        self.commerce
            .get()?
            .accounts_payable()
            .get_payment(ap_workflow_id(&id)?)
            .map(|v| v.map(Into::into))
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get bill payment", e))
    }
    #[napi]
    pub async fn void_payment(&self, id: String) -> Result<BillPaymentOutput> {
        self.commerce
            .get()?
            .accounts_payable()
            .void_payment(ap_workflow_id(&id)?)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to void bill payment", e))
    }
    #[napi]
    pub async fn create_payment_run(
        &self,
        input: CreateApPaymentRunInput,
        actor: String,
    ) -> Result<ApPaymentRunOutput> {
        let mut request: stateset_core::CreatePaymentRun = ap_workflow_input(input)?;
        request.created_by = Some(actor);
        self.commerce
            .get()?
            .accounts_payable()
            .create_payment_run(request)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create payment run", e))
    }
    #[napi]
    pub async fn get_payment_run(&self, id: String) -> Result<Option<ApPaymentRunOutput>> {
        self.commerce
            .get()?
            .accounts_payable()
            .get_payment_run(ap_workflow_id(&id)?)
            .map(|v| v.map(Into::into))
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get payment run", e))
    }
    #[napi]
    pub async fn list_payment_runs(
        &self,
        filter: Option<ApPaymentRunFilterInput>,
    ) -> Result<Vec<ApPaymentRunOutput>> {
        self.commerce
            .get()?
            .accounts_payable()
            .list_payment_runs(ap_workflow_input(filter.unwrap_or_default())?)
            .map(|v| v.into_iter().map(Into::into).collect())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list payment runs", e))
    }
    #[napi]
    pub async fn approve_payment_run(
        &self,
        id: String,
        actor: String,
    ) -> Result<ApPaymentRunOutput> {
        self.commerce
            .get()?
            .accounts_payable()
            .approve_payment_run(ap_workflow_id(&id)?, &actor)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to approve payment run", e))
    }
    #[napi]
    pub async fn process_payment_run(&self, id: String) -> Result<ApPaymentRunOutput> {
        self.commerce
            .get()?
            .accounts_payable()
            .process_payment_run(ap_workflow_id(&id)?)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to process payment run", e))
    }
    #[napi]
    pub async fn cancel_payment_run(&self, id: String) -> Result<ApPaymentRunOutput> {
        self.commerce
            .get()?
            .accounts_payable()
            .cancel_payment_run(ap_workflow_id(&id)?)
            .map(Into::into)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel payment run", e))
    }

    /// Create a bill
    #[napi]
    pub async fn create_bill(&self, input: CreateBillInput) -> Result<BillOutput> {
        let commerce = self.commerce.get()?;
        let supplier_id = input
            .supplier_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid supplier UUID"))?;
        let due_date = chrono::DateTime::parse_from_rfc3339(&input.due_date)
            .map_err(|_| coded(ErrCode::Validation, "Invalid due date format"))?
            .with_timezone(&chrono::Utc);
        let bill = commerce
            .accounts_payable()
            .create_bill(stateset_core::CreateBill {
                created_by: input.created_by,
                bill_number: None,
                supplier_id,
                purchase_order_id: parse_optional_id(input.purchase_order_id, "purchase order")?,
                bill_date: None,
                due_date,
                payment_terms: input.payment_terms,
                currency: parse_optional_enum(input.currency, "currency")?,
                reference_number: input.reference_number,
                memo: input.notes,
                items: input
                    .items
                    .unwrap_or_default()
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>>>()?,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create bill", e))?;
        convert_output(bill)
    }

    /// The lines of a bill, in line order.
    #[napi]
    pub async fn get_bill_items(&self, bill_id: String) -> Result<Vec<BillItemOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            bill_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid bill UUID"))?;
        let items = commerce
            .accounts_payable()
            .get_bill_items(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get bill items", e))?;
        convert_outputs(items)
    }

    /// Get a bill by ID
    #[napi]
    pub async fn get_bill(&self, id: String) -> Result<Option<BillOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let bill = commerce
            .accounts_payable()
            .get_bill(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get bill", e))?;
        convert_optional_output(bill)
    }

    /// Get a bill by bill number
    #[napi]
    pub async fn get_bill_by_number(&self, number: String) -> Result<Option<BillOutput>> {
        let commerce = self.commerce.get()?;
        let bill = commerce
            .accounts_payable()
            .get_bill_by_number(&number)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get bill", e))?;
        convert_optional_output(bill)
    }

    /// List bills, optionally filtered and paginated. No argument lists all.
    #[napi]
    pub async fn list_bills(&self, filter: Option<BillFilterInput>) -> Result<Vec<BillOutput>> {
        let commerce = self.commerce.get()?;
        let filter: stateset_core::BillFilter = filter.unwrap_or_default().try_into()?;
        let bills = commerce
            .accounts_payable()
            .list_bills(filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list bills", e))?;
        convert_outputs(bills)
    }

    /// Approve a bill
    #[napi]
    pub async fn approve_bill(
        &self,
        id: String,
        override_reason: Option<String>,
        actor: Option<String>,
    ) -> Result<BillOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let bill = if let Some(actor) = actor {
            commerce.accounts_payable().approve_bill_as(uuid, &actor, override_reason.as_deref())
        } else {
            match override_reason.as_deref() {
                Some(reason) => {
                    commerce.accounts_payable().approve_bill_with_override(uuid, reason)
                }
                None => commerce.accounts_payable().approve_bill(uuid),
            }
        }
        .map_err(|e| wrap(ErrCode::Internal, "Failed to approve bill", e))?;
        convert_output(bill)
    }

    /// Dispute a bill.
    #[napi]
    pub async fn dispute_bill(&self, id: String) -> Result<BillOutput> {
        let commerce = self.commerce.get()?;
        let id = id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid bill UUID"))?;
        let bill = commerce
            .accounts_payable()
            .dispute_bill(id)
            .map_err(|e| wrap(ErrCode::Internal, "Dispute a bill failed", e))?;
        convert_output(bill)
    }
    /// Resolve a bill dispute.
    #[napi]
    pub async fn resolve_bill_dispute(&self, id: String) -> Result<BillOutput> {
        let commerce = self.commerce.get()?;
        let id = id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid bill UUID"))?;
        let bill = commerce
            .accounts_payable()
            .resolve_bill_dispute(id)
            .map_err(|e| wrap(ErrCode::Internal, "Resolve a bill dispute failed", e))?;
        convert_output(bill)
    }
    /// Cancel a bill
    #[napi]
    pub async fn cancel_bill(&self, id: String) -> Result<BillOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let bill = commerce
            .accounts_payable()
            .cancel_bill(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to cancel bill", e))?;
        convert_output(bill)
    }

    /// Get overdue bills
    #[napi]
    pub async fn get_overdue_bills(&self) -> Result<Vec<BillOutput>> {
        let commerce = self.commerce.get()?;
        let bills = commerce
            .accounts_payable()
            .get_overdue_bills()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get overdue bills", e))?;
        convert_outputs(bills)
    }

    /// Get bills due soon
    #[napi]
    pub async fn get_bills_due_soon(&self, days: i32) -> Result<Vec<BillOutput>> {
        let commerce = self.commerce.get()?;
        let bills = commerce
            .accounts_payable()
            .get_bills_due_soon(days)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get bills", e))?;
        convert_outputs(bills)
    }

    /// Get aging summary
    #[napi]
    pub async fn get_aging_summary(&self) -> Result<ApAgingSummaryOutput> {
        let commerce = self.commerce.get()?;
        let aging = commerce
            .accounts_payable()
            .get_aging_summary()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get aging", e))?;
        convert_output(aging)
    }

    /// Get total outstanding
    #[napi]
    pub async fn get_total_outstanding(&self) -> Result<String> {
        let commerce = self.commerce.get()?;
        let total = commerce
            .accounts_payable()
            .get_total_outstanding()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get total", e))?;
        Ok(total.to_string())
    }

    /// Count bills
    #[napi]
    pub async fn count_bills(&self) -> Result<u32> {
        let commerce = self.commerce.get()?;
        let count = commerce
            .accounts_payable()
            .count_bills(Default::default())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to count bills", e))?;
        Ok(count as u32)
    }

    /// Three-way match a bill against its purchase order and receipts.
    ///
    /// `tolerance_percent` is an exact decimal string (e.g. "5" for 5%);
    /// omit it for exact matching.
    #[napi]
    pub async fn three_way_match(
        &self,
        bill_id: String,
        tolerance_percent: Option<String>,
    ) -> Result<ThreeWayMatchOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            bill_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let tolerance = tolerance_percent
            .map(|s| {
                s.parse::<Decimal>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid tolerance_percent decimal"))
            })
            .transpose()?;
        let result = commerce
            .accounts_payable()
            .three_way_match(uuid, tolerance)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to three-way match bill", e))?;
        Ok(result.into())
    }
}

fn ap_workflow_input<T: serde::de::DeserializeOwned>(value: impl Serialize) -> Result<T> {
    let value = serde_json::to_value(value)
        .map_err(|e| wrap(ErrCode::Validation, "Invalid finance input", e))?;
    serde_json::from_value(value).map_err(|e| wrap(ErrCode::Validation, "Invalid finance input", e))
}
fn ap_workflow_id(id: &str) -> Result<uuid::Uuid> {
    id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid finance UUID"))
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateBillPaymentInput {
    pub supplier_id: String,
    pub payment_date: Option<String>,
    pub payment_method: String,
    pub amount: String,
    pub currency: Option<String>,
    pub reference_number: Option<String>,
    pub bank_account: Option<String>,
    pub check_number: Option<String>,
    pub memo: Option<String>,
    pub allocations: Vec<BillPaymentAllocationInput>,
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BillPaymentAllocationInput {
    pub bill_id: String,
    pub amount: String,
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateApPaymentRunInput {
    pub payment_date: String,
    pub payment_method: String,
    pub bill_ids: Vec<String>,
    pub notes: Option<String>,
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ApPaymentRunFilterInput {
    pub status: Option<String>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct BillPaymentOutput {
    pub id: String,
    pub supplier_id: String,
    pub payment_method: String,
    pub amount: String,
    pub currency: String,
    pub status: String,
    pub payment_number: String,
    pub reference_number: Option<String>,
    pub bank_account: Option<String>,
    pub check_number: Option<String>,
    pub memo: Option<String>,
    pub payment_date: String,
    pub created_at: String,
    pub updated_at: String,
}
impl From<stateset_core::BillPayment> for BillPaymentOutput {
    fn from(v: stateset_core::BillPayment) -> Self {
        Self {
            id: v.id.to_string(),
            supplier_id: v.supplier_id.to_string(),
            payment_method: v.payment_method.to_string(),
            amount: v.amount.to_string(),
            currency: v.currency.to_string(),
            status: v.status.to_string(),
            payment_number: v.payment_number,
            reference_number: v.reference_number,
            bank_account: v.bank_account,
            check_number: v.check_number,
            memo: v.memo,
            payment_date: v.payment_date.to_rfc3339(),
            created_at: v.created_at.to_rfc3339(),
            updated_at: v.updated_at.to_rfc3339(),
        }
    }
}
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ApPaymentRunOutput {
    pub id: String,
    pub status: String,
    pub payment_method: String,
    pub total_amount: String,
    pub currency: Option<String>,
    pub run_number: String,
    pub payment_count: i32,
    pub notes: Option<String>,
    pub created_by: Option<String>,
    pub approved_by: Option<String>,
    pub approved_at: Option<String>,
    pub processed_at: Option<String>,
    pub payment_date: String,
    pub created_at: String,
    pub updated_at: String,
}
impl From<stateset_core::PaymentRun> for ApPaymentRunOutput {
    fn from(v: stateset_core::PaymentRun) -> Self {
        Self {
            id: v.id.to_string(),
            status: v.status.to_string(),
            payment_method: v.payment_method.to_string(),
            total_amount: v.total_amount.to_string(),
            currency: v.currency.map(|x| x.to_string()),
            run_number: v.run_number,
            payment_count: v.payment_count,
            notes: v.notes,
            created_by: v.created_by,
            approved_by: v.approved_by,
            approved_at: v.approved_at.map(|x| x.to_rfc3339()),
            processed_at: v.processed_at.map(|x| x.to_rfc3339()),
            payment_date: v.payment_date.to_rfc3339(),
            created_at: v.created_at.to_rfc3339(),
            updated_at: v.updated_at.to_rfc3339(),
        }
    }
}
