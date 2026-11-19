//! Card capture against a real payment processor.
//!
//! The engine has always recorded payment rows and flipped local status flags
//! (`mark_completed`); this module is the missing rail to actual money
//! movement. A [`PaymentProvider`] authorizes, captures, and refunds through
//! an upstream processor; the [`Payments`] facade persists the outcome so the
//! ledger, refunds invariant, and `open_captures_for_order` keep working
//! unchanged.
//!
//! [`Payments`]: crate::payments::Payments
//!
//! [`StripeProvider`] speaks
//! `PaymentIntents` (manual capture) in test or live mode depending on the key. [`crate::SquareProvider`] supports Square delayed capture.
//! [`MockPaymentProvider`] is a deterministic in-memory stand-in for unit
//! tests, demos, and CI without network.
//!
//! Secrets never live here: providers take an already-loaded key (read
//! `STRIPE_TEST_KEY` / `STRIPE_LIVE_KEY` from the environment at the call
//! site). Nothing in this crate reads process environment.

use rust_decimal::Decimal;
use serde::Deserialize;
use stateset_core::{CommerceError, Result};

/// Amount, currency, and routing for one provider call.
#[derive(Debug, Clone)]
pub struct ProviderCharge {
    /// Exact-decimal major units (e.g. dollars).
    pub amount: Decimal,
    /// ISO 4217 code, e.g. `"usd"`.
    pub currency: String,
    /// Processor-native payment-method handle (e.g. a Stripe `pm_` id).
    pub payment_method: Option<String>,
    /// Idempotency key, reused across retries of the same intent.
    pub idempotency_key: String,
    /// Operator-visible memo attached to the upstream object.
    pub description: Option<String>,
}

/// What the processor decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderDecision {
    /// Funds are held, not yet moved. `reference` is the upstream id.
    Authorized {
        /// Upstream object id (e.g. `pi_...`).
        reference: String,
    },
    /// Funds moved (possibly partially).
    Captured {
        /// Upstream object id.
        reference: String,
    },
    /// The processor accepted the request but the outcome is not final yet
    /// (e.g. a Stripe refund in `pending` for an ACH or bank-debit payment).
    /// Nothing is recorded as settled until the processor confirms.
    Pending {
        /// Upstream object id (e.g. `re_...` for a refund).
        reference: String,
    },
    /// The customer must act (e.g. 3-D Secure) before the hold completes.
    RequiresAction {
        /// Upstream object id.
        reference: String,
        /// Processor-hosted URL or client secret, when provided.
        action_url: Option<String>,
    },
    /// The processor refused. Never throws: declines are data.
    Declined {
        /// Machine code (e.g. `card_declined`).
        code: String,
        /// Human message.
        message: String,
    },
}

/// Upstream payment processor. Synchronous on purpose: the engine is a
/// single-process library, and `reqwest::blocking` is already on board via
/// the `events` feature, so providers add no new dependencies.
pub trait PaymentProvider {
    /// Human name for logs and audit (`"stripe"`, `"mock"`).
    fn name(&self) -> &'static str;
    /// Hold funds without moving them.
    fn authorize(&self, charge: &ProviderCharge) -> Result<ProviderDecision>;
    /// Read the authoritative result of a previously identified authorization.
    fn lookup_authorization(
        &self,
        _reference: &str,
        _charge: &ProviderCharge,
    ) -> Result<ProviderDecision> {
        Err(CommerceError::ExternalServiceError(
            "This provider cannot reconcile an authorization".into(),
        ))
    }
    /// Establish that an operator-supplied reference originated from this exact request.
    fn lookup_owned_authorization(
        &self,
        _reference: &str,
        _charge: &ProviderCharge,
    ) -> Result<ProviderDecision> {
        Err(CommerceError::ExternalServiceError(
            "This processor cannot prove request ownership".into(),
        ))
    }
    /// Move previously held funds. `reference` is the authorize-time id;
    /// `amount` caps a partial capture (`None` = full remaining).
    fn capture(
        &self,
        reference: &str,
        amount: Option<Decimal>,
        currency: &str,
    ) -> Result<ProviderDecision>;

