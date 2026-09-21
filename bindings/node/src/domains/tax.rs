//! Tax API.
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Tax API
// ============================================================================

// --- Input Types ---

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TaxAddressInput {
    pub line1: Option<String>,
    pub line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxLineItemInput {
    pub id: String,
    pub sku: Option<String>,
    pub product_id: Option<String>,
    pub quantity: f64,
    pub unit_price: f64,
    pub discount_amount: Option<f64>,
    #[napi(ts_type = "ProductTaxCategory")]
    pub tax_category: Option<String>,
    pub tax_code: Option<String>,
    pub description: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TaxCalculationInput {
    pub line_items: Vec<TaxLineItemInput>,
    pub shipping_address: TaxAddressInput,
    pub billing_address: Option<TaxAddressInput>,
    pub customer_id: Option<String>,
    pub shipping_amount: Option<f64>,
    pub currency: Option<String>,
    pub transaction_date: Option<String>,
    pub prices_include_tax: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CreateJurisdictionInput {
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    #[napi(ts_type = "TaxJurisdictionLevel")]
    pub level: Option<String>,
    pub country_code: String,
    pub state_code: Option<String>,
    pub county: Option<String>,
    pub city: Option<String>,
    pub postal_codes: Option<Vec<String>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CreateTaxRateInput {
    pub jurisdiction_id: String,
    #[napi(ts_type = "TaxType")]
    pub tax_type: Option<String>,
    #[napi(ts_type = "ProductTaxCategory")]
    pub product_category: Option<String>,
    pub rate: f64,
    pub name: String,
    pub description: Option<String>,
    pub is_compound: Option<bool>,
    pub priority: Option<i32>,
    pub threshold_min: Option<f64>,
    pub threshold_max: Option<f64>,
    pub fixed_amount: Option<f64>,
    pub effective_from: String,
    pub effective_to: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CreateExemptionInput {
    pub customer_id: String,
    #[napi(ts_type = "TaxExemptionTypeInput")]
    pub exemption_type: String,
    pub certificate_number: Option<String>,
    pub issuing_authority: Option<String>,
    pub jurisdiction_ids: Option<Vec<String>>,
    #[napi(ts_type = "ProductTaxCategory[]")]
    pub exempt_categories: Option<Vec<String>>,
    pub effective_from: String,
    pub expires_at: Option<String>,
    pub notes: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TaxRateFilterInput {
    pub jurisdiction_id: Option<String>,
    #[napi(ts_type = "TaxType")]
    pub tax_type: Option<String>,
    #[napi(ts_type = "ProductTaxCategory")]
    pub product_category: Option<String>,
    pub active_only: Option<bool>,
    pub effective_date: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct JurisdictionFilterInput {
    pub country_code: Option<String>,
    pub state_code: Option<String>,
    #[napi(ts_type = "TaxJurisdictionLevel")]
    pub level: Option<String>,
    pub active_only: Option<bool>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct TaxSettingsInput {
    pub enabled: Option<bool>,
    #[napi(ts_type = "TaxCalculationMethod")]
    pub calculation_method: Option<String>,
    #[napi(ts_type = "TaxCompoundMethod")]
    pub compound_method: Option<String>,
    pub tax_shipping: Option<bool>,
    pub tax_handling: Option<bool>,
    pub tax_gift_wrap: Option<bool>,
    #[napi(ts_type = "ProductTaxCategory")]
    pub default_product_category: Option<String>,
    pub rounding_mode: Option<String>,
    pub decimal_places: Option<i32>,
    pub validate_addresses: Option<bool>,
    pub tax_provider: Option<String>,
}

// --- Output Types ---

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxJurisdictionOutput {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub code: String,
    #[napi(ts_type = "TaxJurisdictionLevel")]
    pub level: String,
    pub country_code: String,
    pub state_code: Option<String>,
    pub county: Option<String>,
    pub city: Option<String>,
    pub postal_codes: Vec<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::TaxJurisdiction> for TaxJurisdictionOutput {
    fn from(j: stateset_core::TaxJurisdiction) -> Self {
        Self {
            id: j.id.to_string(),
            parent_id: j.parent_id.map(|u| u.to_string()),
            name: j.name,
            code: j.code,
            level: format!("{:?}", j.level).to_lowercase(),
            country_code: j.country_code,
            state_code: j.state_code,
            county: j.county,
            city: j.city,
            postal_codes: j.postal_codes,
            active: j.active,
            created_at: j.created_at.to_rfc3339(),
            updated_at: j.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxRateOutput {
    pub id: String,
    pub jurisdiction_id: String,
    #[napi(ts_type = "TaxType")]
    pub tax_type: String,
    #[napi(ts_type = "ProductTaxCategory")]
    pub product_category: String,
    pub rate: f64,
    pub name: String,
    pub description: Option<String>,
    pub is_compound: bool,
    pub priority: i32,
    /// @deprecated Use the `thresholdMinExact` twin; float money will be removed in 2.0.
    pub threshold_min: Option<f64>,
    /// Exact base-10 minimum amount at which the rate starts to apply.
    pub threshold_min_exact: Option<String>,
    /// @deprecated Use the `thresholdMaxExact` twin; float money will be removed in 2.0.
    pub threshold_max: Option<f64>,
    /// Exact base-10 cap on the amount this rate is charged against.
    pub threshold_max_exact: Option<String>,
    /// @deprecated Use the `fixedAmountExact` twin; float money will be removed in 2.0.
    pub fixed_amount: Option<f64>,
    /// Exact base-10 fixed amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub fixed_amount_exact: Option<String>,
    pub effective_from: String,
    pub effective_to: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl TryFrom<stateset_core::TaxRate> for TaxRateOutput {
    type Error = Error;

    fn try_from(r: stateset_core::TaxRate) -> Result<Self> {
        let (threshold_min, threshold_min_exact) =
            optional_money_pair(r.threshold_min, "tax rate threshold min")?;
        let (threshold_max, threshold_max_exact) =
            optional_money_pair(r.threshold_max, "tax rate threshold max")?;
        let (fixed_amount, fixed_amount_exact) =
            optional_money_pair(r.fixed_amount, "tax rate fixed amount")?;
        Ok(Self {
            id: r.id.to_string(),
            jurisdiction_id: r.jurisdiction_id.to_string(),
            tax_type: r.tax_type.as_str().to_string(),
            product_category: r.product_category.as_str().to_string(),
            rate: to_f64_checked(r.rate, "tax rate")?,
            name: r.name,
            description: r.description,
            is_compound: r.is_compound,
            priority: r.priority,
            threshold_min,
            threshold_min_exact,
            threshold_max,
            threshold_max_exact,
            fixed_amount,
            fixed_amount_exact,
            effective_from: r.effective_from.to_string(),
            effective_to: r.effective_to.map(|d| d.to_string()),
            active: r.active,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxExemptionOutput {
    pub id: String,
    pub customer_id: String,
    #[napi(ts_type = "TaxExemptionType")]
    pub exemption_type: String,
    pub certificate_number: Option<String>,
    pub issuing_authority: Option<String>,
    pub jurisdiction_ids: Vec<String>,
    #[napi(ts_type = "ProductTaxCategory[]")]
    pub exempt_categories: Vec<String>,
    pub effective_from: String,
    pub expires_at: Option<String>,
    pub verified: bool,
    pub verified_at: Option<String>,
    pub notes: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::TaxExemption> for TaxExemptionOutput {
    fn from(e: stateset_core::TaxExemption) -> Self {
        Self {
            id: e.id.to_string(),
            customer_id: e.customer_id.to_string(),
            exemption_type: format!("{:?}", e.exemption_type).to_lowercase(),
            certificate_number: e.certificate_number,
            issuing_authority: e.issuing_authority,
            jurisdiction_ids: e.jurisdiction_ids.iter().map(|u| u.to_string()).collect(),
            exempt_categories: e.exempt_categories.iter().map(|c| c.as_str().to_string()).collect(),
            effective_from: e.effective_from.to_string(),
            expires_at: e.expires_at.map(|d| d.to_string()),
            verified: e.verified,
            verified_at: e.verified_at.map(|d| d.to_rfc3339()),
            notes: e.notes,
            active: e.active,
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxBreakdownOutput {
    pub jurisdiction_id: String,
    pub jurisdiction_name: String,
    #[napi(ts_type = "TaxType")]
    pub tax_type: String,
    pub rate_name: String,
    pub rate: f64,
    /// @deprecated Use the `taxableAmountExact` twin; float money will be removed in 2.0.
    pub taxable_amount: f64,
    /// Exact base-10 taxable amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub taxable_amount_exact: String,
    /// @deprecated Use the `taxAmountExact` twin; float money will be removed in 2.0.
    pub tax_amount: f64,
    /// Exact base-10 tax amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub tax_amount_exact: String,
    pub is_compound: bool,
}

impl TryFrom<stateset_core::TaxBreakdown> for TaxBreakdownOutput {
    type Error = Error;

    fn try_from(b: stateset_core::TaxBreakdown) -> Result<Self> {
        let (taxable_amount, taxable_amount_exact) =
            money_pair(b.taxable_amount, "tax breakdown taxable amount")?;
        let (tax_amount, tax_amount_exact) = money_pair(b.tax_amount, "tax breakdown tax amount")?;
        Ok(Self {
            jurisdiction_id: b.jurisdiction_id.to_string(),
            jurisdiction_name: b.jurisdiction_name,
            tax_type: b.tax_type.as_str().to_string(),
            rate_name: b.rate_name,
            rate: to_f64_checked(b.rate, "tax breakdown rate")?,
            taxable_amount,
            taxable_amount_exact,
            tax_amount,
            tax_amount_exact,
            is_compound: b.is_compound,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxDetailOutput {
    #[napi(ts_type = "TaxType")]
    pub tax_type: String,
    pub jurisdiction_name: String,
    pub rate: f64,
    /// @deprecated Use the `amountExact` twin; float money will be removed in 2.0.
    pub amount: f64,
    /// Exact base-10 amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub amount_exact: String,
}

impl TryFrom<stateset_core::TaxDetail> for TaxDetailOutput {
    type Error = Error;

    fn try_from(d: stateset_core::TaxDetail) -> Result<Self> {
        let (amount, amount_exact) = money_pair(d.amount, "tax detail amount")?;
        Ok(Self {
            tax_type: d.tax_type.as_str().to_string(),
            jurisdiction_name: d.jurisdiction_name,
            rate: to_f64_checked(d.rate, "tax detail rate")?,
            amount,
            amount_exact,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LineItemTaxOutput {
    pub line_item_id: String,
    /// @deprecated Use the `taxableAmountExact` twin; float money will be removed in 2.0.
    pub taxable_amount: f64,
    /// Exact base-10 taxable amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub taxable_amount_exact: String,
    /// @deprecated Use the `taxAmountExact` twin; float money will be removed in 2.0.
    pub tax_amount: f64,
    /// Exact base-10 tax amount, straight from the engine's `Decimal`. Prefer this field for money.
    pub tax_amount_exact: String,
    pub effective_rate: f64,
    pub is_exempt: bool,
    pub exemption_reason: Option<String>,
    pub tax_details: Vec<TaxDetailOutput>,
}

impl TryFrom<stateset_core::LineItemTax> for LineItemTaxOutput {
    type Error = Error;

    fn try_from(t: stateset_core::LineItemTax) -> Result<Self> {
        let (taxable_amount, taxable_amount_exact) =
            money_pair(t.taxable_amount, "line item taxable amount")?;
        let (tax_amount, tax_amount_exact) = money_pair(t.tax_amount, "line item tax amount")?;
        Ok(Self {
            line_item_id: t.line_item_id,
            taxable_amount,
            taxable_amount_exact,
            tax_amount,
            tax_amount_exact,
            effective_rate: to_f64_checked(t.effective_rate, "line item effective tax rate")?,
            is_exempt: t.is_exempt,
            exemption_reason: t.exemption_reason,
            tax_details: convert_outputs(t.tax_details)?,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ExemptionDetailsOutput {
    pub exemption_id: String,
    #[napi(ts_type = "TaxExemptionType")]
    pub exemption_type: String,
    pub certificate_number: Option<String>,
    /// @deprecated Use the `amountExemptExact` twin; float money will be removed in 2.0.
    pub amount_exempt: f64,
    /// Exact base-10 amount exempt, straight from the engine's `Decimal`. Prefer this field for money.
    pub amount_exempt_exact: String,
    /// @deprecated Use the `taxSavedExact` twin; float money will be removed in 2.0.
    pub tax_saved: f64,
    /// Exact base-10 tax saved, straight from the engine's `Decimal`. Prefer this field for money.
    pub tax_saved_exact: String,
}

impl TryFrom<stateset_core::ExemptionDetails> for ExemptionDetailsOutput {
    type Error = Error;

    fn try_from(e: stateset_core::ExemptionDetails) -> Result<Self> {
        let (amount_exempt, amount_exempt_exact) = money_pair(e.amount_exempt, "amount exempt")?;
        let (tax_saved, tax_saved_exact) = money_pair(e.tax_saved, "tax saved")?;
        Ok(Self {
            exemption_id: e.exemption_id.to_string(),
            exemption_type: format!("{:?}", e.exemption_type).to_lowercase(),
            certificate_number: e.certificate_number,
            amount_exempt,
            amount_exempt_exact,
            tax_saved,
            tax_saved_exact,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct JurisdictionSummaryOutput {
    pub id: String,
    pub name: String,
    pub code: String,
    #[napi(ts_type = "TaxJurisdictionLevel")]
    pub level: String,
    pub total_rate: f64,
    /// @deprecated Use the `totalTaxExact` twin; float money will be removed in 2.0.
    pub total_tax: f64,
    /// Exact base-10 total tax, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_tax_exact: String,
}

impl TryFrom<stateset_core::JurisdictionSummary> for JurisdictionSummaryOutput {
    type Error = Error;

    fn try_from(s: stateset_core::JurisdictionSummary) -> Result<Self> {
        let (total_tax, total_tax_exact) = money_pair(s.total_tax, "jurisdiction total tax")?;
        Ok(Self {
            id: s.id.to_string(),
            name: s.name,
            code: s.code,
            level: format!("{:?}", s.level).to_lowercase(),
            total_rate: to_f64_checked(s.total_rate, "jurisdiction total rate")?,
            total_tax,
            total_tax_exact,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxCalculationOutput {
    pub id: String,
    /// @deprecated Use the `totalTaxExact` twin; float money will be removed in 2.0.
    pub total_tax: f64,
    /// Exact base-10 total tax, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_tax_exact: String,
    /// @deprecated Use the `subtotalExact` twin; float money will be removed in 2.0.
    pub subtotal: f64,
    /// Exact base-10 subtotal, straight from the engine's `Decimal`. Prefer this field for money.
    pub subtotal_exact: String,
    /// @deprecated Use the `totalExact` twin; float money will be removed in 2.0.
    pub total: f64,
    /// Exact base-10 total, straight from the engine's `Decimal`. Prefer this field for money.
    pub total_exact: String,
    /// @deprecated Use the `shippingTaxExact` twin; float money will be removed in 2.0.
    pub shipping_tax: f64,
    /// Exact base-10 shipping tax, straight from the engine's `Decimal`. Prefer this field for money.
    pub shipping_tax_exact: String,
    pub tax_breakdown: Vec<TaxBreakdownOutput>,
    pub line_item_taxes: Vec<LineItemTaxOutput>,
    pub exemptions_applied: bool,
    pub exemption_details: Option<ExemptionDetailsOutput>,
    pub jurisdictions: Vec<JurisdictionSummaryOutput>,
    pub calculated_at: String,
    pub is_estimate: bool,
}

impl TryFrom<stateset_core::TaxCalculationResult> for TaxCalculationOutput {
    type Error = Error;

    fn try_from(r: stateset_core::TaxCalculationResult) -> Result<Self> {
        let (total_tax, total_tax_exact) = money_pair(r.total_tax, "total tax")?;
        let (subtotal, subtotal_exact) = money_pair(r.subtotal, "tax calculation subtotal")?;
        let (total, total_exact) = money_pair(r.total, "tax calculation total")?;
        let (shipping_tax, shipping_tax_exact) = money_pair(r.shipping_tax, "shipping tax")?;
        Ok(Self {
            id: r.id.to_string(),
            total_tax,
            total_tax_exact,
            subtotal,
            subtotal_exact,
            total,
            total_exact,
            shipping_tax,
            shipping_tax_exact,
            tax_breakdown: convert_outputs(r.tax_breakdown)?,
            line_item_taxes: convert_outputs(r.line_item_taxes)?,
            exemptions_applied: r.exemptions_applied,
            exemption_details: convert_optional_output(r.exemption_details)?,
            jurisdictions: convert_outputs(r.jurisdictions)?,
            calculated_at: r.calculated_at.to_rfc3339(),
            is_estimate: r.is_estimate,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct TaxSettingsOutput {
    pub id: String,
    pub enabled: bool,
    #[napi(ts_type = "TaxCalculationMethod")]
    pub calculation_method: String,
    #[napi(ts_type = "TaxCompoundMethod")]
    pub compound_method: String,
    pub tax_shipping: bool,
    pub tax_handling: bool,
    pub tax_gift_wrap: bool,
    #[napi(ts_type = "ProductTaxCategory")]
    pub default_product_category: String,
    pub rounding_mode: String,
    pub decimal_places: i32,
    pub validate_addresses: bool,
    pub tax_provider: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::TaxSettings> for TaxSettingsOutput {
    fn from(s: stateset_core::TaxSettings) -> Self {
        Self {
            id: s.id.to_string(),
            enabled: s.enabled,
            calculation_method: format!("{:?}", s.calculation_method).to_lowercase(),
            compound_method: format!("{:?}", s.compound_method).to_lowercase(),
            tax_shipping: s.tax_shipping,
            tax_handling: s.tax_handling,
            tax_gift_wrap: s.tax_gift_wrap,
            default_product_category: s.default_product_category.as_str().to_string(),
            rounding_mode: s.rounding_mode,
            decimal_places: s.decimal_places,
            validate_addresses: s.validate_addresses,
            tax_provider: s.tax_provider,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct UsStateTaxInfoOutput {
    pub state_code: String,
    pub state_name: String,
    pub state_rate: f64,
    pub has_local_taxes: bool,
    pub origin_based: bool,
    pub tax_shipping: bool,
    pub tax_clothing: bool,
    pub tax_food: bool,
    pub tax_digital: bool,
}

impl TryFrom<stateset_core::UsStateTaxInfo> for UsStateTaxInfoOutput {
    type Error = Error;

    fn try_from(i: stateset_core::UsStateTaxInfo) -> Result<Self> {
        Ok(Self {
            state_code: i.state_code,
            state_name: i.state_name,
            state_rate: to_f64_checked(i.state_rate, "US state tax rate")?,
            has_local_taxes: i.has_local_taxes,
            origin_based: i.origin_based,
            tax_shipping: i.tax_shipping,
            tax_clothing: i.tax_clothing,
            tax_food: i.tax_food,
            tax_digital: i.tax_digital,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct EuVatInfoOutput {
    pub country_code: String,
    pub country_name: String,
    pub standard_rate: f64,
    pub reduced_rate: Option<f64>,
    pub super_reduced_rate: Option<f64>,
    pub parking_rate: Option<f64>,
}

impl TryFrom<stateset_core::EuVatInfo> for EuVatInfoOutput {
    type Error = Error;

    fn try_from(i: stateset_core::EuVatInfo) -> Result<Self> {
        Ok(Self {
            country_code: i.country_code,
            country_name: i.country_name,
            standard_rate: to_f64_checked(i.standard_rate, "EU VAT standard rate")?,
            reduced_rate: optional_to_f64_checked(i.reduced_rate, "EU VAT reduced rate")?,
            super_reduced_rate: optional_to_f64_checked(
                i.super_reduced_rate,
                "EU VAT super reduced rate",
            )?,
            parking_rate: optional_to_f64_checked(i.parking_rate, "EU VAT parking rate")?,
        })
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CanadianTaxInfoOutput {
    pub province_code: String,
    pub province_name: String,
    pub gst_rate: f64,
    pub pst_rate: Option<f64>,
    pub hst_rate: Option<f64>,
    pub qst_rate: Option<f64>,
    pub total_rate: f64,
}

impl TryFrom<stateset_core::CanadianTaxInfo> for CanadianTaxInfoOutput {
    type Error = Error;

    fn try_from(i: stateset_core::CanadianTaxInfo) -> Result<Self> {
        Ok(Self {
            province_code: i.province_code,
            province_name: i.province_name,
            gst_rate: to_f64_checked(i.gst_rate, "Canadian tax GST rate")?,
            pst_rate: optional_to_f64_checked(i.pst_rate, "Canadian tax PST rate")?,
            hst_rate: optional_to_f64_checked(i.hst_rate, "Canadian tax HST rate")?,
            qst_rate: optional_to_f64_checked(i.qst_rate, "Canadian tax QST rate")?,
            total_rate: to_f64_checked(i.total_rate, "Canadian tax total rate")?,
        })
    }
}

// --- Helper Functions ---

pub(crate) fn parse_tax_type(s: &str) -> Result<stateset_core::TaxType> {
    Ok(match s.to_lowercase().as_str() {
        "sales_tax" => stateset_core::TaxType::SalesTax,
        "vat" => stateset_core::TaxType::Vat,
        "gst" => stateset_core::TaxType::Gst,
        "hst" => stateset_core::TaxType::Hst,
        "pst" => stateset_core::TaxType::Pst,
        "qst" => stateset_core::TaxType::Qst,
        "consumption_tax" => stateset_core::TaxType::ConsumptionTax,
        "custom" => stateset_core::TaxType::Custom,
        _ => {
            return Err(unknown_variant(
                "tax type",
                s,
                &["sales_tax", "vat", "gst", "hst", "pst", "qst", "consumption_tax", "custom"],
            ));
        }
    })
}

pub(crate) fn parse_product_tax_category(s: &str) -> Result<stateset_core::ProductTaxCategory> {
    Ok(match s.to_lowercase().as_str() {
        "standard" => stateset_core::ProductTaxCategory::Standard,
        "reduced" => stateset_core::ProductTaxCategory::Reduced,
        "super_reduced" => stateset_core::ProductTaxCategory::SuperReduced,
        "zero_rated" => stateset_core::ProductTaxCategory::ZeroRated,
        "exempt" => stateset_core::ProductTaxCategory::Exempt,
        "digital" => stateset_core::ProductTaxCategory::Digital,
        "clothing" => stateset_core::ProductTaxCategory::Clothing,
        "food" => stateset_core::ProductTaxCategory::Food,
        "prepared_food" => stateset_core::ProductTaxCategory::PreparedFood,
        "medical" => stateset_core::ProductTaxCategory::Medical,
        "educational" => stateset_core::ProductTaxCategory::Educational,
        "luxury" => stateset_core::ProductTaxCategory::Luxury,
        _ => {
            return Err(unknown_variant(
                "product tax category",
                s,
                &[
                    "standard",
                    "reduced",
                    "super_reduced",
                    "zero_rated",
                    "exempt",
                    "digital",
                    "clothing",
                    "food",
                    "prepared_food",
                    "medical",
                    "educational",
                    "luxury",
                ],
            ));
        }
    })
}

pub(crate) fn parse_jurisdiction_level(s: &str) -> Result<stateset_core::JurisdictionLevel> {
    Ok(match s.to_lowercase().as_str() {
        "country" => stateset_core::JurisdictionLevel::Country,
        "state" => stateset_core::JurisdictionLevel::State,
        "county" => stateset_core::JurisdictionLevel::County,
        "city" => stateset_core::JurisdictionLevel::City,
        "district" => stateset_core::JurisdictionLevel::District,
        "special" => stateset_core::JurisdictionLevel::Special,
        _ => {
            return Err(unknown_variant(
                "jurisdiction level",
                s,
                &["country", "state", "county", "city", "district", "special"],
            ));
        }
    })
}

pub(crate) fn parse_exemption_type(s: &str) -> Result<stateset_core::ExemptionType> {
    Ok(match s.to_lowercase().as_str() {
        "resale" => stateset_core::ExemptionType::Resale,
        "non_profit" | "nonprofit" => stateset_core::ExemptionType::NonProfit,
        "government" => stateset_core::ExemptionType::Government,
        "educational" => stateset_core::ExemptionType::Educational,
        "religious" => stateset_core::ExemptionType::Religious,
        "medical" => stateset_core::ExemptionType::Medical,
        "manufacturing" => stateset_core::ExemptionType::Manufacturing,
        "agricultural" => stateset_core::ExemptionType::Agricultural,
        "export" => stateset_core::ExemptionType::Export,
        "diplomatic" => stateset_core::ExemptionType::Diplomatic,
        "other" => stateset_core::ExemptionType::Other,
        _ => {
            return Err(unknown_variant(
                "exemption type",
                s,
                &[
                    "resale",
                    "nonprofit",
                    "non_profit",
                    "government",
                    "educational",
                    "religious",
                    "medical",
                    "manufacturing",
                    "agricultural",
                    "export",
                    "diplomatic",
                    "other",
                ],
            ));
        }
    })
}

pub(crate) fn parse_calculation_method(s: &str) -> Result<stateset_core::TaxCalculationMethod> {
    Ok(match s.to_lowercase().as_str() {
        "exclusive" => stateset_core::TaxCalculationMethod::Exclusive,
        "inclusive" => stateset_core::TaxCalculationMethod::Inclusive,
        _ => return Err(unknown_variant("tax calculation method", s, &["exclusive", "inclusive"])),
    })
}

pub(crate) fn parse_compound_method(s: &str) -> Result<stateset_core::TaxCompoundMethod> {
    Ok(match s.to_lowercase().as_str() {
        "combined" => stateset_core::TaxCompoundMethod::Combined,
        "compound" => stateset_core::TaxCompoundMethod::Compound,
        "separate" => stateset_core::TaxCompoundMethod::Separate,
        _ => {
            return Err(unknown_variant(
                "tax compound method",
                s,
                &["combined", "compound", "separate"],
            ));
        }
    })
}

// --- Tax Struct ---

#[napi]
pub struct Tax {
    pub(crate) commerce: Handle,
}

#[napi]
impl Tax {
    // ========================================================================
    // Tax Calculation
    // ========================================================================

    /// Calculate tax for a transaction
    #[napi]
    pub async fn calculate(&self, input: TaxCalculationInput) -> Result<TaxCalculationOutput> {
        let commerce = self.commerce.get()?;

        let line_items: Vec<stateset_core::TaxLineItem> = input
            .line_items
            .into_iter()
            .map(|item| {
                Ok(stateset_core::TaxLineItem {
                    id: item.id,
                    sku: item.sku,
                    product_id: parse_optional_id::<uuid::Uuid>(item.product_id, "product")?
                        .map(ProductId::from),
                    quantity: decimal_from_f64(item.quantity, "tax line item quantity")?,
                    unit_price: decimal_from_f64(item.unit_price, "tax line item unit price")?,
                    discount_amount: optional_decimal_from_f64(
                        item.discount_amount,
                        "tax line item discount amount",
                    )?
                    .unwrap_or_default(),
                    tax_category: item
                        .tax_category
                        .map(|s| parse_product_tax_category(&s))
                        .transpose()?
                        .unwrap_or_default(),
                    tax_code: item.tax_code,
                    description: item.description,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let shipping_address = stateset_core::TaxAddress {
            line1: input.shipping_address.line1,
            line2: input.shipping_address.line2,
            city: input.shipping_address.city,
            state: input.shipping_address.state,
            postal_code: input.shipping_address.postal_code,
            country: input.shipping_address.country,
        };

        let billing_address = input.billing_address.map(|addr| stateset_core::TaxAddress {
            line1: addr.line1,
            line2: addr.line2,
            city: addr.city,
            state: addr.state,
            postal_code: addr.postal_code,
            country: addr.country,
        });

        let request = stateset_core::TaxCalculationRequest {
            line_items,
            shipping_address,
            billing_address,
            customer_id: parse_optional_id::<uuid::Uuid>(input.customer_id, "customer")?,
            shipping_amount: optional_decimal_from_f64(
                input.shipping_amount,
                "tax shipping amount",
            )?,
            currency: parse_optional_currency(input.currency)?.unwrap_or(CurrencyCode::USD),
            transaction_date: parse_optional_date(input.transaction_date, "transaction date")?,
            prices_include_tax: input.prices_include_tax.unwrap_or(false),
        };

        let result = commerce
            .tax()
            .calculate(request)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to calculate tax", e))?;

        convert_output(result)
    }

    /// Calculate tax for a single item
    #[napi]
    pub async fn calculate_for_item(
        &self,
        unit_price: f64,
        quantity: f64,
        category: Option<String>,
        shipping_address: TaxAddressInput,
    ) -> Result<f64> {
        let commerce = self.commerce.get()?;

        let address = stateset_core::TaxAddress {
            line1: shipping_address.line1,
            line2: shipping_address.line2,
            city: shipping_address.city,
            state: shipping_address.state,
            postal_code: shipping_address.postal_code,
            country: shipping_address.country,
        };

        let tax = commerce
            .tax()
            .calculate_for_item(
                decimal_from_f64(unit_price, "tax unit price")?,
                decimal_from_f64(quantity, "tax quantity")?,
                category.map(|s| parse_product_tax_category(&s)).transpose()?.unwrap_or_default(),
                &address,
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to calculate tax", e))?;

        to_f64_checked(tax, "tax amount")
    }

    /// Get the effective tax rate for an address and category
    #[napi]
    pub async fn get_effective_rate(
        &self,
        address: TaxAddressInput,
        category: Option<String>,
    ) -> Result<f64> {
        let commerce = self.commerce.get()?;

        let tax_address = stateset_core::TaxAddress {
            line1: address.line1,
            line2: address.line2,
            city: address.city,
            state: address.state,
            postal_code: address.postal_code,
            country: address.country,
        };

        let rate = commerce
            .tax()
            .get_effective_rate(
                &tax_address,
                category.map(|s| parse_product_tax_category(&s)).transpose()?.unwrap_or_default(),
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get rate", e))?;

        to_f64_checked(rate, "tax rate")
    }

    // ========================================================================
    // Jurisdiction Operations
    // ========================================================================

    /// Get a jurisdiction by ID
    #[napi]
    pub async fn get_jurisdiction(&self, id: String) -> Result<Option<TaxJurisdictionOutput>> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let jurisdiction = commerce
            .tax()
            .get_jurisdiction(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get jurisdiction", e))?;

        Ok(jurisdiction.map(|j| j.into()))
    }

    /// Get a jurisdiction by code
    #[napi]
    pub async fn get_jurisdiction_by_code(
        &self,
        code: String,
    ) -> Result<Option<TaxJurisdictionOutput>> {
        let commerce = self.commerce.get()?;

        let jurisdiction = commerce
            .tax()
            .get_jurisdiction_by_code(&code)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get jurisdiction", e))?;

        Ok(jurisdiction.map(|j| j.into()))
    }

    /// List jurisdictions with optional filtering
    #[napi]
    pub async fn list_jurisdictions(
        &self,
        filter: Option<JurisdictionFilterInput>,
    ) -> Result<Vec<TaxJurisdictionOutput>> {
        let commerce = self.commerce.get()?;

        let f = filter.unwrap_or_default();
        let core_filter = stateset_core::TaxJurisdictionFilter {
            country_code: f.country_code,
            state_code: f.state_code,
            level: f.level.map(|s| parse_jurisdiction_level(&s)).transpose()?,
            active_only: f.active_only.unwrap_or(false),
            ..Default::default()
        };

        let jurisdictions = commerce
            .tax()
            .list_jurisdictions(core_filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list jurisdictions", e))?;

        Ok(jurisdictions.into_iter().map(|j| j.into()).collect())
    }

    /// Create a new jurisdiction
    #[napi]
    pub async fn create_jurisdiction(
        &self,
        input: CreateJurisdictionInput,
    ) -> Result<TaxJurisdictionOutput> {
        let commerce = self.commerce.get()?;

        let create = stateset_core::CreateTaxJurisdiction {
            parent_id: parse_optional_id::<uuid::Uuid>(input.parent_id, "parent")?,
            name: input.name,
            code: input.code,
            level: input
                .level
                .map(|s| parse_jurisdiction_level(&s))
                .transpose()?
                .unwrap_or_default(),
            country_code: input.country_code,
            state_code: input.state_code,
            county: input.county,
            city: input.city,
            postal_codes: input.postal_codes.unwrap_or_default(),
        };

        let jurisdiction = commerce
            .tax()
            .create_jurisdiction(create)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create jurisdiction", e))?;

        Ok(jurisdiction.into())
    }

    // ========================================================================
    // Tax Rate Operations
    // ========================================================================

    /// Get a tax rate by ID
    #[napi]
    pub async fn get_rate(&self, id: String) -> Result<Option<TaxRateOutput>> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let rate = commerce
            .tax()
            .get_rate(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get rate", e))?;

        convert_optional_output(rate)
    }

    /// List tax rates with optional filtering
    #[napi]
    pub async fn list_rates(
        &self,
        filter: Option<TaxRateFilterInput>,
    ) -> Result<Vec<TaxRateOutput>> {
        let commerce = self.commerce.get()?;

        let f = filter.unwrap_or_default();
        let core_filter = stateset_core::TaxRateFilter {
            jurisdiction_id: parse_optional_id::<uuid::Uuid>(f.jurisdiction_id, "jurisdiction")?,
            tax_type: f.tax_type.map(|s| parse_tax_type(&s)).transpose()?,
            product_category: f
                .product_category
                .map(|s| parse_product_tax_category(&s))
                .transpose()?,
            active_only: f.active_only.unwrap_or(false),
            effective_date: parse_optional_date(f.effective_date, "effective date")?,
            ..Default::default()
        };

        let rates = commerce
            .tax()
            .list_rates(core_filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list rates", e))?;

        convert_outputs(rates)
    }

    /// Create a new tax rate
    #[napi]
    pub async fn create_rate(&self, input: CreateTaxRateInput) -> Result<TaxRateOutput> {
        let commerce = self.commerce.get()?;

        let jurisdiction_id = uuid::Uuid::parse_str(&input.jurisdiction_id)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid jurisdiction UUID", e))?;

        let effective_from = chrono::NaiveDate::parse_from_str(&input.effective_from, "%Y-%m-%d")
            .map_err(|e| wrap(ErrCode::Validation, "Invalid date format", e))?;

        let create = stateset_core::CreateTaxRate {
            jurisdiction_id,
            tax_type: input.tax_type.map(|s| parse_tax_type(&s)).transpose()?.unwrap_or_default(),
            product_category: input
                .product_category
                .map(|s| parse_product_tax_category(&s))
                .transpose()?
                .unwrap_or_default(),
            rate: decimal_from_f64(input.rate, "tax rate")?,
            name: input.name,
            description: input.description,
            is_compound: input.is_compound.unwrap_or(false),
            priority: input.priority.unwrap_or(0),
            threshold_min: optional_decimal_from_f64(input.threshold_min, "tax threshold min")?,
            threshold_max: optional_decimal_from_f64(input.threshold_max, "tax threshold max")?,
            fixed_amount: optional_decimal_from_f64(input.fixed_amount, "tax fixed amount")?,
            effective_from,
            effective_to: parse_optional_date(input.effective_to, "effective to")?,
        };

        let rate = commerce
            .tax()
            .create_rate(create)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create rate", e))?;

        convert_output(rate)
    }

    // ========================================================================
    // Exemption Operations
    // ========================================================================

    /// Get an exemption by ID
    #[napi]
    pub async fn get_exemption(&self, id: String) -> Result<Option<TaxExemptionOutput>> {
        let commerce = self.commerce.get()?;
        let uuid =
            uuid::Uuid::parse_str(&id).map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let exemption = commerce
            .tax()
            .get_exemption(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get exemption", e))?;

        Ok(exemption.map(|e| e.into()))
    }

    /// Get exemptions for a customer
    #[napi]
    pub async fn get_customer_exemptions(
        &self,
        customer_id: String,
    ) -> Result<Vec<TaxExemptionOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = uuid::Uuid::parse_str(&customer_id)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let exemptions = commerce
            .tax()
            .get_customer_exemptions(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get exemptions", e))?;

        Ok(exemptions.into_iter().map(|e| e.into()).collect())
    }

    /// Create a tax exemption
    #[napi]
    pub async fn create_exemption(
        &self,
        input: CreateExemptionInput,
    ) -> Result<TaxExemptionOutput> {
        let commerce = self.commerce.get()?;

        let customer_id = uuid::Uuid::parse_str(&input.customer_id)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid customer UUID", e))?;

        let effective_from = chrono::NaiveDate::parse_from_str(&input.effective_from, "%Y-%m-%d")
            .map_err(|e| wrap(ErrCode::Validation, "Invalid date format", e))?;

        let create = stateset_core::CreateTaxExemption {
            customer_id,
            exemption_type: parse_exemption_type(&input.exemption_type)?,
            certificate_number: input.certificate_number,
            issuing_authority: input.issuing_authority,
            jurisdiction_ids: parse_id_list::<uuid::Uuid>(input.jurisdiction_ids, "jurisdiction")?
                .unwrap_or_default(),
            exempt_categories: input
                .exempt_categories
                .unwrap_or_default()
                .into_iter()
                .map(|s| parse_product_tax_category(&s))
                .collect::<Result<Vec<_>>>()?,
            effective_from,
            expires_at: parse_optional_date(input.expires_at, "expires at")?,
            notes: input.notes,
        };

        let exemption = commerce
            .tax()
            .create_exemption(create)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create exemption", e))?;

        Ok(exemption.into())
    }

    /// Check if a customer is tax exempt
    #[napi]
    pub async fn customer_is_exempt(&self, customer_id: String) -> Result<bool> {
        let commerce = self.commerce.get()?;
        let uuid = uuid::Uuid::parse_str(&customer_id)
            .map_err(|e| wrap(ErrCode::Validation, "Invalid UUID", e))?;

        let is_exempt = commerce
            .tax()
            .customer_is_exempt(uuid)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to check exemption", e))?;

        Ok(is_exempt)
    }

    // ========================================================================
    // Settings Operations
    // ========================================================================

    /// Get tax settings
    #[napi]
    pub async fn get_settings(&self) -> Result<TaxSettingsOutput> {
        let commerce = self.commerce.get()?;

        let settings = commerce
            .tax()
            .get_settings()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get settings", e))?;

        Ok(settings.into())
    }

    /// Update tax settings
    #[napi]
    pub async fn update_settings(&self, input: TaxSettingsInput) -> Result<TaxSettingsOutput> {
        let commerce = self.commerce.get()?;

        let mut settings = commerce
            .tax()
            .get_settings()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get settings", e))?;

        if let Some(enabled) = input.enabled {
            settings.enabled = enabled;
        }
        if let Some(method) = input.calculation_method {
            settings.calculation_method = parse_calculation_method(&method)?;
        }
        if let Some(method) = input.compound_method {
            settings.compound_method = parse_compound_method(&method)?;
        }
        if let Some(tax_shipping) = input.tax_shipping {
            settings.tax_shipping = tax_shipping;
        }
        if let Some(tax_handling) = input.tax_handling {
            settings.tax_handling = tax_handling;
        }
        if let Some(tax_gift_wrap) = input.tax_gift_wrap {
            settings.tax_gift_wrap = tax_gift_wrap;
        }
        if let Some(category) = input.default_product_category {
            settings.default_product_category = parse_product_tax_category(&category)?;
        }
        if let Some(mode) = input.rounding_mode {
            settings.rounding_mode = mode;
        }
        if let Some(places) = input.decimal_places {
            settings.decimal_places = places;
        }
        if let Some(validate) = input.validate_addresses {
            settings.validate_addresses = validate;
        }
        if let Some(provider) = input.tax_provider {
            settings.tax_provider = Some(provider);
        }

        let updated = commerce
            .tax()
            .update_settings(settings)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update settings", e))?;

        Ok(updated.into())
    }

    /// Enable or disable tax calculation
    #[napi]
    pub async fn set_enabled(&self, enabled: bool) -> Result<TaxSettingsOutput> {
        let commerce = self.commerce.get()?;

        let settings = commerce
            .tax()
            .set_enabled(enabled)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to update settings", e))?;

        Ok(settings.into())
    }

    /// Check if tax calculation is enabled
    #[napi]
    pub async fn is_enabled(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;

        let enabled = commerce
            .tax()
            .is_enabled()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to check settings", e))?;

        Ok(enabled)
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Get US state tax information
    #[napi]
    pub fn get_us_state_info(state_code: String) -> Result<Option<UsStateTaxInfoOutput>> {
        guard(|| convert_optional_output(stateset_core::get_us_state_tax_info(&state_code)))
    }

    /// Get EU VAT information
    #[napi]
    pub fn get_eu_vat_info(country_code: String) -> Result<Option<EuVatInfoOutput>> {
        guard(|| convert_optional_output(stateset_core::get_eu_vat_info(&country_code)))
    }

    /// Get Canadian tax information
    #[napi]
    pub fn get_canadian_tax_info(province_code: String) -> Result<Option<CanadianTaxInfoOutput>> {
        guard(|| convert_optional_output(stateset_core::get_canadian_tax_info(&province_code)))
    }

    /// Check if a country is in the EU
    #[napi]
    pub fn is_eu_country(country_code: String) -> Result<bool> {
        guard(|| Ok(stateset_core::is_eu_member(&country_code)))
    }
}
