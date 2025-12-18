//! Multi-currency support types
//!
//! Provides ISO 4217 currency codes, money representation, and exchange rates.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// ISO 4217 Currency codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum Currency {
    /// US Dollar
    #[default]
    USD,
    /// Euro
    EUR,
    /// British Pound Sterling
    GBP,
    /// Japanese Yen
    JPY,
    /// Canadian Dollar
    CAD,
    /// Australian Dollar
    AUD,
    /// Swiss Franc
    CHF,
    /// Chinese Yuan
    CNY,
    /// Hong Kong Dollar
    HKD,
    /// Singapore Dollar
    SGD,
    /// Swedish Krona
    SEK,
    /// Norwegian Krone
    NOK,
    /// Danish Krone
    DKK,
    /// New Zealand Dollar
    NZD,
    /// Mexican Peso
    MXN,
    /// Indian Rupee
    INR,
    /// Brazilian Real
    BRL,
    /// South Korean Won
    KRW,
    /// South African Rand
    ZAR,
    /// Russian Ruble
    RUB,
    /// Turkish Lira
    TRY,
    /// Polish Zloty
    PLN,
    /// Thai Baht
    THB,
    /// Indonesian Rupiah
    IDR,
    /// Malaysian Ringgit
    MYR,
    /// Philippine Peso
    PHP,
    /// Czech Koruna
    CZK,
    /// Israeli New Shekel
    ILS,
    /// United Arab Emirates Dirham
    AED,
    /// Saudi Riyal
    SAR,
    /// Taiwan Dollar
    TWD,
    /// Vietnamese Dong
    VND,
    /// Bitcoin (crypto)
    BTC,
    /// Ethereum (crypto)
    ETH,
    /// USD Coin (stablecoin)
    USDC,
    /// Tether (stablecoin)
    USDT,
}

