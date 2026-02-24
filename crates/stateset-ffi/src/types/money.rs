//! ABI-safe monetary type.
//!
//! Amounts are represented as minor-unit integers (e.g. cents for USD) to
//! avoid floating-point issues across the FFI boundary.

use std::os::raw::c_char;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use stateset_primitives::{CurrencyCode, Money};

use crate::error::clear_last_error;
use crate::strings::rust_to_c_string;

/// ABI-safe monetary amount in minor units with a 3-byte ISO 4217 currency code.
///
/// For example, `$12.34 USD` is represented as `{ amount_cents: 1234, currency: *b"USD" }`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FfiMoney {
    /// Amount in minor units (e.g. cents). Negative values represent credits.
    pub amount_cents: i64,
    /// ISO 4217 three-letter currency code, e.g. `b"USD"`.
    pub currency: [u8; 3],
}

impl FfiMoney {
    /// Create a zero amount in the given currency.
    #[inline]
    pub const fn zero(currency: [u8; 3]) -> Self {
        Self { amount_cents: 0, currency }
    }
}

// ---------------------------------------------------------------------------
// Conversions: Money ↔ FfiMoney
// ---------------------------------------------------------------------------

impl From<Money> for FfiMoney {
    fn from(m: Money) -> Self {
        // Convert the decimal to cents (minor units).
        let cents = (m.amount() * Decimal::from(100)).to_i64().unwrap_or(0);

        let mut currency = [0u8; 3];
        let code = m.currency();
        let code_str = code.as_str();
        let code_bytes = code_str.as_bytes();
        let len = code_bytes.len().min(3);
        currency[..len].copy_from_slice(&code_bytes[..len]);

        Self { amount_cents: cents, currency }
    }
}

impl TryFrom<FfiMoney> for Money {
    type Error = &'static str;

    fn try_from(ffi: FfiMoney) -> Result<Self, Self::Error> {
        let code = CurrencyCode::from_bytes(ffi.currency).ok_or("invalid currency code bytes")?;
        let amount = Decimal::from(ffi.amount_cents) / Decimal::from(100);
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
/// The caller **must** free the returned pointer with [`stateset_string_free`].
#[unsafe(no_mangle)]
#[allow(unsafe_code)]
pub extern "C" fn stateset_money_format(money: FfiMoney) -> *mut c_char {
    clear_last_error();

    let major = money.amount_cents / 100;
    let minor = (money.amount_cents % 100).unsigned_abs();
    let code = std::str::from_utf8(&money.currency).unwrap_or("???");

    let formatted = match code {
        "USD" => format!("${major}.{minor:02}"),
        "EUR" => format!("\u{20ac}{major}.{minor:02}"),
        "GBP" => format!("\u{00a3}{major}.{minor:02}"),
        _ => format!("{major}.{minor:02} {code}"),
    };

    rust_to_c_string(&formatted)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::ffi::CStr;

    #[test]
    fn money_roundtrip() {
        let money = Money::new(dec!(42.50), CurrencyCode::USD);
        let ffi: FfiMoney = money.into();
        assert_eq!(ffi.amount_cents, 4250);
        assert_eq!(&ffi.currency, b"USD");

        let back: Money = ffi.try_into().unwrap();
        assert_eq!(back.amount(), dec!(42.50));
        assert_eq!(back.currency(), CurrencyCode::USD);
    }

    #[test]
    fn money_zero() {
        let money = Money::zero(CurrencyCode::EUR);
        let ffi: FfiMoney = money.into();
        assert_eq!(ffi.amount_cents, 0);
        assert_eq!(&ffi.currency, b"EUR");
    }

    #[test]
    fn money_negative() {
        let money = Money::new(dec!(-5.99), CurrencyCode::USD);
        let ffi: FfiMoney = money.into();
        assert_eq!(ffi.amount_cents, -599);
    }

    #[test]
    fn money_large_amount() {
        let money = Money::new(dec!(999999.99), CurrencyCode::USD);
        let ffi: FfiMoney = money.into();
        assert_eq!(ffi.amount_cents, 99999999);
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

        crate::strings::stateset_string_free(ptr);
    }

    #[test]
    fn ffi_money_format_eur() {
        let ffi = FfiMoney { amount_cents: 500, currency: *b"EUR" };
        let ptr = stateset_money_format(ffi);
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "\u{20ac}5.00");
        crate::strings::stateset_string_free(ptr);
    }

    #[test]
    fn ffi_money_format_gbp() {
        let ffi = FfiMoney { amount_cents: 99, currency: *b"GBP" };
        let ptr = stateset_money_format(ffi);
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "\u{00a3}0.99");
        crate::strings::stateset_string_free(ptr);
    }

    #[test]
    fn ffi_money_format_other() {
        let ffi = FfiMoney { amount_cents: 4200, currency: *b"JPY" };
        let ptr = stateset_money_format(ffi);
        let s = unsafe { CStr::from_ptr(ptr) };
        assert_eq!(s.to_str().unwrap(), "42.00 JPY");
        crate::strings::stateset_string_free(ptr);
    }

    #[test]
    fn ffi_money_format_negative() {
        let ffi = FfiMoney { amount_cents: -1050, currency: *b"USD" };
        let ptr = stateset_money_format(ffi);
        let s = unsafe { CStr::from_ptr(ptr) };
        // -1050 / 100 = -10, abs(-1050 % 100) = 50
        assert_eq!(s.to_str().unwrap(), "$-10.50");
        crate::strings::stateset_string_free(ptr);
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
}
