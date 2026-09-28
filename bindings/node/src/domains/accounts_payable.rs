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
/// base-10 string (preferred) or the float.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateBillItemInput {
    pub description: String,
    pub account_code: Option<String>,
    /// Float quantity. Optional: send `quantity_exact` instead.
    pub quantity: Option<f64>,
    /// Exact base-10 quantity. Takes precedence over `quantity` when present.
    pub quantity_exact: Option<String>,
    /// Float unit price. Optional: send `unit_price_exact` instead for exact money.
    pub unit_price: Option<f64>,
    /// Exact base-10 unit price. Takes precedence over `unit_price` when present.
    pub unit_price_exact: Option<String>,
    /// Tax rate as a fraction (`0.08` = 8%).
    pub tax_rate: Option<f64>,
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
            quantity: money_input(i.quantity_exact.as_deref(), i.quantity, "bill item quantity")?,
            unit_price: money_input(
                i.unit_price_exact.as_deref(),
                i.unit_price,
                "bill item unit price",
            )?,
            tax_rate: optional_money_input(
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
    /// @deprecated Use the `unitPriceExact` twin; float money will be removed in 2.0.
    pub unit_price: f64,
    /// Exact base-10 unit price.
    pub unit_price_exact: String,
    /// @deprecated Use the `amountExact` twin; float money will be removed in 2.0.
    pub amount: f64,
    /// Exact base-10 line amount (quantity x unit price).
    pub amount_exact: String,
    /// Exact base-10 tax rate (a fraction), when set.
    pub tax_rate_exact: Option<String>,
    /// @deprecated Use the `taxAmountExact` twin; float money will be removed in 2.0.
    pub tax_amount: f64,
    /// Exact base-10 line tax.
    pub tax_amount_exact: String,
    pub po_line_id: Option<String>,
    pub created_at: String,
}

impl TryFrom<stateset_core::BillItem> for BillItemOutput {
    type Error = Error;

    fn try_from(i: stateset_core::BillItem) -> Result<Self> {
        let (unit_price, unit_price_exact) = money_pair(i.unit_price, "bill item unit price")?;
        let (amount, amount_exact) = money_pair(i.amount, "bill item amount")?;
        let (tax_amount, tax_amount_exact) = money_pair(i.tax_amount, "bill item tax amount")?;
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
    pub id: String,
    pub bill_number: String,
    pub supplier_id: String,
    #[napi(ts_type = "BillStatus")]
    pub status: String,
    /// @deprecated Use the `totalAmountExact` twin; float money will be removed in 2.0.
    pub total_amount: f64,
    /// Exact base-10 total amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_amount_exact: String,
    /// @deprecated Use the `amountPaidExact` twin; float money will be removed in 2.0.
    pub amount_paid: f64,
    /// Exact base-10 amount paid, straight from the engine's `Decimal`. Prefer this field for money.
    pub amount_paid_exact: String,
    /// @deprecated Use the `amountDueExact` twin; float money will be removed in 2.0.
    pub amount_due: f64,
    /// Exact base-10 amount due, straight from the engine's `Decimal`. Prefer this field for money.
    pub amount_due_exact: String,
    pub due_date: String,
    pub created_at: String,
}

impl TryFrom<stateset_core::Bill> for BillOutput {
    type Error = Error;

    fn try_from(b: stateset_core::Bill) -> Result<Self> {
        let (total_amount, total_amount_exact) = money_pair(b.total_amount, "bill total amount")?;
        let (amount_paid, amount_paid_exact) = money_pair(b.amount_paid, "bill amount paid")?;
        let (amount_due, amount_due_exact) = money_pair(b.amount_due, "bill amount due")?;
        Ok(Self {
            id: b.id.to_string(),
            bill_number: b.bill_number,
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
    /// @deprecated Use the `currentExact` twin; float money will be removed in 2.0.
    pub current: f64,
    /// Exact base-10 current, straight from the engine's `Decimal`. Prefer this field for money.
    pub current_exact: String,
    /// @deprecated Use the `days130Exact` twin; float money will be removed in 2.0.
    pub days_1_30: f64,
    /// Exact base-10 days 1 30, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_1_30_exact: String,
    /// @deprecated Use the `days3160Exact` twin; float money will be removed in 2.0.
    pub days_31_60: f64,
    /// Exact base-10 days 31 60, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_31_60_exact: String,
    /// @deprecated Use the `days6190Exact` twin; float money will be removed in 2.0.
    pub days_61_90: f64,
    /// Exact base-10 days 61 90, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_61_90_exact: String,
    /// @deprecated Use the `daysOver90Exact` twin; float money will be removed in 2.0.
    pub days_over_90: f64,
    /// Exact base-10 days over 90, straight from the engine's `Decimal`. Prefer this field for money.
    pub days_over_90_exact: String,
    /// @deprecated Use the `totalExact` twin; float money will be removed in 2.0.
    pub total: f64,
    /// Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_exact: String,
}

impl TryFrom<stateset_core::ApAgingSummary> for ApAgingSummaryOutput {
    type Error = Error;

    fn try_from(a: stateset_core::ApAgingSummary) -> Result<Self> {
        let (current, current_exact) = money_pair(a.current, "AP aging current")?;
        let (days_1_30, days_1_30_exact) = money_pair(a.days_1_30, "AP aging 1-30 days")?;
        let (days_31_60, days_31_60_exact) = money_pair(a.days_31_60, "AP aging 31-60 days")?;
        let (days_61_90, days_61_90_exact) = money_pair(a.days_61_90, "AP aging 61-90 days")?;
        let (days_over_90, days_over_90_exact) =
            money_pair(a.days_over_90, "AP aging over 90 days")?;
        let (total, total_exact) = money_pair(a.total, "AP aging total")?;
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
                bill_number: None,
                supplier_id,
                purchase_order_id: None,
                bill_date: None,
                due_date,
                payment_terms: input.payment_terms,
                currency: None,
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
    pub async fn approve_bill(&self, id: String) -> Result<BillOutput> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let bill = commerce
            .accounts_payable()
            .approve_bill(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to approve bill", e))?;
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
    pub async fn get_total_outstanding(&self) -> Result<f64> {
        let commerce = self.commerce.get()?;
        let total = commerce
            .accounts_payable()
            .get_total_outstanding()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get total", e))?;
        to_f64_checked(total, "accounts payable total outstanding")
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