impl Currency {
    /// Get the currency code as a string
    pub fn code(&self) -> &'static str {
        match self {
            Currency::USD => "USD",
            Currency::EUR => "EUR",
            Currency::GBP => "GBP",
            Currency::JPY => "JPY",
            Currency::CAD => "CAD",
            Currency::AUD => "AUD",
            Currency::CHF => "CHF",
            Currency::CNY => "CNY",
            Currency::HKD => "HKD",
            Currency::SGD => "SGD",
            Currency::SEK => "SEK",
            Currency::NOK => "NOK",
            Currency::DKK => "DKK",
            Currency::NZD => "NZD",
            Currency::MXN => "MXN",
            Currency::INR => "INR",
            Currency::BRL => "BRL",
            Currency::KRW => "KRW",
            Currency::ZAR => "ZAR",
            Currency::RUB => "RUB",
            Currency::TRY => "TRY",
            Currency::PLN => "PLN",
            Currency::THB => "THB",
            Currency::IDR => "IDR",
            Currency::MYR => "MYR",
            Currency::PHP => "PHP",
            Currency::CZK => "CZK",
            Currency::ILS => "ILS",
            Currency::AED => "AED",
            Currency::SAR => "SAR",
            Currency::TWD => "TWD",
            Currency::VND => "VND",
            Currency::BTC => "BTC",
            Currency::ETH => "ETH",
            Currency::USDC => "USDC",
            Currency::USDT => "USDT",
        }
    }

    /// Get the currency symbol
    pub fn symbol(&self) -> &'static str {
        match self {
            Currency::USD => "$",
            Currency::EUR => "€",
            Currency::GBP => "£",
            Currency::JPY => "¥",
            Currency::CAD => "C$",
            Currency::AUD => "A$",
            Currency::CHF => "CHF",
            Currency::CNY => "¥",
            Currency::HKD => "HK$",
            Currency::SGD => "S$",
            Currency::SEK => "kr",
            Currency::NOK => "kr",
            Currency::DKK => "kr",
            Currency::NZD => "NZ$",
            Currency::MXN => "$",
            Currency::INR => "₹",
            Currency::BRL => "R$",
            Currency::KRW => "₩",
            Currency::ZAR => "R",
            Currency::RUB => "₽",
            Currency::TRY => "₺",
            Currency::PLN => "zł",
            Currency::THB => "฿",
            Currency::IDR => "Rp",
            Currency::MYR => "RM",
            Currency::PHP => "₱",
            Currency::CZK => "Kč",
            Currency::ILS => "₪",
            Currency::AED => "د.إ",
            Currency::SAR => "﷼",
            Currency::TWD => "NT$",
            Currency::VND => "₫",
            Currency::BTC => "₿",
            Currency::ETH => "Ξ",
            Currency::USDC => "USDC",
            Currency::USDT => "USDT",
        }
    }

    /// Get the currency name
    pub fn name(&self) -> &'static str {
        match self {
            Currency::USD => "US Dollar",
            Currency::EUR => "Euro",
            Currency::GBP => "British Pound",
            Currency::JPY => "Japanese Yen",
            Currency::CAD => "Canadian Dollar",
            Currency::AUD => "Australian Dollar",
            Currency::CHF => "Swiss Franc",
            Currency::CNY => "Chinese Yuan",
            Currency::HKD => "Hong Kong Dollar",
            Currency::SGD => "Singapore Dollar",
            Currency::SEK => "Swedish Krona",
            Currency::NOK => "Norwegian Krone",
            Currency::DKK => "Danish Krone",
            Currency::NZD => "New Zealand Dollar",
            Currency::MXN => "Mexican Peso",
            Currency::INR => "Indian Rupee",
            Currency::BRL => "Brazilian Real",
            Currency::KRW => "South Korean Won",
            Currency::ZAR => "South African Rand",
            Currency::RUB => "Russian Ruble",
            Currency::TRY => "Turkish Lira",
            Currency::PLN => "Polish Zloty",
            Currency::THB => "Thai Baht",
            Currency::IDR => "Indonesian Rupiah",
            Currency::MYR => "Malaysian Ringgit",
            Currency::PHP => "Philippine Peso",
            Currency::CZK => "Czech Koruna",
            Currency::ILS => "Israeli Shekel",
            Currency::AED => "UAE Dirham",
            Currency::SAR => "Saudi Riyal",
            Currency::TWD => "Taiwan Dollar",
            Currency::VND => "Vietnamese Dong",
            Currency::BTC => "Bitcoin",
            Currency::ETH => "Ethereum",
            Currency::USDC => "USD Coin",
            Currency::USDT => "Tether",
        }
    }

    /// Get the number of decimal places for this currency
    pub fn decimal_places(&self) -> u8 {
        match self {
            // Zero decimal currencies
            Currency::JPY | Currency::KRW | Currency::VND => 0,
            // Crypto with 8 decimals
            Currency::BTC => 8,
            // Crypto with 18 decimals (but we'll use 8 for practical purposes)
            Currency::ETH => 8,
            // All others use 2 decimals
            _ => 2,
        }
    }

    /// Check if this is a cryptocurrency
    pub fn is_crypto(&self) -> bool {
        matches!(
            self,
            Currency::BTC | Currency::ETH | Currency::USDC | Currency::USDT
        )
    }

    /// Check if this is a fiat currency
    pub fn is_fiat(&self) -> bool {
        !self.is_crypto()
    }

    /// Get all supported currencies
    pub fn all() -> Vec<Currency> {
        vec![
            Currency::USD,
            Currency::EUR,
            Currency::GBP,
            Currency::JPY,
            Currency::CAD,
            Currency::AUD,
            Currency::CHF,
            Currency::CNY,
            Currency::HKD,
            Currency::SGD,
            Currency::SEK,
            Currency::NOK,
            Currency::DKK,
            Currency::NZD,
            Currency::MXN,
            Currency::INR,
            Currency::BRL,
            Currency::KRW,
            Currency::ZAR,
            Currency::RUB,
            Currency::TRY,
            Currency::PLN,
            Currency::THB,
            Currency::IDR,
            Currency::MYR,
            Currency::PHP,
            Currency::CZK,
            Currency::ILS,
            Currency::AED,
            Currency::SAR,
            Currency::TWD,
            Currency::VND,
            Currency::BTC,
            Currency::ETH,
            Currency::USDC,
            Currency::USDT,
        ]
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

impl FromStr for Currency {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "USD" => Ok(Currency::USD),
            "EUR" => Ok(Currency::EUR),
            "GBP" => Ok(Currency::GBP),
            "JPY" => Ok(Currency::JPY),
            "CAD" => Ok(Currency::CAD),
            "AUD" => Ok(Currency::AUD),
            "CHF" => Ok(Currency::CHF),
            "CNY" => Ok(Currency::CNY),
            "HKD" => Ok(Currency::HKD),
            "SGD" => Ok(Currency::SGD),
            "SEK" => Ok(Currency::SEK),
            "NOK" => Ok(Currency::NOK),
            "DKK" => Ok(Currency::DKK),
            "NZD" => Ok(Currency::NZD),
            "MXN" => Ok(Currency::MXN),
            "INR" => Ok(Currency::INR),
            "BRL" => Ok(Currency::BRL),
            "KRW" => Ok(Currency::KRW),
            "ZAR" => Ok(Currency::ZAR),
            "RUB" => Ok(Currency::RUB),
            "TRY" => Ok(Currency::TRY),
            "PLN" => Ok(Currency::PLN),
            "THB" => Ok(Currency::THB),
            "IDR" => Ok(Currency::IDR),
            "MYR" => Ok(Currency::MYR),
            "PHP" => Ok(Currency::PHP),
            "CZK" => Ok(Currency::CZK),
            "ILS" => Ok(Currency::ILS),
            "AED" => Ok(Currency::AED),
            "SAR" => Ok(Currency::SAR),
            "TWD" => Ok(Currency::TWD),
            "VND" => Ok(Currency::VND),
            "BTC" => Ok(Currency::BTC),
            "ETH" => Ok(Currency::ETH),
            "USDC" => Ok(Currency::USDC),
            "USDT" => Ok(Currency::USDT),
            _ => Err(format!("Unknown currency code: {}", s)),
        }
    }
}