    /// Capture the entire hold and verify its settlement against the local payment.
    /// Providers without settlement detail retain their original full-capture behavior.
    fn capture_full(
        &self,
        reference: &str,
        _expected: Decimal,
        currency: &str,
    ) -> Result<ProviderDecision> {
        self.capture(reference, None, currency)
    }

    /// Return captured funds (full when `amount` is `None`).
    ///
    /// `idempotency_key` must be unique per engine refund: several partial
    /// refunds against one payment are distinct upstream operations.
    fn refund(
        &self,
        reference: &str,
        amount: Option<Decimal>,
        currency: &str,
        idempotency_key: &str,
    ) -> Result<ProviderDecision>;
    /// Read a previously accepted refund and verify its payment, currency and amount.
    fn lookup_refund(
        &self,
        _refund: &str,
        _payment: &str,
        _amount: Decimal,
        _currency: &str,
    ) -> Result<ProviderDecision> {
        Err(CommerceError::ExternalServiceError("This provider cannot reconcile a refund".into()))
    }
}

/// Convert major units to the processor's minor units.
///
/// Most currencies use 2 decimals; the zero-decimal set (JPY, KRW, …)
/// has no minor unit and the three-decimal set (KWD, BHD, …) has 1/1000.
/// An amount finer than the currency's minor unit is rejected rather than
/// rounded: the engine never charges a different amount than it recorded.
pub fn minor_units(amount: Decimal, currency: &str) -> Result<i64> {
    const ZERO_DECIMAL: &[&str] = &[
        "bif", "clp", "djf", "gnf", "jpy", "kmf", "krw", "mga", "pyg", "rwf", "ugx", "uyi", "vnd",
        "vuv", "xaf", "xof", "xpf",
    ];
    const THREE_DECIMAL: &[&str] = &["bhd", "jod", "kwd", "omr", "tnd"];
    let code = currency.to_lowercase();
    if amount < Decimal::ZERO {
        return Err(CommerceError::ValidationError("charge amount is negative".into()));
    }
    let scale = if ZERO_DECIMAL.contains(&code.as_str()) {
        1
    } else if THREE_DECIMAL.contains(&code.as_str()) {
        1000
    } else {
        100
    };
    let minor = amount * Decimal::from(scale);
    if minor.fract() != Decimal::ZERO {
        return Err(CommerceError::ValidationError(format!(
            "amount {amount} is finer than the minor unit of {currency}"
        )));
    }
    // Stripe requires three-decimal amounts to end in 0 (10-fils steps).
    if scale == 1000 && (minor % Decimal::from(10)) != Decimal::ZERO {
        return Err(CommerceError::ValidationError(format!(
            "amount {amount} {currency} must be a multiple of 0.010"
        )));
    }
    minor.trunc().to_string().parse::<i64>().map_err(|_| {
        CommerceError::ValidationError(format!("amount {amount} {currency} overflows minor units"))
    })
}

// ============================================================================
// Stripe (PaymentIntents, manual capture)
// ============================================================================

#[derive(Debug, Deserialize)]
struct StripeIntent {
    id: String,
    status: String,
    #[serde(default)]
    next_action: Option<StripeNextAction>,
    #[serde(default)]
    last_payment_error: Option<StripeErrorBody>,
}

#[derive(Debug, Deserialize)]
struct StripeNextAction {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    redirect_to_url: Option<StripeRedirect>,
}

