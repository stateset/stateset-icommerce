//! Tax calculation operations
//!
//! Provides multi-jurisdiction tax calculation with support for:
//! - US sales tax (state, county, city)
//! - EU VAT (standard, reduced, zero-rated)
//! - Canadian GST/HST/PST/QST
//! - Customer exemptions (resale, non-profit, etc.)

use chrono::NaiveDate;
use rust_decimal::Decimal;
use stateset_core::{
    CreateTaxExemption, CreateTaxJurisdiction, CreateTaxRate, ProductTaxCategory,
    Result, TaxAddress, TaxCalculationRequest, TaxCalculationResult, TaxExemption, TaxJurisdiction,
    TaxJurisdictionFilter, TaxLineItem, TaxRate, TaxRateFilter, TaxSettings,
};
use stateset_db::sqlite::SqliteTaxRepository;
use uuid::Uuid;

/// Tax calculation and management interface.
///
/// Provides tax rate management, exemption handling, and tax calculation
/// for commerce operations.
///
/// # Example
///
/// ```rust,no_run
/// use stateset_embedded::{Commerce, TaxAddress, TaxCalculationRequest, TaxLineItem, ProductTaxCategory};
/// use rust_decimal_macros::dec;
///
/// let commerce = Commerce::new("./store.db")?;
///
/// // Calculate tax for a transaction
/// let result = commerce.tax().calculate(TaxCalculationRequest {
///     line_items: vec![TaxLineItem {
///         id: "item-1".into(),
///         quantity: dec!(2),
///         unit_price: dec!(29.99),
///         tax_category: ProductTaxCategory::Standard,
///         ..Default::default()
///     }],
///     shipping_address: TaxAddress {
///         country: "US".into(),
///         state: Some("CA".into()),
///         postal_code: Some("90210".into()),
///         ..Default::default()
///     },
///     ..Default::default()
/// })?;
///
/// println!("Subtotal: ${}", result.subtotal);
/// println!("Tax: ${}", result.total_tax);
/// println!("Total: ${}", result.total);
/// # Ok::<(), stateset_embedded::CommerceError>(())
/// ```
pub struct Tax {
    repo: SqliteTaxRepository,
}

impl Tax {
    pub(crate) fn new(repo: SqliteTaxRepository) -> Self {
        Self { repo }
    }

    // ========================================================================
    // Tax Calculation
    // ========================================================================

    /// Calculate tax for a transaction.
    ///
    /// Given line items and shipping address, calculates applicable taxes
    /// based on jurisdiction rules, product categories, and customer exemptions.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// use rust_decimal_macros::dec;
    ///
    /// # let commerce = Commerce::new(":memory:")?;
    /// let result = commerce.tax().calculate(TaxCalculationRequest {
    ///     line_items: vec![TaxLineItem {
    ///         id: "item-1".into(),
    ///         quantity: dec!(2),
    ///         unit_price: dec!(29.99),
    ///         tax_category: ProductTaxCategory::Standard,
    ///         ..Default::default()
    ///     }],
    ///     shipping_address: TaxAddress {
    ///         country: "US".into(),
    ///         state: Some("CA".into()),
    ///         ..Default::default()
    ///     },
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Tax breakdown:");
    /// for breakdown in &result.tax_breakdown {
    ///     println!("  {}: {}% = ${}", breakdown.rate_name, breakdown.rate * dec!(100), breakdown.tax_amount);
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn calculate(&self, request: TaxCalculationRequest) -> Result<TaxCalculationResult> {
        self.repo.calculate_tax(request)
    }

