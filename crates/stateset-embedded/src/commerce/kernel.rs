use super::Commerce;

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use stateset_core::{
    ChargeSubscription, CommandEnvelope, CommerceError, CommitCheckout,
    ConfirmInventoryReservation, CreateA2AEscrow, CreateInventoryItem, CreatePayment,
    CreateProduct, CreateRefund, DisputeA2AEscrow, FileA2ADispute, FundA2AEscrow, KernelPolicy,
    PostJournalEntry, RefundA2AEscrow, ReleaseA2AEscrow, ReleaseInventoryReservation,
    ReserveInventory, ResolveA2ADispute, SettleX402Intent, ShipOrderCommand,
    SubmitA2ADisputeEvidence, TransitionOrder, TransitionReturn,
};
use stateset_db::sqlite::SqliteKernelExecutor;

fn decode_command<T: DeserializeOwned>(
    command: Value,
) -> Result<CommandEnvelope<T>, CommerceError> {
    serde_json::from_value(command).map_err(|error| {
        CommerceError::ValidationError(format!("invalid kernel command envelope: {error}"))
    })
}

fn encode_receipt<T: Serialize>(receipt: T) -> Result<Value, CommerceError> {
    serde_json::to_value(receipt).map_err(|error| {
        CommerceError::Internal(format!("failed to serialize kernel execution receipt: {error}"))
    })
}

impl Commerce {
    /// Create a governed executor over this instance's SQLite database.
    ///
    /// The policy is host configuration, not agent input. Applications should
    /// construct it outside the model/tool argument surface and reuse it for
    /// every command issued under the same policy revision.
    pub fn kernel_executor(
        &self,
        policy: KernelPolicy,
    ) -> Result<SqliteKernelExecutor, CommerceError> {
        self.sqlite_db.as_ref().map(|database| database.kernel_executor(policy)).ok_or_else(|| {
            CommerceError::Internal(
                "the synchronous kernel executor requires a SQLite commerce instance".into(),
            )
        })
    }

    /// Execute one of the versioned, high-risk commerce commands and return
    /// its durable receipt as JSON.
    ///
    /// Dispatch is closed over the governed command catalog: an unknown
    /// command type is rejected before any repository can be reached.
    pub fn execute_kernel_command(
        &self,
        command: Value,
        policy: KernelPolicy,
    ) -> Result<Value, CommerceError> {
        let command_type = command
            .get("command_type")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                CommerceError::ValidationError(
                    "kernel command envelope is missing command_type".into(),
                )
            })?
            .to_owned();
        let executor = self.kernel_executor(policy)?;

        macro_rules! execute {
            ($payload:ty, $method:ident) => {{
                let envelope: CommandEnvelope<$payload> = decode_command(command)?;
                encode_receipt(executor.$method(&envelope)?)
            }};
        }

        match command_type.as_str() {
            "inventory.item.create" => {
                execute!(CreateInventoryItem, execute_create_inventory_item)
            }
            "products.create" => execute!(CreateProduct, execute_create_product),
            "payments.create" => execute!(CreatePayment, execute_create_payment),
            "payments.create_refund" => execute!(CreateRefund, execute_create_refund),
            "inventory.reserve" => execute!(ReserveInventory, execute_reserve_inventory),
            "inventory.reservation.confirm" => {
                execute!(ConfirmInventoryReservation, execute_confirm_inventory_reservation)
            }
            "inventory.reservation.release" => {
                execute!(ReleaseInventoryReservation, execute_release_inventory_reservation)
            }
            "orders.transition" => execute!(TransitionOrder, execute_transition_order),
            "orders.ship" => execute!(ShipOrderCommand, execute_ship_order),
            "returns.transition" => execute!(TransitionReturn, execute_transition_return),
            "ledger.post" => execute!(PostJournalEntry, execute_post_journal_entry),
            "x402.settle" => execute!(SettleX402Intent, execute_settle_x402_intent),
            "checkout.commit" => execute!(CommitCheckout, execute_commit_checkout),
            "subscriptions.charge" => execute!(ChargeSubscription, execute_charge_subscription),
            "a2a.escrow.create" => execute!(CreateA2AEscrow, execute_create_a2a_escrow),
            "a2a.escrow.dispute" => execute!(DisputeA2AEscrow, execute_dispute_a2a_escrow),
            "a2a.escrow.fund" => execute!(FundA2AEscrow, execute_fund_a2a_escrow),
            "a2a.escrow.release" => execute!(ReleaseA2AEscrow, execute_release_a2a_escrow),
            "a2a.escrow.refund" => execute!(RefundA2AEscrow, execute_refund_a2a_escrow),
            "a2a.dispute.file" => execute!(FileA2ADispute, execute_file_a2a_dispute),
            "a2a.dispute.evidence.submit" => {
                execute!(SubmitA2ADisputeEvidence, execute_submit_a2a_dispute_evidence)
            }
            "a2a.dispute.resolve" => {
                execute!(ResolveA2ADispute, execute_resolve_a2a_dispute)
            }
            _ => Err(CommerceError::ValidationError(format!(
                "unsupported governed kernel command type: {command_type}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use stateset_core::{
        ExecutionStatus, KernelCommandPolicy, KernelPrincipal, PaymentMethodType, PrincipalKind,
    };
    use std::str::FromStr;

    fn payment_command() -> CommandEnvelope<CreatePayment> {
        let mut command = CommandEnvelope::preview(
            "payments.create",
            "binding-preview-1",
            KernelPrincipal {
                id: "agent:test".into(),
                kind: PrincipalKind::Agent,
                tenant_id: Some("tenant:test".into()),
                delegated_by: Some("user:test".into()),
                capabilities: vec!["payments.create".into()],
            },
            CreatePayment {
                amount: Decimal::from_str("12.34").expect("valid test decimal"),
                payment_method: PaymentMethodType::CreditCard,
                ..Default::default()
            },
        );
        command.store_id = Some("store:test".into());
        command.issued_at = Utc::now();
        command
    }

    #[test]
    fn json_dispatch_preserves_preview_safety_and_receipt_contract() {
        let commerce = Commerce::in_memory().expect("in-memory commerce");
        let policy = KernelPolicy::new("test-policy")
            .allow("payments.create", KernelCommandPolicy::requiring(["payments.create"]));
        let command = payment_command();
        let receipt = commerce
            .execute_kernel_command(
                serde_json::to_value(&command).expect("serialize command"),
                policy,
            )
            .expect("execute preview");
        let receipt: stateset_core::ExecutionReceipt<stateset_core::Payment> =
            serde_json::from_value(receipt).expect("typed receipt");

        assert_eq!(receipt.status, ExecutionStatus::Previewed);
        assert_eq!(commerce.payments().count(Default::default()).expect("count payments"), 0);
    }

    #[test]
    fn json_dispatch_is_closed_over_the_governed_catalog() {
        let commerce = Commerce::in_memory().expect("in-memory commerce");
        let mut command = serde_json::to_value(payment_command()).expect("serialize command");
        command["command_type"] = serde_json::json!("customers.delete_all");

        let error = commerce
            .execute_kernel_command(command, KernelPolicy::new("test-policy"))
            .expect_err("unsupported command must fail");
        assert!(error.to_string().contains("unsupported governed kernel command type"));
    }
}
