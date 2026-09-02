//! Tax calculation engine types
//!
//! Provides comprehensive tax support including:
//! - Multi-jurisdiction tax rates (US sales tax, EU VAT, etc.)
//! - Product tax categories (taxable, exempt, reduced rate)
//! - Customer tax exemptions (B2B, non-profits)
//! - Tax-inclusive vs tax-exclusive pricing
//! - Compound and tiered tax rules

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, ProductId};
use strum::{Display, EnumString};
use uuid::Uuid;

// ============================================================================
// Tax Types and Enums
// ============================================================================

/// Types of taxes supported
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaxType {
    /// US Sales Tax (state/local)
    #[default]
    SalesTax,
    /// Value Added Tax (EU, UK, etc.)
    Vat,
    /// Goods and Services Tax (Canada, Australia, India)
    Gst,
    /// Harmonized Sales Tax (Canadian provinces)
    Hst,
    /// Provincial Sales Tax (Canadian provinces)
    Pst,
    /// Quebec Sales Tax
    Qst,
    /// Consumption Tax (Japan)
    ConsumptionTax,
    /// Custom/Other tax type
    Custom,
}

impl TaxType {
    /// Return the canonical string representation
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::SalesTax => "sales_tax",
            Self::Vat => "vat",
            Self::Gst => "gst",
            Self::Hst => "hst",
            Self::Pst => "pst",
            Self::Qst => "qst",
            Self::ConsumptionTax => "consumption_tax",
            Self::Custom => "custom",
        }
    }
}

/// Tax calculation method
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaxCalculationMethod {
    /// Tax is calculated on top of the price (US style)
    #[default]
    Exclusive,
    /// Tax is included in the price (EU VAT style)
    Inclusive,
}

/// How to apply multiple tax rates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TaxCompoundMethod {
    /// Add all taxes together, apply to subtotal
    #[default]
    Combined,
    /// Apply taxes sequentially (tax on tax)
    Compound,
    /// Apply taxes separately to subtotal
    Separate,
}

impl std::fmt::Display for TaxCompoundMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Combined => f.write_str("combined"),
            Self::Compound => f.write_str("compound"),
            Self::Separate => f.write_str("separate"),
        }
    }
}

impl std::str::FromStr for TaxCompoundMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "combined" => Ok(Self::Combined),
            "compound" => Ok(Self::Compound),
            "separate" => Ok(Self::Separate),
            _ => Err(format!("Unknown tax compound method: {s}")),
        }
    }
}

/// Product tax category
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProductTaxCategory {
    /// Standard taxable goods
    #[default]
    Standard,
    /// Reduced rate (e.g., food, books in some jurisdictions)
    Reduced,
    /// Super-reduced rate (e.g., essential food)
    SuperReduced,
    /// Zero-rated (taxable at 0%, still reportable)
    ZeroRated,
    /// Exempt from tax entirely
    Exempt,
    /// Digital goods/services (special rules in many jurisdictions)
    Digital,
    /// Clothing (special rules in some US states)
    Clothing,
    /// Food for home consumption
    Food,
    /// Prepared food/restaurant
    PreparedFood,
    /// Medical/health items
    Medical,
    /// Educational materials
    Educational,
    /// Luxury goods (higher rate in some places)
    Luxury,
}

impl ProductTaxCategory {
    /// Return the canonical string representation
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Reduced => "reduced",
            Self::SuperReduced => "super_reduced",
            Self::ZeroRated => "zero_rated",
            Self::Exempt => "exempt",
            Self::Digital => "digital",
            Self::Clothing => "clothing",
            Self::Food => "food",
            Self::PreparedFood => "prepared_food",
            Self::Medical => "medical",
            Self::Educational => "educational",
            Self::Luxury => "luxury",
        }
    }
}

/// Customer exemption type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ExemptionType {
    /// Wholesale/resale (has resale certificate)
    Resale,
    /// Non-profit organization
    NonProfit,
    /// Government entity
    Government,
    /// Educational institution
    Educational,
    /// Religious organization
    Religious,
    /// Medical/healthcare
    Medical,
    /// Manufacturing (raw materials)
    Manufacturing,
    /// Agricultural
    Agricultural,
    /// Export (zero-rated for export)
    Export,
    /// Diplomatic (embassy, consulate)
    Diplomatic,
    /// Other documented exemption
    Other,
}

impl std::fmt::Display for ExemptionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resale => f.write_str("resale"),
            Self::NonProfit => f.write_str("non_profit"),
            Self::Government => f.write_str("government"),
            Self::Educational => f.write_str("educational"),
            Self::Religious => f.write_str("religious"),
            Self::Medical => f.write_str("medical"),
            Self::Manufacturing => f.write_str("manufacturing"),
            Self::Agricultural => f.write_str("agricultural"),
            Self::Export => f.write_str("export"),
            Self::Diplomatic => f.write_str("diplomatic"),
            Self::Other => f.write_str("other"),
        }
    }
}

impl std::str::FromStr for ExemptionType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "resale" => Ok(Self::Resale),
            "non_profit" | "nonprofit" | "non-profit" => Ok(Self::NonProfit),
            "government" => Ok(Self::Government),
            "educational" => Ok(Self::Educational),
            "religious" => Ok(Self::Religious),
            "medical" => Ok(Self::Medical),
            "manufacturing" => Ok(Self::Manufacturing),
            "agricultural" => Ok(Self::Agricultural),
            "export" => Ok(Self::Export),
            "diplomatic" => Ok(Self::Diplomatic),
            "other" => Ok(Self::Other),
            _ => Err(format!("Unknown exemption type: {s}")),
        }
    }
}

// ============================================================================
// Core Tax Entities
// ============================================================================

