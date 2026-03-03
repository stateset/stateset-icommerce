//! Shipping zone and rate domain models
//!
//! Defines geographic shipping zones with configurable shipping methods
//! and rate calculation strategies.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, ShippingMethodId, ShippingZoneId};
use strum::{Display, EnumString};

/// Shipping method pricing type
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ShippingMethodType {
    /// Fixed rate regardless of order details
    #[default]
    Flat,
    /// Rate varies by total weight
    WeightBased,
    /// Rate varies by order total price
    PriceBased,
    /// Rate calculated by external carrier API
    Calculated,
    /// Free shipping (no charge)
    Free,
}

/// A geographic shipping zone
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShippingZone {
    /// Unique zone ID
    pub id: ShippingZoneId,
    /// Human-readable zone name (e.g., "Domestic", "EU", "Rest of World")
    pub name: String,
    /// ISO 3166-1 alpha-2 country codes included in this zone
    pub countries: Vec<String>,
    /// State/province/region codes within the countries
    pub regions: Vec<String>,
    /// Postal/zip code patterns (supports wildcards like "90*")
    pub postal_codes: Vec<String>,
    /// Priority for zone matching (lower = higher priority)
    pub priority: i32,
    /// Whether this zone is active
    pub is_active: bool,
    /// When the zone was created
    pub created_at: DateTime<Utc>,
    /// When the zone was last updated
    pub updated_at: DateTime<Utc>,
}

/// A configurable shipping method within a zone (distinct from the simple
/// `ShippingMethod` enum in the `shipment` module which represents speed tiers).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneShippingMethod {
    /// Unique method ID
    pub id: ShippingMethodId,
    /// Zone this method belongs to
    pub zone_id: ShippingZoneId,
    /// Method name (e.g., "Standard Shipping", "Express")
    pub name: String,
    /// Carrier name (e.g., "USPS", "`FedEx`", "DHL")
    pub carrier: Option<String>,
    /// Pricing type
    pub method_type: ShippingMethodType,
    /// Base rate (used for Flat; minimum for weight/price-based)
    pub base_rate: Decimal,
    /// Currency code
    pub currency: CurrencyCode,
    /// Estimated minimum delivery days
    pub min_delivery_days: Option<i32>,
    /// Estimated maximum delivery days
    pub max_delivery_days: Option<i32>,
    /// Conditional rate tiers
    pub conditions: Vec<ShippingCondition>,
    /// Whether this method is active
    pub is_active: bool,
    /// When the method was created
    pub created_at: DateTime<Utc>,
    /// When the method was last updated
    pub updated_at: DateTime<Utc>,
}

/// A conditional rate tier for weight-based or price-based shipping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShippingCondition {
    /// Minimum weight in grams (for weight-based)
    pub min_weight: Option<Decimal>,
    /// Maximum weight in grams (for weight-based)
    pub max_weight: Option<Decimal>,
    /// Minimum order price (for price-based)
    pub min_price: Option<Decimal>,
    /// Maximum order price (for price-based)
    pub max_price: Option<Decimal>,
    /// Rate for this tier
    pub rate: Decimal,
}

/// Input for creating a shipping zone
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateShippingZone {
    /// Zone name
    pub name: String,
    /// Country codes
    pub countries: Vec<String>,
    /// Region codes
    pub regions: Vec<String>,
    /// Postal code patterns
    pub postal_codes: Vec<String>,
    /// Priority (default 0)
    pub priority: Option<i32>,
}

/// Input for updating a shipping zone
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateShippingZone {
    /// Updated name
    pub name: Option<String>,
    /// Updated countries
    pub countries: Option<Vec<String>>,
    /// Updated regions
    pub regions: Option<Vec<String>>,
    /// Updated postal codes
    pub postal_codes: Option<Vec<String>>,
    /// Updated priority
    pub priority: Option<i32>,
    /// Updated active status
    pub is_active: Option<bool>,
}

/// Input for creating a zone shipping method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateZoneShippingMethod {
    /// Zone this method belongs to
    pub zone_id: ShippingZoneId,
    /// Method name
    pub name: String,
    /// Carrier name
    pub carrier: Option<String>,
    /// Pricing type
    pub method_type: ShippingMethodType,
    /// Base rate
    pub base_rate: Decimal,
    /// Currency code
    pub currency: CurrencyCode,
    /// Min delivery days
    pub min_delivery_days: Option<i32>,
    /// Max delivery days
    pub max_delivery_days: Option<i32>,
    /// Conditional rate tiers
    pub conditions: Vec<ShippingCondition>,
}

/// Filter for listing shipping zones
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShippingZoneFilter {
    /// Filter by country code
    pub country: Option<String>,
    /// Filter by active status
    pub is_active: Option<bool>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

/// Filter for listing zone shipping methods
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ZoneShippingMethodFilter {
    /// Filter by zone
    pub zone_id: Option<ShippingZoneId>,
    /// Filter by carrier
    pub carrier: Option<String>,
    /// Filter by method type
    pub method_type: Option<ShippingMethodType>,
    /// Filter by active status
    pub is_active: Option<bool>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

/// Shipping rate calculation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneShippingRateRequest {
    /// Destination country code
    pub country: String,
    /// Destination region/state
    pub region: Option<String>,
    /// Destination postal code
    pub postal_code: Option<String>,
    /// Total order weight in grams
    pub weight: Option<Decimal>,
    /// Total order price
    pub order_total: Option<Decimal>,
    /// Currency
    pub currency: CurrencyCode,
}

/// Calculated shipping rate result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneShippingRate {
    /// The shipping method
    pub method_id: ShippingMethodId,
    /// Method name
    pub method_name: String,
    /// Carrier
    pub carrier: Option<String>,
    /// Calculated rate
    pub rate: Decimal,
    /// Currency
    pub currency: CurrencyCode,
    /// Estimated min delivery days
    pub min_delivery_days: Option<i32>,
    /// Estimated max delivery days
    pub max_delivery_days: Option<i32>,
}

