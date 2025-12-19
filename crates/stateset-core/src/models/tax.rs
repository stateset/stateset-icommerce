//! Tax calculation engine types
//!
//! Provides comprehensive tax support including:
//! - Multi-jurisdiction tax rates (US sales tax, EU VAT, etc.)
//! - Product tax categories (taxable, exempt, reduced rate)
//! - Customer tax exemptions (B2B, non-profits)
//! - Tax-inclusive vs tax-exclusive pricing
//! - Compound and tiered tax rules

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Tax Types and Enums
// ============================================================================

/// Types of taxes supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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
    pub fn as_str(&self) -> &'static str {
        match self {
            TaxType::SalesTax => "sales_tax",
            TaxType::Vat => "vat",
            TaxType::Gst => "gst",
            TaxType::Hst => "hst",
            TaxType::Pst => "pst",
            TaxType::Qst => "qst",
            TaxType::ConsumptionTax => "consumption_tax",
            TaxType::Custom => "custom",
        }
    }
}

/// Tax calculation method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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
pub enum TaxCompoundMethod {
    /// Add all taxes together, apply to subtotal
    #[default]
    Combined,
    /// Apply taxes sequentially (tax on tax)
    Compound,
    /// Apply taxes separately to subtotal
    Separate,
}

/// Product tax category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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
    pub fn as_str(&self) -> &'static str {
        match self {
            ProductTaxCategory::Standard => "standard",
            ProductTaxCategory::Reduced => "reduced",
            ProductTaxCategory::SuperReduced => "super_reduced",
            ProductTaxCategory::ZeroRated => "zero_rated",
            ProductTaxCategory::Exempt => "exempt",
            ProductTaxCategory::Digital => "digital",
            ProductTaxCategory::Clothing => "clothing",
            ProductTaxCategory::Food => "food",
            ProductTaxCategory::PreparedFood => "prepared_food",
            ProductTaxCategory::Medical => "medical",
            ProductTaxCategory::Educational => "educational",
            ProductTaxCategory::Luxury => "luxury",
        }
    }
}

/// Customer exemption type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
pub enum JurisdictionLevel {
    #[default]
    Country,
    State,
    County,
    City,
    District,
    Special,
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
    pub currency: String,
    /// Transaction date (for rate lookup)
    pub transaction_date: Option<NaiveDate>,
    /// Whether prices include tax
    pub prices_include_tax: bool,
}

fn default_currency() -> String {
    "USD".to_string()
}

/// A line item for tax calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxLineItem {
    /// Line item identifier
    pub id: String,
    /// Product SKU
    pub sku: Option<String>,
    /// Product ID
    pub product_id: Option<Uuid>,
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
    /// Rounding mode (up, down, half_up, half_down)
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
}

/// Filter for querying jurisdictions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaxJurisdictionFilter {
    pub country_code: Option<String>,
    pub state_code: Option<String>,
    pub level: Option<JurisdictionLevel>,
    pub active_only: bool,
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
    "AT", "BE", "BG", "HR", "CY", "CZ", "DK", "EE", "FI", "FR",
    "DE", "GR", "HU", "IE", "IT", "LV", "LT", "LU", "MT", "NL",
    "PL", "PT", "RO", "SK", "SI", "ES", "SE",
];

/// Check if a country is in the EU
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