/// A tax jurisdiction (country, state, city, district)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxJurisdiction {
    pub id: Uuid,
    /// Parent jurisdiction (e.g., state is parent of city)
    pub parent_id: Option<Uuid>,
    /// Jurisdiction name
    pub name: String,
    /// Jurisdiction code (e.g., "US-CA", "US-CA-LA")
    pub code: String,
    /// Jurisdiction level
    pub level: JurisdictionLevel,
    /// Country code (ISO 3166-1 alpha-2)
    pub country_code: String,
    /// State/province code (ISO 3166-2)
    pub state_code: Option<String>,
    /// County/region name
    pub county: Option<String>,
    /// City name
    pub city: Option<String>,
    /// Postal codes covered (can be ranges or patterns)
    pub postal_codes: Vec<String>,
    /// Whether this jurisdiction is active
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Level of tax jurisdiction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum JurisdictionLevel {
    #[default]
    Country,
    State,
    County,
    City,
    District,
    Special,
}

impl std::fmt::Display for JurisdictionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Country => f.write_str("country"),
            Self::State => f.write_str("state"),
            Self::County => f.write_str("county"),
            Self::City => f.write_str("city"),
            Self::District => f.write_str("district"),
            Self::Special => f.write_str("special"),
        }
    }
}

impl std::str::FromStr for JurisdictionLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "country" => Ok(Self::Country),
            "state" => Ok(Self::State),
            "county" => Ok(Self::County),
            "city" => Ok(Self::City),
            "district" => Ok(Self::District),
            "special" => Ok(Self::Special),
            _ => Err(format!("Unknown jurisdiction level: {s}")),
        }
    }
}

/// A tax rate for a specific jurisdiction and category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxRate {
    pub id: Uuid,
    /// Jurisdiction this rate applies to
    pub jurisdiction_id: Uuid,
    /// Type of tax
    pub tax_type: TaxType,
    /// Product category this rate applies to
    pub product_category: ProductTaxCategory,
    /// Tax rate as decimal (e.g., 0.0825 for 8.25%)
    pub rate: Decimal,
    /// Rate name for display (e.g., "California State Tax")
    pub name: String,
    /// Description of the tax
    pub description: Option<String>,
    /// Whether rate is compound (applied after other taxes)
    pub is_compound: bool,
    /// Priority for ordering (lower = applied first)
    pub priority: i32,
    /// Minimum amount for tax to apply
    pub threshold_min: Option<Decimal>,
    /// Maximum amount taxed (cap)
    pub threshold_max: Option<Decimal>,
    /// Fixed amount instead of percentage
    pub fixed_amount: Option<Decimal>,
    /// Effective date
    pub effective_from: NaiveDate,
    /// Expiration date
    pub effective_to: Option<NaiveDate>,
    /// Whether this rate is active
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Customer tax exemption certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxExemption {
    pub id: Uuid,
    /// Customer this exemption belongs to
    pub customer_id: Uuid,
    /// Type of exemption
    pub exemption_type: ExemptionType,
    /// Exemption certificate number
    pub certificate_number: Option<String>,
    /// Issuing authority/state
    pub issuing_authority: Option<String>,
    /// Jurisdictions where exemption applies (empty = all)
    pub jurisdiction_ids: Vec<Uuid>,
    /// Product categories exempt (empty = all)
    pub exempt_categories: Vec<ProductTaxCategory>,
    /// Effective date
    pub effective_from: NaiveDate,
    /// Expiration date
    pub expires_at: Option<NaiveDate>,
    /// Whether exemption has been verified
    pub verified: bool,
    /// Verification date
    pub verified_at: Option<DateTime<Utc>>,
    /// Notes about the exemption
    pub notes: Option<String>,
    /// Whether this exemption is active
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// Tax Calculation Types
// ============================================================================

/// Input for tax calculation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaxCalculationRequest {
    /// Line items to calculate tax for
    pub line_items: Vec<TaxLineItem>,
    /// Shipping address (determines jurisdiction)
    pub shipping_address: TaxAddress,
    /// Optional billing address (for digital goods)
    pub billing_address: Option<TaxAddress>,
    /// Customer ID (for exemption lookup)
    pub customer_id: Option<Uuid>,
    /// Shipping amount (may be taxable)
    pub shipping_amount: Option<Decimal>,
    /// Currency code
    #[serde(default = "default_currency")]
    pub currency: CurrencyCode,
    /// Transaction date (for rate lookup)
    pub transaction_date: Option<NaiveDate>,
    /// Whether prices include tax
    pub prices_include_tax: bool,
}

const fn default_currency() -> CurrencyCode {
    CurrencyCode::USD
}

/// A line item for tax calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxLineItem {
    /// Line item identifier
    pub id: String,
    /// Product SKU
    pub sku: Option<String>,
    /// Product ID
    pub product_id: Option<ProductId>,
    /// Quantity
    pub quantity: Decimal,
    /// Unit price
    pub unit_price: Decimal,
    /// Total discount on this line
    pub discount_amount: Decimal,
    /// Product tax category
    pub tax_category: ProductTaxCategory,
    /// Override tax code (e.g., Avalara tax code)
    pub tax_code: Option<String>,
    /// Description for tax reporting
    pub description: Option<String>,
}

impl Default for TaxLineItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            sku: None,
            product_id: None,
            quantity: Decimal::ONE,
            unit_price: Decimal::ZERO,
            discount_amount: Decimal::ZERO,
            tax_category: ProductTaxCategory::Standard,
            tax_code: None,
            description: None,
        }
    }
}

/// Address for tax jurisdiction determination
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaxAddress {
    /// Street line 1
    pub line1: Option<String>,
    /// Street line 2
    pub line2: Option<String>,
    /// City
    pub city: Option<String>,
    /// State/Province/Region
    pub state: Option<String>,
    /// Postal/ZIP code
    pub postal_code: Option<String>,
    /// Country code (ISO 3166-1 alpha-2)
    pub country: String,
}

/// Result of tax calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxCalculationResult {
    /// Unique calculation ID
    pub id: Uuid,
    /// Total tax amount
    pub total_tax: Decimal,
    /// Subtotal before tax
    pub subtotal: Decimal,
    /// Total including tax
    pub total: Decimal,
    /// Tax on shipping
    pub shipping_tax: Decimal,
    /// Breakdown by jurisdiction
    pub tax_breakdown: Vec<TaxBreakdown>,
    /// Per-line-item tax details
    pub line_item_taxes: Vec<LineItemTax>,
    /// Whether any exemptions were applied
    pub exemptions_applied: bool,
    /// Exemption details if applied
    pub exemption_details: Option<ExemptionDetails>,
    /// Jurisdictions involved
    pub jurisdictions: Vec<JurisdictionSummary>,
    /// Calculation timestamp
    pub calculated_at: DateTime<Utc>,
    /// Whether this is an estimate or committed transaction
    pub is_estimate: bool,
}

