//! Real carrier rates, labels, and tracking.
//!
//! Shipments used to end at a locally recorded tracking number
//! (`ship(id, tracking)`); this module is the rail to an actual carrier.
//! A [`ShipmentProvider`] rates a parcel, buys a label (real money movement
//! on the carrier account), and follows tracking. The [`Shipments`] facade
//! persists the outcome — tracking number, postage cost, and a
//! `label_purchased` event — so the shipment stays the book of record.
//!
//! [`Shipments`]: crate::shipments::Shipments
//!
//! The production provider is [`EasyPostProvider`] (one API across carriers;
//! test vs live is purely the key). [`MockShipmentProvider`] is the
//! deterministic stand-in for tests, demos, and CI without network.
//!
//! Keys stay in the environment (`EASYPOST_TEST_KEY` / `EASYPOST_LIVE_KEY`);
//! nothing here reads process environment.

use rust_decimal::Decimal;
use serde::Deserialize;
use stateset_core::{CommerceError, Result};

/// Structured postal address. The engine's shipment row keeps a freeform
/// display string; the carrier needs fields, so callers pass this in.
#[derive(Debug, Clone, Default)]
pub struct PostalAddress {
    pub name: Option<String>,
    pub street1: String,
    pub street2: Option<String>,
    pub city: String,
    pub state: String,
    pub zip: String,
    pub country: String,
    pub phone: Option<String>,
    pub email: Option<String>,
}

/// Parcel as the carrier sees it.
#[derive(Debug, Clone)]
pub struct Parcel {
    /// Weight in ounces.
    pub weight_oz: Decimal,
    pub length_in: Option<Decimal>,
    pub width_in: Option<Decimal>,
    pub height_in: Option<Decimal>,
}

/// One priced carrier service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateQuote {
    /// Provider-native rate id (e.g. `EasyPost` `rate_...`).
    pub id: String,
    pub carrier: String,
    pub service: String,
    pub amount: Decimal,
    pub currency: String,
    /// Promised transit days, when the carrier states one.
    pub delivery_days: Option<i32>,
}

/// A bought label: money moved on the carrier account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurchasedLabel {
    /// Provider shipment id (e.g. `EasyPost` `shp_...`).
    pub shipment_id: String,
    pub tracking_number: String,
    /// Printable label URL (receipt artifact; also stored as a shipment event).
    pub label_url: Option<String>,
    pub carrier: String,
    pub service: String,
    pub rate_id: String,
    pub amount: Decimal,
    pub currency: String,
}

/// Upstream carrier. Synchronous like [`crate::payment_providers`]: the
/// engine is single-process and `reqwest::blocking` is already on board.
pub trait ShipmentProvider {
    /// Human name for logs and audit (`"easypost"`, `"mock"`).
    fn name(&self) -> &'static str;
    /// Price the parcel across carriers without buying anything.
    fn rates(
        &self,
        from: &PostalAddress,
        to: &PostalAddress,
        parcel: &Parcel,
    ) -> Result<Vec<RateQuote>>;
    /// Buy `rate_id` (from [`ShipmentProvider::rates`]) and return the label.
    ///
    /// Providers whose rate ids belong to an upstream shipment (`EasyPost`)
    /// buy on that shipment; `from` / `to` / `parcel` are for providers
    /// that rate and buy in one call.
    fn buy_label(
        &self,
        from: &PostalAddress,
        to: &PostalAddress,
        parcel: &Parcel,
        rate_id: &str,
    ) -> Result<PurchasedLabel>;
    /// Latest status plus the carrier's scan history, newest last.
    fn tracking(&self, tracking_number: &str, carrier: &str) -> Result<TrackingStatus>;
}

/// Carrier scan history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackingStatus {
    pub status: String,
    pub events: Vec<TrackingEvent>,
}

/// One carrier scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackingEvent {
    pub occurred_at: Option<String>,
    pub description: String,
    pub location: Option<String>,
}

// ============================================================================
// EasyPost (REST v2, basic auth with the API key)
// ============================================================================