#[derive(Debug, Deserialize)]
struct StripeRedirect {
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StripeErrorBody {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    decline_code: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StripeErrorEnvelope {
    error: StripeErrorBody,
}

#[derive(Debug, Deserialize)]
struct StripeRefund {
    id: String,
    status: String,
    #[serde(default)]
    failure_reason: Option<String>,
}

/// A Stripe response: a 2xx body, or a card error (HTTP 402 /
/// `type: card_error`), which is a decline and therefore data, not a fault.
enum StripeReply {
    Body(String),
    Declined { code: String, message: String },
}

/// Stripe `PaymentIntents` provider.
///
/// Test vs live is purely the key: `sk_test_…` hits test mode, `sk_live_…`
/// moves real money. `STRIPE_TEST_KEY` / `STRIPE_LIVE_KEY` stay in the
/// environment; pass the value in.
#[derive(Debug, Clone)]
pub struct StripeProvider {
    api_key: String,
    customer: Option<String>,
    base_url: String,
    client: reqwest::blocking::Client,
}

impl StripeProvider {
    /// Production endpoint with the given secret key.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| CommerceError::ExternalServiceError(format!("stripe client: {e}")))?;
        Ok(Self {
            api_key: api_key.into(),
            customer: None,
            base_url: "https://api.stripe.com".into(),
            client,
        })
    }

    /// Point at a different base (tests, mock server). Path layout is unchanged.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    /// Scope reuse of a saved payment method to its verified Stripe customer.
    pub fn with_customer(mut self, customer: String) -> Result<Self> {
        if !customer.starts_with("cus_")
            || !customer.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err(CommerceError::ValidationError("Invalid Stripe customer reference".into()));
        }
        self.customer = Some(customer);
        Ok(self)
    }

    fn post(
        &self,
        path: &str,
        form: &[(String, String)],
        idempotency: &str,
    ) -> Result<StripeReply> {
        let body = self
            .client
            .post(format!("{}{path}", self.base_url))
            .basic_auth(&self.api_key, Option::<&str>::None)
            .header("Idempotency-Key", idempotency)
            .form(&form)
            .send()
            .map_err(|e| CommerceError::ExternalServiceError(format!("stripe request: {e}")))?;
        let status = body.status();
        let text = body
            .text()
            .map_err(|e| CommerceError::ExternalServiceError(format!("stripe read: {e}")))?;
        if !status.is_success() {
            let envelope: Option<StripeErrorEnvelope> = serde_json::from_str(&text).ok();
            let card_error = status.is_client_error()
                && envelope
                    .as_ref()
                    .is_some_and(|env| env.error.kind.as_deref() == Some("card_error"));
            let (code, message) = match envelope {
                Some(env) => (
                    env.error.decline_code.or(env.error.code).unwrap_or_else(|| status.to_string()),
                    env.error.message.unwrap_or_else(|| "stripe error".to_string()),
                ),
                None => (status.to_string(), text.chars().take(300).collect()),
            };
            if card_error {
                return Ok(StripeReply::Declined { code, message });
            }
            return Err(CommerceError::ExternalServiceError(format!(
                "stripe {path} failed ({code}): {message}"
            )));
        }
        Ok(StripeReply::Body(text))
    }

    fn submit_capture(
        &self,
        reference: &str,
        requested: Option<Decimal>,
        expected: Option<Decimal>,
        currency: &str,
    ) -> Result<ProviderDecision> {
        if !Self::valid_id(reference, "pi_") {
            return Err(CommerceError::ValidationError("Invalid Stripe intent reference".into()));
        }
        let expected = expected.map(|value| minor_units(value, currency)).transpose()?;
        let mut form = Vec::new();
        if let Some(amount) = requested {
            form.push((
                "amount_to_capture".to_string(),
                minor_units(amount, currency)?.to_string(),
            ));
        }
        Self::intent_decision(
            self.post(
                &format!("/v1/payment_intents/{reference}/capture"),
                &form,
                &format!("capture-{reference}"),
            )?,
            Some(reference),
            currency,
            expected,
            requested.is_some(),
        )
    }

    fn intent_decision(
        reply: StripeReply,
        reference: Option<&str>,
        currency: &str,
        amount: Option<i64>,
        capturing: bool,
    ) -> Result<ProviderDecision> {
        match reply {
            StripeReply::Declined { code, message } => {
                Ok(ProviderDecision::Declined { code, message })
            }
            StripeReply::Body(text) => {
                let object = serde_json::from_str(&text).map_err(|e| {
                    CommerceError::ExternalServiceError(format!("stripe decode: {e}"))
                })?;
                Self::check_intent(object, reference, currency, amount, capturing)
            }
        }
    }

    fn valid_id(id: &str, prefix: &str) -> bool {
        id.starts_with(prefix)
            && id.len() > prefix.len()
            && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
    }

