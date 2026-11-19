//! Square delayed capture with processor-native payment tokens.
use crate::{PaymentProvider, ProviderCharge, ProviderDecision, minor_units};
use reqwest::{Method, blocking::Client};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use stateset_core::{CommerceError, Result};

/// A Square merchant account and location. Tokens are supplied by the host.
#[derive(Clone)]
pub struct SquareProvider {
    client: Client,
    origin: String,
    token: String,
    location: String,
}

impl std::fmt::Debug for SquareProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SquareProvider").field("location", &self.location).finish_non_exhaustive()
    }
}

fn fault(message: impl Into<String>) -> CommerceError {
    CommerceError::ExternalServiceError(message.into())
}

fn key(value: &str) -> String {
    // Square requires a key of at most 45 characters.
    format!("{:x}", Sha256::digest(value.as_bytes()))[..40].to_owned()
}

impl SquareProvider {
    /// Construct a live or sandbox client. Square credentials are scoped to the selected environment.
    pub fn new(
        token: impl Into<String>,
        location: impl Into<String>,
        sandbox: bool,
    ) -> Result<Self> {
        let token = token.into();
        let location = location.into();
        if token.trim().is_empty() || location.trim().is_empty() {
            return Err(CommerceError::ValidationError(
                "Square access token and location are required".into(),
            ));
        }
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| fault(e.to_string()))?;
        let origin = if sandbox {
            "https://connect.squareupsandbox.com"
        } else {
            "https://connect.squareup.com"
        };
        Ok(Self { client, origin: origin.into(), token, location })
    }

    fn request(&self, method: Method, path: &str, body: Option<&Value>) -> Result<Value> {
        let request = self
            .client
            .request(method, format!("{}{path}", self.origin))
            .bearer_auth(&self.token)
            .header("Square-Version", "2025-10-16");
        let response = if let Some(body) = body { request.json(body) } else { request }
            .send()
            .map_err(|e| fault(format!("Square transport: {e}")))?;
        let status = response.status();
        let result =
            response.json::<Value>().map_err(|e| fault(format!("Square response: {e}")))?;
        if !status.is_success() {
            // Only a definitive payment-method refusal is safe for another provider to follow.
            if status.is_client_error()
                && result["errors"].as_array().is_some_and(|errors| {
                    !errors.is_empty()
                        && errors.iter().all(|e| {
                            e["category"] == "PAYMENT_METHOD_ERROR"
                                && matches!(
                                    e["code"].as_str(),
                                    Some(
                                        "CARD_DECLINED"
                                            | "GENERIC_DECLINE"
                                            | "INSUFFICIENT_FUNDS"
                                            | "CARD_EXPIRED"
                                    )
                                )
                        })
                })
            {
                return Ok(result);
            }
            return Err(fault(format!(
                "Square HTTP {status}: {}",
                result["errors"][0]["code"].as_str().unwrap_or("unknown")
            )));
        }
        Ok(result)
    }

    fn complete_payment(
        &self,
        reference: &str,
        amount: Option<Decimal>,
        expected_hold: Option<Decimal>,
        currency: &str,
    ) -> Result<ProviderDecision> {
        let mut response = self.payment(reference, currency)?;
        let current = Self::decision(&response, "payment")?;
        let requested = amount.map(|a| minor_units(a, currency)).transpose()?;
        let held = response["payment"]["amount_money"]["amount"]
            .as_i64()
            .ok_or_else(|| fault("Missing Square payment amount"))?;
        if expected_hold
            .map(|value| minor_units(value, currency))
            .transpose()?
            .is_some_and(|n| n != held)
        {
            return Err(fault("Square authorization differs from the local payment amount"));
        }
        if !matches!(current, ProviderDecision::Authorized { .. }) {
            if matches!(current, ProviderDecision::Captured { .. })
                && requested.is_some_and(|n| n != held)
            {
                return Err(fault("Square already captured a different amount"));
            }
            return Ok(current);
        }
        if let Some(units) = requested.filter(|n| *n != held) {
            if units <= 0 || units > held {
                return Err(CommerceError::ValidationError("Invalid Square capture amount".into()));
            }
            let body = json!({ "idempotency_key": key(&format!("adjust:{reference}:{units}")), "payment": {
                "amount_money": { "currency": currency.to_ascii_uppercase(), "amount": units },
                "version_token": response["payment"]["version_token"] } });
            response =
                self.request(Method::PUT, &format!("/v2/payments/{reference}"), Some(&body))?;
            let decision = Self::decision(&response, "payment")?;
            if !matches!(decision, ProviderDecision::Authorized { .. }) {
                return Ok(decision);
            }
            if response["payment"]["amount_money"]["amount"].as_i64() != Some(units) {
                return Err(fault("Square did not adjust the authorized amount"));
            }
        }
        let body = json!({ "version_token": response["payment"]["version_token"] });
        let completed =
            self.request(Method::POST, &format!("/v2/payments/{reference}/complete"), Some(&body))?;
        if let Some(payment) = completed.get("payment") {
            if payment["id"] != reference
                || payment["location_id"] != self.location
                || payment["amount_money"]["currency"] != currency.to_ascii_uppercase()
                || payment["amount_money"]["amount"].as_i64() != Some(requested.unwrap_or(held))
            {
                return Err(fault("Square capture differs from authorized payment or amount"));
            }
        }
        Self::decision(&completed, "payment")
    }

    fn decision(response: &Value, field: &str) -> Result<ProviderDecision> {
        if let Some(code) = response["errors"][0]["code"].as_str() {
            return Ok(ProviderDecision::Declined {
                code: code.into(),
                message: "Square declined the payment method".into(),
            });
        }
        let object = &response[field];
        let id = object["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| fault("Square response has no reference"))?
            .to_owned();
        match object["status"].as_str() {
            Some("APPROVED") if field == "payment" => {
                Ok(ProviderDecision::Authorized { reference: id })
            }
            Some("COMPLETED") => Ok(ProviderDecision::Captured { reference: id }),
            Some("PENDING") => Ok(ProviderDecision::Pending { reference: id }),
            Some(status @ ("FAILED" | "CANCELED" | "REJECTED")) => Ok(ProviderDecision::Declined {
                code: status.into(),
                message: format!("Square {field} {id} is {status}"),
            }),
            _ => Err(fault("Square returned an unknown operation status")),
        }
    }

    fn payment(&self, reference: &str, currency: &str) -> Result<Value> {
        if reference.is_empty()
            || !reference.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        {
            return Err(CommerceError::ValidationError("Invalid Square payment reference".into()));
        }
        let response = self.request(Method::GET, &format!("/v2/payments/{reference}"), None)?;
        let payment = &response["payment"];
        if payment["id"] != reference
            || payment["location_id"] != self.location
            || payment["amount_money"]["currency"] != currency.to_ascii_uppercase()
        {
            return Err(fault("Square payment does not match the merchant location or currency"));
        }
        Ok(response)
    }

    /// Resolve a known upstream reference before retrying a capture.
    pub fn lookup(&self, reference: &str, currency: &str) -> Result<ProviderDecision> {
        Self::decision(&self.payment(reference, currency)?, "payment")
    }
}

