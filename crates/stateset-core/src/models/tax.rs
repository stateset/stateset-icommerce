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

impl TaxJurisdiction {
    /// Whether this jurisdiction levies tax on `address`.
    ///
    /// Every attribute the jurisdiction **defines** must agree with the
    /// address; attributes it leaves unset are unconstrained. Country and
    /// state codes compare case-insensitively, city names compare
    /// case-insensitively after trimming, and postal codes match by
    /// [`postal_code_matches`].
    ///
    /// Below the state level the jurisdiction must be pinned by something
    /// narrower than the state — a `city` that matched, or a postal-code
    /// list that matched — or it does not apply. This fails **closed**: a
    /// `County`-level jurisdiction identified only by `county` cannot be
    /// resolved, because [`TaxAddress`] carries no county, and charging
    /// every address in the state its county rate would be worse than
    /// charging none. Give such a jurisdiction its `postal_codes` (or a
    /// `city`) and it resolves.
    #[must_use]
    pub fn covers_address(&self, address: &TaxAddress) -> bool {
        fn same(a: &str, b: &str) -> bool {
            a.trim().eq_ignore_ascii_case(b.trim())
        }

        if !self.active || !same(&self.country_code, &address.country) {
            return false;
        }
        if let Some(state) = &self.state_code {
            if !address.state.as_deref().is_some_and(|a| same(a, state)) {
                return false;
            }
        }
        let city_matched = match &self.city {
            Some(city) => {
                if !address.city.as_deref().is_some_and(|a| same(a, city)) {
                    return false;
                }
                true
            }
            None => false,
        };
        let postal_matched = if self.postal_codes.is_empty() {
            false
        } else {
            if !address
                .postal_code
                .as_deref()
                .is_some_and(|code| postal_code_matches(&self.postal_codes, code))
            {
                return false;
            }
            true
        };
        if self.level > JurisdictionLevel::State && !(city_matched || postal_matched) {
            return false;
        }
        true
    }
}

/// Whether `code` falls in any of `patterns`.
///
/// A pattern is one of:
/// * an exact postal code (`"90210"`), compared case-insensitively after
///   trimming and stripping spaces and hyphens;
/// * a prefix wildcard (`"902*"`), matching every code that starts with it;
/// * an inclusive range (`"90210-90220"`), matched numerically when both
///   ends and the code are all digits, and lexicographically otherwise.
#[must_use]
pub fn postal_code_matches(patterns: &[String], code: &str) -> bool {
    fn canon(value: &str) -> String {
        value
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .flat_map(char::to_uppercase)
            .collect()
    }

    let code_canon = canon(code);
    if code_canon.is_empty() {
        return false;
    }
    patterns.iter().any(|pattern| {
        let pattern = pattern.trim();
        if let Some(prefix) = pattern.strip_suffix('*') {
            return code_canon.starts_with(&canon(prefix));
        }
        // A range needs a hyphen with non-empty sides; anything else (such as
        // a Canadian "K1A-0B1" written with a separator) is an exact code.
        if let Some((low, high)) = pattern.split_once('-') {
            let (low, high) = (low.trim(), high.trim());
            if !low.is_empty() && !high.is_empty() {
                let (low, high) = (canon(low), canon(high));
                let numeric = [&low, &high, &code_canon]
                    .iter()
                    .all(|v| v.chars().all(|c| c.is_ascii_digit()));
                if numeric {
                    let parse = |v: &str| v.parse::<u64>().ok();
                    if let (Some(low), Some(high), Some(value)) =
                        (parse(&low), parse(&high), parse(&code_canon))
                    {
                        return low <= value && value <= high;
                    }
                }
                return low.as_str() <= code_canon.as_str() && code_canon.as_str() <= high.as_str();
            }
        }
        canon(pattern) == code_canon
    })
}

/// Level of tax jurisdiction.
///
/// Ordered broadest-first (country before state before city); tax breakdowns
/// are presented and summed in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
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
    /// Whether the line `unit_price`s (and `shipping_amount`) are
    /// tax-inclusive (gross). When true — or when the store's
    /// [`TaxSettings::calculation_method`] is `Inclusive` — the engine backs
    /// tax out of each amount so the customer pays exactly the listed price:
    /// `net = gross − tax`, `tax = gross × k / (1 + k)` for combined rate `k`.
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
    /// The first entry of [`Self::applied_exemptions`], kept for callers that
    /// only ever expected a single certificate. Prefer `applied_exemptions`:
    /// a cart can hit more than one exemption.
    pub exemption_details: Option<ExemptionDetails>,
    /// Every exemption that actually removed tax, each with the amount it
    /// covered and the tax IT saved (not the cart's total saving), in
    /// first-seen order.
    #[serde(default)]
    pub applied_exemptions: Vec<ExemptionDetails>,
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
    /// Default calculation method. `Exclusive` (default) taxes on top of
    /// the listed price. `Inclusive` treats every listed price (and shipping)
    /// as gross and backs the tax out (`net = gross / (1 + rate)`), so
    /// `total` equals the listed prices exactly. A request may also opt in
    /// per call via [`TaxCalculationRequest::prices_include_tax`]; either
    /// flag makes the calculation inclusive.
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

/// A tax rate paired with the jurisdiction that levies it — the unit the
/// pure engine works on. Storage backends resolve these; [`compute_tax`]
/// never touches storage.
#[derive(Debug, Clone)]
pub struct ResolvedTaxRate {
    pub rate: TaxRate,
    pub jurisdiction: TaxJurisdiction,
}

/// Everything [`compute_tax`] needs besides the request itself. Both storage
/// backends build one of these (data loading only) and hand it to the shared
/// engine, so SQLite and Postgres agree on every cent by construction.
#[derive(Debug, Clone, Default)]
pub struct TaxComputationInputs {
    /// Store settings (rounding, shipping taxability, inclusive pricing).
    pub settings: TaxSettings,
    /// Rates applicable to the shipping address on the transaction date, for
    /// every product category the request touches (plus `Standard` for
    /// shipping). Each rate carries its `product_category`; the engine picks
    /// the ones matching each line.
    pub rates: Vec<ResolvedTaxRate>,
    /// The customer's exemptions (any state). The engine honours only those
    /// that are [`TaxExemption::is_effective_on`] the transaction date —
    /// active, verified, and inside their validity window.
    pub exemptions: Vec<TaxExemption>,
}