#[derive(Debug, Deserialize)]
struct EasyPostRate {
    id: String,
    carrier: String,
    service: String,
    rate: String,
    currency: String,
    #[serde(default)]
    delivery_days: Option<i32>,
    /// The `EasyPost` shipment this rate was quoted on; rates are only
    /// purchasable on their own shipment.
    #[serde(default)]
    shipment_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EasyPostTrackerList {
    #[serde(default)]
    trackers: Vec<EasyPostTracker>,
}

#[derive(Debug, Deserialize)]
struct EasyPostShipment {
    id: String,
    #[serde(default)]
    rates: Vec<EasyPostRate>,
    #[serde(default)]
    tracking_code: Option<String>,
    #[serde(default)]
    postage_label: Option<EasyPostLabel>,
    #[serde(default)]
    selected_rate: Option<EasyPostRate>,
}

#[derive(Debug, Deserialize)]
struct EasyPostLabel {
    #[serde(default)]
    label_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EasyPostTracker {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    tracking_details: Vec<EasyPostTrackingDetail>,
}

#[derive(Debug, Deserialize)]
struct EasyPostTrackingDetail {
    #[serde(default)]
    datetime: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    tracking_location: Option<EasyPostTrackingLocation>,
}

#[derive(Debug, Deserialize)]
struct EasyPostTrackingLocation {
    #[serde(default)]
    city: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    country: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EasyPostErrorEnvelope {
    #[serde(default)]
    error: Option<EasyPostErrorBody>,
}

#[derive(Debug, Deserialize)]
struct EasyPostErrorBody {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// `EasyPost` provider: rates, labels, and tracking across carriers.
///
/// Test vs live is purely the key (`EK_TEST_…` vs `EK_LIVE_…`).
#[derive(Debug, Clone)]
pub struct EasyPostProvider {
    api_key: String,
    base_url: String,
    client: reqwest::blocking::Client,
}

impl EasyPostProvider {
    /// Production endpoint with the given key.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| CommerceError::ExternalServiceError(format!("easypost client: {e}")))?;
        Ok(Self { api_key: api_key.into(), base_url: "https://api.easypost.com/v2".into(), client })
    }

    /// Point at a different base (tests, mock server).
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    fn shipment_form(
        from: &PostalAddress,
        to: &PostalAddress,
        parcel: &Parcel,
    ) -> Vec<(String, String)> {
        let mut form = vec![
            ("shipment[to_address][street1]".to_string(), to.street1.clone()),
            ("shipment[to_address][city]".to_string(), to.city.clone()),
            ("shipment[to_address][state]".to_string(), to.state.clone()),
            ("shipment[to_address][zip]".to_string(), to.zip.clone()),
            ("shipment[to_address][country]".to_string(), to.country.clone()),
            ("shipment[from_address][street1]".to_string(), from.street1.clone()),
            ("shipment[from_address][city]".to_string(), from.city.clone()),
            ("shipment[from_address][state]".to_string(), from.state.clone()),
            ("shipment[from_address][zip]".to_string(), from.zip.clone()),
            ("shipment[from_address][country]".to_string(), from.country.clone()),
            ("shipment[parcel][weight]".to_string(), parcel.weight_oz.to_string()),
        ];
        let mut opt = |key: &str, value: &Option<String>| {
            if let Some(value) = value {
                form.push((key.to_string(), value.clone()));
            }
        };
        opt("shipment[to_address][name]", &to.name);
        opt("shipment[to_address][street2]", &to.street2);
        opt("shipment[to_address][phone]", &to.phone);
        opt("shipment[to_address][email]", &to.email);
        opt("shipment[from_address][name]", &from.name);
        opt("shipment[from_address][street2]", &from.street2);
        opt("shipment[from_address][phone]", &from.phone);
        opt("shipment[from_address][email]", &from.email);
        for (key, value) in [
            ("shipment[parcel][length]", &parcel.length_in),
            ("shipment[parcel][width]", &parcel.width_in),
            ("shipment[parcel][height]", &parcel.height_in),
        ] {
            if let Some(value) = value {
                form.push((key.to_string(), value.to_string()));
            }
        }
        form
    }

    fn post(&self, path: &str, form: &[(String, String)]) -> Result<String> {
        let body = self
            .client
            .post(format!("{}{path}", self.base_url))
            .basic_auth(&self.api_key, Option::<&str>::None)
            .form(&form)
            .send()
            .map_err(|e| CommerceError::ExternalServiceError(format!("easypost request: {e}")))?;
        Self::read("POST", path, body)
    }

    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<String> {
        let body = self
            .client
            .get(format!("{}{path}", self.base_url))
            .basic_auth(&self.api_key, Option::<&str>::None)
            .query(query)
            .send()
            .map_err(|e| CommerceError::ExternalServiceError(format!("easypost request: {e}")))?;
        Self::read("GET", path, body)
    }

    fn read(method: &str, path: &str, body: reqwest::blocking::Response) -> Result<String> {
        let status = body.status();
        let text = body
            .text()
            .map_err(|e| CommerceError::ExternalServiceError(format!("easypost read: {e}")))?;
        if !status.is_success() {
            let envelope: Option<EasyPostErrorEnvelope> = serde_json::from_str(&text).ok();
            let (code, message) = envelope
                .and_then(|env| env.error)
                .map(|e| {
                    (
                        e.code.unwrap_or_else(|| status.to_string()),
                        e.message.unwrap_or_else(|| "easypost error".to_string()),
                    )
                })
                .unwrap_or_else(|| (status.to_string(), text.chars().take(300).collect()));
            return Err(CommerceError::ExternalServiceError(format!(
                "easypost {method} {path} failed ({code}): {message}"
            )));
        }
        Ok(text)
    }

