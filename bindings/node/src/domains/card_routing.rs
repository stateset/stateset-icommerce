//! Operator-owned processor accounts for server-side card routing.
use super::*;
use stateset_embedded::{
    PaymentProvider, ProcessorRoute, ProviderDecision, RoutedAuthorization, SquareProvider,
    StripeProvider,
};

#[napi(object)]
pub struct CardProcessorAccount {
    pub account: String,
    pub provider: String,
    pub secret: String,
    pub location: Option<String>,
    pub sandbox: Option<bool>,
    pub currencies: Vec<String>,
    pub methods: Vec<String>,
    pub available: bool,
    pub token: Option<String>,
    pub customer: Option<String>,
}

#[napi(object)]
pub struct RouteCardInput {
    pub payment_id: String,
    pub action: String,
    pub accounts: Vec<CardProcessorAccount>,
    pub amount: Option<String>,
    pub request_key: Option<String>,
    pub reason: Option<String>,
    pub account: Option<String>,
    pub reference: Option<String>,
}

fn status(decision: ProviderDecision) -> serde_json::Value {
    use serde_json::json;
    match decision {
        ProviderDecision::Authorized { reference } => {
            json!({ "state": "authorized", "reference": reference })
        }
        ProviderDecision::Captured { reference } => {
            json!({ "state": "captured", "reference": reference })
        }
        ProviderDecision::Pending { reference } => {
            json!({ "state": "pending", "reference": reference })
        }
        ProviderDecision::RequiresAction { reference, action_url } => {
            json!({ "state": "requires_action", "reference": reference, "actionUrl": action_url })
        }
        ProviderDecision::Declined { code, message } => {
            json!({ "state": "declined", "code": code, "message": message })
        }
    }
}

pub(super) fn run(engine: &RustCommerce, input: RouteCardInput) -> Result<serde_json::Value> {
    use serde_json::json;
    let id =
        input.payment_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid payment ID"))?;
    let accounts = input.accounts;
    let providers = accounts
        .iter()
        .map(|route| -> Result<Box<dyn PaymentProvider>> {
            let make_error =
                |e| wrap(ErrCode::Validation, "Invalid card processor configuration", e);
            match route.provider.as_str() {
                "stripe" => {
                    let client = StripeProvider::new(&route.secret).map_err(make_error)?;
                    Ok(Box::new(match &route.customer {
                        Some(id) => client.with_customer(id.clone()).map_err(make_error)?,
                        None => client,
                    }))
                }
                "square" => Ok(Box::new(
                    SquareProvider::new(
                        &route.secret,
                        route.location.as_deref().unwrap_or(""),
                        route.sandbox.unwrap_or(true),
                    )
                    .map_err(make_error)?,
                )),
                _ => Err(coded(ErrCode::Validation, "Choose a supported card processor")),
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let currencies: Vec<Vec<&str>> =
        accounts.iter().map(|r| r.currencies.iter().map(String::as_str).collect()).collect();
    let methods: Vec<Vec<stateset_core::PaymentMethodType>> = accounts
        .iter()
        .map(|r| {
            r.methods
                .iter()
                .map(|m| {
                    m.parse()
                        .map_err(|_| coded(ErrCode::Validation, "Invalid route payment method"))
                })
                .collect()
        })
        .collect::<Result<_>>()?;
    let routes: Vec<_> = accounts
        .iter()
        .enumerate()
        .map(|(i, item)| ProcessorRoute {
            account: &item.account,
            provider: providers[i].as_ref(),
            currencies: &currencies[i],
            methods: &methods[i],
            available: item.available,
            token: item.token.as_deref().unwrap_or(""),
        })
        .collect();
    let payments = engine.payments();
    let failure = |e| wrap(ErrCode::ExternalService, "Card routing failed", e);
    let value = match input.action.as_str() {
        "expire" => json!({ "state": if payments.expire_unstarted_authorization(id).map_err(failure)? { "expired" } else { "retained" } }),
        "selected" => match payments.selected_processor(id).map_err(failure)? {
            Some((account, provider, reference)) => {
                json!({ "state": "selected", "account": account, "provider": provider, "reference": reference })
            }
            None => json!({ "state": "unselected" }),
        },
        "authorize" | "reconcile" => match (if input.action == "reconcile" {
            payments.reconcile_routed_authorization(
                id,
                &routes,
                input.account.as_deref().unwrap_or(""),
                input.reference.as_deref().unwrap_or(""),
            )
        } else {
            payments.authorize_interactive(id, &routes)
        })
        .map_err(failure)?
        {
            RoutedAuthorization::CollectToken { account } => {
                json!({ "state": "collect_token", "account": account })
            }
            RoutedAuthorization::Decision { account, decision } => {
                let mut value = status(decision);
                value["account"] = json!(account);
                value
            }
        },
        "capture" => status(payments.capture_routed(id, &routes).map_err(failure)?),
        "refund" => {
            let amount = parse_decimal_str(input.amount.as_deref().unwrap_or(""), "refund amount")?;
            let refund = payments
                .refund_routed(
                    id,
                    &routes,
                    amount,
                    input.request_key.as_deref().unwrap_or(""),
                    input.reason.as_deref().unwrap_or("Merchant refund"),
                )
                .map_err(failure)?;
            json!({ "state": refund.status.to_string(), "refundId": refund.id.to_string(), "amount": refund.amount.normalize().to_string(), "currency": refund.currency.to_string() })
        }
        _ => return Err(coded(ErrCode::Validation, "Unknown card routing operation")),
    };
    Ok(value)
}