/// Backend-independent tax calculation.
///
/// Given the request and the data a backend resolved for it, produce the
/// result. Guarantees:
///
/// * every amount is rounded to `settings.decimal_places` with
///   `settings.rounding_mode`, per line and per rate, allocating rounding
///   residue by largest remainder — so
///   `Σ line_item_taxes + shipping_tax == total_tax` and
///   `Σ tax_breakdown == total_tax` hold exactly;
/// * customer exemptions apply only when effective on the transaction date
///   (active + verified + window), covering the line's category and the
///   rate's jurisdiction; jurisdiction-scoped exemptions remove only the
///   rates of those jurisdictions;
/// * when prices are tax-inclusive (`request.prices_include_tax` or
///   `settings.calculation_method == Inclusive`) line amounts and shipping
///   are gross: tax is backed out (`net = gross − tax`), the rounded tax is
///   allocated across rates, and `total` equals the listed prices exactly;
/// * `subtotal` is the (net) sum of line amounts; `total = subtotal +
///   total_tax + net shipping`.
///
/// # Disabled tax
///
/// When [`TaxSettings::enabled`] is `false` the store does not charge tax at
/// all, and no rate, exemption or jurisdiction is consulted. The result is
/// the request priced at zero tax:
///
/// * `total_tax == 0` and `shipping_tax == 0`;
/// * `tax_breakdown` and `jurisdictions` are empty;
/// * `exemptions_applied` is `false` and `exemption_details` is `None`;
/// * every line appears in `line_item_taxes` with `tax_amount == 0`, an empty
///   `tax_details` and `is_exempt == false` — the line is not exempt, the
///   store simply charges no tax;
/// * `subtotal` is the sum of the line amounts **unchanged** (nothing is
///   backed out of tax-inclusive prices, because there is no tax inside
///   them), and `total == subtotal + shipping`.
#[must_use]
pub fn compute_tax(
    request: &TaxCalculationRequest,
    inputs: &TaxComputationInputs,
    now: DateTime<Utc>,
) -> TaxCalculationResult {
    let settings = &inputs.settings;
    let transaction_date = request.transaction_date.unwrap_or_else(|| now.date_naive());
    let inclusive = request.prices_include_tax
        || settings.calculation_method == TaxCalculationMethod::Inclusive;
    let decimal_places = u32::try_from(settings.decimal_places).unwrap_or(2);
    let mut acc = TaxAccumulator::new(decimal_places, settings.rounding_strategy());
    acc.inclusive = inclusive;

    // Tax turned off at the store level: price everything at zero tax. Done
    // before any rate or exemption is looked at, so `enabled = false` is a
    // hard switch rather than a hint.
    if !settings.enabled {
        for item in &request.line_items {
            acc.add_taxed_line(&item.id, taxable_line_amount(item), &[]);
        }
        if let Some(shipping_amount) = request.shipping_amount {
            acc.add_shipping(shipping_amount.max(Decimal::ZERO), &[]);
        }
        return acc.finish(now);
    }

    let exemptions: Vec<&TaxExemption> =
        inputs.exemptions.iter().filter(|e| e.is_effective_on(transaction_date)).collect();

    for item in &request.line_items {
        let line_amount = taxable_line_amount(item);

        if item.tax_category == ProductTaxCategory::Exempt {
            acc.push_exempt_line(&item.id, line_amount, "Exempt product category");
            continue;
        }

        let rates: Vec<&ResolvedTaxRate> =
            inputs.rates.iter().filter(|r| r.rate.product_category == item.tax_category).collect();
        let rates = sorted_by_priority(rates);
        let (remaining, removed) = attribute_exemptions(&rates, &exemptions, item.tax_category);
        let exempted = !removed.is_empty();

        if exempted {
            acc.note_exemptions(line_amount, &removed);
        }
        let remaining: Vec<&ResolvedTaxRate> = remaining.iter().map(|&i| rates[i]).collect();

        if exempted && remaining.is_empty() {
            acc.push_exempt_line(&item.id, line_amount, "Customer exemption");
        } else {
            acc.add_taxed_line(&item.id, line_amount, &remaining);
        }
    }

    if let Some(shipping_amount) = request.shipping_amount {
        let shipping_amount = shipping_amount.max(Decimal::ZERO);
        if settings.tax_shipping {
            let rates: Vec<&ResolvedTaxRate> = inputs
                .rates
                .iter()
                .filter(|r| r.rate.product_category == ProductTaxCategory::Standard)
                .collect();
            let rates = sorted_by_priority(rates);
            let (remaining, _) =
                attribute_exemptions(&rates, &exemptions, ProductTaxCategory::Standard);
            let remaining: Vec<&ResolvedTaxRate> = remaining.iter().map(|&i| rates[i]).collect();
            acc.add_shipping(shipping_amount, &remaining);
        } else {
            acc.add_shipping(shipping_amount, &[]);
        }
    }

    acc.finish(now)
}

/// Order the rates that apply to a line: by explicit priority, then broadest
/// jurisdiction first, then by jurisdiction code and rate name.
///
/// The order is what the customer sees in `tax_details` and what the
/// largest-remainder allocation walks, so it must not depend on anything
/// random. Rate ids are the last resort only when two rates are otherwise
/// indistinguishable; ordering on them alone made the breakdown vary between
/// runs.
fn sorted_by_priority(mut rates: Vec<&ResolvedTaxRate>) -> Vec<&ResolvedTaxRate> {
    rates.sort_by(|a, b| {
        a.rate
            .priority
            .cmp(&b.rate.priority)
            .then_with(|| a.jurisdiction.level.cmp(&b.jurisdiction.level))
            .then_with(|| a.jurisdiction.code.cmp(&b.jurisdiction.code))
            .then_with(|| a.rate.name.cmp(&b.rate.name))
            .then_with(|| a.rate.id.cmp(&b.rate.id))
    });
    rates
}

/// Accumulates a tax calculation line by line, rounding **per line** (and
/// allocating each line's rounded tax across its rates by largest remainder)
/// so that `sum(line_item_taxes) + shipping_tax == total_tax` and
/// `sum(tax_breakdown) == total_tax` hold exactly. [`compute_tax`] drives
/// this; backends never need to touch it directly.
#[derive(Debug)]
pub struct TaxAccumulator {
    /// Sum of (non-negative, net) line amounts.
    pub subtotal: Decimal,
    /// Sum of rounded line taxes plus rounded shipping tax.
    pub total_tax: Decimal,
    /// Rounded shipping tax.
    pub shipping_tax: Decimal,
    /// Net shipping amount (gross minus shipping tax when inclusive).
    pub shipping_amount: Decimal,
    /// One entry per line item, in request order.
    pub line_item_taxes: Vec<LineItemTax>,
    /// Per (jurisdiction, tax type) totals, in first-seen order.
    pub tax_breakdown: Vec<TaxBreakdown>,
    /// Whether any exemption removed tax from any line.
    pub exemptions_applied: bool,
    /// Whether amounts are tax-inclusive (gross) and tax must be backed out.
    pub inclusive: bool,
    /// One entry per exemption that actually removed tax, in first-seen order.
    exemptions: Vec<ExemptionDetails>,
    jurisdictions: std::collections::HashMap<Uuid, JurisdictionSummary>,
    /// Rates already counted into a jurisdiction's `total_rate`.
    seen_rates: std::collections::HashSet<Uuid>,
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
            shipping_amount: Decimal::ZERO,
            line_item_taxes: Vec::new(),
            tax_breakdown: Vec::new(),
            exemptions_applied: false,
            inclusive: false,
            exemptions: Vec::new(),
            jurisdictions: std::collections::HashMap::new(),
            seen_rates: std::collections::HashSet::new(),
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

