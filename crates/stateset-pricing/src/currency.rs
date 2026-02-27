//! Currency conversion with inverse rates and triangulation.
//!
//! All conversions are pure — the [`CurrencyConverter`] holds a static table
//! of exchange rates and performs no network I/O.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::error::{PricingError, PricingResult};

/// An exchange rate between two currencies at a point in time.
///
/// # Example
///
/// ```rust
/// use stateset_pricing::ExchangeRate;
/// use rust_decimal_macros::dec;
/// use chrono::Utc;
///
/// let rate = ExchangeRate {
///     from: "USD".into(),
///     to: "EUR".into(),
///     rate: dec!(0.92),
///     as_of: Utc::now(),
/// };
/// assert_eq!(rate.from, "USD");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeRate {
    /// Source currency (ISO 4217).
    pub from: String,
    /// Target currency (ISO 4217).
    pub to: String,
    /// Conversion rate (1 unit of `from` = `rate` units of `to`).
    pub rate: Decimal,
    /// When this rate was observed.
    pub as_of: DateTime<Utc>,
}

/// Result of a currency conversion.
///
/// ```rust
/// use stateset_pricing::ConversionResult;
/// use rust_decimal_macros::dec;
///
/// let r = ConversionResult {
///     amount: dec!(92.00),
///     rate: dec!(0.92),
///     from: "USD".into(),
///     to: "EUR".into(),
/// };
/// assert_eq!(r.amount, dec!(92.00));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversionResult {
    /// Converted amount.
    pub amount: Decimal,
    /// Rate used.
    pub rate: Decimal,
    /// Source currency.
    pub from: String,
    /// Target currency.
    pub to: String,
}

/// A currency converter backed by an in-memory rate table.
///
/// Supports:
/// - Direct rates (USD -> EUR)
/// - Inverse rates (EUR -> USD derived from USD -> EUR)
/// - Triangulation through a base currency (GBP -> JPY via GBP -> USD -> JPY)
///
/// # Example
///
/// ```rust
/// use stateset_pricing::{CurrencyConverter, ExchangeRate};
/// use rust_decimal_macros::dec;
/// use chrono::Utc;
///
/// let mut converter = CurrencyConverter::new();
/// converter.add_rate(ExchangeRate {
///     from: "USD".into(),
///     to: "EUR".into(),
///     rate: dec!(0.92),
///     as_of: Utc::now(),
/// });
///
/// let result = converter.convert(dec!(100.00), "USD", "EUR").unwrap();
/// assert_eq!(result.amount, dec!(92.00));
/// ```
#[derive(Debug, Clone)]
pub struct CurrencyConverter {
    rates: HashMap<(String, String), ExchangeRate>,
    /// Base currency used for triangulation (defaults to "USD").
    base_currency: String,
}

fn normalize_currency_code(code: &str) -> String {
    code.trim().to_ascii_uppercase()
}

impl CurrencyConverter {
    /// Create a new empty converter with USD as the base currency.
    #[must_use]
    pub fn new() -> Self {
        Self { rates: HashMap::new(), base_currency: "USD".into() }
    }

    /// Create a new converter with a custom base currency for triangulation.
    #[must_use]
    pub fn with_base_currency(base: impl Into<String>) -> Self {
        let base = base.into();
        Self { rates: HashMap::new(), base_currency: normalize_currency_code(&base) }
    }

    /// Add an exchange rate to the converter.
    pub fn add_rate(&mut self, rate: ExchangeRate) {
        if !is_valid_rate_value(rate.rate) {
            return;
        }
        let normalized_from = normalize_currency_code(&rate.from);
        let normalized_to = normalize_currency_code(&rate.to);
        let key = (normalized_from.clone(), normalized_to.clone());
        let rate = ExchangeRate { from: normalized_from, to: normalized_to, ..rate };
        self.rates.insert(key, rate);
    }