/// Tax breakdown by jurisdiction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxBreakdown {
    /// Jurisdiction ID
    pub jurisdiction_id: Uuid,
    /// Jurisdiction name
    pub jurisdiction_name: String,
    /// Tax type
    pub tax_type: TaxType,
    /// Rate name
    pub rate_name: String,
    /// Tax rate applied
    pub rate: Decimal,
    /// Taxable amount
    pub taxable_amount: Decimal,
    /// Tax amount
    pub tax_amount: Decimal,
    /// Whether this is a compound tax
    pub is_compound: bool,
}

/// Tax for a specific line item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineItemTax {
    /// Line item ID
    pub line_item_id: String,
    /// Taxable amount for this item
    pub taxable_amount: Decimal,
    /// Total tax for this item
    pub tax_amount: Decimal,
    /// Effective tax rate
    pub effective_rate: Decimal,
    /// Whether item was exempt
    pub is_exempt: bool,
    /// Reason for exemption if exempt
    pub exemption_reason: Option<String>,
    /// Breakdown by tax type
    pub tax_details: Vec<TaxDetail>,
}

/// Detailed tax information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxDetail {
    pub tax_type: TaxType,
    pub jurisdiction_name: String,
    pub rate: Decimal,
    pub amount: Decimal,
}

/// Summary of exemptions applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExemptionDetails {
    pub exemption_id: Uuid,
    pub exemption_type: ExemptionType,
    pub certificate_number: Option<String>,
    pub amount_exempt: Decimal,
    pub tax_saved: Decimal,
}

/// Summary of a jurisdiction involved in calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JurisdictionSummary {
    pub id: Uuid,
    pub name: String,
    pub code: String,
    pub level: JurisdictionLevel,
    pub total_rate: Decimal,
    pub total_tax: Decimal,
}

// ============================================================================
// Tax Configuration
// ============================================================================

