//! ABI-safe monetary type.
//!
//! Amounts are represented as minor-unit integers (e.g. cents for USD) to
//! avoid floating-point issues across the FFI boundary.

use std::os::raw::c_char;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use stateset_primitives::{CurrencyCode, Money};

use crate::error::{FfiErrorCode, catch_ffi_mut_ptr, clear_last_error, set_last_error};
use crate::strings::rust_to_c_string;

/// ABI-safe monetary amount in minor units with a 3-byte ISO 4217 currency code.
///
/// For example, `$12.34 USD` is represented as `{ amount_cents: 1234, currency: *b"USD" }`
/// while `JPY 42` is represented as `{ amount_cents: 42, currency: *b"JPY" }`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FfiMoney {
    /// Amount in minor units. Field name is historical (`amount_cents`).
    /// Negative values represent credits.
    pub amount_cents: i64,
    /// ISO 4217 three-letter currency code, e.g. `b"USD"`.
    pub currency: [u8; 3],
}

impl FfiMoney {
    /// Create a zero amount in the given currency.
    #[inline]
    #[must_use]
    pub const fn zero(currency: [u8; 3]) -> Self {
        Self { amount_cents: 0, currency }
    }
}

// ---------------------------------------------------------------------------
// Conversions: Money ↔ FfiMoney
// ---------------------------------------------------------------------------

impl TryFrom<Money> for FfiMoney {
    type Error = FfiErrorCode;

    fn try_from(m: Money) -> Result<Self, Self::Error> {
        Self::try_from_money(m)
    }
}

impl FfiMoney {
    /// Fallible conversion from domain [`Money`] into [`FfiMoney`].
    pub(crate) fn try_from_money(m: Money) -> Result<Self, FfiErrorCode> {
        let code = m.currency();
        let code_str = code.as_str();
        let cents = decimal_to_minor_units_for_currency(m.amount(), code_str, "money.amount")?;
        let mut currency = [0u8; 3];
        let code_bytes = code_str.as_bytes();
        let len = code_bytes.len().min(3);
        currency[..len].copy_from_slice(&code_bytes[..len]);
        Ok(Self { amount_cents: cents, currency })
    }
}

impl TryFrom<FfiMoney> for Money {
    type Error = &'static str;

    fn try_from(ffi: FfiMoney) -> Result<Self, Self::Error> {
        let code = CurrencyCode::from_bytes(ffi.currency).ok_or("invalid currency code bytes")?;
        let amount = major_units_from_minor(ffi.amount_cents, code.as_str());
        Ok(Self::new(amount, code))
    }
}

// ---------------------------------------------------------------------------
// Public C API
// ---------------------------------------------------------------------------

/// Format an [`FfiMoney`] as a human-readable string (e.g. `"$12.34"`).
///
/// Currently supports `USD` (`$`), `EUR` (`\u{20ac}`), `GBP` (`\u{00a3}`), and falls
/// back to `"12.34 XYZ"` for other currencies.
///
/// The caller **must** free the returned pointer with `stateset_string_free`.
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub extern "C" fn stateset_money_format(money: FfiMoney) -> *mut c_char {
    catch_ffi_mut_ptr(|| {
        clear_last_error();

        let is_negative = money.amount_cents < 0;
        let absolute = money.amount_cents.unsigned_abs();
        let sign = if is_negative { "-" } else { "" };
        let code = std::str::from_utf8(&money.currency).unwrap_or("???");
        let minor_units = currency_minor_unit_scale(code);
        let factor = minor_unit_factor(minor_units);
        let major = absolute / factor;
        let minor = absolute % factor;
        let width = minor_units as usize;

        let formatted = match code {
            "USD" => {
                if minor_units == 0 {
                    format!("${sign}{major}")
                } else {
                    format!("${sign}{major}.{minor:0width$}")
                }
            }
            "EUR" => {
                if minor_units == 0 {
                    format!("\u{20ac}{sign}{major}")
                } else {
                    format!("\u{20ac}{sign}{major}.{minor:0width$}")
                }
            }
            "GBP" => {
                if minor_units == 0 {
                    format!("\u{00a3}{sign}{major}")
                } else {
                    format!("\u{00a3}{sign}{major}.{minor:0width$}")
                }
            }
            _ => {
                if minor_units == 0 {
                    format!("{sign}{major} {code}")
                } else {
                    format!("{sign}{major}.{minor:0width$} {code}")
                }
            }
        };

        rust_to_c_string(&formatted)
    })
}

