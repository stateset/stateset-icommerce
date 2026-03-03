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
#[non_exhaustive]
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
    pub const fn code(&self) -> &'static str {
        match self {
            Self::USD => "USD",
            Self::EUR => "EUR",
            Self::GBP => "GBP",
            Self::JPY => "JPY",
            Self::CAD => "CAD",
            Self::AUD => "AUD",
            Self::CHF => "CHF",
            Self::CNY => "CNY",
            Self::HKD => "HKD",
            Self::SGD => "SGD",
            Self::SEK => "SEK",
            Self::NOK => "NOK",
            Self::DKK => "DKK",
            Self::NZD => "NZD",
            Self::MXN => "MXN",
            Self::INR => "INR",
            Self::BRL => "BRL",
            Self::KRW => "KRW",
            Self::ZAR => "ZAR",
            Self::RUB => "RUB",
            Self::TRY => "TRY",
            Self::PLN => "PLN",
            Self::THB => "THB",
            Self::IDR => "IDR",
            Self::MYR => "MYR",
            Self::PHP => "PHP",
            Self::CZK => "CZK",
            Self::ILS => "ILS",
            Self::AED => "AED",
            Self::SAR => "SAR",
            Self::TWD => "TWD",
            Self::VND => "VND",
            Self::BTC => "BTC",
            Self::ETH => "ETH",
            Self::USDC => "USDC",
            Self::USDT => "USDT",
        }
    }

    /// Get the currency symbol
    pub const fn symbol(&self) -> &'static str {
        match self {
            Self::USD => "$",
            Self::EUR => "€",
            Self::GBP => "£",
            Self::JPY => "¥",
            Self::CAD => "C$",
            Self::AUD => "A$",
            Self::CHF => "CHF",
            Self::CNY => "¥",
            Self::HKD => "HK$",
            Self::SGD => "S$",
            Self::SEK => "kr",
            Self::NOK => "kr",
            Self::DKK => "kr",
            Self::NZD => "NZ$",
            Self::MXN => "$",
            Self::INR => "₹",
            Self::BRL => "R$",
            Self::KRW => "₩",
            Self::ZAR => "R",
            Self::RUB => "₽",
            Self::TRY => "₺",
            Self::PLN => "zł",
            Self::THB => "฿",
            Self::IDR => "Rp",
            Self::MYR => "RM",
            Self::PHP => "₱",
            Self::CZK => "Kč",
            Self::ILS => "₪",
            Self::AED => "د.إ",
            Self::SAR => "﷼",
            Self::TWD => "NT$",
            Self::VND => "₫",
            Self::BTC => "₿",
            Self::ETH => "Ξ",
            Self::USDC => "USDC",
            Self::USDT => "USDT",
        }
    }

    /// Get the currency name
    pub const fn name(&self) -> &'static str {
        match self {
            Self::USD => "US Dollar",
            Self::EUR => "Euro",
            Self::GBP => "British Pound",
            Self::JPY => "Japanese Yen",
            Self::CAD => "Canadian Dollar",
            Self::AUD => "Australian Dollar",
            Self::CHF => "Swiss Franc",
            Self::CNY => "Chinese Yuan",
            Self::HKD => "Hong Kong Dollar",
            Self::SGD => "Singapore Dollar",
            Self::SEK => "Swedish Krona",
            Self::NOK => "Norwegian Krone",
            Self::DKK => "Danish Krone",
            Self::NZD => "New Zealand Dollar",
            Self::MXN => "Mexican Peso",
            Self::INR => "Indian Rupee",
            Self::BRL => "Brazilian Real",
            Self::KRW => "South Korean Won",
            Self::ZAR => "South African Rand",
            Self::RUB => "Russian Ruble",
            Self::TRY => "Turkish Lira",
            Self::PLN => "Polish Zloty",
            Self::THB => "Thai Baht",
            Self::IDR => "Indonesian Rupiah",
            Self::MYR => "Malaysian Ringgit",
            Self::PHP => "Philippine Peso",
            Self::CZK => "Czech Koruna",
            Self::ILS => "Israeli Shekel",
            Self::AED => "UAE Dirham",
            Self::SAR => "Saudi Riyal",
            Self::TWD => "Taiwan Dollar",
            Self::VND => "Vietnamese Dong",
            Self::BTC => "Bitcoin",
            Self::ETH => "Ethereum",
            Self::USDC => "USD Coin",
            Self::USDT => "Tether",
        }
    }

    /// Get the number of decimal places for this currency
    pub const fn decimal_places(&self) -> u8 {
        match self {
            // Zero decimal currencies
            Self::JPY | Self::KRW | Self::VND => 0,
            // Crypto with 8 decimals
            Self::BTC => 8,
            // Crypto with 18 decimals (but we'll use 8 for practical purposes)
            Self::ETH => 8,
            // All others use 2 decimals
            _ => 2,
        }
    }

    /// Check if this is a cryptocurrency
    pub const fn is_crypto(&self) -> bool {
        matches!(self, Self::BTC | Self::ETH | Self::USDC | Self::USDT)
    }

    /// Check if this is a fiat currency
    pub const fn is_fiat(&self) -> bool {
        !self.is_crypto()
    }

    /// Get all supported currencies
    pub fn all() -> Vec<Self> {
        vec![
            Self::USD,
            Self::EUR,
            Self::GBP,
            Self::JPY,
            Self::CAD,
            Self::AUD,
            Self::CHF,
            Self::CNY,
            Self::HKD,
            Self::SGD,
            Self::SEK,
            Self::NOK,
            Self::DKK,
            Self::NZD,
            Self::MXN,
            Self::INR,
            Self::BRL,
            Self::KRW,
            Self::ZAR,
            Self::RUB,
            Self::TRY,
            Self::PLN,
            Self::THB,
            Self::IDR,
            Self::MYR,
            Self::PHP,
            Self::CZK,
            Self::ILS,
            Self::AED,
            Self::SAR,
            Self::TWD,
            Self::VND,
            Self::BTC,
            Self::ETH,
            Self::USDC,
            Self::USDT,
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
            "USD" => Ok(Self::USD),
            "EUR" => Ok(Self::EUR),
            "GBP" => Ok(Self::GBP),
            "JPY" => Ok(Self::JPY),
            "CAD" => Ok(Self::CAD),
            "AUD" => Ok(Self::AUD),
            "CHF" => Ok(Self::CHF),
            "CNY" => Ok(Self::CNY),
            "HKD" => Ok(Self::HKD),
            "SGD" => Ok(Self::SGD),
            "SEK" => Ok(Self::SEK),
            "NOK" => Ok(Self::NOK),
            "DKK" => Ok(Self::DKK),
            "NZD" => Ok(Self::NZD),
            "MXN" => Ok(Self::MXN),
            "INR" => Ok(Self::INR),
            "BRL" => Ok(Self::BRL),
            "KRW" => Ok(Self::KRW),
            "ZAR" => Ok(Self::ZAR),
            "RUB" => Ok(Self::RUB),
            "TRY" => Ok(Self::TRY),
            "PLN" => Ok(Self::PLN),
            "THB" => Ok(Self::THB),
            "IDR" => Ok(Self::IDR),
            "MYR" => Ok(Self::MYR),
            "PHP" => Ok(Self::PHP),
            "CZK" => Ok(Self::CZK),
            "ILS" => Ok(Self::ILS),
            "AED" => Ok(Self::AED),
            "SAR" => Ok(Self::SAR),
            "TWD" => Ok(Self::TWD),
            "VND" => Ok(Self::VND),
            "BTC" => Ok(Self::BTC),
            "ETH" => Ok(Self::ETH),
            "USDC" => Ok(Self::USDC),
            "USDT" => Ok(Self::USDT),
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
    pub const fn new(amount: Decimal, currency: Currency) -> Self {
        Self { amount, currency }
    }

    /// Create Money from a major unit amount (e.g., dollars, not cents)
    pub const fn from_major(amount: Decimal, currency: Currency) -> Self {
        Self { amount, currency }
    }

    /// Create zero money in a currency
    pub const fn zero(currency: Currency) -> Self {
        Self { amount: Decimal::ZERO, currency }
    }

    /// Check if the amount is zero
    pub const fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    /// Check if the amount is positive
    pub const fn is_positive(&self) -> bool {
        self.amount.is_sign_positive() && !self.amount.is_zero()
    }

    /// Check if the amount is negative
    pub const fn is_negative(&self) -> bool {
        self.amount.is_sign_negative()
    }

    /// Get the absolute value
    pub fn abs(&self) -> Self {
        Self { amount: self.amount.abs(), currency: self.currency }
    }

    /// Round to the currency's decimal places
    pub fn round(&self) -> Self {
        let places = self.currency.decimal_places() as u32;
        Self { amount: self.amount.round_dp(places), currency: self.currency }
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
        format!("{} {}", Self::format_amount_fixed(rounded.amount, places), self.currency.code())
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
        if self.rate.is_zero() { Decimal::ZERO } else { amount / self.rate }
    }

    /// Get the inverse rate
    pub fn inverse(&self) -> Decimal {
        if self.rate.is_zero() { Decimal::ZERO } else { Decimal::ONE / self.rate }
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
    pub const fn new(base: Money) -> Self {
        Self { base, prices: Vec::new() }
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
#[non_exhaustive]
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

impl fmt::Display for RoundingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HalfUp => write!(f, "half_up"),
            Self::HalfDown => write!(f, "half_down"),
            Self::Up => write!(f, "up"),
            Self::Down => write!(f, "down"),
            Self::HalfEven => write!(f, "half_even"),
        }
    }
}

impl FromStr for RoundingMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "half_up" | "halfup" | "half-up" => Ok(Self::HalfUp),
            "half_down" | "halfdown" | "half-down" => Ok(Self::HalfDown),
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            "half_even" | "halfeven" | "half-even" | "bankers" | "bankers_rounding" => {
                Ok(Self::HalfEven)
            }
            _ => Err(format!("Unknown rounding mode: {}", s)),
        }
    }
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

    #[test]
    fn test_rounding_mode_from_str() {
        assert_eq!(RoundingMode::from_str("half_up").unwrap(), RoundingMode::HalfUp);
        assert_eq!(RoundingMode::from_str("HalfDown").unwrap(), RoundingMode::HalfDown);
        assert_eq!(RoundingMode::from_str("half-even").unwrap(), RoundingMode::HalfEven);
        assert!(RoundingMode::from_str("nope").is_err());
    }
}
