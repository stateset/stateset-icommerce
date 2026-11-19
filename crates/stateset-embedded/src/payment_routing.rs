//! Durable account selection for operations on an existing native payment.
use super::Payments;
use crate::{PaymentProvider, ProviderCharge, ProviderDecision};
use chrono::Utc;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use stateset_core::{
    CommerceError, CreateCustomObject, CreateCustomObjectType, CustomFieldDefinition,
    CustomFieldType, PaymentId, PaymentMethodType, PaymentTransactionStatus, Result,
};

const RECORD_TYPE: &str = "payment_processor_route";

/// Next action when authorizing through processor-specific hosted fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutedAuthorization {
    /// Collect a token in this account's hosted fields.
    CollectToken { account: String },
    /// The original processor returned an authoritative decision.
    Decision { account: Option<String>, decision: ProviderDecision },
}

/// Operator-owned account, eligibility and tokenized payment method for a single processor.
pub struct ProcessorRoute<'a> {
    /// Stable account identifier. Keep this attached to the same merchant account across retries.
    pub account: &'a str,
    /// Configured processor client.
    pub provider: &'a dyn PaymentProvider,
    /// Eligible ISO currency codes.
    pub currencies: &'a [&'a str],
    /// Eligible native payment methods.
    pub methods: &'a [PaymentMethodType],
    /// Whether the account is currently accepting new authorizations.
    pub available: bool,
    /// Token issued by this account's processor. Never send raw card data.
    pub token: &'a str,
}

impl std::fmt::Debug for ProcessorRoute<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessorRoute")
            .field("account", &self.account)
            .field("provider", &self.provider.name())
            .field("available", &self.available)
            .finish_non_exhaustive()
    }
}

fn invalid(message: &str) -> CommerceError {
    CommerceError::ValidationError(message.into())
}

impl Payments {
    fn route_record(&self, key: &str) -> Result<Option<Value>> {
        Ok(self
            .db
            .custom_objects()
            .get_object_by_handle(RECORD_TYPE, key)?
            .map(|row| row.values["data"].clone()))
    }

    // Unique native handles serialize the first decision across processes and replicas.
    fn first_route_record(&self, key: &str, value: Value) -> Result<Value> {
        if let Some(winner) = self.route_record(key)? {
            return Ok(winner);
        }
        let repo = self.db.custom_objects();
        if repo.get_type_by_handle(RECORD_TYPE)?.is_none() {
            if let Err(err) = repo.create_type(CreateCustomObjectType {
                handle: RECORD_TYPE.into(),
                display_name: "Payment processor routes".into(),
                description: None,
                fields: vec![CustomFieldDefinition {
                    key: "data".into(),
                    field_type: CustomFieldType::Json,
                    required: true,
                    list: false,
                    description: None,
                }],
            }) {
                if repo.get_type_by_handle(RECORD_TYPE)?.is_none() {
                    return Err(err);
                }
            }
        }
        match repo.create_object(CreateCustomObject {
            type_handle: RECORD_TYPE.into(),
            handle: Some(key.into()),
            values: json!({ "data": value }),
            owner_id: None,
            owner_type: None,
        }) {
            Ok(_) => Ok(value),
            Err(err) => self.route_record(key)?.ok_or(err),
        }
    }