pub(crate) fn decimal_to_minor_units_for_currency(
    value: Decimal,
    currency_code: &str,
    field: &str,
) -> Result<i64, FfiErrorCode> {
    let minor_units = currency_minor_unit_scale(currency_code);
    decimal_to_minor_units(value, minor_units, field)
}

fn decimal_to_minor_units(
    value: Decimal,
    minor_units: u32,
    field: &str,
) -> Result<i64, FfiErrorCode> {
    let unit = minor_unit_decimal(minor_units);
    let cents = value.checked_div(unit).ok_or_else(|| {
        set_last_error(&format!(
            "{field} value `{value}` is out of range for i64 minor units or has too much precision"
        ));
        FfiErrorCode::InvalidArgument
    })?;

    if !cents.fract().is_zero() {
        set_last_error(&format!(
            "{field} value `{value}` is out of range for i64 minor units or has too much precision"
        ));
        return Err(FfiErrorCode::InvalidArgument);
    }

    cents.to_i64().ok_or_else(|| {
        set_last_error(&format!(
            "{field} value `{value}` is out of range for i64 minor units or has too much precision"
        ));
        FfiErrorCode::InvalidArgument
    })
}

fn major_units_from_minor(minor_amount: i64, currency_code: &str) -> Decimal {
    let unit = minor_unit_decimal(currency_minor_unit_scale(currency_code));
    Decimal::from(minor_amount) * unit
}

fn minor_unit_decimal(minor_units: u32) -> Decimal {
    Decimal::new(1, minor_units)
}

const fn minor_unit_factor(minor_units: u32) -> u64 {
    10_u64.saturating_pow(minor_units)
}