impl PaymentProvider for SquareProvider {
    fn lookup_owned_authorization(
        &self,
        reference: &str,
        charge: &ProviderCharge,
    ) -> Result<ProviderDecision> {
        let response = self.payment(reference, &charge.currency)?;
        let payment = &response["payment"];
        if payment["reference_id"] != key(&charge.idempotency_key)
            || payment["amount_money"]["amount"].as_i64()
                != Some(minor_units(charge.amount, &charge.currency)?)
        {
            return Err(fault("Square payment belongs to another request"));
        }
        Self::decision(&response, "payment")
    }
    fn lookup_refund(
        &self,
        refund: &str,
        payment: &str,
        amount: Decimal,
        currency: &str,
    ) -> Result<ProviderDecision> {
        if refund.is_empty()
            || !refund.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        {
            return Err(CommerceError::ValidationError("Invalid Square refund reference".into()));
        }
        let result = self.request(Method::GET, &format!("/v2/refunds/{refund}"), None)?;
        let value = &result["refund"];
        if value["id"] != refund
            || value["payment_id"] != payment
            || value["amount_money"]["amount"].as_i64() != Some(minor_units(amount, currency)?)
            || value["amount_money"]["currency"] != currency.to_ascii_uppercase()
        {
            return Err(fault("Square refund differs from local payment or amount"));
        }
        Self::decision(&result, "refund")
    }
    fn lookup_authorization(
        &self,
        reference: &str,
        charge: &ProviderCharge,
    ) -> Result<ProviderDecision> {
        let payment = self.payment(reference, &charge.currency)?;
        if payment["payment"]["amount_money"]["amount"].as_i64()
            != Some(minor_units(charge.amount, &charge.currency)?)
        {
            return Err(fault("Square payment differs from the expected amount"));
        }
        Self::decision(&payment, "payment")
    }