    fn first_tracker(text: &str, tracking_number: &str) -> Result<EasyPostTracker> {
        let list: EasyPostTrackerList = serde_json::from_str(text)
            .map_err(|e| CommerceError::ExternalServiceError(format!("easypost decode: {e}")))?;
        list.trackers.into_iter().next().ok_or_else(|| {
            CommerceError::ExternalServiceError(format!(
                "easypost has no tracker for {tracking_number}"
            ))
        })
    }

    fn quote(rate: EasyPostRate) -> Result<RateQuote> {
        Ok(RateQuote {
            id: rate.id,
            carrier: rate.carrier,
            service: rate.service,
            amount: rate.rate.parse().map_err(|_| {
                CommerceError::ExternalServiceError(format!("easypost bad rate {}", rate.rate))
            })?,
            currency: rate.currency,
            delivery_days: rate.delivery_days,
        })
    }
}

impl ShipmentProvider for EasyPostProvider {
    fn name(&self) -> &'static str {
        "easypost"
    }

    fn rates(
        &self,
        from: &PostalAddress,
        to: &PostalAddress,
        parcel: &Parcel,
    ) -> Result<Vec<RateQuote>> {
        let text = self.post("/shipments", &Self::shipment_form(from, to, parcel))?;
        let shipment: EasyPostShipment = serde_json::from_str(&text)
            .map_err(|e| CommerceError::ExternalServiceError(format!("easypost decode: {e}")))?;
        shipment.rates.into_iter().map(Self::quote).collect()
    }

    fn buy_label(
        &self,
        _from: &PostalAddress,
        _to: &PostalAddress,
        _parcel: &Parcel,
        rate_id: &str,
    ) -> Result<PurchasedLabel> {
        // EasyPost rate ids are scoped to the shipment they were quoted on.
        // Buying on a freshly created shipment would be rejected (and orphan
        // a shipment per attempt), so resolve the rate's own shipment.
        let rate: EasyPostRate =
            serde_json::from_str(&self.get(&format!("/rates/{rate_id}"), &[])?).map_err(|e| {
                CommerceError::ExternalServiceError(format!("easypost decode: {e}"))
            })?;
        let shipment_id = rate.shipment_id.ok_or_else(|| {
            CommerceError::ExternalServiceError(format!("easypost rate {rate_id} has no shipment"))
        })?;
        let text = self.post(
            &format!("/shipments/{shipment_id}/buy"),
            &[("rate[id]".to_string(), rate_id.to_string())],
        )?;
        let bought: EasyPostShipment = serde_json::from_str(&text)
            .map_err(|e| CommerceError::ExternalServiceError(format!("easypost decode: {e}")))?;
        let rate = bought.selected_rate.ok_or_else(|| {
            CommerceError::ExternalServiceError("easypost buy returned no selected rate".into())
        })?;
        let tracking_number = bought.tracking_code.ok_or_else(|| {
            CommerceError::ExternalServiceError("easypost buy returned no tracking code".into())
        })?;
        let quoted = Self::quote(rate)?;
        Ok(PurchasedLabel {
            shipment_id: bought.id,
            tracking_number,
            label_url: bought.postage_label.and_then(|label| label.label_url),
            carrier: quoted.carrier,
            service: quoted.service,
            rate_id: quoted.id,
            amount: quoted.amount,
            currency: quoted.currency,
        })
    }

    fn tracking(&self, tracking_number: &str, carrier: &str) -> Result<TrackingStatus> {
        let text =
            self.get("/trackers", &[("tracking_code", tracking_number), ("carrier", carrier)])?;
        // The list endpoint wraps results: `{"trackers": [...]}`.
        let tracker = Self::first_tracker(&text, tracking_number)?;
        Ok(TrackingStatus {
            status: tracker.status.unwrap_or_else(|| "unknown".to_string()),
            events: tracker
                .tracking_details
                .into_iter()
                .map(|detail| {
                    let location = detail.tracking_location.map(|loc| {
                        [loc.city, loc.state, loc.country]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .join(", ")
                    });
                    TrackingEvent {
                        occurred_at: detail.datetime,
                        description: detail.message.unwrap_or_else(|| "carrier scan".to_string()),
                        location,
                    }
                })
                .collect(),
        })
    }
}

// ============================================================================
// Mock (tests, demos, CI without network)
// ============================================================================

/// Scripted in-memory carrier. Rates and labels come from queues; every call
/// is recorded so tests can assert the buy-before-ship ordering.
#[derive(Debug, Default)]
pub struct MockShipmentProvider {
    calls: std::sync::Mutex<Vec<String>>,
    rates: std::sync::Mutex<Vec<RateQuote>>,
    labels: std::sync::Mutex<Vec<PurchasedLabel>>,
}

impl MockShipmentProvider {
    /// Serve these rates (in order, last repeats) and these labels.
    pub const fn new(rates: Vec<RateQuote>, labels: Vec<PurchasedLabel>) -> Self {
        Self {
            calls: std::sync::Mutex::new(Vec::new()),
            rates: std::sync::Mutex::new(rates),
            labels: std::sync::Mutex::new(labels),
        }
    }