pub(crate) fn currency_minor_unit_scale(code: &str) -> u32 {
    match code {
        // ISO 4217 zero-decimal currencies (subset commonly used in commerce).
        "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF" | "UGX"
        | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
        // ISO 4217 three-decimal currencies.
        "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
        // Most fiat currencies use two decimals.
        _ => 2,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{clear_last_error, last_error_as_str};
    use rust_decimal_macros::dec;
    use std::ffi::CStr;

    #[test]
    fn money_roundtrip() {
        let money = Money::new(dec!(42.50), CurrencyCode::USD);
        let ffi: FfiMoney = money.try_into().unwrap();
        assert_eq!(ffi.amount_cents, 4250);
        assert_eq!(&ffi.currency, b"USD");

        let back: Money = ffi.try_into().unwrap();
        assert_eq!(back.amount(), dec!(42.50));
        assert_eq!(back.currency(), CurrencyCode::USD);
    }

    #[test]
    fn money_zero() {
        let money = Money::zero(CurrencyCode::EUR);
        let ffi: FfiMoney = money.try_into().unwrap();
        assert_eq!(ffi.amount_cents, 0);
        assert_eq!(&ffi.currency, b"EUR");
    }

    #[test]
    fn money_negative() {
        let money = Money::new(dec!(-5.99), CurrencyCode::USD);
        let ffi: FfiMoney = money.try_into().unwrap();
        assert_eq!(ffi.amount_cents, -599);
    }

    #[test]
    fn money_large_amount() {
        let money = Money::new(dec!(999999.99), CurrencyCode::USD);
        let ffi: FfiMoney = money.try_into().unwrap();
        assert_eq!(ffi.amount_cents, 99999999);
    }

    #[test]
    fn money_roundtrip_jpy_uses_zero_minor_units() {
        let money = Money::new(dec!(42), CurrencyCode::JPY);
        let ffi: FfiMoney = money.try_into().unwrap();
        assert_eq!(ffi.amount_cents, 42);
        assert_eq!(&ffi.currency, b"JPY");

        let back: Money = ffi.try_into().unwrap();
        assert_eq!(back.amount(), dec!(42));
        assert_eq!(back.currency(), CurrencyCode::JPY);
    }

    #[test]
    fn money_roundtrip_three_decimal_currency() {
        let kwd = CurrencyCode::from_bytes(*b"KWD").unwrap();
        let money = Money::new(dec!(1.234), kwd);
        let ffi: FfiMoney = money.try_into().unwrap();
        assert_eq!(ffi.amount_cents, 1234);

        let back: Money = ffi.try_into().unwrap();
        assert_eq!(back.amount(), dec!(1.234));
        assert_eq!(back.currency(), kwd);
    }

    #[test]
    fn ffi_money_zero_helper() {
        let z = FfiMoney::zero(*b"GBP");
        assert_eq!(z.amount_cents, 0);
        assert_eq!(&z.currency, b"GBP");
    }

    #[test]
    fn ffi_money_default() {
        let d = FfiMoney::default();
        assert_eq!(d.amount_cents, 0);
        assert_eq!(d.currency, [0u8; 3]);
    }

    #[test]
    fn ffi_money_invalid_currency() {
        let ffi = FfiMoney { amount_cents: 100, currency: [0x01, 0x02, 0x03] };
        let result = Money::try_from(ffi);
        assert!(result.is_err());
    }

    #[test]
    fn ffi_money_format_usd() {
        let ffi = FfiMoney { amount_cents: 1234, currency: *b"USD" };
        let ptr = stateset_money_format(ffi);
        assert!(!ptr.is_null());

        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "$12.34");

        unsafe { crate::strings::stateset_string_free(ptr) };
    }

    #[test]
    fn ffi_money_format_eur() {
        let ffi = FfiMoney { amount_cents: 500, currency: *b"EUR" };
        let ptr = stateset_money_format(ffi);
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "\u{20ac}5.00");
        unsafe { crate::strings::stateset_string_free(ptr) };
    }

    #[test]
    fn ffi_money_format_gbp() {
        let ffi = FfiMoney { amount_cents: 99, currency: *b"GBP" };
        let ptr = stateset_money_format(ffi);
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "\u{00a3}0.99");
        unsafe { crate::strings::stateset_string_free(ptr) };
    }

    #[test]
    fn ffi_money_format_other() {
        let ffi = FfiMoney { amount_cents: 42, currency: *b"JPY" };
        let ptr = stateset_money_format(ffi);
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "42 JPY");
        unsafe { crate::strings::stateset_string_free(ptr) };
    }

    #[test]
    fn ffi_money_format_negative() {
        let ffi = FfiMoney { amount_cents: -1050, currency: *b"USD" };
        let ptr = stateset_money_format(ffi);
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "$-10.50");
        unsafe { crate::strings::stateset_string_free(ptr) };
    }

    #[test]
    fn ffi_money_format_negative_subunit() {
        let ffi = FfiMoney { amount_cents: -50, currency: *b"USD" };
        let ptr = stateset_money_format(ffi);
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "$-0.50");
        unsafe { crate::strings::stateset_string_free(ptr) };
    }

    #[test]
    fn ffi_money_debug() {
        let ffi = FfiMoney { amount_cents: 100, currency: *b"USD" };
        let debug = format!("{:?}", ffi);
        assert!(debug.contains("FfiMoney"));
        assert!(debug.contains("100"));
    }

    #[test]
    fn ffi_money_eq_and_clone() {
        let a = FfiMoney { amount_cents: 100, currency: *b"USD" };
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn money_try_from_overflow_is_rejected() {
        let money = Money::new(Decimal::MAX, CurrencyCode::USD);
        clear_last_error();
        let err = FfiMoney::try_from_money(money).unwrap_err();
        assert_eq!(err, FfiErrorCode::InvalidArgument);
        let msg = last_error_as_str().unwrap();
        assert!(msg.contains("money.amount"));
    }

    #[test]
    fn money_try_from_excess_precision_is_rejected() {
        let money = Money::new(dec!(1.999), CurrencyCode::USD);
        clear_last_error();
        let err = FfiMoney::try_from_money(money).unwrap_err();
        assert_eq!(err, FfiErrorCode::InvalidArgument);
        let msg = last_error_as_str().unwrap();
        assert!(msg.contains("money.amount"));
    }
}