    fn check_intent(
        object: serde_json::Value,
        reference: Option<&str>,
        currency: &str,
        amount: Option<i64>,
        capturing: bool,
    ) -> Result<ProviderDecision> {
        let id = object["id"].as_str().unwrap_or_default();
        let authorized = object["amount"].as_i64().unwrap_or_default();
        let expected = amount.unwrap_or(authorized);
        if !Self::valid_id(id, "pi_")
            || reference.is_some_and(|r| id != r)
            || object["currency"] != currency.to_ascii_lowercase()
            || expected <= 0
            || expected > authorized
            || (!capturing && authorized != expected)
            || (object["status"] == "succeeded"
                && object["amount_received"].as_i64() != Some(expected))
            || (object["status"] == "requires_capture"
                && object["amount_capturable"].as_i64() != Some(authorized))
        {
            return Err(CommerceError::ValidationError(
                "Stripe intent differs from payment".into(),
            ));
        }
        if !matches!(
            object["status"].as_str(),
            Some(
                "requires_capture"
                    | "succeeded"
                    | "requires_action"
                    | "requires_source_action"
                    | "processing"
                    | "requires_confirmation"
                    | "requires_payment_method"
                    | "requires_source"
                    | "canceled"
            )
        ) {
            return Err(CommerceError::ExternalServiceError(
                "Unrecognized Stripe payment status".into(),
            ));
        }
        serde_json::from_value(object)
            .map(Self::decide)
            .map_err(|e| CommerceError::ExternalServiceError(format!("stripe decode: {e}")))
    }

    fn read_intent(
        &self,
        reference: &str,
        charge: &ProviderCharge,
        verify_owner: bool,
    ) -> Result<ProviderDecision> {
        if !Self::valid_id(reference, "pi_") {
            return Err(CommerceError::ValidationError("Invalid Stripe intent reference".into()));
        }
        let object: serde_json::Value = self
            .client
            .get(format!("{}/v1/payment_intents/{reference}", self.base_url))
            .basic_auth(&self.api_key, Option::<&str>::None)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .and_then(reqwest::blocking::Response::json)
            .map_err(|e| CommerceError::ExternalServiceError(e.to_string()))?;
        if verify_owner && object["metadata"]["stateset_request"] != charge.idempotency_key {
            return Err(CommerceError::ValidationError(
                "Stripe intent belongs to another request".into(),
            ));
        }
        Self::check_intent(
            object,
            Some(reference),
            &charge.currency,
            Some(minor_units(charge.amount, &charge.currency)?),
            false,
        )
    }

    fn check_refund(
        object: serde_json::Value,
        refund: Option<&str>,
        payment: &str,
        amount: Option<i64>,
        currency: &str,
    ) -> Result<ProviderDecision> {
        let id = object["id"].as_str().unwrap_or_default();
        let actual = object["amount"].as_i64().unwrap_or_default();
        if !Self::valid_id(id, "re_")
            || refund.is_some_and(|r| id != r)
            || object["payment_intent"] != payment
            || actual <= 0
            || amount.is_some_and(|a| a != actual)
            || object["currency"] != currency.to_ascii_lowercase()
        {
            return Err(CommerceError::ValidationError(
                "Stripe refund differs from local payment or amount".into(),
            ));
        }
        if !matches!(
            object["status"].as_str(),
            Some("succeeded" | "pending" | "requires_action" | "failed" | "canceled")
        ) {
            return Err(CommerceError::ExternalServiceError(
                "Unrecognized Stripe refund status".into(),
            ));
        }
        let value = serde_json::from_value(object)
            .map_err(|e| CommerceError::ExternalServiceError(format!("stripe decode: {e}")))?;
        Ok(Self::refund_decision(payment, value))
    }

    /// Map a refund object. Only `succeeded` settles; `pending` waits for
    /// the processor; `failed` / `canceled` are declines.
    fn refund_decision(reference: &str, refund: StripeRefund) -> ProviderDecision {
        match refund.status.as_str() {
            "succeeded" => ProviderDecision::Captured { reference: reference.to_string() },
            "pending" | "requires_action" => ProviderDecision::Pending { reference: refund.id },
            other => ProviderDecision::Declined {
                code: refund.failure_reason.unwrap_or_else(|| format!("refund_{other}")),
                message: format!("stripe refund {} is {other}", refund.id),
            },
        }
    }