    /// What was called, in order (`"rates"`, `"buy:<rate_id>"`, …).
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("mock calls").clone()
    }
}

impl ShipmentProvider for MockShipmentProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn rates(
        &self,
        _from: &PostalAddress,
        _to: &PostalAddress,
        _parcel: &Parcel,
    ) -> Result<Vec<RateQuote>> {
        self.calls.lock().expect("mock calls").push("rates".into());
        Ok(self.rates.lock().expect("mock rates").clone())
    }

    fn buy_label(
        &self,
        _from: &PostalAddress,
        _to: &PostalAddress,
        _parcel: &Parcel,
        rate_id: &str,
    ) -> Result<PurchasedLabel> {
        self.calls.lock().expect("mock calls").push(format!("buy:{rate_id}"));
        self.labels
            .lock()
            .expect("mock labels")
            .first()
            .cloned()
            .ok_or_else(|| CommerceError::ValidationError("mock has no scripted label".into()))
    }

    fn tracking(&self, tracking_number: &str, carrier: &str) -> Result<TrackingStatus> {
        self.calls
            .lock()
            .expect("mock calls")
            .push(format!("tracking:{tracking_number}:{carrier}"));
        Ok(TrackingStatus { status: "in_transit".into(), events: Vec::new() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn easypost_rate_and_label_decode() {
        let shipment: EasyPostShipment = serde_json::from_str(
            r#"{"id":"shp_1","rates":[{"id":"rate_1","carrier":"USPS","service":"GroundAdvantage","rate":"7.34","currency":"USD","delivery_days":4}]}"#,
        )
        .unwrap();
        let quote = EasyPostProvider::quote(shipment.rates.into_iter().next().unwrap()).unwrap();
        assert_eq!(quote.amount, dec!(7.34));
        assert_eq!(quote.carrier, "USPS");
    }

    #[test]
    fn easypost_rate_carries_its_shipment_and_tracker_list_decodes() {
        let rate: EasyPostRate = serde_json::from_str(
            r#"{"id":"rate_1","shipment_id":"shp_A","carrier":"USPS","service":"Priority","rate":"9.10","currency":"USD"}"#,
        )
        .unwrap();
        assert_eq!(rate.shipment_id.as_deref(), Some("shp_A"));

        let tracker = EasyPostProvider::first_tracker(
            r#"{"trackers":[{"status":"in_transit","tracking_details":[{"message":"Accepted","tracking_location":{"city":"Springfield","state":"IL"}}]}],"has_more":false}"#,
            "9400",
        )
        .unwrap();
        assert_eq!(tracker.status.as_deref(), Some("in_transit"));
        assert_eq!(tracker.tracking_details.len(), 1);
        assert!(EasyPostProvider::first_tracker(r#"{"trackers":[]}"#, "9400").is_err());
    }

    #[test]
    fn easypost_buy_without_rate_or_tracking_is_an_error_not_a_label() {
        let bought: EasyPostShipment =
            serde_json::from_str(r#"{"id":"shp_1","rates":[]}"#).unwrap();
        assert!(bought.selected_rate.is_none());
        assert!(bought.tracking_code.is_none());
    }

    #[test]
    fn unreachable_host_is_external_service_error() {
        let provider =
            EasyPostProvider::new("EK_TEST_dead").unwrap().with_base_url("http://127.0.0.1:9");
        let address = PostalAddress {
            street1: "x".into(),
            city: "y".into(),
            state: "z".into(),
            zip: "0".into(),
            country: "US".into(),
            ..Default::default()
        };
        let parcel =
            Parcel { weight_oz: dec!(16), length_in: None, width_in: None, height_in: None };
        let err = provider.rates(&address, &address, &parcel).expect_err("must fail");
        assert!(matches!(err, CommerceError::ExternalServiceError(_)));
    }
}