/// Store-level tax configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxSettings {
    pub id: Uuid,
    /// Whether tax calculation is enabled
    pub enabled: bool,
    /// Default calculation method
    pub calculation_method: TaxCalculationMethod,
    /// Default compound method
    pub compound_method: TaxCompoundMethod,
    /// Whether to tax shipping
    pub tax_shipping: bool,
    /// Whether to tax handling fees
    pub tax_handling: bool,
    /// Whether to tax gift wrapping
    pub tax_gift_wrap: bool,
    /// Origin address for origin-based tax states
    pub origin_address: Option<TaxAddress>,
    /// Default product tax category
    pub default_product_category: ProductTaxCategory,
    /// Rounding mode applied to computed tax amounts. One of `half_up`
    /// (default), `half_even`/`bankers`, `half_down`, `up`, `down`/`truncate`,
    /// `ceil`, or `floor`. See [`Self::rounding_strategy`].
    pub rounding_mode: String,
    /// Decimal places for tax amounts
    pub decimal_places: i32,
    /// Whether to validate addresses
    pub validate_addresses: bool,
    /// External tax service provider (avalara, taxjar, vertex, none)
    pub tax_provider: Option<String>,
    /// Provider API credentials (encrypted)
    pub provider_credentials: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for TaxSettings {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            enabled: true,
            calculation_method: TaxCalculationMethod::Exclusive,
            compound_method: TaxCompoundMethod::Combined,
            tax_shipping: true,
            tax_handling: true,
            tax_gift_wrap: true,
            origin_address: None,
            default_product_category: ProductTaxCategory::Standard,
            rounding_mode: "half_up".to_string(),
            decimal_places: 2,
            validate_addresses: false,
            tax_provider: None,
            provider_credentials: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl TaxSettings {
    /// Resolve the configured [`rounding_mode`](Self::rounding_mode) string to a
    /// concrete [`RoundingStrategy`] for rounding computed tax amounts.
    ///
    /// Recognized modes (case- and surrounding-whitespace-insensitive):
    /// `half_up` (round half away from zero — the default and conventional
    /// retail tax rounding), `half_even`/`bankers` (round half to even),
    /// `half_down`, `up`, `down`/`truncate`, `ceil`, and `floor`. Any
    /// unrecognized or empty value falls back to `half_up`.
    #[must_use]
    pub fn rounding_strategy(&self) -> RoundingStrategy {
        match self.rounding_mode.trim().to_ascii_lowercase().as_str() {
            "half_even" | "half_to_even" | "bankers" | "banker" => {
                RoundingStrategy::MidpointNearestEven
            }
            "half_down" => RoundingStrategy::MidpointTowardZero,
            "up" | "away_from_zero" => RoundingStrategy::AwayFromZero,
            "down" | "truncate" | "toward_zero" => RoundingStrategy::ToZero,
            "ceil" | "ceiling" => RoundingStrategy::ToPositiveInfinity,
            "floor" => RoundingStrategy::ToNegativeInfinity,
            // "half_up" and any unrecognized value: round half away from zero.
            _ => RoundingStrategy::MidpointAwayFromZero,
        }
    }
}

// ============================================================================
// Shared calculation helpers
// ============================================================================

impl TaxExemption {
    /// Whether this exemption may be honoured on `transaction_date`: it must
    /// be active, **verified**, and inside its validity window.
    #[must_use]
    pub fn is_effective_on(&self, transaction_date: NaiveDate) -> bool {
        self.active
            && self.verified
            && self.effective_from <= transaction_date
            && self.expires_at.is_none_or(|expires| expires >= transaction_date)
    }

    /// Whether this exemption covers `category` (an empty category list
    /// covers every category).
    #[must_use]
    pub fn covers_category(&self, category: ProductTaxCategory) -> bool {
        self.exempt_categories.is_empty() || self.exempt_categories.contains(&category)
    }

    /// Whether this exemption covers `jurisdiction_id` (an empty jurisdiction
    /// list covers every jurisdiction).
    #[must_use]
    pub fn covers_jurisdiction(&self, jurisdiction_id: Uuid) -> bool {
        self.jurisdiction_ids.is_empty() || self.jurisdiction_ids.contains(&jurisdiction_id)
    }
}

/// Round `parts` to `decimal_places` so that they sum EXACTLY to
/// `total.round(decimal_places)`, allocating the rounding residue by largest
/// remainder. Returns `(rounded_total, rounded_parts)`.
///
/// Both tax engines use this to round per line (and per rate within a line)
/// while keeping `sum(lines) == total_tax` — three `$1.11` lines at 8.25%
/// each round to `$0.09`, and the total is `$0.27`, never `$0.28`.
#[must_use]
pub fn allocate_rounded(
    total: Decimal,
    parts: &[Decimal],
    decimal_places: u32,
    strategy: RoundingStrategy,
) -> (Decimal, Vec<Decimal>) {
    let rounded_total = total.round_dp_with_strategy(decimal_places, strategy);
    if parts.is_empty() {
        return (rounded_total, Vec::new());
    }
    let mut rounded: Vec<Decimal> =
        parts.iter().map(|p| p.round_dp_with_strategy(decimal_places, strategy)).collect();
    let unit = Decimal::new(1, decimal_places);
    let mut residue = rounded_total - rounded.iter().sum::<Decimal>();
    if residue.is_zero() {
        return (rounded_total, rounded);
    }
    // Order parts by how much rounding took from (or gave to) them, and
    // nudge the most deserving ones one minor unit at a time.
    let mut order: Vec<usize> = (0..parts.len()).collect();
    let remainders: Vec<Decimal> = parts.iter().zip(&rounded).map(|(p, r)| p - r).collect();
    if residue > Decimal::ZERO {
        order.sort_by(|a, b| remainders[*b].cmp(&remainders[*a]));
    } else {
        order.sort_by(|a, b| remainders[*a].cmp(&remainders[*b]));
    }
    let step = if residue > Decimal::ZERO { unit } else { -unit };
    let mut cursor = 0usize;
    while !residue.is_zero() && cursor < order.len() * 1000 {
        let idx = order[cursor % order.len()];
        rounded[idx] += step;
        residue -= step;
        cursor += 1;
    }
    (rounded_total, rounded)
}

/// Accumulates a tax calculation line by line, rounding **per line** (and
/// allocating each line's rounded tax across its rates by largest remainder)
/// so that `sum(line_item_taxes) == total_tax` and
/// `sum(tax_breakdown) == total_tax` hold exactly. Both storage backends
/// feed this with the rates they resolved, so the arithmetic agrees by
/// construction.
#[derive(Debug)]
pub struct TaxAccumulator {
    /// Sum of (non-negative) line amounts.
    pub subtotal: Decimal,
    /// Sum of rounded line taxes plus rounded shipping tax.
    pub total_tax: Decimal,
    /// Rounded shipping tax.
    pub shipping_tax: Decimal,
    /// One entry per line item, in request order.
    pub line_item_taxes: Vec<LineItemTax>,
    /// Per (jurisdiction, tax type) totals, in first-seen order.
    pub tax_breakdown: Vec<TaxBreakdown>,
    /// Whether any exemption removed tax from any line.
    pub exemptions_applied: bool,
    jurisdictions: std::collections::HashMap<Uuid, JurisdictionSummary>,
    decimal_places: u32,
    strategy: RoundingStrategy,
}

/// One rate's share of a base amount, before rounding.
struct RateShare {
    index: usize,
    taxable_amount: Decimal,
    raw_tax: Decimal,
}

impl TaxAccumulator {
    /// Start a calculation rounding to `decimal_places` with `strategy`.
    #[must_use]
    pub fn new(decimal_places: u32, strategy: RoundingStrategy) -> Self {
        Self {
            subtotal: Decimal::ZERO,
            total_tax: Decimal::ZERO,
            shipping_tax: Decimal::ZERO,
            line_item_taxes: Vec::new(),
            tax_breakdown: Vec::new(),
            exemptions_applied: false,
            jurisdictions: std::collections::HashMap::new(),
            decimal_places,
            strategy,
        }
    }

    /// A line that owes no tax.
    pub fn push_exempt_line(&mut self, line_item_id: &str, line_amount: Decimal, reason: &str) {
        self.subtotal += line_amount;
        self.line_item_taxes.push(LineItemTax {
            line_item_id: line_item_id.to_string(),
            taxable_amount: line_amount,
            tax_amount: Decimal::ZERO,
            effective_rate: Decimal::ZERO,
            is_exempt: true,
            exemption_reason: Some(reason.to_string()),
            tax_details: Vec::new(),
        });
    }

    /// Tax `line_amount` with `rates` (already sorted by priority, each with
    /// its jurisdiction). Returns the line's rounded tax.
    pub fn add_taxed_line(
        &mut self,
        line_item_id: &str,
        line_amount: Decimal,
        rates: &[(&TaxRate, &TaxJurisdiction)],
    ) -> Decimal {
        self.subtotal += line_amount;
        let (line_tax, allocated) = self.apply_rates(line_amount, rates);
        let tax_details = allocated
            .iter()
            .map(|(share, amount)| {
                let (rate, jurisdiction) = rates[share.index];
                TaxDetail {
                    tax_type: rate.tax_type,
                    jurisdiction_name: jurisdiction.name.clone(),
                    rate: rate.rate,
                    amount: *amount,
                }
            })
            .collect();
        let effective_rate =
            if line_amount.is_zero() { Decimal::ZERO } else { line_tax / line_amount };
        self.total_tax += line_tax;
        self.line_item_taxes.push(LineItemTax {
            line_item_id: line_item_id.to_string(),
            taxable_amount: line_amount,
            tax_amount: line_tax,
            effective_rate,
            is_exempt: false,
            exemption_reason: None,
            tax_details,
        });
        line_tax
    }

    /// Tax `shipping_amount` with `rates`. Returns the rounded shipping tax.
    pub fn add_shipping(
        &mut self,
        shipping_amount: Decimal,
        rates: &[(&TaxRate, &TaxJurisdiction)],
    ) -> Decimal {
        let (shipping_tax, _) = self.apply_rates(shipping_amount, rates);
        self.shipping_tax += shipping_tax;
        self.total_tax += shipping_tax;
        shipping_tax
    }

    /// Apply `rates` to `base`, round the sum once, allocate it across the
    /// rates, and fold the allocation into the breakdown and jurisdiction
    /// summaries. Returns the rounded total and the per-rate allocation.
    fn apply_rates(
        &mut self,
        base: Decimal,
        rates: &[(&TaxRate, &TaxJurisdiction)],
    ) -> (Decimal, Vec<(RateShare, Decimal)>) {
        let shares = rate_shares(base, rates.iter().map(|(rate, _)| *rate));
        let raw: Vec<Decimal> = shares.iter().map(|s| s.raw_tax).collect();
        let raw_total: Decimal = raw.iter().sum();
        let (rounded_total, rounded) =
            allocate_rounded(raw_total, &raw, self.decimal_places, self.strategy);

        let allocated: Vec<(RateShare, Decimal)> = shares.into_iter().zip(rounded).collect();
        for (share, amount) in &allocated {
            let (rate, jurisdiction) = rates[share.index];
            let summary =
                self.jurisdictions.entry(jurisdiction.id).or_insert_with(|| JurisdictionSummary {
                    id: jurisdiction.id,
                    name: jurisdiction.name.clone(),
                    code: jurisdiction.code.clone(),
                    level: jurisdiction.level,
                    total_rate: Decimal::ZERO,
                    total_tax: Decimal::ZERO,
                });
            summary.total_rate += rate.rate;
            summary.total_tax += *amount;

            if let Some(existing) = self
                .tax_breakdown
                .iter_mut()
                .find(|b| b.jurisdiction_id == jurisdiction.id && b.tax_type == rate.tax_type)
            {
                existing.taxable_amount += share.taxable_amount;
                existing.tax_amount += *amount;
            } else {
                self.tax_breakdown.push(TaxBreakdown {
                    jurisdiction_id: jurisdiction.id,
                    jurisdiction_name: jurisdiction.name.clone(),
                    tax_type: rate.tax_type,
                    rate_name: rate.name.clone(),
                    rate: rate.rate,
                    taxable_amount: share.taxable_amount,
                    tax_amount: *amount,
                    is_compound: rate.is_compound,
                });
            }
        }
        (rounded_total, allocated)
    }

    /// Produce the result. `total = subtotal + total_tax + shipping`.
    #[must_use]
    pub fn finish(
        self,
        shipping_amount: Option<Decimal>,
        now: DateTime<Utc>,
    ) -> TaxCalculationResult {
        // Emit jurisdictions in a stable order (by code, then id) rather than
        // the HashMap's iteration order, which varies run-to-run and between
        // the SQLite and Postgres backends — the tax result must be
        // deterministic.
        let mut jurisdictions: Vec<JurisdictionSummary> =
            self.jurisdictions.into_values().collect();
        jurisdictions.sort_by(|a, b| a.code.cmp(&b.code).then_with(|| a.id.cmp(&b.id)));

        let total = self.subtotal + self.total_tax + shipping_amount.unwrap_or_default();
        TaxCalculationResult {
            id: Uuid::new_v4(),
            total_tax: self.total_tax,
            subtotal: self.subtotal,
            total,
            shipping_tax: self.shipping_tax,
            tax_breakdown: self.tax_breakdown,
            line_item_taxes: self.line_item_taxes,
            exemptions_applied: self.exemptions_applied,
            exemption_details: None,
            jurisdictions,
            calculated_at: now,
            is_estimate: true,
        }
    }
}

/// Each rate's unrounded share of `base`, honouring thresholds, fixed
/// amounts and compounding (a compound rate taxes the base plus the tax
/// accumulated so far).
fn rate_shares<'a>(base: Decimal, rates: impl Iterator<Item = &'a TaxRate>) -> Vec<RateShare> {
    let mut shares = Vec::new();
    let mut tax_so_far = Decimal::ZERO;
    for (index, rate) in rates.enumerate() {
        if base <= Decimal::ZERO {
            continue;
        }
        if rate.threshold_min.is_some_and(|min| base < min) {
            continue;
        }
        let capped_base = match rate.threshold_max {
            Some(max) if base > max => max,
            _ => base,
        };
        if capped_base <= Decimal::ZERO {
            continue;
        }
        let taxable_amount = if rate.fixed_amount.is_none() && rate.is_compound {
            capped_base + tax_so_far
        } else {
            capped_base
        };
        let raw_tax = rate.fixed_amount.map_or_else(|| taxable_amount * rate.rate, |fixed| fixed);
        tax_so_far += raw_tax;
        shares.push(RateShare { index, taxable_amount, raw_tax });
    }
    shares
}