    fn name(&self) -> &'static str {
        "square"
    }

    fn authorize(&self, charge: &ProviderCharge) -> Result<ProviderDecision> {
        let source =
            charge.payment_method.as_deref().filter(|s| !s.is_empty()).ok_or_else(|| {
                CommerceError::ValidationError(
                    "Square requires a processor-issued source token".into(),
                )
            })?;
        let money = json!({ "amount": minor_units(charge.amount, &charge.currency)?, "currency": charge.currency.to_ascii_uppercase() });
        let body = json!({ "source_id": source, "idempotency_key": key(&charge.idempotency_key), "amount_money": money,
            "location_id": self.location, "autocomplete": false, "reference_id": key(&charge.idempotency_key) });
        let response = self.request(Method::POST, "/v2/payments", Some(&body))?;
        if let Some(payment) = response.get("payment") {
            if payment["location_id"] != self.location || payment["amount_money"] != money {
                return Err(fault("Square authorization amount or location differs"));
            }
        }
        Self::decision(&response, "payment")
    }

    fn capture(
        &self,
        reference: &str,
        amount: Option<Decimal>,
        currency: &str,
    ) -> Result<ProviderDecision> {
        self.complete_payment(reference, amount, None, currency)
    }

    fn capture_full(
        &self,
        reference: &str,
        expected: Decimal,
        currency: &str,
    ) -> Result<ProviderDecision> {
        self.complete_payment(reference, None, Some(expected), currency)
    }

    fn refund(
        &self,
        reference: &str,
        amount: Option<Decimal>,
        currency: &str,
        idempotency: &str,
    ) -> Result<ProviderDecision> {
        let response = self.payment(reference, currency)?;
        let payment = &response["payment"];
        let units = match amount {
            Some(a) => minor_units(a, currency)?,
            None => payment["amount_money"]["amount"]
                .as_i64()
                .ok_or_else(|| fault("Missing Square payment amount"))?
                .checked_sub(payment["refunded_money"]["amount"].as_i64().unwrap_or_default())
                .ok_or_else(|| fault("Invalid Square refundable balance"))?,
        };
        if units <= 0 {
            return Err(CommerceError::ValidationError("No Square amount to refund".into()));
        }
        let body = json!({ "payment_id": reference, "idempotency_key": key(idempotency),
            "amount_money": { "amount": units, "currency": currency.to_ascii_uppercase() } });
        let refund = self.request(Method::POST, "/v2/refunds", Some(&body))?;
        if let Some(object) = refund.get("refund") {
            if object["payment_id"] != reference || object["amount_money"] != body["amount_money"] {
                return Err(fault("Square refund does not match its payment or amount"));
            }
        }
        Self::decision(&refund, "refund")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    #[test]
    fn square_delayed_capture_and_partial_refund_use_exact_minor_units() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            for (verb, path, status) in [
                ("POST", "/v2/payments", "APPROVED"),
                ("GET", "/v2/payments/card1", "APPROVED"),
                ("POST", "/v2/payments/card1/complete", "COMPLETED"),
                ("GET", "/v2/payments/card1", "COMPLETED"),
                ("POST", "/v2/refunds", "PENDING"),
                ("GET", "/v2/refunds/refund1", "COMPLETED"),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert_eq!(line.trim(), format!("{verb} {path} HTTP/1.1"));
                let mut len = 0;
                loop {
                    line.clear();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(length) = line.to_lowercase().strip_prefix("content-length:") {
                        len = length.trim().parse().unwrap();
                    }
                }
                let mut buffer = vec![0; len];
                reader.read_exact(&mut buffer).unwrap();
                let money = json!({ "amount": 1525, "currency": "USD" });
                let data = if path == "/v2/refunds" {
                    let body: Value = serde_json::from_slice(&buffer).unwrap();
                    assert_eq!(body["payment_id"], "card1");
                    assert_eq!(body["amount_money"]["amount"], 125);
                    json!({ "refund": { "id": "refund1", "payment_id": "card1", "status": status, "amount_money": body["amount_money"] } })
                } else if path == "/v2/refunds/refund1" {
                    json!({ "refund": { "id": "refund1", "payment_id": "card1", "status": status, "amount_money": { "amount": 125, "currency": "USD" } } })
                } else {
                    if path == "/v2/payments" {
                        let body: Value = serde_json::from_slice(&buffer).unwrap();
                        assert_eq!(body["autocomplete"], false);
                        assert_eq!(body["source_id"], "square-token");
                        assert_eq!(body["amount_money"], money);
                        assert!(body["idempotency_key"].as_str().unwrap().len() <= 45);
                    }
                    json!({ "payment": { "id": "card1", "location_id": "location1", "version_token": "v1", "status": status, "amount_money": money } })
                };
                let body = data.to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        let mut provider = SquareProvider::new("test", "location1", true).unwrap();
        provider.origin = origin;
        let charge = ProviderCharge {
            amount: Decimal::new(1525, 2),
            currency: "USD".into(),
            payment_method: Some("square-token".into()),
            idempotency_key: "checkout-attempt".into(),
            description: None,
        };
        assert!(matches!(
            provider.authorize(&charge).unwrap(),
            ProviderDecision::Authorized { .. }
        ));
        assert!(matches!(
            provider.capture("card1", None, "USD").unwrap(),
            ProviderDecision::Captured { .. }
        ));
        assert!(matches!(
            provider.refund("card1", Some(Decimal::new(125, 2)), "USD", "r1").unwrap(),
            ProviderDecision::Pending { .. }
        ));
        assert!(matches!(
            provider.lookup_refund("refund1", "card1", Decimal::new(125, 2), "USD").unwrap(),
            ProviderDecision::Captured { .. }
        ));
        worker.join().unwrap();
    }
}
