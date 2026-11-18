//! Commerce and quality machines: store credits, non-conformance reports.

use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

use super::customer;
use super::harness::{Conformance, Report, SetStatus};

pub(crate) fn store_credit(db: &dyn Database, set: SetStatus<'_>) -> Report {
    let customer = customer(db);
    let repo = db.store_credits();
    let create = || {
        repo.create(CreateStoreCredit {
            customer_id: customer.id,
            amount: dec!(100),
            currency: CurrencyCode::USD,
            reason: StoreCreditReason::Return,
            reference_id: None,
            note: None,
            expires_at: None,
        })
        .unwrap()
        .id
    };
    let credit = |id| repo.get(id).unwrap().unwrap();
    let status = |id| credit(id).status.to_string();
    let adjust = |id, amount| {
        repo.adjust(id, AdjustStoreCredit { amount, note: None, reference_id: None }).map(drop)
    };
    let seed =
        |state: &'static str| move |id: StoreCreditId| set("store_credits", &id.to_string(), state);
    Conformance::new("store_credit", create, status)
        .op("depleted", "adjust (to a zero balance)", |id| adjust(id, -credit(id).current_balance))
        .op("active", "adjust (+10)", |id| adjust(id, dec!(10)))
        .op("depleted", "apply (the whole balance)", |id| {
            repo.apply(id, credit(id).current_balance, None).map(drop)
        })
        .only_from(&["active"])
        .op("active", "apply (part of the balance)", |id| repo.apply(id, dec!(1), None).map(drop))
        .only_from(&["active"])
        .seeded("expired", seed("expired"))
        .seeded("voided", seed("voided"))
        .check()
}

pub(crate) fn ncr(db: &dyn Database, _set: SetStatus<'_>) -> Report {
    let repo = db.quality();
    let create = || {
        repo.create_ncr(CreateNonConformance {
            source: NonConformanceSource::InternalAudit,
            severity: Severity::Minor,
            sku: format!("SM-NCR-{}", Uuid::new_v4().simple()),
            quantity_affected: dec!(1),
            description: "conformance".into(),
            ..Default::default()
        })
        .unwrap()
        .id
    };
    let status = |id| repo.get_ncr(id).unwrap().unwrap().status.to_string();
    let update = |id, status: NcrStatus, disposition: Option<Disposition>| {
        repo.update_ncr(
            id,
            UpdateNonConformance { status: Some(status), disposition, ..Default::default() },
        )
        .map(drop)
    };
    let mut check = Conformance::new("ncr", create, status);
    for (target, label, value) in [
        ("open", "update_ncr status=open", NcrStatus::Open),
        ("under_review", "update_ncr status=under_review", NcrStatus::UnderReview),
        (
            "pending_disposition",
            "update_ncr status=pending_disposition",
            NcrStatus::PendingDisposition,
        ),
        ("corrective_action", "update_ncr status=corrective_action", NcrStatus::CorrectiveAction),
        ("preventive_action", "update_ncr status=preventive_action", NcrStatus::PreventiveAction),
        ("verification", "update_ncr status=verification", NcrStatus::Verification),
    ] {
        check = check.op(target, label, move |id| update(id, value, None));
    }
    check
        .op("closed", "update_ncr status=closed (with a disposition)", |id| {
            update(id, NcrStatus::Closed, Some(Disposition::UseAsIs))
        })
        .refuses_same_status()
        .op("cancelled", "update_ncr status=cancelled", |id| update(id, NcrStatus::Cancelled, None))
        .refuses_same_status()
        .op("closed", "close_ncr (disposition recorded first)", |id| {
            if !repo.get_ncr(id)?.ok_or(CommerceError::NotFound)?.status.is_terminal() {
                repo.update_ncr(
                    id,
                    UpdateNonConformance {
                        disposition: Some(Disposition::UseAsIs),
                        ..Default::default()
                    },
                )?;
            }
            repo.close_ncr(id).map(drop)
        })
        .op("cancelled", "cancel_ncr", |id| repo.cancel_ncr(id).map(drop))
        .check()
}