    /// Authorize through the first eligible account in priority order. Persist the route
    /// before dispatch. A timeout remains pinned to the original processor until that
    /// processor confirms a refusal. Network errors never authorize a second charge.
    pub fn authorize_routed(
        &self,
        id: PaymentId,
        routes: &[ProcessorRoute<'_>],
    ) -> Result<ProviderDecision> {
        match self.authorize_card_routes(id, routes, false)? {
            RoutedAuthorization::Decision { decision, .. } => Ok(decision),
            RoutedAuthorization::CollectToken { .. } => Err(invalid("Missing processor token")),
        }
    }

    /// Select a processor before collecting card details in its hosted fields.
    /// On a confirmed refusal the buyer can supply a token for the next account.
    pub fn authorize_interactive(
        &self,
        id: PaymentId,
        routes: &[ProcessorRoute<'_>],
    ) -> Result<RoutedAuthorization> {
        self.authorize_card_routes(id, routes, true)
    }

    fn authorize_card_routes(
        &self,
        id: PaymentId,
        routes: &[ProcessorRoute<'_>],
        interactive: bool,
    ) -> Result<RoutedAuthorization> {
        let payment = self.get(id)?.ok_or_else(|| invalid("Unknown payment"))?;
        if !matches!(
            payment.status,
            PaymentTransactionStatus::Pending
                | PaymentTransactionStatus::Processing
                | PaymentTransactionStatus::RequiresAction
        ) {
            return Err(invalid("Payment cannot be authorized in its current state"));
        }
        let currency = payment.currency.to_string();
        if let Some((account, provider, reference)) = self.selected_processor(id)? {
            let route = routes
                .iter()
                .find(|r| r.account == account && r.provider.name() == provider)
                .ok_or_else(|| invalid("Restore the account holding this authorization"))?;
            let charge = ProviderCharge {
                amount: payment.amount,
                currency,
                payment_method: None,
                idempotency_key: String::new(),
                description: None,
            };
            let decision = route.provider.lookup_authorization(&reference, &charge)?;
            self.persist_provider_decision(id, &decision)?;
            return Ok(RoutedAuthorization::Decision { account: Some(account), decision });
        }
        let mut eligible = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for route in routes {
            if route.account.is_empty() || !seen.insert(route.account) {
                return Err(invalid("Processor account identifiers must be distinct"));
            }
            if route.available
                && (interactive || !route.token.is_empty())
                && route.currencies.iter().any(|c| c.eq_ignore_ascii_case(&currency))
                && route.methods.contains(&payment.payment_method)
            {
                eligible.push(route.account);
            }
        }
        let plan_key = format!("plan-{id}");
        let plan = match self.route_record(&plan_key)? {
            Some(plan) => plan,
            None => {
                if eligible.is_empty() || payment.external_id.is_some() {
                    return Err(invalid("No eligible new payment route"));
                }
                self.first_route_record(&plan_key, json!({ "accounts": eligible, "currency": currency, "method": payment.payment_method, "amount": payment.amount.normalize().to_string() }))?
            }
        };
        if plan["method"] != json!(payment.payment_method)
            || plan["currency"] != currency
            || plan["amount"] != payment.amount.normalize().to_string()
        {
            return Err(invalid("Payment changed after routing"));
        }
        let mut last_decline = None;
        for (index, account) in plan["accounts"]
            .as_array()
            .ok_or_else(|| invalid("Invalid stored payment route"))?
            .iter()
            .enumerate()
        {
            let leg = format!("{id}-{index}");
            let refusal = format!("refused-{leg}");
            if let Some(stored) = self.route_record(&refusal)? {
                last_decline = Some(ProviderDecision::Declined {
                    code: stored["code"].as_str().unwrap_or_default().into(),
                    message: "Processor refused authorization".into(),
                });
                continue;
            }
            let route = routes.iter().find(|r| account == r.account).ok_or_else(|| {
                invalid("Restore the original processor account to reconcile this payment")
            })?;
            let start_key = format!("started-{leg}");
            let prior = self.route_record(&start_key)?;
            if prior.is_none() && !route.available {
                // Do not skip a persisted candidate based on a transient local health view.
                return Err(invalid("Previously selected processor account is unavailable"));
            }
            let reference_key = format!("reference-{leg}");
            let known_reference = self.route_record(&reference_key)?;
            if interactive && route.token.is_empty() && known_reference.is_none() {
                return Ok(RoutedAuthorization::CollectToken { account: route.account.into() });
            }
            if self.first_route_record(&format!("dispatch-{id}"), json!("started"))? != "started" {
                return Err(invalid("Card checkout expired before authorization"));
            }
            let fingerprint = format!("{:x}", Sha256::digest(route.token.as_bytes()));
            let start = self.first_route_record(&start_key, json!({ "provider": route.provider.name(), "token_hash": fingerprint, "time": Utc::now().timestamp() }))?;
            if start["provider"] != route.provider.name()
                || (known_reference.is_none() && start["token_hash"] != fingerprint)
            {
                return Err(invalid(
                    "Restore the original processor and token to reconcile this attempt",
                ));
            }
            let charge = ProviderCharge {
                amount: payment.amount,
                currency: currency.clone(),
                payment_method: Some(route.token.into()),
                idempotency_key: format!("route-{leg}"),
                description: Some(format!("StateSet payment {id}")),
            };
            let decision = if let Some(reference) = known_reference {
                route.provider.lookup_authorization(
                    reference
                        .as_str()
                        .ok_or_else(|| invalid("Invalid stored processor reference"))?,
                    &charge,
                )?
            } else {
                // Stripe guarantees idempotent replay for at least 24h. Without a known
                // reference, an older uncertain request requires operator reconciliation.
                if Utc::now().timestamp() - start["time"].as_i64().unwrap_or_default() >= 23 * 3600
                {
                    return Err(invalid("Uncertain authorization needs processor reconciliation"));
                }
                route.provider.authorize(&charge)?
            };
            match &decision {
                ProviderDecision::Declined { code, .. } => {
                    if !matches!(
                        code.as_str(),
                        "card_declined"
                            | "generic_decline"
                            | "insufficient_funds"
                            | "CARD_DECLINED"
                            | "GENERIC_DECLINE"
                            | "INSUFFICIENT_FUNDS"
                            | "FAILED"
                            | "CANCELED"
                            | "canceled"
                    ) {
                        self.persist_provider_decision(id, &decision)?;
                        return Ok(RoutedAuthorization::Decision {
                            account: Some(route.account.into()),
                            decision,
                        });
                    }
                    self.first_route_record(&refusal, json!({ "code": code }))?;
                    last_decline = Some(decision);
                }
                ProviderDecision::Authorized { reference }
                | ProviderDecision::Captured { reference }
                | ProviderDecision::Pending { reference }
                | ProviderDecision::RequiresAction { reference, .. } => {
                    if self.first_route_record(&reference_key, json!(reference))? != *reference {
                        return Err(invalid("Processor returned two authorization references"));
                    }
                    if matches!(
                        decision,
                        ProviderDecision::Authorized { .. } | ProviderDecision::Captured { .. }
                    ) {
                        self.first_route_record(&format!("selected-{id}"), json!({ "account": route.account, "provider": route.provider.name(), "reference": reference }))?;
                    }
                    self.persist_provider_decision(id, &decision)?;
                    return Ok(RoutedAuthorization::Decision {
                        account: Some(route.account.into()),
                        decision,
                    });
                }
            }
        }
        let decision = last_decline.ok_or_else(|| invalid("No processor accepted the payment"))?;
        self.persist_provider_decision(id, &decision)?;
        Ok(RoutedAuthorization::Decision { account: None, decision })
    }

    /// Atomically expire a checkout older than one hour before any processor dispatch.
    pub fn expire_unstarted_authorization(&self, id: PaymentId) -> Result<bool> {
        let payment = self.get(id)?.ok_or_else(|| invalid("Unknown payment"))?;
        if payment.external_id.is_some()
            || payment.created_at + chrono::Duration::hours(1) > Utc::now()
        {
            return Ok(false);
        }
        if payment.status == PaymentTransactionStatus::Cancelled {
            return Ok(self.route_record(&format!("dispatch-{id}"))? == Some(json!("expired")));
        }
        if payment.status != PaymentTransactionStatus::Pending {
            return Ok(false);
        }
        if let Some(plan) = self.route_record(&format!("plan-{id}"))? {
            for i in
                0..plan["accounts"].as_array().ok_or_else(|| invalid("Invalid card plan"))?.len()
            {
                if self.route_record(&format!("started-{id}-{i}"))?.is_some() {
                    return Ok(false);
                }
            }
        }
        if self.first_route_record(&format!("dispatch-{id}"), json!("expired"))? != "expired" {
            return Ok(false);
        }
        self.cancel(id)?;
        Ok(true)
    }

    /// Bind an ambiguous attempt to an operator-recovered upstream reference.
    /// The provider must prove both the request identity and the expected amount.
    pub fn reconcile_routed_authorization(
        &self,
        id: PaymentId,
        routes: &[ProcessorRoute<'_>],
        account: &str,
        reference: &str,
    ) -> Result<RoutedAuthorization> {
        let payment = self.get(id)?.ok_or_else(|| invalid("Unknown payment"))?;
        let plan = self
            .route_record(&format!("plan-{id}"))?
            .ok_or_else(|| invalid("Missing authorization plan"))?;
        let accounts =
            plan["accounts"].as_array().ok_or_else(|| invalid("Invalid authorization plan"))?;
        let index = accounts
            .iter()
            .position(|candidate| candidate == account)
            .ok_or_else(|| invalid("Account is not in original plan"))?;
        let leg = format!("{id}-{index}");
        if self.route_record(&format!("refused-{leg}"))?.is_some() {
            return Err(invalid("Processor has already refused this attempt"));
        }
        let start = self
            .route_record(&format!("started-{leg}"))?
            .ok_or_else(|| invalid("No attempt was sent to this account"))?;
        let route = routes
            .iter()
            .find(|r| r.account == account && start["provider"] == r.provider.name())
            .ok_or_else(|| invalid("Missing original processor"))?;
        let charge = ProviderCharge {
            amount: payment.amount,
            currency: payment.currency.to_string(),
            payment_method: None,
            idempotency_key: format!("route-{leg}"),
            description: None,
        };
        route.provider.lookup_owned_authorization(reference, &charge)?;
        if self.first_route_record(&format!("reference-{leg}"), json!(reference))? != reference {
            return Err(invalid("Authorization reference is already bound"));
        }
        self.authorize_interactive(id, routes)
    }

    /// Capture the full authorization with the originally selected account. Retry by
    /// looking up its current status before making another capture request.
    pub fn capture_routed(
        &self,
        id: PaymentId,
        routes: &[ProcessorRoute<'_>],
    ) -> Result<ProviderDecision> {
        let payment = self.get(id)?.ok_or_else(|| invalid("Unknown payment"))?;
        let (account, name, reference) = self
            .selected_processor(id)?
            .ok_or_else(|| invalid("Authorize the payment before capture"))?;
        let route = routes
            .iter()
            .find(|r| r.account == account && r.provider.name() == name)
            .ok_or_else(|| invalid("Missing original capture account"))?;
        let currency = payment.currency.to_string();
        let charge = ProviderCharge {
            amount: payment.amount,
            currency: currency.clone(),
            payment_method: None,
            idempotency_key: String::new(),
            description: None,
        };
        let remote = route.provider.lookup_authorization(&reference, &charge)?;
        if payment.status == PaymentTransactionStatus::Completed
            && matches!(remote, ProviderDecision::Captured { .. })
        {
            return Ok(remote);
        }
        if !matches!(
            payment.status,
            PaymentTransactionStatus::Processing
                | PaymentTransactionStatus::Pending
                | PaymentTransactionStatus::RequiresAction
        ) {
            return Err(invalid("Payment cannot be captured in its current state"));
        }
        let decision = if matches!(remote, ProviderDecision::Authorized { .. }) {
            route.provider.capture_full(&reference, payment.amount, &currency)?
        } else {
            remote
        };
        self.persist_provider_decision(id, &decision)?;
        Ok(decision)
    }

    /// Refund through the captured account. A network failure reserves the local
    /// refund for reconciliation and retries use the same upstream request key.
    pub fn refund_routed(
        &self,
        id: PaymentId,
        routes: &[ProcessorRoute<'_>],
        amount: rust_decimal::Decimal,
        request_key: &str,
        reason: &str,
    ) -> Result<stateset_core::Refund> {
        if request_key.trim().is_empty() || amount <= rust_decimal::Decimal::ZERO {
            return Err(invalid("Refund amount and request key are required"));
        }
        let (account, name, reference) = self
            .selected_processor(id)?
            .ok_or_else(|| invalid("Missing original authorization"))?;
        let route = routes
            .iter()
            .find(|r| r.account == account && r.provider.name() == name)
            .ok_or_else(|| invalid("Missing original refund account"))?;
        let refund = self.create_refund(stateset_core::CreateRefund {
            payment_id: id,
            amount: Some(amount),
            idempotency_key: Some(request_key.into()),
            reason: Some(reason.into()),
            ..Default::default()
        })?;
        if refund.payment_id != id || refund.amount != amount {
            return Err(invalid("Refund key is bound to different terms"));
        }
        if !matches!(
            refund.status,
            stateset_core::RefundStatus::Pending | stateset_core::RefundStatus::Processing
        ) {
            return Ok(refund);
        }
        let key = format!("refund-{}", refund.id);
        let lookup_key = format!("refund-reference-{}", refund.id);
        let currency = refund.currency.to_string();
        let decision = if let Some(remote) = self.route_record(&lookup_key)? {
            let remote =
                remote.as_str().ok_or_else(|| invalid("Invalid stored refund reference"))?;
            route.provider.lookup_refund(remote, &reference, amount, &currency)?
        } else {
            let started = self.first_route_record(&key, json!(Utc::now().timestamp()))?;
            if Utc::now().timestamp() - started.as_i64().unwrap_or_default() >= 23 * 3600 {
                return Err(invalid("Uncertain refund requires upstream reconciliation"));
            }
            route.provider.refund(&reference, Some(amount), &currency, &key)?
        };
        match decision {
            ProviderDecision::Captured { .. } => self.complete_refund(refund.id),
            ProviderDecision::Declined { code, message } => {
                self.db.payments().fail_refund(refund.id, &format!("{code}: {message}"))
            }
            ProviderDecision::Pending { reference: remote }
            | ProviderDecision::RequiresAction { reference: remote, .. } => {
                if self.first_route_record(&lookup_key, json!(remote))? != remote {
                    return Err(invalid("Processor returned inconsistent refund references"));
                }
                Ok(refund)
            }
            ProviderDecision::Authorized { .. } => {
                Err(invalid("Unexpected authorization during refund"))
            }
        }
    }

    /// Identify the account for subsequent capture, refund, or settlement lookup.
    /// Check the provider name and external reference recorded during authorization.
    pub fn selected_processor(&self, id: PaymentId) -> Result<Option<(String, String, String)>> {
        self.route_record(&format!("selected-{id}"))?
            .map(|record| {
                let field = |name: &str| {
                    record[name]
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| invalid("Invalid stored processor binding"))
                };
                Ok((field("account")?, field("provider")?, field("reference")?))
            })
            .transpose()
    }
}
