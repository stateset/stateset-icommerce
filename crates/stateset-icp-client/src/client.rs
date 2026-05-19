//! `Client` — the high-level entry point for an Agent to drive a merchant handler.

use crate::identity::verify_ed25519;
use crate::intents::{build_intent_envelope, intent_base};
use crate::{Error, Identity, LineItem, Money, Signature, canonical_json};
use serde_json::{Value, json};
use std::sync::RwLock;
use std::time::Duration;

/// Top-level signed response from a merchant verb endpoint.
///
/// The merchant wraps every verb response in a `{ <payload_key>: ..., signature: {alg, kid, sig} }`
/// envelope; this struct surfaces both the raw JSON payload and the signature so the
/// caller can verify or forward as needed.
#[derive(Debug, Clone)]
pub struct SignedResponse {
    /// Verb-specific payload as raw JSON (e.g. the `quote`, `snapshot`,
    /// `authorization`, or `proposal` sub-object).
    pub payload: Value,
    /// Outer signature envelope.
    pub signature: Signature,
}

/// High-level ICP client.
///
/// Wraps an `Identity` and a merchant base URL. Every verb method
/// constructs the right Intent shape, canonicalizes + signs it, POSTs
/// to `/icp/v1/intents`, and deserializes the response into a
/// `SignedResponse`.
///
/// The `merchant` AID and `settler` identifier are required by the
/// handler on every Intent. Discover them via [`Client::well_known`]
/// and `Client::settlers`, then pass them into each verb call.
pub struct Client {
    handler_url: String,
    identity: Identity,
    agent: ureq::Agent,
    /// Cached merchant Ed25519 public key (hex), populated lazily by
    /// `well_known()` and used to verify the merchant signature on
    /// every response payload.
    merchant_pubkey_cache: RwLock<Option<String>>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("handler_url", &self.handler_url)
            .field("aid", &self.identity.aid())
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Construct a new client bound to a merchant handler URL + Agent identity.
    pub fn new<S: Into<String>>(handler_url: S, identity: Identity) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout_read(Duration::from_secs(30))
            .build();
        Self {
            handler_url: trim_trailing_slash(handler_url.into()),
            identity,
            agent,
            merchant_pubkey_cache: RwLock::new(None),
        }
    }

    /// Returns the Agent's AID.
    pub fn aid(&self) -> &str {
        self.identity.aid()
    }

    /// `GET /.well-known/icp` — discover merchant capabilities. Returns
    /// the raw discovery JSON; the shape is `merchant_aid`, `merchant_pubkey`,
    /// `capabilities`, etc.
    ///
    /// **Side effect:** caches the merchant's Ed25519 public key (from
    /// `merchant_pubkey.raw_hex`) so subsequent verb calls can verify
    /// merchant signatures via [`Client::verify_signed_response`].
    pub fn well_known(&self) -> Result<Value, Error> {
        let url = format!("{}/icp/v1/.well-known/icp", self.handler_url);
        let resp = self.agent.get(&url).call().map_err(map_ureq)?;
        let text = resp.into_string().map_err(|e| Error::Network(e.to_string()))?;
        let v = parse_value(&text)?;
        if let Some(hex) =
            v.get("merchant_pubkey").and_then(|p| p.get("raw_hex")).and_then(Value::as_str)
        {
            if let Ok(mut cache) = self.merchant_pubkey_cache.write() {
                *cache = Some(hex.to_string());
            }
        }
        Ok(v)
    }

    /// Returns the cached merchant public key hex, if `well_known()`
    /// has been called and the merchant exposed one.
    pub fn merchant_pubkey_hex(&self) -> Option<String> {
        self.merchant_pubkey_cache.read().ok().and_then(|g| g.clone())
    }

    /// Verify the merchant signature on a `SignedResponse`.
    ///
    /// Re-canonicalizes `response.payload` (RFC 8785 JCS) and verifies
    /// `response.signature.sig` against the cached merchant pubkey
    /// (populated by [`Client::well_known`]).
    ///
    /// Returns `Err(Error::SignatureInvalid)` on any failure: missing
    /// pubkey cache, malformed signature, or cryptographic mismatch.
    pub fn verify_signed_response(&self, response: &SignedResponse) -> Result<(), Error> {
        let pubkey_hex = self.merchant_pubkey_hex().ok_or(Error::SignatureInvalid)?;
        let canonical = canonical_json(&response.payload)?;
        if verify_ed25519(canonical.as_bytes(), &response.signature.sig, &pubkey_hex) {
            Ok(())
        } else {
            Err(Error::SignatureInvalid)
        }
    }

    /// `GET /icp/v1/settlers` — list the merchant's accepted Settlers.
    pub fn settlers(&self) -> Result<Value, Error> {
        let url = format!("{}/icp/v1/settlers", self.handler_url);
        let resp = self.agent.get(&url).call().map_err(map_ureq)?;
        let text = resp.into_string().map_err(|e| Error::Network(e.to_string()))?;
        parse_value(&text)
    }

    fn zero_money() -> Money {
        Money { amount: "0".to_string(), currency: "USDC".to_string() }
    }

    /// `inventory.query` — read prices and availability for the given SKUs.
    /// SKUs are wrapped per the JS SDK shape (`{sku: "..."}`).
    pub fn inventory(
        &self,
        merchant: &str,
        settler: &str,
        skus: &[&str],
    ) -> Result<SignedResponse, Error> {
        let base = intent_base(
            &self.identity,
            "inventory.query",
            merchant,
            settler,
            Self::zero_money(),
            vec!["inventory.query".to_string()],
            None,
        );
        let mut intent = serde_json::to_value(base)?;
        let skus_json: Vec<Value> = skus.iter().map(|s| json!({ "sku": s })).collect();
        intent.as_object_mut().expect("object").insert("skus".to_string(), Value::Array(skus_json));
        let env = build_intent_envelope(&self.identity, intent)?;
        self.post_intent(&env, "snapshot")
    }

    /// `purchase.create` — submit a signed purchase Intent and receive a Quote.
    pub fn purchase(
        &self,
        merchant: &str,
        settler: &str,
        items: Vec<LineItem>,
        max_total: Money,
    ) -> Result<SignedResponse, Error> {
        let base = intent_base(
            &self.identity,
            "purchase.create",
            merchant,
            settler,
            max_total.clone(),
            vec!["purchase.create".to_string()],
            None,
        );
        let mut intent = serde_json::to_value(base)?;
        let obj = intent.as_object_mut().expect("object");
        obj.insert("items".to_string(), serde_json::to_value(&items)?);
        obj.insert("max_total".to_string(), serde_json::to_value(&max_total)?);
        let env = build_intent_envelope(&self.identity, intent)?;
        self.post_intent(&env, "quote")
    }

    /// `subscription.create` — open a recurring subscription. Field names match
    /// the JS reference SDK (`service_id`, `cadence`, `max_total_per_period`,
    /// `max_occurrences`, `first_charge_at` — RFC 3339).
    #[allow(clippy::too_many_arguments)]
    pub fn subscribe(
        &self,
        merchant: &str,
        settler: &str,
        service_id: &str,
        cadence: &str,
        max_total_per_period: Money,
        max_occurrences: Option<u32>,
        first_charge_at: &str,
    ) -> Result<SignedResponse, Error> {
        let base = intent_base(
            &self.identity,
            "subscription.create",
            merchant,
            settler,
            max_total_per_period.clone(),
            vec!["subscription.create".to_string()],
            None,
        );
        let mut intent = serde_json::to_value(base)?;
        let obj = intent.as_object_mut().expect("object");
        obj.insert("service_id".to_string(), Value::String(service_id.to_string()));
        obj.insert("cadence".to_string(), Value::String(cadence.to_string()));
        obj.insert(
            "max_total_per_period".to_string(),
            serde_json::to_value(&max_total_per_period)?,
        );
        obj.insert(
            "max_occurrences".to_string(),
            max_occurrences.map_or(Value::Null, |n| Value::Number(n.into())),
        );
        obj.insert("first_charge_at".to_string(), Value::String(first_charge_at.to_string()));
        let env = build_intent_envelope(&self.identity, intent)?;
        self.post_intent(&env, "authorization")
    }

    /// `subscription.cancel` — cancel an existing subscription.
    pub fn cancel(
        &self,
        merchant: &str,
        settler: &str,
        subscription_id: &str,
        effective: &str,
        reason: Option<&str>,
    ) -> Result<SignedResponse, Error> {
        let base = intent_base(
            &self.identity,
            "subscription.cancel",
            merchant,
            settler,
            Self::zero_money(),
            vec!["subscription.cancel".to_string()],
            None,
        );
        let mut intent = serde_json::to_value(base)?;
        let obj = intent.as_object_mut().expect("object");
        obj.insert("subscription_id".to_string(), Value::String(subscription_id.to_string()));
        obj.insert("effective".to_string(), Value::String(effective.to_string()));
        if let Some(r) = reason {
            obj.insert("reason".to_string(), Value::String(r.to_string()));
        }
        let env = build_intent_envelope(&self.identity, intent)?;
        self.post_intent(&env, "authorization")
    }

    /// `purchase.return` — request a refund / return authorization. Field
    /// names match the JS reference SDK (`original_settlement_id`, `items`
    /// as a free-form JSON array with optional per-line `reason`,
    /// `desired_outcome`).
    pub fn return_purchase(
        &self,
        merchant: &str,
        settler: &str,
        original_settlement_id: &str,
        items: Value,
        desired_outcome: &str,
    ) -> Result<SignedResponse, Error> {
        let base = intent_base(
            &self.identity,
            "purchase.return",
            merchant,
            settler,
            Self::zero_money(),
            vec!["purchase.return".to_string()],
            None,
        );
        let mut intent = serde_json::to_value(base)?;
        let obj = intent.as_object_mut().expect("object");
        obj.insert(
            "original_settlement_id".to_string(),
            Value::String(original_settlement_id.to_string()),
        );
        obj.insert("items".to_string(), items);
        obj.insert("desired_outcome".to_string(), Value::String(desired_outcome.to_string()));
        let env = build_intent_envelope(&self.identity, intent)?;
        self.post_intent(&env, "authorization")
    }

    /// `quote.request` — request an RFQ proposal. `items` is the JS-SDK-shaped
    /// JSON array (each item has `sku`, `quantity`, optionally
    /// `target_unit_price` and `specifications`).
    pub fn request_quote(
        &self,
        merchant: &str,
        settler: &str,
        items: Value,
    ) -> Result<SignedResponse, Error> {
        let base = intent_base(
            &self.identity,
            "quote.request",
            merchant,
            settler,
            Self::zero_money(),
            vec!["quote.request".to_string()],
            None,
        );
        let mut intent = serde_json::to_value(base)?;
        let obj = intent.as_object_mut().expect("object");
        obj.insert("items".to_string(), items);
        let env = build_intent_envelope(&self.identity, intent)?;
        self.post_intent(&env, "proposal")
    }

    /// `payout.request` — request a marketplace payout to a seller (ICPIP-0004).
    /// Caller provides the marketplace `platform` AID separately from the
    /// merchant; the handler reads `intent.platform`.
    pub fn payout(
        &self,
        merchant: &str,
        settler: &str,
        seller: &str,
        platform: &str,
        amount: Money,
    ) -> Result<SignedResponse, Error> {
        let base = intent_base(
            &self.identity,
            "payout.request",
            merchant,
            settler,
            amount.clone(),
            vec!["payout.request".to_string()],
            Some(amount.clone()),
        );
        let mut intent = serde_json::to_value(base)?;
        let obj = intent.as_object_mut().expect("object");
        obj.insert("seller".to_string(), Value::String(seller.to_string()));
        obj.insert("platform".to_string(), Value::String(platform.to_string()));
        obj.insert("amount".to_string(), serde_json::to_value(&amount)?);
        let env = build_intent_envelope(&self.identity, intent)?;
        self.post_intent(&env, "authorization")
    }

    /// `channel.register` — register a webhook OR SSE push channel (ICPIP-0005).
    ///
    /// For webhooks, supply `url` (https:// required in production; loopback
    /// http:// allowed against dev/CI handlers). For SSE, pass `channel_type
    /// = "sse"` and `None` for `url`. The merchant signs the returned
    /// `ChannelRegistration`; pair this with [`Client::verify_signed_response`]
    /// to verify, or use the receiver-side [`crate::verify_webhook`] helper
    /// on each inbound event.
    ///
    /// `event_filters` is the list of ICPIP-0005 §3 event types this channel
    /// should receive (e.g. `["settlement.released", "escrow.refunded"]`).
    pub fn register_webhook(
        &self,
        merchant: &str,
        settler: &str,
        channel_type: &str,
        url: Option<&str>,
        event_filters: &[&str],
    ) -> Result<SignedResponse, Error> {
        let base = intent_base(
            &self.identity,
            "channel.register",
            merchant,
            settler,
            Self::zero_money(),
            vec!["channel.register".to_string()],
            None,
        );
        let mut intent = serde_json::to_value(base)?;
        let obj = intent.as_object_mut().expect("object");
        let mut channel = serde_json::Map::new();
        channel.insert("type".to_string(), Value::String(channel_type.to_string()));
        if let Some(u) = url {
            channel.insert("url".to_string(), Value::String(u.to_string()));
        }
        let filters: Vec<Value> =
            event_filters.iter().map(|s| Value::String((*s).to_string())).collect();
        channel.insert("event_filters".to_string(), Value::Array(filters));
        obj.insert("channel".to_string(), Value::Object(channel));
        let env = build_intent_envelope(&self.identity, intent)?;
        self.post_intent(&env, "channel")
    }

    /// `POST /icp/v1/quotes/:id/accept` — accept a quote and open an escrow.
    pub fn accept_quote(&self, quote_id: &str) -> Result<Value, Error> {
        let url = format!("{}/icp/v1/quotes/{}/accept", self.handler_url, quote_id);
        let resp = self.agent.post(&url).send_json(json!({})).map_err(map_ureq)?;
        let text = resp.into_string().map_err(|e| Error::Network(e.to_string()))?;
        parse_value(&text)
    }

    /// ICPIP-0005 §5 — fetch missed events from a registered channel.
    ///
    /// GETs `/icp/v1/channels/:channel_id/events?since=N` and returns
    /// every signed envelope the handler has retained with
    /// `sequence > since`. Each envelope's signature is verified against
    /// the cached merchant pubkey (populated by [`Client::well_known`]);
    /// on any envelope failure, returns `Err(Error::SignatureInvalid)`.
    ///
    /// Returns the verified envelope JSON values in ascending sequence
    /// order. Callers needing the raw `{envelope, signature}` pairs can
    /// use [`Client::fetch_channel_events_raw`] instead.
    ///
    /// Typed `Error::Icp` codes surfaced by this method:
    ///   - `channel.not_found` (404)
    ///   - `channel.expired` (410)
    ///   - `channel.sequence_gap` (409 — agent must re-register)
    ///   - `format.bad_query_param` (400)
    pub fn fetch_channel_events(&self, channel_id: &str, since: u64) -> Result<Vec<Value>, Error> {
        let raw = self.fetch_channel_events_raw(channel_id, since)?;
        let pubkey_hex = self.merchant_pubkey_hex().ok_or(Error::SignatureInvalid)?;
        let mut verified = Vec::with_capacity(raw.len());
        for entry in raw {
            let envelope = entry
                .get("envelope")
                .cloned()
                .ok_or_else(|| Error::MalformedResponse("event missing envelope".to_string()))?;
            let sig_hex = entry
                .get("signature")
                .and_then(|s| s.get("sig"))
                .and_then(Value::as_str)
                .ok_or_else(|| Error::MalformedResponse("event missing signature.sig".to_string()))?
                .to_string();
            let canonical = canonical_json(&envelope)?;
            if !verify_ed25519(canonical.as_bytes(), &sig_hex, &pubkey_hex) {
                return Err(Error::SignatureInvalid);
            }
            verified.push(envelope);
        }
        Ok(verified)
    }

    /// Like [`Client::fetch_channel_events`] but returns the raw
    /// `{envelope, signature}` pairs without verifying each envelope.
    /// Useful when you want to delegate verification to a different
    /// component (e.g. an off-thread verifier or audit pipeline).
    pub fn fetch_channel_events_raw(
        &self,
        channel_id: &str,
        since: u64,
    ) -> Result<Vec<Value>, Error> {
        let url =
            format!("{}/icp/v1/channels/{}/events?since={}", self.handler_url, channel_id, since,);
        let resp = self.agent.get(&url).call().map_err(map_ureq)?;
        let text = resp.into_string().map_err(|e| Error::Network(e.to_string()))?;
        let body = parse_value(&text)?;
        body.get("events")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| Error::MalformedResponse(format!("expected {{events: [...]}}: {body}")))
    }

    /// `GET /icp/v1/settlements/:id` — fetch a co-signed `SettlementReceipt`.
    pub fn get_settlement(&self, settlement_id: &str) -> Result<Value, Error> {
        let url = format!("{}/icp/v1/settlements/{}", self.handler_url, settlement_id);
        let resp = self.agent.get(&url).call().map_err(map_ureq)?;
        let text = resp.into_string().map_err(|e| Error::Network(e.to_string()))?;
        parse_value(&text)
    }

    fn post_intent(
        &self,
        envelope: &crate::IntentEnvelope,
        payload_key: &str,
    ) -> Result<SignedResponse, Error> {
        let url = format!("{}/icp/v1/intents", self.handler_url);
        let resp =
            self.agent.post(&url).send_json(serde_json::to_value(envelope)?).map_err(map_ureq)?;
        let text = resp.into_string().map_err(|e| Error::Network(e.to_string()))?;
        parse_signed_response(&text, payload_key)
    }
}