/// Split the rates applicable to a line into those still owed and those
/// removed by a customer exemption covering `category`.
///
/// An effective exemption with no jurisdiction restriction removes every
/// rate; one restricted to jurisdictions removes only the rates of those
/// jurisdictions. Returns `(remaining rate indexes, any_exempted)`.
#[must_use]
pub fn rates_after_exemptions(
    rates: &[(&TaxRate, &TaxJurisdiction)],
    exemptions: &[TaxExemption],
    category: ProductTaxCategory,
) -> (Vec<usize>, bool) {
    let covering: Vec<&TaxExemption> =
        exemptions.iter().filter(|e| e.covers_category(category)).collect();
    if covering.is_empty() {
        return ((0..rates.len()).collect(), false);
    }
    let mut remaining = Vec::with_capacity(rates.len());
    let mut any_exempted = false;
    for (index, (rate, _)) in rates.iter().enumerate() {
        if covering.iter().any(|e| e.covers_jurisdiction(rate.jurisdiction_id)) {
            any_exempted = true;
        } else {
            remaining.push(index);
        }
    }
    (remaining, any_exempted)
}

// ============================================================================
// Create/Update DTOs
// ============================================================================

/// Create a new tax jurisdiction
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateTaxJurisdiction {
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub code: String,
    pub level: JurisdictionLevel,
    pub country_code: String,
    pub state_code: Option<String>,
    pub county: Option<String>,
    pub city: Option<String>,
    pub postal_codes: Vec<String>,
}

