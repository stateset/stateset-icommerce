//! Execute Lean's entire replay-decision table against the real seal verifier.
use super::*;
use crate::kernel::receipt::receipt_record;
use stateset_core::{KernelPrincipal, PrincipalKind};

#[test]
fn receipt_replay_matches_lean() {
    let mut checked = 0;
    for line in include_str!("../../../../formal/lean/receipt_promotion.golden").lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        assert_eq!(fields.len(), 5);
        let apply: bool = fields[1].parse().expect("apply");
        let same: bool = fields[2].parse().expect("same fingerprint");
        let sealed: bool = fields[3].parse().expect("seal");
        let mut command = CommandEnvelope::preview(
            "inventory.reservation.confirm",
            "formal-replay",
            KernelPrincipal {
                id: "agent:formal".into(),
                kind: PrincipalKind::Agent,
                tenant_id: Some("formal-tenant".into()),
                delegated_by: Some("user:formal".into()),
                capabilities: vec!["inventory.reservation.confirm".into()],
            },
            (),
        );
        command.mode = if apply { ExecutionMode::Apply } else { ExecutionMode::Preview };
        let mut stored: ExecutionReceipt<Value> = rejected_receipt(
            &command,
            None,
            "formal.test",
            "fixture",
            RetryDisposition::Never,
            "reservation",
        );
        stored.status = serde_json::from_value(Value::String(fields[0].into())).expect("status");
        stored.result = Some(serde_json::json!({"unchanged": true}));
        let audit = SealedAuditEntry { previous_audit_hash: None, request_hash: "same".into() };
        let hash = receipt_audit_hash(None, "same", &serde_json::to_value(&stored).expect("json"))
            .expect("seal hash");
        stored.audit_hash = Some(hash);
        let record = receipt_record("same", &stored).expect("record");
        // A valid seal runs once; invalid seals exercise three independent
        // failure routes, including precedence over semantic-key conflicts.
        let variants: &[&str] =
            if sealed { &["valid"] } else { &["body", "missing_audit", "hash"] };
        for variant in variants {
            let mut record = record.clone();
            if *variant == "body" {
                record.receipt["result"] = serde_json::json!({"unchanged": false});
            }
            if *variant == "hash" {
                record.receipt["audit_hash"] = Value::String("forged".into());
            }
            let result = resolve_replay::<_, Value>(
                &command,
                if same { "same" } else { "different" },
                record,
                if *variant == "missing_audit" { None } else { Some(&audit) },
                "reservation",
            );
            let observed = match result {
                Err(CommerceError::KernelReceiptTampered { .. }) => "tampered",
                Ok(Replay::Promote) => "promote",
                Ok(Replay::Return(receipt)) => {
                    if receipt.error_code.as_deref() == Some("kernel.idempotency_conflict") {
                        assert_eq!(receipt.status, ExecutionStatus::Rejected);
                        "conflict"
                    } else {
                        assert_eq!(
                            serde_json::to_value(receipt).unwrap(),
                            serde_json::to_value(&stored).unwrap(),
                            "{line}"
                        );
                        "replay"
                    }
                }
                other => panic!("unexpected result for {line}/{variant}: {other:?}"),
            };
            assert_eq!(observed, fields[4], "{line}/{variant}");
            checked += 1;
        }
    }
    assert_eq!(checked, 64, "all 32 decisions and seal failure variants must run");
}