impl ZoneShippingMethod {
    /// Calculate the shipping rate for the given conditions
    pub fn calculate_rate(&self, weight: Option<Decimal>, order_total: Option<Decimal>) -> Decimal {
        match self.method_type {
            ShippingMethodType::Free => Decimal::ZERO,
            ShippingMethodType::Flat | ShippingMethodType::Calculated => self.base_rate,
            ShippingMethodType::WeightBased => {
                if let Some(w) = weight {
                    for condition in &self.conditions {
                        let above_min = condition.min_weight.is_none_or(|min| w >= min);
                        let below_max = condition.max_weight.is_none_or(|max| w <= max);
                        if above_min && below_max {
                            return condition.rate;
                        }
                    }
                }
                self.base_rate
            }
            ShippingMethodType::PriceBased => {
                if let Some(total) = order_total {
                    for condition in &self.conditions {
                        let above_min = condition.min_price.is_none_or(|min| total >= min);
                        let below_max = condition.max_price.is_none_or(|max| total <= max);
                        if above_min && below_max {
                            return condition.rate;
                        }
                    }
                }
                self.base_rate
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use stateset_primitives::{CurrencyCode, ShippingMethodId, ShippingZoneId};

    fn make_method(
        method_type: ShippingMethodType,
        base_rate: Decimal,
        conditions: Vec<ShippingCondition>,
    ) -> ZoneShippingMethod {
        ZoneShippingMethod {
            id: ShippingMethodId::new(),
            zone_id: ShippingZoneId::new(),
            name: "Test Method".to_string(),
            carrier: Some("USPS".to_string()),
            method_type,
            base_rate,
            currency: CurrencyCode::USD,
            min_delivery_days: Some(3),
            max_delivery_days: Some(7),
            conditions,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ---- calculate_rate: flat ----

    #[test]
    fn calculate_rate_flat_returns_base_rate() {
        let method = make_method(ShippingMethodType::Flat, dec!(5.99), vec![]);
        assert_eq!(method.calculate_rate(None, None), dec!(5.99));
    }

    // ---- calculate_rate: free ----

    #[test]
    fn calculate_rate_free_returns_zero() {
        let method = make_method(ShippingMethodType::Free, dec!(5.99), vec![]);
        assert_eq!(method.calculate_rate(None, None), Decimal::ZERO);
    }

    // ---- calculate_rate: weight-based ----

    #[test]
    fn calculate_rate_weight_based_matches_condition() {
        let conditions = vec![
            ShippingCondition {
                min_weight: Some(dec!(0)),
                max_weight: Some(dec!(500)),
                min_price: None,
                max_price: None,
                rate: dec!(3.99),
            },
            ShippingCondition {
                min_weight: Some(dec!(501)),
                max_weight: Some(dec!(2000)),
                min_price: None,
                max_price: None,
                rate: dec!(7.99),
            },
        ];
        let method = make_method(ShippingMethodType::WeightBased, dec!(9.99), conditions);
        assert_eq!(method.calculate_rate(Some(dec!(300)), None), dec!(3.99));
        assert_eq!(method.calculate_rate(Some(dec!(1000)), None), dec!(7.99));
    }

    #[test]
    fn calculate_rate_weight_based_falls_back_to_base_rate() {
        let method = make_method(ShippingMethodType::WeightBased, dec!(9.99), vec![]);
        // No conditions match — should return base_rate
        assert_eq!(method.calculate_rate(Some(dec!(300)), None), dec!(9.99));
    }

    #[test]
    fn calculate_rate_weight_based_falls_back_when_no_weight_provided() {
        let conditions = vec![ShippingCondition {
            min_weight: Some(dec!(0)),
            max_weight: Some(dec!(1000)),
            min_price: None,
            max_price: None,
            rate: dec!(3.99),
        }];
        let method = make_method(ShippingMethodType::WeightBased, dec!(9.99), conditions);
        // weight is None — should fall back to base_rate
        assert_eq!(method.calculate_rate(None, None), dec!(9.99));
    }

    // ---- calculate_rate: price-based ----

    #[test]
    fn calculate_rate_price_based_free_over_threshold() {
        let conditions = vec![
            ShippingCondition {
                min_weight: None,
                max_weight: None,
                min_price: Some(dec!(75.00)),
                max_price: None,
                rate: dec!(0.00),
            },
            ShippingCondition {
                min_weight: None,
                max_weight: None,
                min_price: Some(dec!(0.00)),
                max_price: Some(dec!(74.99)),
                rate: dec!(5.99),
            },
        ];
        let method = make_method(ShippingMethodType::PriceBased, dec!(5.99), conditions);
        assert_eq!(method.calculate_rate(None, Some(dec!(100.00))), dec!(0.00));
        assert_eq!(method.calculate_rate(None, Some(dec!(50.00))), dec!(5.99));
    }

    // ---- enum Display / FromStr round-trips ----

    #[test]
    fn shipping_method_type_display_fromstr_roundtrip() {
        for method_type in [
            ShippingMethodType::Flat,
            ShippingMethodType::WeightBased,
            ShippingMethodType::PriceBased,
            ShippingMethodType::Calculated,
            ShippingMethodType::Free,
        ] {
            let s = method_type.to_string();
            let parsed: ShippingMethodType = s.parse().unwrap();
            assert_eq!(parsed, method_type, "round-trip failed for {s}");
        }
    }

    // ---- Default ----

    #[test]
    fn shipping_method_type_default_is_flat() {
        assert_eq!(ShippingMethodType::default(), ShippingMethodType::Flat);
    }
}