/// Create a new tax rate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaxRate {
    pub jurisdiction_id: Uuid,
    pub tax_type: TaxType,
    pub product_category: ProductTaxCategory,
    pub rate: Decimal,
    pub name: String,
    pub description: Option<String>,
    pub is_compound: bool,
    pub priority: i32,
    pub threshold_min: Option<Decimal>,
    pub threshold_max: Option<Decimal>,
    pub fixed_amount: Option<Decimal>,
    pub effective_from: NaiveDate,
    pub effective_to: Option<NaiveDate>,
}

impl Default for CreateTaxRate {
    fn default() -> Self {
        Self {
            jurisdiction_id: Uuid::nil(),
            tax_type: TaxType::SalesTax,
            product_category: ProductTaxCategory::Standard,
            rate: Decimal::ZERO,
            name: String::new(),
            description: None,
            is_compound: false,
            priority: 0,
            threshold_min: None,
            threshold_max: None,
            fixed_amount: None,
            effective_from: Utc::now().date_naive(),
            effective_to: None,
        }
    }
}

/// Create a tax exemption for a customer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaxExemption {
    pub customer_id: Uuid,
    pub exemption_type: ExemptionType,
    pub certificate_number: Option<String>,
    pub issuing_authority: Option<String>,
    pub jurisdiction_ids: Vec<Uuid>,
    pub exempt_categories: Vec<ProductTaxCategory>,
    pub effective_from: NaiveDate,
    pub expires_at: Option<NaiveDate>,
    pub notes: Option<String>,
}

/// Filter for querying tax rates
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaxRateFilter {
    pub jurisdiction_id: Option<Uuid>,
    pub tax_type: Option<TaxType>,
    pub product_category: Option<ProductTaxCategory>,
    pub active_only: bool,
    pub effective_date: Option<NaiveDate>,
    /// Maximum number of rows to return (server default/cap applies when unset)
    pub limit: Option<u32>,
    /// Number of rows to skip
    pub offset: Option<u32>,
}

/// Filter for querying jurisdictions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaxJurisdictionFilter {
    pub country_code: Option<String>,
    pub state_code: Option<String>,
    pub level: Option<JurisdictionLevel>,
    pub active_only: bool,
    /// Maximum number of rows to return (server default/cap applies when unset)
    pub limit: Option<u32>,
    /// Number of rows to skip
    pub offset: Option<u32>,
}

// ============================================================================
// US-Specific Tax Helpers
// ============================================================================

/// US State tax information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsStateTaxInfo {
    pub state_code: String,
    pub state_name: String,
    pub state_rate: Decimal,
    pub has_local_taxes: bool,
    pub origin_based: bool,
    pub tax_shipping: bool,
    pub tax_clothing: bool,
    pub tax_food: bool,
    pub tax_digital: bool,
}

/// Pre-configured US state tax data
#[must_use]
pub fn get_us_state_tax_info(state_code: &str) -> Option<UsStateTaxInfo> {
    match state_code.to_uppercase().as_str() {
        "AL" => Some(UsStateTaxInfo {
            state_code: "AL".into(),
            state_name: "Alabama".into(),
            state_rate: Decimal::new(4, 2), // 4%
            has_local_taxes: true,
            origin_based: false,
            tax_shipping: true,
            tax_clothing: true,
            tax_food: true,
            tax_digital: true,
        }),
        "AK" => Some(UsStateTaxInfo {
            state_code: "AK".into(),
            state_name: "Alaska".into(),
            state_rate: Decimal::ZERO, // No state tax
            has_local_taxes: true,
            origin_based: false,
            tax_shipping: false,
            tax_clothing: false,
            tax_food: false,
            tax_digital: false,
        }),
        "AZ" => Some(UsStateTaxInfo {
            state_code: "AZ".into(),
            state_name: "Arizona".into(),
            state_rate: Decimal::new(56, 3), // 5.6%
            has_local_taxes: true,
            origin_based: true,
            tax_shipping: true,
            tax_clothing: true,
            tax_food: false,
            tax_digital: true,
        }),
        "CA" => Some(UsStateTaxInfo {
            state_code: "CA".into(),
            state_name: "California".into(),
            state_rate: Decimal::new(725, 4), // 7.25%
            has_local_taxes: true,
            origin_based: true,
            tax_shipping: false,
            tax_clothing: true,
            tax_food: false,
            tax_digital: false,
        }),
        "CO" => Some(UsStateTaxInfo {
            state_code: "CO".into(),
            state_name: "Colorado".into(),
            state_rate: Decimal::new(29, 3), // 2.9%
            has_local_taxes: true,
            origin_based: false,
            tax_shipping: true,
            tax_clothing: true,
            tax_food: false,
            tax_digital: true,
        }),
        "DE" => Some(UsStateTaxInfo {
            state_code: "DE".into(),
            state_name: "Delaware".into(),
            state_rate: Decimal::ZERO, // No sales tax
            has_local_taxes: false,
            origin_based: false,
            tax_shipping: false,
            tax_clothing: false,
            tax_food: false,
            tax_digital: false,
        }),
        "FL" => Some(UsStateTaxInfo {
            state_code: "FL".into(),
            state_name: "Florida".into(),
            state_rate: Decimal::new(6, 2), // 6%
            has_local_taxes: true,
            origin_based: false,
            tax_shipping: true,
            tax_clothing: true,
            tax_food: false,
            tax_digital: true,
        }),
        "MT" => Some(UsStateTaxInfo {
            state_code: "MT".into(),
            state_name: "Montana".into(),
            state_rate: Decimal::ZERO, // No sales tax
            has_local_taxes: false,
            origin_based: false,
            tax_shipping: false,
            tax_clothing: false,
            tax_food: false,
            tax_digital: false,
        }),
        "NH" => Some(UsStateTaxInfo {
            state_code: "NH".into(),
            state_name: "New Hampshire".into(),
            state_rate: Decimal::ZERO, // No sales tax
            has_local_taxes: false,
            origin_based: false,
            tax_shipping: false,
            tax_clothing: false,
            tax_food: false,
            tax_digital: false,
        }),
        "NY" => Some(UsStateTaxInfo {
            state_code: "NY".into(),
            state_name: "New York".into(),
            state_rate: Decimal::new(4, 2), // 4%
            has_local_taxes: true,
            origin_based: false,
            tax_shipping: true,
            tax_clothing: false, // Clothing under $110 exempt
            tax_food: false,
            tax_digital: true,
        }),
        "OR" => Some(UsStateTaxInfo {
            state_code: "OR".into(),
            state_name: "Oregon".into(),
            state_rate: Decimal::ZERO, // No sales tax
            has_local_taxes: false,
            origin_based: false,
            tax_shipping: false,
            tax_clothing: false,
            tax_food: false,
            tax_digital: false,
        }),
        "TX" => Some(UsStateTaxInfo {
            state_code: "TX".into(),
            state_name: "Texas".into(),
            state_rate: Decimal::new(625, 4), // 6.25%
            has_local_taxes: true,
            origin_based: true,
            tax_shipping: true,
            tax_clothing: true,
            tax_food: false,
            tax_digital: true,
        }),
        "WA" => Some(UsStateTaxInfo {
            state_code: "WA".into(),
            state_name: "Washington".into(),
            state_rate: Decimal::new(65, 3), // 6.5%
            has_local_taxes: true,
            origin_based: false,
            tax_shipping: true,
            tax_clothing: true,
            tax_food: false,
            tax_digital: true,
        }),
        _ => None,
    }
}