fn trim_trailing_slash(mut s: String) -> String {
    while s.ends_with('/') {
        s.pop();
    }
    s
}

fn parse_value(text: &str) -> Result<Value, Error> {
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        if v.get("type").and_then(Value::as_str) == Some("icp.error") {
            let code = v.get("code").and_then(Value::as_str).unwrap_or("unknown").to_string();
            let message = v.get("message").and_then(Value::as_str).unwrap_or("").to_string();
            return Err(Error::Icp { code, message });
        }
        return Ok(v);
    }
    Err(Error::MalformedResponse(format!("not JSON: {text}")))
}

fn parse_signed_response(text: &str, payload_key: &str) -> Result<SignedResponse, Error> {
    let v = parse_value(text)?;
    let obj = v
        .as_object()
        .ok_or_else(|| Error::MalformedResponse(format!("expected object, got: {v}")))?;
    let payload = obj.get(payload_key).cloned().ok_or_else(|| {
        Error::MalformedResponse(format!("response missing '{payload_key}' key: {v}"))
    })?;
    let signature: Signature = serde_json::from_value(
        obj.get("signature")
            .cloned()
            .ok_or_else(|| Error::MalformedResponse(format!("missing 'signature': {v}")))?,
    )
    .map_err(|e| Error::MalformedResponse(format!("signature shape: {e}")))?;
    Ok(SignedResponse { payload, signature })
}

