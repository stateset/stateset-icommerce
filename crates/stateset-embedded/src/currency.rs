//! Multi-currency operations
//!
//! Provides exchange rate management, currency conversion, and multi-currency
//! support for international commerce.

use rust_decimal::Decimal;
use stateset_core::{
    ConversionResult, ConvertCurrency, Currency, ExchangeRate, ExchangeRateFilter, Result,
    SetExchangeRate, StoreCurrencySettings,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Currency operations interface.
///
/// Provides exchange rate management and currency conversion for
/// multi-currency commerce operations.
///
/// # Example
///
/// ```rust,no_run
/// use stateset_embedded::{Commerce, Currency, ConvertCurrency};
/// use rust_decimal_macros::dec;
///
/// let commerce = Commerce::new("./store.db")?;
///
/// // Get exchange rate
/// if let Some(rate) = commerce.currency().get_rate(Currency::USD, Currency::EUR)? {
///     println!("USD to EUR: {}", rate.rate);
/// }
///
/// // Convert currency
/// let result = commerce.currency().convert(ConvertCurrency {
///     from: Currency::USD,
///     to: Currency::EUR,
///     amount: dec!(100.00),
/// })?;
/// println!("$100 USD = €{} EUR", result.converted_amount);
/// # Ok::<(), stateset_embedded::CommerceError>(())
/// ```
pub struct CurrencyOps {
    db: Arc<dyn Database>,
}

impl CurrencyOps {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Exchange Rate Operations
    // ========================================================================

    /// Get exchange rate between two currencies.
    ///
    /// Returns the current rate to convert from the base currency to the quote
    /// currency. If no direct rate exists, attempts to find an inverse rate.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// // Get USD to EUR rate
    /// if let Some(rate) = commerce.currency().get_rate(Currency::USD, Currency::EUR)? {
    ///     println!("1 USD = {} EUR", rate.rate);
    ///     println!("Rate source: {}", rate.source);
    /// } else {
    ///     println!("No rate found for USD/EUR");
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn get_rate(&self, from: Currency, to: Currency) -> Result<Option<ExchangeRate>> {
        self.db.currency().get_rate(from, to)
    }

    /// Get all exchange rates for a base currency.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// // Get all rates from USD
    /// let rates = commerce.currency().get_rates_for(Currency::USD)?;
    /// for rate in rates {
    ///     println!("1 USD = {} {}", rate.rate, rate.quote_currency);
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn get_rates_for(&self, base: Currency) -> Result<Vec<ExchangeRate>> {
        self.db.currency().get_rates_for(base)
    }

    /// List exchange rates with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// // List all rates
    /// let rates = commerce.currency().list_rates(ExchangeRateFilter::default())?;
    ///
    /// // Filter by base currency
    /// let usd_rates = commerce.currency().list_rates(ExchangeRateFilter {
    ///     base_currency: Some(Currency::USD),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn list_rates(&self, filter: ExchangeRateFilter) -> Result<Vec<ExchangeRate>> {
        self.db.currency().list_rates(filter)
    }

    /// Set an exchange rate.
    ///
    /// Creates or updates the exchange rate between two currencies.
    /// The rate is also recorded in history for auditing.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// use rust_decimal_macros::dec;
    ///
    /// # let commerce = Commerce::new(":memory:")?;
    /// // Set USD to EUR rate
    /// let rate = commerce.currency().set_rate(SetExchangeRate {
    ///     base_currency: Currency::USD,
    ///     quote_currency: Currency::EUR,
    ///     rate: dec!(0.92),
    ///     source: Some("manual".into()),
    /// })?;
    ///
    /// println!("Rate set: 1 USD = {} EUR", rate.rate);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn set_rate(&self, input: SetExchangeRate) -> Result<ExchangeRate> {
        self.db.currency().set_rate(input)
    }

    /// Set multiple exchange rates at once.
    ///
    /// Useful for bulk updates from external rate providers.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// use rust_decimal_macros::dec;
    ///
    /// # let commerce = Commerce::new(":memory:")?;
    /// let rates = commerce.currency().set_rates(vec![
    ///     SetExchangeRate {
    ///         base_currency: Currency::USD,
    ///         quote_currency: Currency::EUR,
    ///         rate: dec!(0.92),
    ///         source: Some("api".into()),
    ///     },
    ///     SetExchangeRate {
    ///         base_currency: Currency::USD,
    ///         quote_currency: Currency::GBP,
    ///         rate: dec!(0.79),
    ///         source: Some("api".into()),
    ///     },
    /// ])?;
    ///
    /// println!("Updated {} exchange rates", rates.len());
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn set_rates(&self, rates: Vec<SetExchangeRate>) -> Result<Vec<ExchangeRate>> {
        self.db.currency().set_rates(rates)
    }

    /// Delete an exchange rate by ID.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # use uuid::Uuid;
    /// # let commerce = Commerce::new(":memory:")?;
    /// # let rate_id = Uuid::new_v4();
    /// commerce.currency().delete_rate(rate_id)?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn delete_rate(&self, id: Uuid) -> Result<()> {
        self.db.currency().delete_rate(id)
    }

    // ========================================================================
    // Currency Conversion
    // ========================================================================

    /// Convert an amount from one currency to another.
    ///
    /// Uses the current exchange rate to convert the amount. If the rate
    /// is not found directly, attempts to use the inverse rate.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// use rust_decimal_macros::dec;
    ///
    /// # let commerce = Commerce::new(":memory:")?;
    /// let result = commerce.currency().convert(ConvertCurrency {
    ///     from: Currency::USD,
    ///     to: Currency::EUR,
    ///     amount: dec!(100.00),
    /// })?;
    ///
    /// println!("${} USD = €{} EUR", result.original_amount, result.converted_amount);
    /// println!("Rate used: {} (at {})", result.rate, result.rate_at);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn convert(&self, input: ConvertCurrency) -> Result<ConversionResult> {
        self.db.currency().convert(input)
    }

    /// Convert an amount between currencies (convenience method).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// use rust_decimal_macros::dec;
    ///
    /// # let commerce = Commerce::new(":memory:")?;
    /// let eur_amount = commerce.currency().convert_amount(
    ///     dec!(100.00),
    ///     Currency::USD,
    ///     Currency::EUR
    /// )?;
    ///
    /// println!("€{}", eur_amount);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn convert_amount(
        &self,
        amount: Decimal,
        from: Currency,
        to: Currency,
    ) -> Result<Decimal> {
        let result = self.convert(ConvertCurrency { from, to, amount })?;
        Ok(result.converted_amount)
    }

    // ========================================================================
    // Store Currency Settings
    // ========================================================================

    /// Get store currency settings.
    ///
    /// Returns the base currency, enabled currencies, and conversion settings
    /// for the store.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let settings = commerce.currency().get_settings()?;
    ///
    /// println!("Base currency: {}", settings.base_currency);
    /// println!("Enabled currencies: {:?}", settings.enabled_currencies);
    /// println!("Auto-convert: {}", settings.auto_convert);
    /// println!("Rounding: {:?}", settings.rounding_mode);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn get_settings(&self) -> Result<StoreCurrencySettings> {
        self.db.currency().get_settings()
    }

    /// Update store currency settings.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let settings = commerce.currency().update_settings(StoreCurrencySettings {
    ///     base_currency: Currency::EUR,
    ///     enabled_currencies: vec![Currency::EUR, Currency::USD, Currency::GBP],
    ///     auto_convert: true,
    ///     rounding_mode: RoundingMode::HalfUp,
    /// })?;
    ///
    /// println!("Updated base currency to: {}", settings.base_currency);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn update_settings(&self, settings: StoreCurrencySettings) -> Result<StoreCurrencySettings> {
        self.db.currency().update_settings(settings)
    }

    /// Set the store's base currency.
    ///
    /// Convenience method to update just the base currency.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// commerce.currency().set_base_currency(Currency::EUR)?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn set_base_currency(&self, currency: Currency) -> Result<StoreCurrencySettings> {
        let mut settings = self.get_settings()?;
        settings.base_currency = currency;
        self.update_settings(settings)
    }

    /// Enable currencies for the store.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// commerce.currency().enable_currencies(vec![
    ///     Currency::USD,
    ///     Currency::EUR,
    ///     Currency::GBP,
    ///     Currency::JPY,
    /// ])?;
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn enable_currencies(&self, currencies: Vec<Currency>) -> Result<StoreCurrencySettings> {
        let mut settings = self.get_settings()?;
        settings.enabled_currencies = currencies;
        self.update_settings(settings)
    }

    /// Check if a currency is enabled for the store.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// if commerce.currency().is_enabled(Currency::EUR)? {
    ///     println!("EUR is enabled");
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn is_enabled(&self, currency: Currency) -> Result<bool> {
        let settings = self.get_settings()?;
        Ok(settings.enabled_currencies.contains(&currency))
    }

    // ========================================================================
    // Utility Methods
    // ========================================================================

    /// Get the store's base currency.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let base = commerce.currency().base_currency()?;
    /// println!("Store base currency: {}", base);
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn base_currency(&self) -> Result<Currency> {
        let settings = self.get_settings()?;
        Ok(settings.base_currency)
    }

    /// Get all enabled currencies for the store.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// # let commerce = Commerce::new(":memory:")?;
    /// let currencies = commerce.currency().enabled_currencies()?;
    /// for c in currencies {
    ///     println!("Enabled: {} ({})", c.name(), c.code());
    /// }
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn enabled_currencies(&self) -> Result<Vec<Currency>> {
        let settings = self.get_settings()?;
        Ok(settings.enabled_currencies)
    }

    /// Format an amount with currency symbol.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use stateset_embedded::*;
    /// use rust_decimal_macros::dec;
    ///
    /// # let commerce = Commerce::new(":memory:")?;
    /// let formatted = commerce.currency().format(dec!(99.99), Currency::USD);
    /// println!("{}", formatted); // "$99.99"
    /// # Ok::<(), CommerceError>(())
    /// ```
    pub fn format(&self, amount: Decimal, currency: Currency) -> String {
        format!("{}{}", currency.symbol(), amount)
    }
}