    /// Convert an amount from one currency to another.
    ///
    /// Tries in order:
    /// 1. Direct rate (from -> to)
    /// 2. Inverse rate (to -> from, then invert)
    /// 3. Triangulation through the base currency (from -> base -> to)
    ///
    /// # Errors
    ///
    /// Returns [`PricingError::NoExchangeRate`] if no path can be found.
    pub fn convert(
        &self,
        amount: Decimal,
        from: &str,
        to: &str,
    ) -> PricingResult<ConversionResult> {
        let normalized_from = normalize_currency_code(from);
        let normalized_to = normalize_currency_code(to);

        // Same currency — no conversion needed
        if normalized_from == normalized_to {
            return Ok(ConversionResult {
                amount,
                rate: Decimal::ONE,
                from: normalized_from,
                to: normalized_to,
            });
        }

        // 1. Direct rate
        if let Some(rate) = self.find_rate(&normalized_from, &normalized_to) {
            if !is_valid_rate_value(rate.rate) {
                return Err(PricingError::no_exchange_rate(normalized_from, normalized_to));
            }
            let converted = amount * rate.rate;
            return Ok(ConversionResult {
                amount: converted,
                rate: rate.rate,
                from: normalized_from.clone(),
                to: normalized_to.clone(),
            });
        }

        // 2. Inverse rate
        if let Some(rate) = self.find_rate(&normalized_to, &normalized_from) {
            if !is_valid_rate_value(rate.rate) {
                return Err(PricingError::no_exchange_rate(normalized_from, normalized_to));
            }
            let inverse = Decimal::ONE / rate.rate;
            let converted = amount * inverse;
            return Ok(ConversionResult {
                amount: converted,
                rate: inverse,
                from: normalized_from.clone(),
                to: normalized_to.clone(),
            });
        }

        // 3. Triangulation through base currency
        let base = &self.base_currency;
        if normalized_from != *base && normalized_to != *base {
            let to_base = self.find_effective_rate(&normalized_from, base);
            let from_base = self.find_effective_rate(base, &normalized_to);

            if let (Some(rate_to_base), Some(rate_from_base)) = (to_base, from_base) {
                let composite_rate = rate_to_base * rate_from_base;
                let converted = amount * composite_rate;
                return Ok(ConversionResult {
                    amount: converted,
                    rate: composite_rate,
                    from: normalized_from,
                    to: normalized_to,
                });
            }
        }

        Err(PricingError::no_exchange_rate(normalized_from, normalized_to))
    }

    /// Get all loaded rates.
    #[must_use]
    pub const fn rates(&self) -> &HashMap<(String, String), ExchangeRate> {
        &self.rates
    }

    /// Get the base currency used for triangulation.
    #[must_use]
    pub fn base_currency(&self) -> &str {
        &self.base_currency
    }

    /// Find a direct rate.
    fn find_rate(&self, from: &str, to: &str) -> Option<&ExchangeRate> {
        self.rates.get(&(normalize_currency_code(from), normalize_currency_code(to)))
    }

    /// Find the effective rate from -> to, trying direct then inverse.
    fn find_effective_rate(&self, from: &str, to: &str) -> Option<Decimal> {
        if let Some(rate) = self.find_rate(from, to) {
            if is_valid_rate_value(rate.rate) {
                return Some(rate.rate);
            }
        }
        if let Some(rate) = self.find_rate(to, from) {
            if is_valid_rate_value(rate.rate) {
                return Some(Decimal::ONE / rate.rate);
            }
        }
        None
    }
}

fn is_valid_rate_value(rate: Decimal) -> bool {
    rate > Decimal::ZERO
}