    fn decide(intent: StripeIntent) -> ProviderDecision {
        match intent.status.as_str() {
            "requires_capture" => ProviderDecision::Authorized { reference: intent.id },
            "succeeded" => ProviderDecision::Captured { reference: intent.id },
            "requires_action" | "requires_source_action" => {
                let action_url = intent.next_action.as_ref().and_then(|action| {
                    if action.kind == "redirect_to_url" {
                        action.redirect_to_url.as_ref().and_then(|r| r.url.clone())
                    } else {
                        None
                    }
                });
                ProviderDecision::RequiresAction { reference: intent.id, action_url }
            }
            "processing" | "requires_confirmation" => {
                ProviderDecision::Pending { reference: intent.id }
            }
            other => {
                let (code, message) = intent
                    .last_payment_error
                    .map(|e| {
                        (
                            e.code.unwrap_or_else(|| other.to_string()),
                            e.message.unwrap_or_else(|| "payment failed".to_string()),
                        )
                    })
                    .unwrap_or_else(|| (other.to_string(), "payment failed".to_string()));
                ProviderDecision::Declined { code, message }
            }
        }
    }
}

impl PaymentProvider for StripeProvider {
    fn lookup_owned_authorization(
        &self,
        reference: &str,
        charge: &ProviderCharge,
    ) -> Result<ProviderDecision> {
        self.read_intent(reference, charge, true)
    }
    fn lookup_refund(
        &self,
        refund: &str,
        payment: &str,
        amount: Decimal,
        currency: &str,
    ) -> Result<ProviderDecision> {
        if !Self::valid_id(refund, "re_") {
            return Err(CommerceError::ValidationError("Invalid Stripe refund reference".into()));
        }
        let object: serde_json::Value = self
            .client
            .get(format!("{}/v1/refunds/{refund}", self.base_url))
            .basic_auth(&self.api_key, Option::<&str>::None)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .and_then(reqwest::blocking::Response::json)
            .map_err(|e| CommerceError::ExternalServiceError(e.to_string()))?;
        Self::check_refund(
            object,
            Some(refund),
            payment,
            Some(minor_units(amount, currency)?),
            currency,
        )
    }
    fn lookup_authorization(
        &self,
        reference: &str,
        charge: &ProviderCharge,
    ) -> Result<ProviderDecision> {
        self.read_intent(reference, charge, false)
    }

    fn name(&self) -> &'static str {
        "stripe"
    }

    fn authorize(&self, charge: &ProviderCharge) -> Result<ProviderDecision> {
        let amount = minor_units(charge.amount, &charge.currency)?;
        let mut form = vec![
            ("amount".to_string(), amount.to_string()),
            ("currency".to_string(), charge.currency.to_lowercase()),
            ("capture_method".to_string(), "manual".to_string()),
            ("confirm".to_string(), "true".to_string()),
            ("payment_method_types[]".to_string(), "card".to_string()),
            ("use_stripe_sdk".to_string(), "true".to_string()),
            ("metadata[stateset_request]".to_string(), charge.idempotency_key.clone()),
        ];
        if let Some(pm) = &charge.payment_method {
            form.push(("payment_method".to_string(), pm.clone()));
        }
        if let Some(customer) = &self.customer {
            form.push(("customer".into(), customer.clone()));
        }
        if let Some(description) = &charge.description {
            form.push(("description".to_string(), description.clone()));
        }
        Self::intent_decision(
            self.post("/v1/payment_intents", &form, &charge.idempotency_key)?,
            None,
            &charge.currency,
            Some(amount),
            false,
        )
    }

    fn capture(
        &self,
        reference: &str,
        amount: Option<Decimal>,
        currency: &str,
    ) -> Result<ProviderDecision> {
        self.submit_capture(reference, amount, amount, currency)
    }

    fn capture_full(
        &self,
        reference: &str,
        expected: Decimal,
        currency: &str,
    ) -> Result<ProviderDecision> {
        self.submit_capture(reference, None, Some(expected), currency)
    }

    fn refund(
        &self,
        reference: &str,
        amount: Option<Decimal>,
        currency: &str,
        idempotency_key: &str,
    ) -> Result<ProviderDecision> {
        if !Self::valid_id(reference, "pi_") {
            return Err(CommerceError::ValidationError("Invalid Stripe intent reference".into()));
        }
        let amount = amount.map(|v| minor_units(v, currency)).transpose()?;
        let mut form = vec![("payment_intent".to_string(), reference.to_string())];
        if let Some(amount) = amount {
            form.push(("amount".to_string(), amount.to_string()));
        }
        match self.post("/v1/refunds", &form, idempotency_key)? {
            StripeReply::Declined { code, message } => {
                Ok(ProviderDecision::Declined { code, message })
            }
            StripeReply::Body(text) => {
                // A 2xx only means Stripe accepted the request; the refund
                // object's own status says whether money moved.
                let refund = serde_json::from_str(&text).map_err(|e| {
                    CommerceError::ExternalServiceError(format!("stripe decode: {e}"))
                })?;
                Self::check_refund(refund, None, reference, amount, currency)
            }
        }
    }
}