    /// Calculate tax for a single item (convenience method).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// use rust_decimal_macros::dec;
    ///
    /// # let commerce = Commerce::new(":memory:")?;
    /// let tax = commerce.tax().calculate_for_item(
    ///     dec!(99.99),                    // unit price
    ///     dec!(2),                        // quantity
    ///     ProductTaxCategory::Standard,    // category
    ///     &TaxAddress {
    ///         country: "US".into(),
    ///         state: Some("TX".into()),
    ///         ..Default::default()
    ///     },
    /// )?;
    ///
    /// println!("Tax: ${}", tax);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn calculate_for_item(
        &self,
        unit_price: Decimal,
        quantity: Decimal,
        category: ProductTaxCategory,
        shipping_address: &TaxAddress,
    ) -> Result<Decimal> {
        let request = TaxCalculationRequest {
            line_items: vec![TaxLineItem {
                id: "single".into(),
                quantity,
                unit_price,
                tax_category: category,
                ..Default::default()
            }],
            shipping_address: shipping_address.clone(),
            ..Default::default()
        };

        let result = self.calculate(request)?;
        Ok(result.total_tax)
    }

    /// Get the effective tax rate for an address and category.
    ///
    /// Returns the combined tax rate that would apply to a standard purchase.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let rate = commerce.tax().get_effective_rate(
    ///     &TaxAddress {
    ///         country: "US".into(),
    ///         state: Some("CA".into()),
    ///         city: Some("Los Angeles".into()),
    ///         ..Default::default()
    ///     },
    ///     ProductTaxCategory::Standard,
    /// )?;
    ///
    /// println!("Effective tax rate: {}%", rate * rust_decimal_macros::dec!(100));
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn get_effective_rate(
        &self,
        address: &TaxAddress,
        category: ProductTaxCategory,
    ) -> Result<Decimal> {
        let today = chrono::Utc::now().date_naive();
        let rates = self.repo.get_rates_for_address(address, category, today)?;

        let total_rate: Decimal = rates.iter()
            .filter(|r| !r.is_compound)
            .map(|r| r.rate)
            .sum();

        Ok(total_rate)
    }

    // ========================================================================
    // Jurisdiction Operations
    // ========================================================================

    /// Get a tax jurisdiction by ID.
    pub fn get_jurisdiction(&self, id: Uuid) -> Result<Option<TaxJurisdiction>> {
        self.repo.get_jurisdiction(id)
    }

    /// Get a tax jurisdiction by code (e.g., "US-CA").
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// if let Some(jurisdiction) = commerce.tax().get_jurisdiction_by_code("US-CA")? {
    ///     println!("{}: {}", jurisdiction.code, jurisdiction.name);
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn get_jurisdiction_by_code(&self, code: &str) -> Result<Option<TaxJurisdiction>> {
        self.repo.get_jurisdiction_by_code(code)
    }

    /// List tax jurisdictions with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// // List all US state jurisdictions
    /// let states = commerce.tax().list_jurisdictions(TaxJurisdictionFilter {
    ///     country_code: Some("US".into()),
    ///     level: Some(JurisdictionLevel::State),
    ///     active_only: true,
    ///     ..Default::default()
    /// })?;
    ///
    /// for state in states {
    ///     println!("{}: {}", state.code, state.name);
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn list_jurisdictions(&self, filter: TaxJurisdictionFilter) -> Result<Vec<TaxJurisdiction>> {
        self.repo.list_jurisdictions(filter)
    }

    /// Create a new tax jurisdiction.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let jurisdiction = commerce.tax().create_jurisdiction(CreateTaxJurisdiction {
    ///     name: "Los Angeles".into(),
    ///     code: "US-CA-LA".into(),
    ///     level: JurisdictionLevel::City,
    ///     country_code: "US".into(),
    ///     state_code: Some("CA".into()),
    ///     city: Some("Los Angeles".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn create_jurisdiction(&self, input: CreateTaxJurisdiction) -> Result<TaxJurisdiction> {
        self.repo.create_jurisdiction(input)
    }

    // ========================================================================
    // Tax Rate Operations
    // ========================================================================

    /// Get a tax rate by ID.
    pub fn get_rate(&self, id: Uuid) -> Result<Option<TaxRate>> {
        self.repo.get_rate(id)
    }

    /// List tax rates with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// // Get all active rates for a jurisdiction
    /// let rates = commerce.tax().list_rates(TaxRateFilter {
    ///     jurisdiction_id: Some(jurisdiction_id),
    ///     active_only: true,
    ///     ..Default::default()
    /// })?;
    ///
    /// for rate in rates {
    ///     println!("{}: {}%", rate.name, rate.rate * rust_decimal_macros::dec!(100));
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn list_rates(&self, filter: TaxRateFilter) -> Result<Vec<TaxRate>> {
        self.repo.list_rates(filter)
    }

    /// Create a new tax rate.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// use rust_decimal_macros::dec;
    ///
    /// # let commerce = Commerce::new(":memory:")?;
    /// let rate = commerce.tax().create_rate(CreateTaxRate {
    ///     jurisdiction_id,
    ///     tax_type: TaxType::SalesTax,
    ///     product_category: ProductTaxCategory::Standard,
    ///     rate: dec!(0.0825),  // 8.25%
    ///     name: "City Sales Tax".into(),
    ///     effective_from: chrono::Utc::now().date_naive(),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn create_rate(&self, input: CreateTaxRate) -> Result<TaxRate> {
        self.repo.create_rate(input)
    }

    /// Get rates for a specific address and product category.
    ///
    /// Returns all applicable tax rates sorted by priority.
    pub fn get_rates_for_address(
        &self,
        address: &TaxAddress,
        category: ProductTaxCategory,
        date: NaiveDate,
    ) -> Result<Vec<TaxRate>> {
        self.repo.get_rates_for_address(address, category, date)
    }

    // ========================================================================
    // Exemption Operations
    // ========================================================================

    /// Get an exemption by ID.
    pub fn get_exemption(&self, id: Uuid) -> Result<Option<TaxExemption>> {
        self.repo.get_exemption(id)
    }

    /// Get active exemptions for a customer.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let exemptions = commerce.tax().get_customer_exemptions(customer_id)?;
    ///
    /// for exemption in exemptions {
    ///     println!("Type: {:?}", exemption.exemption_type);
    ///     if let Some(cert) = &exemption.certificate_number {
    ///         println!("Certificate: {}", cert);
    ///     }
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn get_customer_exemptions(&self, customer_id: Uuid) -> Result<Vec<TaxExemption>> {
        self.repo.get_customer_exemptions(customer_id)
    }

    /// Create a tax exemption for a customer.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let exemption = commerce.tax().create_exemption(CreateTaxExemption {
    ///     customer_id,
    ///     exemption_type: ExemptionType::Resale,
    ///     certificate_number: Some("RS-12345".into()),
    ///     issuing_authority: Some("California".into()),
    ///     effective_from: chrono::Utc::now().date_naive(),
    ///     expires_at: Some(chrono::Utc::now().date_naive() + chrono::Duration::days(365)),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn create_exemption(&self, input: CreateTaxExemption) -> Result<TaxExemption> {
        self.repo.create_exemption(input)
    }

    /// Check if a customer has an active exemption.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// if commerce.tax().customer_is_exempt(customer_id)? {
    ///     println!("Customer has tax exemption");
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn customer_is_exempt(&self, customer_id: Uuid) -> Result<bool> {
        let exemptions = self.get_customer_exemptions(customer_id)?;
        Ok(!exemptions.is_empty())
    }

    // ========================================================================
    // Settings Operations
    // ========================================================================

    /// Get tax settings.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let settings = commerce.tax().get_settings()?;
    ///
    /// println!("Tax enabled: {}", settings.enabled);
    /// println!("Tax shipping: {}", settings.tax_shipping);
    /// println!("Calculation method: {:?}", settings.calculation_method);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn get_settings(&self) -> Result<TaxSettings> {
        self.repo.get_settings()
    }

    /// Update tax settings.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let mut settings = commerce.tax().get_settings()?;
    /// settings.tax_shipping = true;
    /// settings.decimal_places = 2;
    ///
    /// commerce.tax().update_settings(settings)?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn update_settings(&self, settings: TaxSettings) -> Result<TaxSettings> {
        self.repo.update_settings(settings)
    }

    /// Enable or disable tax calculation.
    pub fn set_enabled(&self, enabled: bool) -> Result<TaxSettings> {
        let mut settings = self.get_settings()?;
        settings.enabled = enabled;
        self.update_settings(settings)
    }

    /// Check if tax calculation is enabled.
    pub fn is_enabled(&self) -> Result<bool> {
        let settings = self.get_settings()?;
        Ok(settings.enabled)
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Get US state tax information.
    ///
    /// Returns pre-configured tax information for a US state.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// if let Some(info) = stateset_core::get_us_state_tax_info("CA") {
    ///     println!("California state rate: {}%", info.state_rate * rust_decimal_macros::dec!(100));
    ///     println!("Has local taxes: {}", info.has_local_taxes);
    ///     println!("Tax shipping: {}", info.tax_shipping);
    /// }
    /// ```
    pub fn get_us_state_info(state_code: &str) -> Option<stateset_core::UsStateTaxInfo> {
        stateset_core::get_us_state_tax_info(state_code)
    }

    /// Get EU VAT information.
    ///
    /// Returns pre-configured VAT rates for an EU country.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// if let Some(info) = stateset_core::get_eu_vat_info("DE") {
    ///     println!("Germany standard VAT: {}%", info.standard_rate * rust_decimal_macros::dec!(100));
    ///     if let Some(reduced) = info.reduced_rate {
    ///         println!("Reduced rate: {}%", reduced * rust_decimal_macros::dec!(100));
    ///     }
    /// }
    /// ```
    pub fn get_eu_vat_info(country_code: &str) -> Option<stateset_core::EuVatInfo> {
        stateset_core::get_eu_vat_info(country_code)
    }

    /// Get Canadian tax information.
    ///
    /// Returns pre-configured tax rates for a Canadian province.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// if let Some(info) = stateset_core::get_canadian_tax_info("ON") {
    ///     println!("Ontario total rate: {}%", info.total_rate * rust_decimal_macros::dec!(100));
    ///     if let Some(hst) = info.hst_rate {
    ///         println!("HST: {}%", hst * rust_decimal_macros::dec!(100));
    ///     }
    /// }
    /// ```
    pub fn get_canadian_tax_info(province_code: &str) -> Option<stateset_core::CanadianTaxInfo> {
        stateset_core::get_canadian_tax_info(province_code)
    }

    /// Check if a country is in the EU.
    pub fn is_eu_country(country_code: &str) -> bool {
        stateset_core::is_eu_member(country_code)
    }
}
