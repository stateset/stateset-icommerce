//! Checked arithmetic shared by all fallible pricing paths.

use crate::{PricingError, PricingResult};
use rust_decimal::Decimal;

pub(crate) fn add(a: Decimal, b: Decimal) -> PricingResult<Decimal> {
    a.checked_add(b).ok_or_else(|| PricingError::overflow("addition"))
}

pub(crate) fn sub(a: Decimal, b: Decimal) -> PricingResult<Decimal> {
    a.checked_sub(b).ok_or_else(|| PricingError::overflow("subtraction"))
}

pub(crate) fn mul(a: Decimal, b: Decimal) -> PricingResult<Decimal> {
    a.checked_mul(b).ok_or_else(|| PricingError::overflow("multiplication"))
}

pub(crate) fn div(a: Decimal, b: Decimal) -> PricingResult<Decimal> {
    a.checked_div(b).ok_or_else(|| PricingError::overflow("division"))
}

pub(crate) fn sum(values: impl IntoIterator<Item = Decimal>) -> PricingResult<Decimal> {
    values.into_iter().try_fold(Decimal::ZERO, add)
}