impl Default for CurrencyConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;

    fn make_rate(from: &str, to: &str, rate: Decimal) -> ExchangeRate {
        ExchangeRate { from: from.into(), to: to.into(), rate, as_of: Utc::now() }
    }

    fn usd_eur_converter() -> CurrencyConverter {
        let mut c = CurrencyConverter::new();
        c.add_rate(make_rate("USD", "EUR", dec!(0.92)));
        c
    }

    // ---- direct conversion ----

    #[test]
    fn direct_conversion() {
        let c = usd_eur_converter();
        let r = c.convert(dec!(100.00), "USD", "EUR").unwrap();
        assert_eq!(r.amount, dec!(92.00));
        assert_eq!(r.rate, dec!(0.92));
        assert_eq!(r.from, "USD");
        assert_eq!(r.to, "EUR");
    }

    #[test]
    fn direct_conversion_zero_amount() {
        let c = usd_eur_converter();
        let r = c.convert(Decimal::ZERO, "USD", "EUR").unwrap();
        assert_eq!(r.amount, Decimal::ZERO);
    }

    #[test]
    fn direct_conversion_large_amount() {
        let c = usd_eur_converter();
        let r = c.convert(dec!(1000000.00), "USD", "EUR").unwrap();
        assert_eq!(r.amount, dec!(920000.00));
    }

    #[test]
    fn direct_conversion_lookup_is_case_insensitive() {
        let mut c = CurrencyConverter::new();
        c.add_rate(make_rate("usd", "eur", dec!(0.92)));
        let r = c.convert(dec!(100.00), "USD", "EuR").unwrap();
        assert_eq!(r.amount, dec!(92.00));
        assert_eq!(r.from, "USD");
        assert_eq!(r.to, "EUR");
    }

    // ---- inverse conversion ----

    #[test]
    fn inverse_conversion() {
        let c = usd_eur_converter();
        let r = c.convert(dec!(92.00), "EUR", "USD").unwrap();
        // 92 / 0.92 = 100
        assert_eq!(r.amount, dec!(100.00));
    }

    #[test]
    fn inverse_rate_value() {
        let c = usd_eur_converter();
        let r = c.convert(dec!(1.00), "EUR", "USD").unwrap();
        // 1 / 0.92 ≈ 1.08695652...
        assert!(r.rate > dec!(1.08));
        assert!(r.rate < dec!(1.09));
    }

    // ---- same currency (no-op) ----

    #[test]
    fn same_currency_noop() {
        let c = CurrencyConverter::new();
        let r = c.convert(dec!(42.50), "USD", "USD").unwrap();
        assert_eq!(r.amount, dec!(42.50));
        assert_eq!(r.rate, Decimal::ONE);
    }

    #[test]
    fn same_currency_case_insensitive() {
        let c = CurrencyConverter::new();
        let r = c.convert(dec!(10.00), "usd", "USD").unwrap();
        assert_eq!(r.amount, dec!(10.00));
    }

    // ---- missing rate ----

    #[test]
    fn missing_rate() {
        let c = CurrencyConverter::new();
        let r = c.convert(dec!(100.00), "USD", "JPY");
        assert!(r.is_err());
        assert!(matches!(r, Err(PricingError::NoExchangeRate { .. })));
    }

    // ---- triangulation ----

    #[test]
    fn triangulation_through_base() {
        let mut c = CurrencyConverter::new();
        c.add_rate(make_rate("USD", "EUR", dec!(0.92)));
        c.add_rate(make_rate("USD", "JPY", dec!(150.00)));

        // EUR -> JPY via USD
        // EUR -> USD: 1/0.92
        // USD -> JPY: 150
        // composite: (1/0.92) * 150 ≈ 163.04...
        let r = c.convert(dec!(100.00), "EUR", "JPY").unwrap();
        assert!(r.amount > dec!(16300.00));
        assert!(r.amount < dec!(16310.00));
    }

    #[test]
    fn triangulation_using_inverse_leg() {
        let mut c = CurrencyConverter::new();
        // Only have GBP->USD and EUR->USD (both going TO base)
        c.add_rate(make_rate("GBP", "USD", dec!(1.27)));
        c.add_rate(make_rate("EUR", "USD", dec!(1.09)));

        // GBP -> EUR: GBP -> USD (1.27) then USD -> EUR (1/1.09)
        let r = c.convert(dec!(100.00), "GBP", "EUR").unwrap();
        // 100 * 1.27 / 1.09 ≈ 116.51
        assert!(r.amount > dec!(116.00));
        assert!(r.amount < dec!(117.00));
    }

    #[test]
    fn triangulation_fails_if_no_path() {
        let mut c = CurrencyConverter::new();
        c.add_rate(make_rate("USD", "EUR", dec!(0.92)));
        // No JPY rates at all, and trying GBP->JPY
        let r = c.convert(dec!(100.00), "GBP", "JPY");
        assert!(r.is_err());
    }

    // ---- custom base currency ----

    #[test]
    fn custom_base_currency() {
        let mut c = CurrencyConverter::with_base_currency("EUR");
        c.add_rate(make_rate("EUR", "USD", dec!(1.09)));
        c.add_rate(make_rate("EUR", "GBP", dec!(0.86)));

        // USD -> GBP via EUR
        let r = c.convert(dec!(100.00), "USD", "GBP").unwrap();
        // USD -> EUR: 1/1.09, EUR -> GBP: 0.86
        // 100 / 1.09 * 0.86 ≈ 78.90
        assert!(r.amount > dec!(78.00));
        assert!(r.amount < dec!(80.00));
    }

    #[test]
    fn custom_base_currency_lookup_is_case_insensitive() {
        let mut c = CurrencyConverter::with_base_currency("usd");
        c.add_rate(make_rate("Usd", "eur", dec!(0.92)));
        c.add_rate(make_rate("USD", "JPY", dec!(150.00)));

        let r = c.convert(dec!(100.00), "eUr", "jPy").unwrap();
        assert!(r.amount > dec!(16300.00));
        assert!(r.amount < dec!(16310.00));
    }

    // ---- multiple rates ----

    #[test]
    fn add_overwrites_rate() {
        let mut c = CurrencyConverter::new();
        c.add_rate(make_rate("USD", "EUR", dec!(0.90)));
        c.add_rate(make_rate("USD", "EUR", dec!(0.95)));
        let r = c.convert(dec!(100.00), "USD", "EUR").unwrap();
        assert_eq!(r.amount, dec!(95.00));
    }

    // ---- accessor methods ----

    #[test]
    fn rates_accessor() {
        let mut c = CurrencyConverter::new();
        assert!(c.rates().is_empty());
        c.add_rate(make_rate("USD", "EUR", dec!(0.92)));
        assert_eq!(c.rates().len(), 1);
    }

    #[test]
    fn base_currency_accessor() {
        let c = CurrencyConverter::new();
        assert_eq!(c.base_currency(), "USD");
        let c2 = CurrencyConverter::with_base_currency("EUR");
        assert_eq!(c2.base_currency(), "EUR");
    }

    // ---- default ----

    #[test]
    fn default_converter() {
        let c = CurrencyConverter::default();
        assert_eq!(c.base_currency(), "USD");
        assert!(c.rates().is_empty());
    }

    // ---- serde roundtrip ----

    #[test]
    fn exchange_rate_serde() {
        let rate = make_rate("USD", "EUR", dec!(0.92));
        let json = serde_json::to_string(&rate).unwrap();
        let parsed: ExchangeRate = serde_json::from_str(&json).unwrap();
        assert_eq!(rate, parsed);
    }

    #[test]
    fn conversion_result_serde() {
        let r = ConversionResult {
            amount: dec!(92.00),
            rate: dec!(0.92),
            from: "USD".into(),
            to: "EUR".into(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: ConversionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, parsed);
    }

    // ---- zero rate edge case ----

    #[test]
    fn zero_rate_inverse_fails() {
        let mut c = CurrencyConverter::new();
        c.add_rate(make_rate("USD", "EUR", Decimal::ZERO));
        let r = c.convert(dec!(100.00), "EUR", "USD");
        assert!(r.is_err());
    }

    #[test]
    fn negative_rate_is_rejected() {
        let mut c = CurrencyConverter::new();
        c.add_rate(make_rate("USD", "EUR", dec!(-1.23)));
        let r = c.convert(dec!(100.00), "USD", "EUR");
        assert!(r.is_err());
    }
}