// ============================================================================
// Money Type
// ============================================================================

/// Represents a monetary amount with its currency
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    /// The amount in the smallest unit (e.g., cents for USD)
    pub amount: Decimal,
    /// The currency
    pub currency: Currency,
}

impl Money {
    /// Create a new Money instance
    pub fn new(amount: Decimal, currency: Currency) -> Self {
        Self { amount, currency }
    }

    /// Create Money from a major unit amount (e.g., dollars, not cents)
    pub fn from_major(amount: Decimal, currency: Currency) -> Self {
        Self { amount, currency }
    }

    /// Create zero money in a currency
    pub fn zero(currency: Currency) -> Self {
        Self {
            amount: Decimal::ZERO,
            currency,
        }
    }

    /// Check if the amount is zero
    pub fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    /// Check if the amount is positive
    pub fn is_positive(&self) -> bool {
        self.amount.is_sign_positive() && !self.amount.is_zero()
    }

    /// Check if the amount is negative
    pub fn is_negative(&self) -> bool {
        self.amount.is_sign_negative()
    }

    /// Get the absolute value
    pub fn abs(&self) -> Self {
        Self {
            amount: self.amount.abs(),
            currency: self.currency,
        }
    }

    /// Round to the currency's decimal places
    pub fn round(&self) -> Self {
        let places = self.currency.decimal_places() as u32;
        Self {
            amount: self.amount.round_dp(places),
            currency: self.currency,
        }
    }

    /// Format as a string with symbol
    pub fn format(&self) -> String {
        let rounded = self.round();
        let places = self.currency.decimal_places();
        if places == 0 {
            format!("{}{}", self.currency.symbol(), rounded.amount)
        } else {
            format!(
                "{}{}",
                self.currency.symbol(),
                Self::format_amount_fixed(rounded.amount, places)
            )
        }
    }

    /// Format as a string with currency code
    pub fn format_with_code(&self) -> String {
        let rounded = self.round();
        let places = self.currency.decimal_places();
        format!(
            "{} {}",
            Self::format_amount_fixed(rounded.amount, places),
            self.currency.code()
        )
    }

    fn format_amount_fixed(amount: Decimal, places: u8) -> String {
        if places == 0 {
            return amount.to_string();
        }

        let mut s = amount.to_string();
        let places = places as usize;

        match s.find('.') {
            Some(dot) => {
                let fractional_len = s.len().saturating_sub(dot + 1);
                if fractional_len < places {
                    s.push_str(&"0".repeat(places - fractional_len));
                } else if fractional_len > places {
                    s.truncate(dot + 1 + places);
                }
            }
            None => {
                s.push('.');
                s.push_str(&"0".repeat(places));
            }
        }

        s
    }
}

impl Default for Money {
    fn default() -> Self {
        Self::zero(Currency::USD)
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

// ============================================================================
// Exchange Rate
// ============================================================================

/// An exchange rate between two currencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRate {
    /// Unique identifier
    pub id: Uuid,
    /// Base currency (from)
    pub base_currency: Currency,
    /// Quote currency (to)
    pub quote_currency: Currency,
    /// Exchange rate (1 base = rate quote)
    pub rate: Decimal,
    /// Rate source (e.g., "ECB", "manual", "openexchangerates")
    pub source: String,
    /// When this rate was fetched/set
    pub rate_at: DateTime<Utc>,
    /// When this record was created
    pub created_at: DateTime<Utc>,
    /// When this record was last updated
    pub updated_at: DateTime<Utc>,
}

impl ExchangeRate {
    /// Convert an amount from base to quote currency
    pub fn convert(&self, amount: Decimal) -> Decimal {
        amount * self.rate
    }

    /// Convert an amount from quote to base currency (inverse)
    pub fn convert_inverse(&self, amount: Decimal) -> Decimal {
        if self.rate.is_zero() {
            Decimal::ZERO
        } else {
            amount / self.rate
        }
    }