// ============================================================================
// Mock (tests, demos, CI without network)
// ============================================================================

/// Scripted in-memory provider. Every call is recorded; decisions come from
/// a queue so tests can stage authorize → capture → refund precisely.
#[derive(Debug, Default)]
pub struct MockPaymentProvider {
    calls: std::sync::Mutex<Vec<String>>,
    script: std::sync::Mutex<Vec<ProviderDecision>>,
}

impl MockPaymentProvider {
    /// Serve these decisions in order; calls past the end repeat the last.
    pub const fn new(script: Vec<ProviderDecision>) -> Self {
        Self { calls: std::sync::Mutex::new(Vec::new()), script: std::sync::Mutex::new(script) }
    }

    /// What was called, in order (`"authorize:<amount>:<cur>:<key>"`,
    /// `"capture:<ref>:<amount>:<cur>"`, `"refund:<ref>:<amount>:<cur>:<key>"`).
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("mock calls").clone()
    }

    fn next(&self, label: String) -> ProviderDecision {
        self.calls.lock().expect("mock calls").push(label);
        let mut script = self.script.lock().expect("mock script");
        if script.len() > 1 {
            script.remove(0)
        } else {
            script.first().cloned().unwrap_or(ProviderDecision::Declined {
                code: "mock_empty".into(),
                message: "no scripted decision".into(),
            })
        }
    }
}