// ============================================================================
// EU VAT Helpers
// ============================================================================

/// EU VAT rates by country
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EuVatInfo {
    pub country_code: String,
    pub country_name: String,
    pub standard_rate: Decimal,
    pub reduced_rate: Option<Decimal>,
    pub super_reduced_rate: Option<Decimal>,
    pub parking_rate: Option<Decimal>,
}

/// Get EU VAT information for a country
#[must_use]
pub fn get_eu_vat_info(country_code: &str) -> Option<EuVatInfo> {
    match country_code.to_uppercase().as_str() {
        "AT" => Some(EuVatInfo {
            country_code: "AT".into(),
            country_name: "Austria".into(),
            standard_rate: Decimal::new(20, 2),
            reduced_rate: Some(Decimal::new(10, 2)),
            super_reduced_rate: None,
            parking_rate: Some(Decimal::new(13, 2)),
        }),
        "BE" => Some(EuVatInfo {
            country_code: "BE".into(),
            country_name: "Belgium".into(),
            standard_rate: Decimal::new(21, 2),
            reduced_rate: Some(Decimal::new(12, 2)),
            super_reduced_rate: Some(Decimal::new(6, 2)),
            parking_rate: Some(Decimal::new(12, 2)),
        }),
        "DE" => Some(EuVatInfo {
            country_code: "DE".into(),
            country_name: "Germany".into(),
            standard_rate: Decimal::new(19, 2),
            reduced_rate: Some(Decimal::new(7, 2)),
            super_reduced_rate: None,
            parking_rate: None,
        }),
        "ES" => Some(EuVatInfo {
            country_code: "ES".into(),
            country_name: "Spain".into(),
            standard_rate: Decimal::new(21, 2),
            reduced_rate: Some(Decimal::new(10, 2)),
            super_reduced_rate: Some(Decimal::new(4, 2)),
            parking_rate: None,
        }),
        "FR" => Some(EuVatInfo {
            country_code: "FR".into(),
            country_name: "France".into(),
            standard_rate: Decimal::new(20, 2),
            reduced_rate: Some(Decimal::new(10, 2)),
            super_reduced_rate: Some(Decimal::new(55, 3)), // 5.5%
            parking_rate: None,
        }),
        "GB" => Some(EuVatInfo {
            country_code: "GB".into(),
            country_name: "United Kingdom".into(),
            standard_rate: Decimal::new(20, 2),
            reduced_rate: Some(Decimal::new(5, 2)),
            super_reduced_rate: None,
            parking_rate: None,
        }),
        "IE" => Some(EuVatInfo {
            country_code: "IE".into(),
            country_name: "Ireland".into(),
            standard_rate: Decimal::new(23, 2),
            reduced_rate: Some(Decimal::new(135, 3)), // 13.5%
            super_reduced_rate: Some(Decimal::new(48, 3)), // 4.8%
            parking_rate: Some(Decimal::new(135, 3)),
        }),
        "IT" => Some(EuVatInfo {
            country_code: "IT".into(),
            country_name: "Italy".into(),
            standard_rate: Decimal::new(22, 2),
            reduced_rate: Some(Decimal::new(10, 2)),
            super_reduced_rate: Some(Decimal::new(4, 2)),
            parking_rate: None,
        }),
        "NL" => Some(EuVatInfo {
            country_code: "NL".into(),
            country_name: "Netherlands".into(),
            standard_rate: Decimal::new(21, 2),
            reduced_rate: Some(Decimal::new(9, 2)),
            super_reduced_rate: None,
            parking_rate: None,
        }),
        "SE" => Some(EuVatInfo {
            country_code: "SE".into(),
            country_name: "Sweden".into(),
            standard_rate: Decimal::new(25, 2),
            reduced_rate: Some(Decimal::new(12, 2)),
            super_reduced_rate: Some(Decimal::new(6, 2)),
            parking_rate: None,
        }),
        _ => None,
    }
}

/// List of EU member state country codes
pub const EU_MEMBER_STATES: &[&str] = &[
    "AT", "BE", "BG", "HR", "CY", "CZ", "DK", "EE", "FI", "FR", "DE", "GR", "HU", "IE", "IT", "LV",
    "LT", "LU", "MT", "NL", "PL", "PT", "RO", "SK", "SI", "ES", "SE",
];

/// Check if a country is in the EU
#[must_use]
pub fn is_eu_member(country_code: &str) -> bool {
    EU_MEMBER_STATES.contains(&country_code.to_uppercase().as_str())
}

// ============================================================================
// Canadian Tax Helpers
// ============================================================================