    /// Get the inverse rate
    pub fn inverse(&self) -> Decimal {
        if self.rate.is_zero() {
            Decimal::ZERO
        } else {
            Decimal::ONE / self.rate
        }
    }
}

// ============================================================================
// Currency Conversion Request/Result
// ============================================================================

/// Request to convert money between currencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertCurrency {
    /// Amount to convert
    pub amount: Decimal,
    /// Source currency
    pub from: Currency,
    /// Target currency
    pub to: Currency,
}

/// Result of a currency conversion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    /// Original amount
    pub original_amount: Decimal,
    /// Original currency
    pub original_currency: Currency,
    /// Converted amount
    pub converted_amount: Decimal,
    /// Target currency
    pub target_currency: Currency,
    /// Exchange rate used
    pub rate: Decimal,
    /// Inverse rate
    pub inverse_rate: Decimal,
    /// Rate timestamp
    pub rate_at: DateTime<Utc>,
}

// ============================================================================
// Multi-Currency Price
// ============================================================================

/// A price that can be displayed in multiple currencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiCurrencyPrice {
    /// Base price (source of truth)
    pub base: Money,
    /// Prices in other currencies (cached/calculated)
    pub prices: Vec<Money>,
}

impl MultiCurrencyPrice {
    /// Create a new multi-currency price with just the base
    pub fn new(base: Money) -> Self {
        Self {
            base,
            prices: Vec::new(),
        }
    }

    /// Get the price in a specific currency if available
    pub fn get(&self, currency: Currency) -> Option<&Money> {
        if self.base.currency == currency {
            Some(&self.base)
        } else {
            self.prices.iter().find(|p| p.currency == currency)
        }
    }

    /// Add a price in another currency
    pub fn add_price(&mut self, price: Money) {
        // Don't add if it's the base currency or already exists
        if price.currency != self.base.currency
            && !self.prices.iter().any(|p| p.currency == price.currency)
        {
            self.prices.push(price);
        }
    }
}

// ============================================================================
// Exchange Rate Management
// ============================================================================

/// Request to set an exchange rate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetExchangeRate {
    /// Base currency
    pub base_currency: Currency,
    /// Quote currency
    pub quote_currency: Currency,
    /// Exchange rate
    pub rate: Decimal,
    /// Source of the rate
    pub source: Option<String>,
}

/// Filter for listing exchange rates
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExchangeRateFilter {
    /// Filter by base currency
    pub base_currency: Option<Currency>,
    /// Filter by quote currency
    pub quote_currency: Option<Currency>,
    /// Only rates newer than this
    pub since: Option<DateTime<Utc>>,
}

// ============================================================================
// Store Currency Settings
// ============================================================================

/// Store-level currency configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreCurrencySettings {
    /// Default/base currency for the store
    pub base_currency: Currency,
    /// Currencies enabled for display
    pub enabled_currencies: Vec<Currency>,
    /// Whether to auto-convert prices
    pub auto_convert: bool,
    /// Rounding mode for conversions
    pub rounding_mode: RoundingMode,
}

impl Default for StoreCurrencySettings {
    fn default() -> Self {
        Self {
            base_currency: Currency::USD,
            enabled_currencies: vec![Currency::USD, Currency::EUR, Currency::GBP],
            auto_convert: true,
            rounding_mode: RoundingMode::HalfUp,
        }
    }
}

/// Rounding mode for currency conversions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RoundingMode {
    /// Round half up (standard)
    #[default]
    HalfUp,
    /// Round half down
    HalfDown,
    /// Always round up
    Up,
    /// Always round down
    Down,
    /// Round to nearest even (banker's rounding)
    HalfEven,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_currency_from_str() {
        assert_eq!(Currency::from_str("USD").unwrap(), Currency::USD);
        assert_eq!(Currency::from_str("eur").unwrap(), Currency::EUR);
        assert_eq!(Currency::from_str("Gbp").unwrap(), Currency::GBP);
        assert!(Currency::from_str("XXX").is_err());
    }

    #[test]
    fn test_money_format() {
        let usd = Money::new(Decimal::from(1234), Currency::USD);
        assert_eq!(usd.format(), "$1234.00");

        let jpy = Money::new(Decimal::from(1234), Currency::JPY);
        assert_eq!(jpy.format(), "¥1234");
    }

    #[test]
    fn test_exchange_rate_convert() {
        let rate = ExchangeRate {
            id: Uuid::new_v4(),
            base_currency: Currency::USD,
            quote_currency: Currency::EUR,
            rate: Decimal::new(85, 2), // 0.85
            source: "test".into(),
            rate_at: Utc::now(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let result = rate.convert(Decimal::from(100));
        assert_eq!(result, Decimal::from(85));
    }
}