    /// Record that `removed` rates were taken off a line of `line_amount`,
    /// crediting each removed rate's tax to the exemption that removed it.
    ///
    /// Each exemption gets its OWN entry in the result: `amount_exempt` sums
    /// the line amounts on which that exemption removed at least one rate,
    /// and `tax_saved` sums only the tax that exemption itself removed. A
    /// cart hitting two certificates therefore reports two — attributing the
    /// whole saving to whichever certificate happened to be listed first
    /// misstates both.
    pub fn note_exemptions(
        &mut self,
        line_amount: Decimal,
        removed: &[(&ResolvedTaxRate, &TaxExemption)],
    ) {
        if removed.is_empty() {
            return;
        }
        self.exemptions_applied = true;
        // Group this line's removed rates by the exemption that removed them,
        // preserving first-seen order so the output is deterministic.
        let mut groups: Vec<(&TaxExemption, Vec<&ResolvedTaxRate>)> = Vec::new();
        for (rate, exemption) in removed {
            match groups.iter_mut().find(|(e, _)| e.id == exemption.id) {
                Some((_, rates)) => rates.push(rate),
                None => groups.push((exemption, vec![rate])),
            }
        }
        for (exemption, rates) in groups {
            let (raw_total, _, _) = self.split(line_amount, &rates);
            let tax_saved = raw_total.round_dp_with_strategy(self.decimal_places, self.strategy);
            if !self.exemptions.iter().any(|d| d.exemption_id == exemption.id) {
                self.exemptions.push(ExemptionDetails {
                    exemption_id: exemption.id,
                    exemption_type: exemption.exemption_type,
                    certificate_number: exemption.certificate_number.clone(),
                    amount_exempt: Decimal::ZERO,
                    tax_saved: Decimal::ZERO,
                });
            }
            if let Some(details) =
                self.exemptions.iter_mut().find(|d| d.exemption_id == exemption.id)
            {
                details.amount_exempt += line_amount;
                details.tax_saved += tax_saved;
            }
        }
    }