/// Canadian province/territory tax information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanadianTaxInfo {
    pub province_code: String,
    pub province_name: String,
    pub gst_rate: Decimal,
    pub pst_rate: Option<Decimal>,
    pub hst_rate: Option<Decimal>,
    pub qst_rate: Option<Decimal>,
    pub total_rate: Decimal,
}

/// Get Canadian tax information for a province
#[must_use]
pub fn get_canadian_tax_info(province_code: &str) -> Option<CanadianTaxInfo> {
    let gst = Decimal::new(5, 2); // Federal GST is 5%

    match province_code.to_uppercase().as_str() {
        "AB" => Some(CanadianTaxInfo {
            province_code: "AB".into(),
            province_name: "Alberta".into(),
            gst_rate: gst,
            pst_rate: None,
            hst_rate: None,
            qst_rate: None,
            total_rate: gst,
        }),
        "BC" => Some(CanadianTaxInfo {
            province_code: "BC".into(),
            province_name: "British Columbia".into(),
            gst_rate: gst,
            pst_rate: Some(Decimal::new(7, 2)),
            hst_rate: None,
            qst_rate: None,
            total_rate: Decimal::new(12, 2),
        }),
        "ON" => Some(CanadianTaxInfo {
            province_code: "ON".into(),
            province_name: "Ontario".into(),
            gst_rate: Decimal::ZERO, // Replaced by HST
            pst_rate: None,
            hst_rate: Some(Decimal::new(13, 2)),
            qst_rate: None,
            total_rate: Decimal::new(13, 2),
        }),
        "QC" => Some(CanadianTaxInfo {
            province_code: "QC".into(),
            province_name: "Quebec".into(),
            gst_rate: gst,
            pst_rate: None,
            hst_rate: None,
            qst_rate: Some(Decimal::new(9975, 4)), // 9.975%
            total_rate: Decimal::new(14975, 4),
        }),
        "SK" => Some(CanadianTaxInfo {
            province_code: "SK".into(),
            province_name: "Saskatchewan".into(),
            gst_rate: gst,
            pst_rate: Some(Decimal::new(6, 2)),
            hst_rate: None,
            qst_rate: None,
            total_rate: Decimal::new(11, 2),
        }),
        "MB" => Some(CanadianTaxInfo {
            province_code: "MB".into(),
            province_name: "Manitoba".into(),
            gst_rate: gst,
            pst_rate: Some(Decimal::new(7, 2)),
            hst_rate: None,
            qst_rate: None,
            total_rate: Decimal::new(12, 2),
        }),
        "NS" => Some(CanadianTaxInfo {
            province_code: "NS".into(),
            province_name: "Nova Scotia".into(),
            gst_rate: Decimal::ZERO,
            pst_rate: None,
            hst_rate: Some(Decimal::new(15, 2)),
            qst_rate: None,
            total_rate: Decimal::new(15, 2),
        }),
        "NB" => Some(CanadianTaxInfo {
            province_code: "NB".into(),
            province_name: "New Brunswick".into(),
            gst_rate: Decimal::ZERO,
            pst_rate: None,
            hst_rate: Some(Decimal::new(15, 2)),
            qst_rate: None,
            total_rate: Decimal::new(15, 2),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn tax_type_from_str() {
        assert_eq!(TaxType::from_str("sales_tax").unwrap(), TaxType::SalesTax);
        assert!(TaxType::from_str("unknown").is_err());
    }

    #[test]
    fn tax_calculation_method_from_str() {
        assert_eq!(
            TaxCalculationMethod::from_str("inclusive").unwrap(),
            TaxCalculationMethod::Inclusive
        );
        assert!(TaxCalculationMethod::from_str("other").is_err());
    }

    #[test]
    fn tax_compound_method_from_str() {
        assert_eq!(TaxCompoundMethod::from_str("combined").unwrap(), TaxCompoundMethod::Combined);
        assert!(TaxCompoundMethod::from_str("other").is_err());
    }

    #[test]
    fn product_tax_category_from_str() {
        assert_eq!(
            ProductTaxCategory::from_str("super_reduced").unwrap(),
            ProductTaxCategory::SuperReduced
        );
        assert!(ProductTaxCategory::from_str("other").is_err());
    }

    #[test]
    fn exemption_type_from_str() {
        assert_eq!(ExemptionType::from_str("non_profit").unwrap(), ExemptionType::NonProfit);
        assert!(ExemptionType::from_str("unknown").is_err());
    }

    #[test]
    fn jurisdiction_level_from_str() {
        assert_eq!(JurisdictionLevel::from_str("state").unwrap(), JurisdictionLevel::State);
        assert!(JurisdictionLevel::from_str("unknown").is_err());
    }

    #[test]
    fn tax_settings_rounding_strategy_maps_modes() {
        use rust_decimal::RoundingStrategy;
        let strat = |mode: &str| {
            let s = TaxSettings { rounding_mode: mode.to_string(), ..Default::default() };
            s.rounding_strategy()
        };
        assert_eq!(strat("half_up"), RoundingStrategy::MidpointAwayFromZero);
        assert_eq!(strat("half_even"), RoundingStrategy::MidpointNearestEven);
        assert_eq!(strat("bankers"), RoundingStrategy::MidpointNearestEven);
        assert_eq!(strat("half_down"), RoundingStrategy::MidpointTowardZero);
        assert_eq!(strat("up"), RoundingStrategy::AwayFromZero);
        assert_eq!(strat("down"), RoundingStrategy::ToZero);
        assert_eq!(strat("truncate"), RoundingStrategy::ToZero);
        assert_eq!(strat("ceil"), RoundingStrategy::ToPositiveInfinity);
        assert_eq!(strat("floor"), RoundingStrategy::ToNegativeInfinity);
        // Case- and whitespace-insensitive.
        assert_eq!(strat("  Half_Even "), RoundingStrategy::MidpointNearestEven);
        // Unknown and the documented default both resolve to half_up.
        assert_eq!(strat("wat"), RoundingStrategy::MidpointAwayFromZero);
        assert_eq!(
            TaxSettings::default().rounding_strategy(),
            RoundingStrategy::MidpointAwayFromZero
        );
    }
}