impl PaymentProvider for MockPaymentProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn authorize(&self, charge: &ProviderCharge) -> Result<ProviderDecision> {
        Ok(self.next(format!(
            "authorize:{}:{}:{}",
            charge.amount, charge.currency, charge.idempotency_key
        )))
    }

    fn capture(
        &self,
        reference: &str,
        amount: Option<Decimal>,
        currency: &str,
    ) -> Result<ProviderDecision> {
        Ok(self.next(format!(
            "capture:{reference}:{}:{currency}",
            amount.map(|a| a.to_string()).unwrap_or_default()
        )))
    }

    fn refund(
        &self,
        reference: &str,
        amount: Option<Decimal>,
        currency: &str,
        idempotency_key: &str,
    ) -> Result<ProviderDecision> {
        Ok(self.next(format!(
            "refund:{reference}:{}:{currency}:{idempotency_key}",
            amount.map(|a| a.to_string()).unwrap_or_default()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn minor_units_usd_and_jpy() {
        assert_eq!(minor_units(dec!(19.99), "usd").unwrap(), 1999);
        assert_eq!(minor_units(dec!(2000), "jpy").unwrap(), 2000);
        assert!(minor_units(dec!(19.99), "jpy").is_err());
        assert!(minor_units(dec!(-1), "usd").is_err());
    }

    #[test]
    fn minor_units_three_decimal_and_sub_unit_rejection() {
        assert_eq!(minor_units(dec!(12.340), "KWD").unwrap(), 12340);
        assert_eq!(minor_units(dec!(1.5), "bhd").unwrap(), 1500);
        // Stripe needs three-decimal amounts in 10-fils steps.
        assert!(minor_units(dec!(12.345), "kwd").is_err());
        // Sub-cent amounts are rejected, never banker's-rounded.
        assert!(minor_units(dec!(19.995), "usd").is_err());
        assert_eq!(minor_units(dec!(19.990), "usd").unwrap(), 1999);
    }

    #[test]
    fn stripe_refund_status_mapping() {
        let refund = |json: &str| -> StripeRefund { serde_json::from_str(json).unwrap() };
        assert_eq!(
            StripeProvider::refund_decision(
                "pi_1",
                refund(r#"{"id":"re_1","status":"succeeded"}"#)
            ),
            ProviderDecision::Captured { reference: "pi_1".into() }
        );
        assert_eq!(
            StripeProvider::refund_decision("pi_1", refund(r#"{"id":"re_2","status":"pending"}"#)),
            ProviderDecision::Pending { reference: "re_2".into() }
        );
        assert_eq!(
            StripeProvider::refund_decision(
                "pi_1",
                refund(
                    r#"{"id":"re_3","status":"failed","failure_reason":"expired_or_canceled_card"}"#
                )
            ),
            ProviderDecision::Declined {
                code: "expired_or_canceled_card".into(),
                message: "stripe refund re_3 is failed".into(),
            }
        );
    }

    #[test]
    fn stripe_card_error_envelope_is_a_decline() {
        let env: StripeErrorEnvelope = serde_json::from_str(
            r#"{"error":{"type":"card_error","code":"card_declined","decline_code":"insufficient_funds","message":"Your card has insufficient funds."}}"#,
        )
        .unwrap();
        assert_eq!(env.error.kind.as_deref(), Some("card_error"));
        assert_eq!(env.error.decline_code.as_deref(), Some("insufficient_funds"));
        assert!(matches!(
            StripeProvider::intent_decision(
                StripeReply::Declined { code: "card_declined".into(), message: "no".into() },
                None,
                "USD",
                Some(100),
                false
            )
            .unwrap(),
            ProviderDecision::Declined { .. }
        ));
    }

    #[test]
    fn stripe_requires_exact_settlement_evidence() {
        use serde_json::json;
        let paid = json!({"id":"pi_sale","status":"succeeded","currency":"usd","amount":1200,"amount_received":1199});
        assert!(
            StripeProvider::check_intent(paid.clone(), Some("pi_sale"), "USD", Some(1200), true)
                .is_err()
        );
        assert!(
            StripeProvider::check_intent(paid.clone(), Some("pi_other"), "USD", Some(1199), true)
                .is_err()
        );
        assert!(
            StripeProvider::check_intent(paid, Some("pi_sale"), "USD", Some(1199), true).is_ok()
        );
        let short_payment = json!({"id":"pi_sale","status":"succeeded","currency":"usd","amount":1199,"amount_received":1199});
        assert!(
            StripeProvider::check_intent(short_payment, Some("pi_sale"), "USD", Some(1200), false)
                .is_err()
        );
        let held = json!({"id":"pi_sale","status":"requires_capture","currency":"usd","amount":1200,"amount_capturable":1100});
        assert!(StripeProvider::check_intent(held, None, "USD", Some(1200), false).is_err());
        let refund = json!({"id":"re_return","status":"succeeded","payment_intent":"pi_sale","currency":"usd","amount":125});
        assert!(
            StripeProvider::check_refund(refund.clone(), None, "pi_other", Some(125), "USD")
                .is_err()
        );
        assert!(
            StripeProvider::check_refund(refund.clone(), None, "pi_sale", Some(100), "USD")
                .is_err()
        );
        assert!(StripeProvider::check_refund(refund, None, "pi_sale", Some(125), "EUR").is_err());
    }

    #[test]
    fn stripe_intent_status_mapping() {
        let authorized = StripeProvider::decide(
            serde_json::from_str(r#"{"id":"pi_1","status":"requires_capture"}"#).unwrap(),
        );
        assert_eq!(authorized, ProviderDecision::Authorized { reference: "pi_1".into() });
        let action = StripeProvider::decide(serde_json::from_str(
            r#"{"id":"pi_2","status":"requires_action","next_action":{"type":"redirect_to_url","redirect_to_url":{"url":"https://bank.example/3ds"}}}"#,
        ).unwrap());
        assert_eq!(
            action,
            ProviderDecision::RequiresAction {
                reference: "pi_2".into(),
                action_url: Some("https://bank.example/3ds".into()),
            }
        );
        let declined = StripeProvider::decide(serde_json::from_str(
            r#"{"id":"pi_3","status":"requires_payment_method","last_payment_error":{"code":"card_declined","message":"do not honor"}}"#,
        ).unwrap());
        assert_eq!(
            declined,
            ProviderDecision::Declined {
                code: "card_declined".into(),
                message: "do not honor".into()
            }
        );
    }

    #[test]
    fn stripe_error_envelope_surfaces_decline() {
        let provider =
            StripeProvider::new("sk_test_dead").unwrap().with_base_url("http://127.0.0.1:9");
        // Unroutable base: transport failure maps to ExternalServiceError,
        // never a panic, never a silent local-only record.
        let err = provider.capture("pi_dead", None, "usd").expect_err("unreachable host must fail");
        assert!(matches!(err, CommerceError::ExternalServiceError(_)));
    }

    /// Serve exactly one canned HTTP response on a loopback port.
    fn one_shot_server(status_line: &'static str, body: &'static str) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            while let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 8192];
                let Ok(length) = stream.read(&mut buf) else { continue };
                if length == 0 || buf[..length].starts_with(b"GET / HTTP/") {
                    continue;
                }
                let response = format!(
                    "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                break;
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn stripe_http_402_card_error_is_a_declined_decision_not_an_error() {
        let base = one_shot_server(
            "HTTP/1.1 402 Payment Required",
            r#"{"error":{"type":"card_error","code":"card_declined","decline_code":"insufficient_funds","message":"Your card has insufficient funds."}}"#,
        );
        let provider = StripeProvider::new("sk_test_x").unwrap().with_base_url(base);
        let charge = ProviderCharge {
            amount: dec!(10),
            currency: "usd".into(),
            payment_method: Some("pm_card_chargeDeclined".into()),
            idempotency_key: "k".into(),
            description: None,
        };
        assert_eq!(
            provider.authorize(&charge).expect("a decline is data"),
            ProviderDecision::Declined {
                code: "insufficient_funds".into(),
                message: "Your card has insufficient funds.".into(),
            }
        );
    }

    #[test]
    fn stripe_http_500_stays_an_error() {
        let base = one_shot_server(
            "HTTP/1.1 500 Internal Server Error",
            r#"{"error":{"type":"api_error","message":"boom"}}"#,
        );
        let provider = StripeProvider::new("sk_test_x").unwrap().with_base_url(base);
        let err = provider.capture("pi_1", None, "usd").expect_err("outage is not a decline");
        assert!(matches!(err, CommerceError::ExternalServiceError(_)));
    }

    #[test]
    fn stripe_http_pending_refund_is_not_settled() {
        let base = one_shot_server(
            "HTTP/1.1 200 OK",
            r#"{"id":"re_9","object":"refund","status":"pending","amount":500,"currency":"usd","payment_intent":"pi_1"}"#,
        );
        let provider = StripeProvider::new("sk_test_x").unwrap().with_base_url(base);
        assert_eq!(
            provider.refund("pi_1", Some(dec!(5)), "usd", "refund-a").unwrap(),
            ProviderDecision::Pending { reference: "re_9".into() }
        );
    }

    #[test]
    fn mock_replays_script_in_order() {
        let mock = MockPaymentProvider::new(vec![
            ProviderDecision::Authorized { reference: "pi_mock".into() },
            ProviderDecision::Captured { reference: "pi_mock".into() },
        ]);
        let charge = ProviderCharge {
            amount: dec!(10),
            currency: "usd".into(),
            payment_method: None,
            idempotency_key: "k".into(),
            description: None,
        };
        assert!(matches!(mock.authorize(&charge).unwrap(), ProviderDecision::Authorized { .. }));
        assert!(matches!(
            mock.capture("pi_mock", None, "usd").unwrap(),
            ProviderDecision::Captured { .. }
        ));
        assert_eq!(mock.calls().len(), 2);
    }
}