    /// Tax `line_amount` with `rates` (each with its jurisdiction). Returns
    /// the line's rounded tax. When inclusive, `line_amount` is gross and the
    /// recorded taxable amount is the net.
    pub fn add_taxed_line(
        &mut self,
        line_item_id: &str,
        line_amount: Decimal,
        rates: &[&ResolvedTaxRate],
    ) -> Decimal {
        let (line_tax, allocated) = self.apply_rates(line_amount, rates);
        let net = if self.inclusive { line_amount - line_tax } else { line_amount };
        self.subtotal += net;
        // Emitted in ALLOCATION order — the `sorted_by_priority` order the
        // largest-remainder residue was assigned in — so the breakdown the
        // customer reads is the breakdown the engine computed. Re-sorting on
        // a different key (jurisdiction name) printed "Austin" above "Texas"
        // while the residue cent had gone to Texas. The allocation order is
        // already platform-stable, so no second sort is needed.
        let tax_details: Vec<TaxDetail> = allocated
            .iter()
            .map(|(share, amount)| {
                let resolved = rates[share.index];
                TaxDetail {
                    tax_type: resolved.rate.tax_type,
                    jurisdiction_name: resolved.jurisdiction.name.clone(),
                    rate: resolved.rate.rate,
                    amount: *amount,
                }
            })
            .collect();
        let effective_rate = if net.is_zero() {
            Decimal::ZERO
        } else {
            // A quotient of two decimals has unbounded scale; a rate is a
            // rate, so pin it to four digits beyond the money scale
            // (0.082500 at 2dp) instead of leaking 28 digits into the API.
            (line_tax / net).round_dp_with_strategy(self.decimal_places + 4, self.strategy)
        };
        self.total_tax += line_tax;
        self.line_item_taxes.push(LineItemTax {
            line_item_id: line_item_id.to_string(),
            taxable_amount: net,
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
        rates: &[&ResolvedTaxRate],
    ) -> Decimal {
        let (shipping_tax, _) = self.apply_rates(shipping_amount, rates);
        self.shipping_amount +=
            if self.inclusive { shipping_amount - shipping_tax } else { shipping_amount };
        self.shipping_tax += shipping_tax;
        self.total_tax += shipping_tax;
        shipping_tax
    }

    /// Unrounded shares of `base` for `rates`, as
    /// `(raw_total, shares, raw_parts)`. When inclusive, `base` is gross and
    /// each share is scaled by `gross / (gross + tax_on_gross)` so that the
    /// total is the tax embedded in `base`.
    fn split(
        &self,
        base: Decimal,
        rates: &[&ResolvedTaxRate],
    ) -> (Decimal, Vec<RateShare>, Vec<Decimal>) {
        let mut shares = rate_shares(base, rates.iter().map(|r| &r.rate));
        let mut raw: Vec<Decimal> = shares.iter().map(|s| s.raw_tax).collect();
        let mut raw_total: Decimal = raw.iter().sum();
        if self.inclusive && !raw_total.is_zero() {
            // tax_on_gross = k * gross  =>  tax_in_gross = gross * k / (1 + k)
            //                            = gross * tax_on_gross / (gross + tax_on_gross)
            let factor = base / (base + raw_total);
            for (part, share) in raw.iter_mut().zip(shares.iter_mut()) {
                *part *= factor;
                share.raw_tax = *part;
                share.taxable_amount *= factor;
            }
            raw_total = raw.iter().sum();
        }
        (raw_total, shares, raw)
    }

    /// Apply `rates` to `base`, round the sum once, allocate it across the
    /// rates, and fold the allocation into the breakdown and jurisdiction
    /// summaries. Returns the rounded total and the per-rate allocation.
    fn apply_rates(
        &mut self,
        base: Decimal,
        rates: &[&ResolvedTaxRate],
    ) -> (Decimal, Vec<(RateShare, Decimal)>) {
        let (raw_total, shares, raw) = self.split(base, rates);
        let (rounded_total, rounded) =
            allocate_rounded(raw_total, &raw, self.decimal_places, self.strategy);

        let allocated: Vec<(RateShare, Decimal)> = shares.into_iter().zip(rounded).collect();
        for (share, amount) in &allocated {
            let resolved = rates[share.index];
            let (rate, jurisdiction) = (&resolved.rate, &resolved.jurisdiction);
            let summary =
                self.jurisdictions.entry(jurisdiction.id).or_insert_with(|| JurisdictionSummary {
                    id: jurisdiction.id,
                    name: jurisdiction.name.clone(),
                    code: jurisdiction.code.clone(),
                    level: jurisdiction.level,
                    total_rate: Decimal::ZERO,
                    total_tax: Decimal::ZERO,
                });
            if self.seen_rates.insert(rate.id) {
                summary.total_rate += rate.rate;
            }
            summary.total_tax += *amount;

            let taxable_amount =
                share.taxable_amount.round_dp_with_strategy(self.decimal_places, self.strategy);
            if let Some(existing) = self
                .tax_breakdown
                .iter_mut()
                .find(|b| b.jurisdiction_id == jurisdiction.id && b.tax_type == rate.tax_type)
            {
                existing.taxable_amount += taxable_amount;
                existing.tax_amount += *amount;
            } else {
                self.tax_breakdown.push(TaxBreakdown {
                    jurisdiction_id: jurisdiction.id,
                    jurisdiction_name: jurisdiction.name.clone(),
                    tax_type: rate.tax_type,
                    rate_name: rate.name.clone(),
                    rate: rate.rate,
                    taxable_amount,
                    tax_amount: *amount,
                    is_compound: rate.is_compound,
                });
            }
        }
        (rounded_total, allocated)
    }

    /// Produce the result. `total = subtotal + total_tax + net shipping`.
    #[must_use]
    pub fn finish(self, now: DateTime<Utc>) -> TaxCalculationResult {
        // Emit jurisdictions in a stable order (by code, then id) rather than
        // the HashMap's iteration order, which varies run-to-run and between
        // the SQLite and Postgres backends — the tax result must be
        // deterministic.
        let mut jurisdictions: Vec<JurisdictionSummary> =
            self.jurisdictions.into_values().collect();
        jurisdictions.sort_by(|a, b| a.code.cmp(&b.code).then_with(|| a.id.cmp(&b.id)));

        let total = self.subtotal + self.total_tax + self.shipping_amount;
        TaxCalculationResult {
            id: Uuid::new_v4(),
            total_tax: self.total_tax,
            subtotal: self.subtotal,
            total,
            shipping_tax: self.shipping_tax,
            tax_breakdown: self.tax_breakdown,
            line_item_taxes: self.line_item_taxes,
            exemptions_applied: self.exemptions_applied,
            exemption_details: self.exemptions.first().cloned(),
            applied_exemptions: self.exemptions,
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
///
/// Use [`attribute_exemptions`] when you need to know **which** exemption
/// removed each rate.
#[must_use]
pub fn rates_after_exemptions(
    rates: &[&ResolvedTaxRate],
    exemptions: &[&TaxExemption],
    category: ProductTaxCategory,
) -> (Vec<usize>, bool) {
    let (remaining, removed) = attribute_exemptions(rates, exemptions, category);
    (remaining, !removed.is_empty())
}

/// Like [`rates_after_exemptions`], but also says which exemption removed
/// each removed rate.
///
/// Returns `(remaining rate indexes, removed rates paired with the exemption
/// that removed them)`. When several exemptions cover the same rate the
/// **first** one in `exemptions` is credited, so attribution is a function of
/// the caller's (deterministic) exemption order and never of iteration order.
#[must_use]
pub fn attribute_exemptions<'a, 'r>(
    rates: &[&'r ResolvedTaxRate],
    exemptions: &[&'a TaxExemption],
    category: ProductTaxCategory,
) -> (Vec<usize>, Vec<(&'r ResolvedTaxRate, &'a TaxExemption)>) {
    let covering: Vec<&TaxExemption> =
        exemptions.iter().copied().filter(|e| e.covers_category(category)).collect();
    if covering.is_empty() {
        return ((0..rates.len()).collect(), Vec::new());
    }
    let mut remaining = Vec::with_capacity(rates.len());
    let mut removed = Vec::new();
    for (index, resolved) in rates.iter().enumerate() {
        match covering.iter().find(|e| e.covers_jurisdiction(resolved.rate.jurisdiction_id)) {
            Some(exemption) => removed.push((*resolved, *exemption)),
            None => remaining.push(index),
        }
    }
    (remaining, removed)
}

/// The amount a line is taxed on: `unit_price × quantity − discount`, floored
/// at zero (a discount larger than the line never produces negative tax).
#[must_use]
pub fn taxable_line_amount(item: &TaxLineItem) -> Decimal {
    (item.unit_price * item.quantity - item.discount_amount).max(Decimal::ZERO)
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
    use rust_decimal_macros::dec;
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

    // ------------------------------------------------------------------
    // Shared engine (compute_tax)
    // ------------------------------------------------------------------

    fn jur(code: &str) -> TaxJurisdiction {
        jur_named(code, &format!("Jurisdiction {code}"), JurisdictionLevel::State)
    }

    /// A jurisdiction whose NAME and LEVEL are chosen independently of its
    /// code — the only shape that can catch a display order that sorts on a
    /// different key from the allocation order. `jur()` cannot: its name is
    /// derived from its code and every one is a `State`.
    fn jur_named(code: &str, name: &str, level: JurisdictionLevel) -> TaxJurisdiction {
        let now = Utc::now();
        TaxJurisdiction {
            id: Uuid::new_v4(),
            parent_id: None,
            name: name.to_string(),
            code: code.to_string(),
            level,
            country_code: "ZZ".into(),
            state_code: None,
            county: None,
            city: None,
            postal_codes: vec![],
            active: true,
            created_at: now,
            updated_at: now,
        }
    }

    fn rate(jurisdiction: &TaxJurisdiction, pct: Decimal) -> ResolvedTaxRate {
        rate_for(jurisdiction, pct, ProductTaxCategory::Standard)
    }

    fn rate_for(
        jurisdiction: &TaxJurisdiction,
        pct: Decimal,
        category: ProductTaxCategory,
    ) -> ResolvedTaxRate {
        let now = Utc::now();
        ResolvedTaxRate {
            rate: TaxRate {
                id: Uuid::new_v4(),
                jurisdiction_id: jurisdiction.id,
                tax_type: TaxType::SalesTax,
                product_category: category,
                rate: pct,
                name: format!("{} tax", jurisdiction.code),
                description: None,
                is_compound: false,
                priority: 1,
                threshold_min: None,
                threshold_max: None,
                fixed_amount: None,
                effective_from: NaiveDate::from_ymd_opt(2020, 1, 1).expect("date"),
                effective_to: None,
                active: true,
                created_at: now,
                updated_at: now,
            },
            jurisdiction: jurisdiction.clone(),
        }
    }

    fn line(id: &str, unit_price: Decimal, quantity: Decimal) -> TaxLineItem {
        TaxLineItem { id: id.into(), unit_price, quantity, ..Default::default() }
    }

    fn request(lines: Vec<TaxLineItem>) -> TaxCalculationRequest {
        TaxCalculationRequest {
            line_items: lines,
            shipping_address: TaxAddress { country: "ZZ".into(), ..Default::default() },
            transaction_date: Some(NaiveDate::from_ymd_opt(2026, 6, 1).expect("date")),
            ..Default::default()
        }
    }

    fn exemption(customer_id: Uuid) -> TaxExemption {
        let now = Utc::now();
        TaxExemption {
            id: Uuid::new_v4(),
            customer_id,
            exemption_type: ExemptionType::Resale,
            certificate_number: Some("RES-1".into()),
            issuing_authority: None,
            jurisdiction_ids: vec![],
            exempt_categories: vec![],
            effective_from: NaiveDate::from_ymd_opt(2026, 1, 1).expect("date"),
            expires_at: None,
            verified: true,
            verified_at: Some(now),
            notes: None,
            active: true,
            created_at: now,
            updated_at: now,
        }
    }

    fn inputs(rates: Vec<ResolvedTaxRate>, exemptions: Vec<TaxExemption>) -> TaxComputationInputs {
        TaxComputationInputs { settings: TaxSettings::default(), rates, exemptions }
    }

    #[test]
    fn compute_tax_rounds_per_line_and_sums_exactly() {
        // 3 × $1.11 @ 8.25%: each line is 0.091575 → 0.09; the total is the
        // sum of the rounded lines (0.27), never round(0.274725) = 0.27 with
        // unrounded lines that do not add up.
        let j = jur("ZZ-CA");
        let req = request(vec![
            line("a", dec!(1.11), dec!(1)),
            line("b", dec!(1.11), dec!(1)),
            line("c", dec!(1.11), dec!(1)),
        ]);
        let res = compute_tax(&req, &inputs(vec![rate(&j, dec!(0.0825))], vec![]), Utc::now());
        let lines: Vec<Decimal> = res.line_item_taxes.iter().map(|l| l.tax_amount).collect();
        assert_eq!(lines, vec![dec!(0.09), dec!(0.09), dec!(0.09)]);
        assert_eq!(res.total_tax, dec!(0.27));
        assert_eq!(res.subtotal, dec!(3.33));
        assert_eq!(res.total, dec!(3.60));
        assert_eq!(res.tax_breakdown.iter().map(|b| b.tax_amount).sum::<Decimal>(), dec!(0.27));
    }

    #[test]
    fn compute_tax_allocates_line_tax_across_rates_by_largest_remainder() {
        // $10.00 with 7.25% + 1.00%: raw 0.725 + 0.100 = 0.825 → 0.83; the
        // per-rate parts round to 0.73 + 0.10 = 0.83 exactly.
        let state = jur("ZZ-CA");
        let city = jur("ZZ-CA-LA");
        let req = request(vec![line("a", dec!(10), dec!(1))]);
        let res = compute_tax(
            &req,
            &inputs(vec![rate(&state, dec!(0.0725)), rate(&city, dec!(0.01))], vec![]),
            Utc::now(),
        );
        assert_eq!(res.total_tax, dec!(0.83));
        let details: Vec<Decimal> =
            res.line_item_taxes[0].tax_details.iter().map(|d| d.amount).collect();
        assert_eq!(details.iter().sum::<Decimal>(), dec!(0.83));
        assert_eq!(details, vec![dec!(0.73), dec!(0.10)]);
        assert_eq!(res.jurisdictions.len(), 2);
        assert_eq!(res.jurisdictions.iter().map(|j| j.total_tax).sum::<Decimal>(), dec!(0.83));
    }

    /// The rate order that drives `tax_details` (and the largest-remainder
    /// allocation) must not depend on the order rates arrive in, nor on their
    /// randomly generated ids. Tie-breaking on `rate.id` alone made this vary
    /// between runs.
    #[test]
    fn compute_tax_rate_order_is_stable_regardless_of_input_order() {
        let state = jur("ZZ-CA");
        let city = jur("ZZ-CA-LA");
        let req = request(vec![line("a", dec!(10), dec!(1))]);

        let forward = compute_tax(
            &req,
            &inputs(vec![rate(&state, dec!(0.0725)), rate(&city, dec!(0.01))], vec![]),
            Utc::now(),
        );
        // Same two rates, supplied the other way round and with fresh ids.
        let reversed = compute_tax(
            &req,
            &inputs(vec![rate(&city, dec!(0.01)), rate(&state, dec!(0.0725))], vec![]),
            Utc::now(),
        );

        let amounts = |r: &TaxCalculationResult| -> Vec<Decimal> {
            r.line_item_taxes[0].tax_details.iter().map(|d| d.amount).collect()
        };
        let names = |r: &TaxCalculationResult| -> Vec<String> {
            r.line_item_taxes[0].tax_details.iter().map(|d| d.jurisdiction_name.clone()).collect()
        };
        assert_eq!(amounts(&forward), vec![dec!(0.73), dec!(0.10)]);
        assert_eq!(amounts(&reversed), amounts(&forward), "amount order must not depend on input");
        assert_eq!(
            names(&reversed),
            names(&forward),
            "jurisdiction order must not depend on input"
        );
        assert_eq!(forward.total_tax, reversed.total_tax);
    }

    #[test]
    fn compute_tax_backs_out_inclusive_prices() {
        // €19.99 gross @ 19% VAT: net 16.80, tax 3.19, customer pays 19.99.
        let j = jur("DE");
        let mut req = request(vec![line("a", dec!(19.99), dec!(1))]);
        req.prices_include_tax = true;
        req.currency = CurrencyCode::EUR;
        let res = compute_tax(&req, &inputs(vec![rate(&j, dec!(0.19))], vec![]), Utc::now());
        assert_eq!(res.total_tax, dec!(3.19));
        assert_eq!(res.subtotal, dec!(16.80));
        assert_eq!(res.total, dec!(19.99));
        assert_eq!(res.line_item_taxes[0].taxable_amount, dec!(16.80));
        assert_eq!(res.line_item_taxes[0].tax_amount, dec!(3.19));

        // The store setting alone also makes the calculation inclusive, and
        // inclusive shipping is backed out the same way (€5.00 → 0.80 tax).
        let mut req = request(vec![line("a", dec!(19.99), dec!(1))]);
        req.shipping_amount = Some(dec!(5.00));
        let mut inp = inputs(vec![rate(&j, dec!(0.19))], vec![]);
        inp.settings.calculation_method = TaxCalculationMethod::Inclusive;
        let res = compute_tax(&req, &inp, Utc::now());
        assert_eq!(res.shipping_tax, dec!(0.80));
        assert_eq!(res.total_tax, dec!(3.99));
        assert_eq!(res.total, dec!(24.99), "customer pays exactly the listed prices");
    }

    #[test]
    fn compute_tax_ignores_unverified_and_out_of_window_exemptions() {
        let j = jur("ZZ-CA");
        let customer = Uuid::new_v4();
        let mut req = request(vec![line("a", dec!(100), dec!(1))]);
        req.customer_id = Some(customer);
        let rates = vec![rate(&j, dec!(0.05))];

        let unverified = TaxExemption { verified: false, ..exemption(customer) };
        let res = compute_tax(&req, &inputs(rates.clone(), vec![unverified]), Utc::now());
        assert_eq!(res.total_tax, dec!(5.00), "unverified exemption must not apply");
        assert!(!res.exemptions_applied);

        let future = TaxExemption {
            effective_from: NaiveDate::from_ymd_opt(2026, 7, 1).expect("date"),
            ..exemption(customer)
        };
        let res = compute_tax(&req, &inputs(rates.clone(), vec![future]), Utc::now());
        assert_eq!(res.total_tax, dec!(5.00), "exemption not yet effective on transaction_date");

        let expired = TaxExemption {
            expires_at: Some(NaiveDate::from_ymd_opt(2026, 5, 31).expect("date")),
            ..exemption(customer)
        };
        let res = compute_tax(&req, &inputs(rates.clone(), vec![expired]), Utc::now());
        assert_eq!(res.total_tax, dec!(5.00), "expired exemption must not apply");

        let inactive = TaxExemption { active: false, ..exemption(customer) };
        let res = compute_tax(&req, &inputs(rates.clone(), vec![inactive]), Utc::now());
        assert_eq!(res.total_tax, dec!(5.00), "inactive exemption must not apply");

        let res = compute_tax(&req, &inputs(rates, vec![exemption(customer)]), Utc::now());
        assert_eq!(res.total_tax, Decimal::ZERO);
        assert!(res.exemptions_applied);
        assert!(res.line_item_taxes[0].is_exempt);
        let details = res.exemption_details.expect("exemption details");
        assert_eq!(details.amount_exempt, dec!(100));
        assert_eq!(details.tax_saved, dec!(5.00));
    }

    #[test]
    fn compute_tax_exemption_scoped_by_category_and_jurisdiction() {
        let country = jur("ZZ");
        let state = jur("ZZ-CA");
        let customer = Uuid::new_v4();
        let mut req = request(vec![line("a", dec!(100), dec!(1))]);
        req.customer_id = Some(customer);
        let rates = vec![rate(&country, dec!(0.05)), rate(&state, dec!(0.03))];

        // Scoped to the state: only the state's 3% is removed.
        let scoped = TaxExemption { jurisdiction_ids: vec![state.id], ..exemption(customer) };
        let res = compute_tax(&req, &inputs(rates.clone(), vec![scoped]), Utc::now());
        assert_eq!(res.total_tax, dec!(5.00));
        assert!(res.exemptions_applied);
        assert!(!res.line_item_taxes[0].is_exempt);
        assert_eq!(res.exemption_details.expect("details").tax_saved, dec!(3.00));

        // Scoped to a category the line is not in: nothing removed.
        let other_cat = TaxExemption {
            exempt_categories: vec![ProductTaxCategory::Food],
            ..exemption(customer)
        };
        let res = compute_tax(&req, &inputs(rates, vec![other_cat]), Utc::now());
        assert_eq!(res.total_tax, dec!(8.00));
        assert!(!res.exemptions_applied);
    }

    #[test]
    fn compute_tax_uses_rates_of_each_line_category() {
        let j = jur("ZZ-CA");
        let mut food = line("food", dec!(10), dec!(1));
        food.tax_category = ProductTaxCategory::Food;
        let mut exempt = line("exempt", dec!(10), dec!(1));
        exempt.tax_category = ProductTaxCategory::Exempt;
        let req = request(vec![line("std", dec!(10), dec!(1)), food, exempt]);
        let res = compute_tax(
            &req,
            &inputs(
                vec![rate(&j, dec!(0.08)), rate_for(&j, dec!(0.02), ProductTaxCategory::Food)],
                vec![],
            ),
            Utc::now(),
        );
        let by_line: Vec<(String, Decimal, bool)> = res
            .line_item_taxes
            .iter()
            .map(|l| (l.line_item_id.clone(), l.tax_amount, l.is_exempt))
            .collect();
        assert_eq!(
            by_line,
            vec![
                ("std".into(), dec!(0.80), false),
                ("food".into(), dec!(0.20), false),
                ("exempt".into(), Decimal::ZERO, true),
            ]
        );
        assert_eq!(res.total_tax, dec!(1.00));
        assert_eq!(res.subtotal, dec!(30));
    }

    /// `TaxSettings::enabled = false` means the store does not charge tax.
    /// The engine used to read every OTHER setting and ignore this one, so
    /// turning tax off still charged tax.
    #[test]
    fn compute_tax_disabled_settings_charge_no_tax() {
        let j = jur("ZZ-CA");
        let customer = Uuid::new_v4();
        let mut req = request(vec![line("a", dec!(100), dec!(1)), line("b", dec!(19.99), dec!(2))]);
        req.shipping_amount = Some(dec!(9.99));
        req.customer_id = Some(customer);
        let mut inp = inputs(vec![rate(&j, dec!(0.0825))], vec![exemption(customer)]);

        // Sanity: with tax enabled this request is taxed.
        let enabled = compute_tax(&req, &inp, Utc::now());
        assert!(enabled.exemptions_applied);

        inp.settings.enabled = false;
        let res = compute_tax(&req, &inp, Utc::now());
        assert_eq!(res.total_tax, Decimal::ZERO, "disabled tax must charge nothing: {res:?}");
        assert_eq!(res.shipping_tax, Decimal::ZERO);
        assert!(res.tax_breakdown.is_empty(), "no breakdown when tax is off");
        assert!(res.jurisdictions.is_empty(), "no jurisdiction is consulted when tax is off");
        assert!(!res.exemptions_applied);
        assert!(res.exemption_details.is_none());
        // Subtotal is the line amounts unchanged; the customer pays them plus
        // shipping and nothing else.
        assert_eq!(res.subtotal, dec!(139.98));
        assert_eq!(res.total, dec!(149.97));
        assert_eq!(res.line_item_taxes.len(), 2);
        for l in &res.line_item_taxes {
            assert_eq!(l.tax_amount, Decimal::ZERO);
            assert!(l.tax_details.is_empty());
            assert!(!l.is_exempt, "the line is not exempt, the store charges no tax");
        }

        // Tax-inclusive prices are not touched either: there is no tax inside
        // them to back out.
        let mut inclusive = req;
        inclusive.prices_include_tax = true;
        let res = compute_tax(&inclusive, &inp, Utc::now());
        assert_eq!(res.total_tax, Decimal::ZERO);
        assert_eq!(res.subtotal, dec!(139.98));
        assert_eq!(res.total, dec!(149.97));
    }

    /// The customer-facing `tax_details` must be in the SAME order the
    /// allocation walked (`sorted_by_priority`: broadest jurisdiction first),
    /// not a second sort on a different key. Sorting the display by
    /// jurisdiction NAME printed the city above the state while the
    /// largest-remainder residue cent had been assigned to the state.
    #[test]
    fn compute_tax_breakdown_is_in_allocation_order() {
        // Name order (Austin < Texas) and level order (State < City)
        // DISAGREE, which is exactly what the old fixtures could not express.
        let state = jur_named("ZZ-TX", "Texas", JurisdictionLevel::State);
        let city = jur_named("ZZ-TX-AUS", "Austin", JurisdictionLevel::City);
        let req = request(vec![line("a", dec!(10), dec!(1))]);
        let res = compute_tax(
            &req,
            &inputs(vec![rate(&city, dec!(0.01)), rate(&state, dec!(0.0625))], vec![]),
            Utc::now(),
        );

        let details = &res.line_item_taxes[0].tax_details;
        let names: Vec<&str> = details.iter().map(|d| d.jurisdiction_name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Texas", "Austin"],
            "breakdown must be broadest-jurisdiction-first, as allocated: {details:?}"
        );
        // And the amounts line up with those names: 0.625 -> 0.63 (the
        // residue cent), 0.10 -> 0.10.
        let amounts: Vec<Decimal> = details.iter().map(|d| d.amount).collect();
        assert_eq!(amounts, vec![dec!(0.63), dec!(0.10)]);
        assert_eq!(amounts.iter().sum::<Decimal>(), res.total_tax);
    }

    /// `line_item_taxes[].effective_rate` is a quotient of two decimals and
    /// used to carry unbounded scale (28 digits) into the API.
    #[test]
    fn compute_tax_effective_rate_is_rounded() {
        let j = jur("ZZ-CA");
        let req = request(vec![line("a", dec!(3), dec!(1))]);
        let res = compute_tax(&req, &inputs(vec![rate(&j, dec!(0.0825))], vec![]), Utc::now());
        let rate = res.line_item_taxes[0].effective_rate;
        assert!(rate.scale() <= 6, "effective_rate must be bounded, got {rate}");
        assert_eq!(rate, dec!(0.083333)); // 0.25 / 3
    }

    /// Two certificates on one cart must be reported as two, each with the
    /// tax IT saved. The engine used to credit the FIRST exemption covering
    /// the line's category with every line's saving, so a cart hitting two
    /// exemptions reported one, with the other's money on it.
    #[test]
    fn compute_tax_attributes_each_exemption_separately() {
        let country = jur("ZZ");
        let state = jur("ZZ-CA");
        let customer = Uuid::new_v4();
        let mut req = request(vec![line("a", dec!(100), dec!(1))]);
        req.customer_id = Some(customer);

        let country_cert = TaxExemption {
            jurisdiction_ids: vec![country.id],
            certificate_number: Some("FED-1".into()),
            ..exemption(customer)
        };
        let state_cert = TaxExemption {
            jurisdiction_ids: vec![state.id],
            certificate_number: Some("CA-1".into()),
            ..exemption(customer)
        };
        let res = compute_tax(
            &req,
            &inputs(
                vec![rate(&country, dec!(0.05)), rate(&state, dec!(0.03))],
                vec![country_cert.clone(), state_cert.clone()],
            ),
            Utc::now(),
        );

        assert_eq!(res.total_tax, Decimal::ZERO);
        assert!(res.exemptions_applied);
        assert_eq!(res.applied_exemptions.len(), 2, "two certificates, two entries: {res:?}");
        let saved: Vec<(Uuid, Decimal)> =
            res.applied_exemptions.iter().map(|d| (d.exemption_id, d.tax_saved)).collect();
        assert!(
            saved.contains(&(country_cert.id, dec!(5.00))),
            "the federal certificate saved its own 5.00: {saved:?}"
        );
        assert!(
            saved.contains(&(state_cert.id, dec!(3.00))),
            "the state certificate saved its own 3.00: {saved:?}"
        );
        assert_eq!(res.applied_exemptions.iter().map(|d| d.tax_saved).sum::<Decimal>(), dec!(8.00));
        // Certificate numbers travel with their own entry.
        let certs: Vec<Option<&str>> =
            res.applied_exemptions.iter().map(|d| d.certificate_number.as_deref()).collect();
        assert!(certs.contains(&Some("FED-1")) && certs.contains(&Some("CA-1")));

        // A single-exemption cart still reports it through the legacy field.
        let res = compute_tax(
            &req,
            &inputs(vec![rate(&country, dec!(0.05))], vec![country_cert.clone()]),
            Utc::now(),
        );
        assert_eq!(res.applied_exemptions.len(), 1);
        let legacy = res.exemption_details.expect("details");
        assert_eq!(legacy.exemption_id, country_cert.id);
        assert_eq!(legacy.tax_saved, dec!(5.00));
    }

    #[test]
    fn jurisdiction_covers_address_matches_state_city_and_postal_code() {
        let address = |city: Option<&str>, state: Option<&str>, postal: Option<&str>| TaxAddress {
            city: city.map(str::to_string),
            state: state.map(str::to_string),
            postal_code: postal.map(str::to_string),
            country: "US".into(),
            ..Default::default()
        };

        let mut country = jur_named("US", "United States", JurisdictionLevel::Country);
        country.country_code = "US".into();
        assert!(country.covers_address(&address(None, None, None)));
        assert!(
            !country.covers_address(&TaxAddress { country: "CA".into(), ..Default::default() })
        );

        let mut state = jur_named("US-TX", "Texas", JurisdictionLevel::State);
        state.country_code = "US".into();
        state.state_code = Some("TX".into());
        assert!(state.covers_address(&address(None, Some("tx"), None)), "case-insensitive");
        assert!(!state.covers_address(&address(None, Some("CA"), None)));
        assert!(!state.covers_address(&address(None, None, None)), "no state on the address");

        let mut city = jur_named("US-TX-AUS", "Austin", JurisdictionLevel::City);
        city.country_code = "US".into();
        city.state_code = Some("TX".into());
        city.city = Some("Austin".into());
        assert!(city.covers_address(&address(Some(" austin "), Some("TX"), None)));
        assert!(!city.covers_address(&address(Some("Dallas"), Some("TX"), None)));
        assert!(!city.covers_address(&address(None, Some("TX"), None)));

        let mut district = jur_named("US-TX-MTA", "Transit Authority", JurisdictionLevel::District);
        district.country_code = "US".into();
        district.state_code = Some("TX".into());
        district.postal_codes = vec!["787*".into(), "78701-78705".into()];
        assert!(district.covers_address(&address(None, Some("TX"), Some("78751"))));
        assert!(district.covers_address(&address(None, Some("TX"), Some("78703"))));
        assert!(!district.covers_address(&address(None, Some("TX"), Some("75201"))));
        assert!(!district.covers_address(&address(None, Some("TX"), None)));

        // A below-state jurisdiction with nothing narrower than the state
        // fails CLOSED rather than taxing the whole state at the city rate.
        let mut county = jur_named("US-TX-TRV", "Travis County", JurisdictionLevel::County);
        county.country_code = "US".into();
        county.state_code = Some("TX".into());
        county.county = Some("Travis".into());
        assert!(
            !county.covers_address(&address(Some("Austin"), Some("TX"), Some("78701"))),
            "an unpinned county must not apply to the whole state"
        );

        // Inactive jurisdictions never apply.
        let inactive = TaxJurisdiction { active: false, ..state };
        assert!(!inactive.covers_address(&address(None, Some("TX"), None)));
    }

    #[test]
    fn postal_code_matches_exact_prefix_and_range() {
        let patterns = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect::<Vec<String>>();
        assert!(postal_code_matches(&patterns(&["90210"]), "90210"));
        assert!(postal_code_matches(&patterns(&["90210"]), " 90210 "));
        assert!(!postal_code_matches(&patterns(&["90210"]), "90211"));
        assert!(postal_code_matches(&patterns(&["902*"]), "90210"));
        assert!(!postal_code_matches(&patterns(&["902*"]), "91210"));
        assert!(postal_code_matches(&patterns(&["90210-90220"]), "90215"));
        assert!(postal_code_matches(&patterns(&["90210-90220"]), "90210"));
        assert!(postal_code_matches(&patterns(&["90210-90220"]), "90220"));
        assert!(!postal_code_matches(&patterns(&["90210-90220"]), "90221"));
        // Non-numeric codes fall back to a lexicographic range.
        assert!(postal_code_matches(&patterns(&["K1A-K1Z"]), "K1M"));
        assert!(!postal_code_matches(&patterns(&["K1A-K1Z"]), "K2A"));
        // A code written with a separator is an exact code, not a range.
        assert!(postal_code_matches(&patterns(&["K1A 0B1"]), "k1a-0b1"));
        assert!(!postal_code_matches(&patterns(&[]), "90210"));
        assert!(!postal_code_matches(&patterns(&["90210"]), ""));
    }

    #[test]
    fn allocate_rounded_preserves_total() {
        let (total, parts) = allocate_rounded(
            dec!(0.275),
            &[dec!(0.091575), dec!(0.091575), dec!(0.091575)],
            2,
            RoundingStrategy::MidpointAwayFromZero,
        );
        assert_eq!(total, dec!(0.28));
        assert_eq!(parts.iter().sum::<Decimal>(), dec!(0.28));
        assert_eq!(parts, vec![dec!(0.10), dec!(0.09), dec!(0.09)]);
    }

    mod properties {
        use super::*;
        use proptest::prelude::*;

        fn money() -> impl Strategy<Value = Decimal> {
            (0i64..=100_000).prop_map(|cents| Decimal::new(cents, 2))
        }

        fn pct() -> impl Strategy<Value = Decimal> {
            (0i64..=3000).prop_map(|bp| Decimal::new(bp, 4))
        }

        fn lines() -> impl Strategy<Value = Vec<(Decimal, i64, Decimal)>> {
            prop::collection::vec((money(), 1i64..=5, money()), 1..8)
        }

        fn rates() -> impl Strategy<Value = Vec<(Decimal, bool)>> {
            prop::collection::vec((pct(), any::<bool>()), 0..4)
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(512))]
            #[test]
            fn line_taxes_plus_shipping_equal_total(
                lines in lines(),
                rate_specs in rates(),
                shipping in prop::option::of(money()),
                inclusive in any::<bool>(),
                exempt_first in any::<bool>(),
            ) {
                let j_a = jur("ZZ");
                let j_b = jur("ZZ-CA");
                let customer = Uuid::new_v4();
                let resolved: Vec<ResolvedTaxRate> = rate_specs
                    .iter()
                    .enumerate()
                    .map(|(i, (pct, compound))| {
                        let mut r = rate(if i % 2 == 0 { &j_a } else { &j_b }, *pct);
                        r.rate.is_compound = *compound;
                        r.rate.priority = i32::try_from(i).unwrap_or(0);
                        r
                    })
                    .collect();
                let items: Vec<TaxLineItem> = lines
                    .iter()
                    .enumerate()
                    .map(|(i, (price, qty, discount))| {
                        let mut l = line(&format!("l{i}"), *price, Decimal::from(*qty));
                        l.discount_amount = *discount;
                        l
                    })
                    .collect();
                let mut req = request(items);
                req.shipping_amount = shipping;
                req.prices_include_tax = inclusive;
                req.customer_id = Some(customer);
                let exemptions = if exempt_first {
                    vec![TaxExemption { jurisdiction_ids: vec![j_b.id], ..exemption(customer) }]
                } else {
                    vec![]
                };
                let res = compute_tax(&req, &inputs(resolved, exemptions), Utc::now());

                let line_sum: Decimal = res.line_item_taxes.iter().map(|l| l.tax_amount).sum();
                prop_assert_eq!(line_sum + res.shipping_tax, res.total_tax);
                let breakdown_sum: Decimal = res.tax_breakdown.iter().map(|b| b.tax_amount).sum();
                prop_assert_eq!(breakdown_sum, res.total_tax);
                let jur_sum: Decimal = res.jurisdictions.iter().map(|j| j.total_tax).sum();
                prop_assert_eq!(jur_sum, res.total_tax);
                for l in &res.line_item_taxes {
                    let details: Decimal = l.tax_details.iter().map(|d| d.amount).sum();
                    prop_assert_eq!(details, l.tax_amount);
                    prop_assert!(l.tax_amount.scale() <= 2, "line tax not rounded: {}", l.tax_amount);
                    prop_assert!(l.tax_amount >= Decimal::ZERO);
                }
                prop_assert!(res.total_tax.scale() <= 2);
                prop_assert!(res.shipping_tax.scale() <= 2);
                let gross: Decimal = lines
                    .iter()
                    .map(|(p, q, d)| (*p * Decimal::from(*q) - *d).max(Decimal::ZERO))
                    .sum::<Decimal>()
                    + shipping.unwrap_or_default();
                if inclusive {
                    prop_assert_eq!(res.total, gross, "inclusive: customer pays the listed prices");
                } else {
                    prop_assert_eq!(res.total, gross + res.total_tax);
                }
            }
        }
    }
}