fn map_ureq(err: ureq::Error) -> Error {
    match err {
        ureq::Error::Status(_, resp) => {
            let body = resp.into_string().unwrap_or_default();
            if let Ok(v) = serde_json::from_str::<Value>(&body) {
                if v.get("type").and_then(Value::as_str) == Some("icp.error") {
                    let code =
                        v.get("code").and_then(Value::as_str).unwrap_or("unknown").to_string();
                    let message =
                        v.get("message").and_then(Value::as_str).unwrap_or("").to_string();
                    return Error::Icp { code, message };
                }
            }
            Error::Network(format!("http error: {body}"))
        }
        other => Error::Network(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_aid_matches_identity() {
        let id = Identity::from_seeds_hex(
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
            "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a",
        )
        .unwrap();
        let aid = id.aid().to_string();
        let client = Client::new("http://localhost:7402/", id);
        assert_eq!(client.aid(), aid);
    }

    #[test]
    fn trailing_slash_is_trimmed() {
        let id = Identity::generate();
        let client = Client::new("http://x.example.com///", id);
        assert_eq!(client.handler_url, "http://x.example.com");
    }

    #[test]
    fn icp_error_response_is_typed() {
        let body = r#"{"type":"icp.error","code":"signature.invalid","message":"bad sig"}"#;
        match parse_value(body) {
            Err(Error::Icp { code, message }) => {
                assert_eq!(code, "signature.invalid");
                assert_eq!(message, "bad sig");
            }
            other => panic!("expected Icp error, got {other:?}"),
        }
    }

    #[test]
    fn parse_signed_response_extracts_payload_and_signature() {
        let body = r#"{
            "quote": {"quote_id": "icp_qt_abc", "total": {"amount": "29.99", "currency": "USDC"}},
            "signature": {"alg": "ed25519", "kid": "aid:v1:zMerch", "sig": "deadbeef"}
        }"#;
        let resp = parse_signed_response(body, "quote").unwrap();
        assert_eq!(resp.payload["quote_id"], "icp_qt_abc");
        assert_eq!(resp.signature.alg, "ed25519");
        assert_eq!(resp.signature.kid, "aid:v1:zMerch");
        assert_eq!(resp.signature.sig, "deadbeef");
    }
}
