use super::Commerce;

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use stateset_core::{
    CanonicalTransactionApi, ChargeSubscription, CommandEnvelope, CommerceError, CommitCheckout,
    ConfirmInventoryReservation, CreateA2AEscrow, CreateInventoryItem, CreatePayment,
    CreateProduct, CreateRefund, DisputeA2AEscrow, EconomicAgent, EconomicAuthority,
    EconomicBudget, EconomicBudgetStatus, FileA2ADispute, FundA2AEscrow, KernelPolicy,
    PostJournalEntry, RefundA2AEscrow, ReleaseA2AEscrow, ReleaseInventoryReservation,
    ReserveInventory, ResolveA2ADispute, SettleX402Intent, ShipOrderCommand,
    SubmitA2ADisputeEvidence, TransitionOrder, TransitionReturn,
};
use stateset_db::kernel::KernelAuditChain;
use stateset_db::kernel_outbox::{KernelAuditCheckpoint, KernelAuditVerification};
use stateset_db::sqlite::SqliteKernelExecutor;

/// An economic agent bound to its compiled authority and embedded executor.
#[derive(Debug, Clone)]
pub struct EconomicAgentRuntime {
    agent: EconomicAgent,
    transactions: CanonicalTransactionApi,
    executor: SqliteKernelExecutor,
}

impl EconomicAgentRuntime {
    #[must_use]
    pub const fn identity(&self) -> &EconomicAgent {
        &self.agent
    }

    #[must_use]
    pub const fn transactions(&self) -> &CanonicalTransactionApi {
        &self.transactions
    }

    #[must_use]
    pub const fn executor(&self) -> &SqliteKernelExecutor {
        &self.executor
    }

    /// Construct a scoped preview command from this runtime's trusted identity.
    #[must_use]
    pub fn command<T>(
        &self,
        command_type: impl Into<String>,
        idempotency_key: impl Into<String>,
        payload: T,
    ) -> CommandEnvelope<T> {
        self.agent.command(command_type, idempotency_key, payload)
    }
}

/// Read-only access to the kernel receipt audit chain.
///
/// Every governed command seals its receipt into an append-only hash chain;
/// this accessor recomputes that chain and mints portable checkpoints that
/// can be retained outside the database to make later rewrites detectable.
///
/// Both backends seal the same chain, so this accessor is backend-neutral: it
/// holds whichever [`KernelAuditChain`] the configured database provides.
///
/// Verification is synchronous on both backends. The Postgres implementation
/// bridges to async through the shared runtime, so an async caller must run
/// these methods on a blocking thread (the HTTP handlers use
/// `AppState::run_blocking`).
#[derive(Debug)]
pub struct KernelAudit {
    chain: Box<dyn KernelAuditChain>,
}

impl KernelAudit {
    /// Recompute every receipt link and report the first broken position.
    pub fn verify_chain(&self) -> Result<KernelAuditVerification, CommerceError> {
        self.chain.verify_chain()
    }

    /// Mint a portable checkpoint of the current chain head.
    pub fn checkpoint(&self) -> Result<KernelAuditCheckpoint, CommerceError> {
        self.chain.checkpoint()
    }

    /// Verify an externally retained checkpoint against the local chain.
    pub fn verify_checkpoint(
        &self,
        checkpoint: &KernelAuditCheckpoint,
    ) -> Result<bool, CommerceError> {
        self.chain.verify_checkpoint(checkpoint)
    }
}

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
    /// Bind a trusted agent identity to ergonomic authority tiers and compile
    /// them into the canonical deny-by-default kernel executor.
    pub fn agent(
        &self,
        agent: EconomicAgent,
        authority: &EconomicAuthority,
    ) -> Result<EconomicAgentRuntime, CommerceError> {
        let policy = authority
            .compile(&agent)
            .map_err(|error| CommerceError::ValidationError(error.to_string()))?;
        let executor = self.kernel_executor(policy)?;
        Ok(EconomicAgentRuntime {
            transactions: CanonicalTransactionApi::new(agent.clone()),
            agent,
            executor,
        })
    }

    /// Bind the small `quote/buy/sell/pay/fulfill/return/refund/subscribe`
    /// intent API to one trusted economic agent identity.
    #[must_use]
    pub const fn transactions(&self, agent: EconomicAgent) -> CanonicalTransactionApi {
        CanonicalTransactionApi::new(agent)
    }

    /// Provision an immutable durable budget for governed payment/refund commands.
    ///
    /// This operator API is deliberately separate from model-facing command
    /// dispatch. Re-provisioning the identical definition is idempotent;
    /// changing a definition under an existing ID is rejected.
    pub fn provision_economic_budget(
        &self,
        budget: &EconomicBudget,
    ) -> Result<EconomicBudgetStatus, CommerceError> {
        self.kernel_executor(KernelPolicy::new("operator:budget-provisioning"))?
            .provision_economic_budget(budget)
    }

    /// Read committed and available money for an economic budget.
    pub fn economic_budget_status(
        &self,
        budget_id: &str,
    ) -> Result<Option<EconomicBudgetStatus>, CommerceError> {
        self.kernel_executor(KernelPolicy::new("operator:budget-status"))?
            .economic_budget_status(budget_id)
    }

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

    /// Read-only access to the kernel receipt audit chain.
    ///
    /// Works on every backend that seals receipts — SQLite and Postgres both
    /// do — so a Postgres-backed instance verifies and checkpoints the same
    /// chain the SQLite one does.
    pub fn kernel_audit(&self) -> Result<KernelAudit, CommerceError> {
        self.db.kernel_audit_chain().map(|chain| KernelAudit { chain }).ok_or_else(|| {
            CommerceError::Internal(format!(
                "the {} backend does not seal a kernel receipt audit chain",
                self.db.backend_name()
            ))
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
        AddCartItem, BillingInterval, CartAddress, ChargeSubscription, CommitCheckout, CreateCart,
        CreateCustomer, CreateSubscription, CreateSubscriptionPlan, CurrencyCode,
        EconomicCommitment, ExecutionStatus, KernelCommandPolicy, KernelPrincipal, Money,
        PaymentMethodType, PrincipalKind,
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
    fn kernel_audit_accessor_verifies_and_checkpoints_the_receipt_chain() {
        let commerce = Commerce::in_memory().expect("in-memory commerce");
        let policy = KernelPolicy::new("test-policy")
            .allow("payments.create", KernelCommandPolicy::requiring(["payments.create"]));
        let empty = commerce.kernel_audit().expect("audit").verify_chain().expect("verify");
        assert!(empty.valid);
        assert_eq!(empty.entries, 0);

        let command = payment_command();
        commerce
            .execute_kernel_command(
                serde_json::to_value(&command).expect("serialize command"),
                policy,
            )
            .expect("execute preview");
        let audit = commerce.kernel_audit().expect("audit");
        let verification = audit.verify_chain().expect("verify");
        assert!(verification.valid);
        assert_eq!(verification.entries, 1);
        let checkpoint = audit.checkpoint().expect("checkpoint");
        assert_eq!(checkpoint.entries, 1);
        assert_eq!(checkpoint.head_hash, verification.head_hash);
        assert!(audit.verify_checkpoint(&checkpoint).expect("verify checkpoint"));
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

    #[test]
    fn governed_payment_binds_policy_commitment_to_domain_amount() {
        let commerce = Commerce::in_memory().expect("in-memory commerce");
        let policy = KernelPolicy::new("economic-policy").allow(
            "payments.create",
            KernelCommandPolicy::requiring(["payments.create"]).with_max_amount(Money::new(
                Decimal::from_str("100.00").expect("decimal"),
                CurrencyCode::USD,
            )),
        );
        let mut command = payment_command();
        command.commitment = Some(EconomicCommitment {
            budget_id: Some("budget:cx".into()),
            amount: Some(
                Money::new(Decimal::from_str("10.00").expect("decimal"), CurrencyCode::USD)
                    .to_wire(),
            ),
            asset_amount: None,
            counterparty_id: Some("customer:test".into()),
            quantity: None,
            evidence: vec!["ticket:123".into()],
        });

        let receipt = commerce
            .execute_kernel_command(
                serde_json::to_value(&command).expect("serialize command"),
                policy,
            )
            .expect("execute governed preview");
        let receipt: stateset_core::ExecutionReceipt<stateset_core::Payment> =
            serde_json::from_value(receipt).expect("typed receipt");

        assert_eq!(receipt.status, ExecutionStatus::Rejected);
        assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_amount_mismatch"));
        let context = receipt.economic_context.expect("economic receipt context");
        assert_eq!(
            context.commitment.and_then(|commitment| commitment.budget_id),
            Some("budget:cx".into())
        );
        assert_eq!(commerce.payments().count(Default::default()).expect("count payments"), 0);
    }

    #[test]
    fn governed_payment_binds_declared_counterparty_to_customer_identity() {
        let commerce = Commerce::in_memory().expect("in-memory commerce");
        let customer = commerce
            .customers()
            .create(CreateCustomer {
                email: "counterparty@example.com".into(),
                first_name: "Grace".into(),
                last_name: "Buyer".into(),
                ..Default::default()
            })
            .expect("create customer");
        let declared = format!("customer:{}", uuid::Uuid::new_v4());
        let policy = KernelPolicy::new("counterparty-policy").allow(
            "payments.create",
            KernelCommandPolicy::requiring(["payments.create"])
                .for_counterparties([declared.clone()]),
        );
        let mut command = payment_command();
        command.payload.customer_id = Some(customer.id);
        command.commitment = Some(
            EconomicCommitment::for_money(
                "budget:not-required",
                Money::new(Decimal::from_str("12.34").expect("decimal"), CurrencyCode::USD),
            )
            .with_counterparty(declared),
        );

        let receipt = commerce
            .kernel_executor(policy)
            .expect("kernel executor")
            .execute_create_payment(&command)
            .expect("execute preview");

        assert_eq!(receipt.status, ExecutionStatus::Rejected);
        assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_counterparty_mismatch"));
    }

    #[test]
    fn governed_escrow_binds_exact_asset_before_custody_changes() {
        let commerce = Commerce::in_memory().expect("in-memory commerce");
        let rule = KernelCommandPolicy::requiring([] as [&str; 0])
            .with_max_asset_amount(Decimal::from_str("100.00").expect("decimal"), "USDC");
        let policy = KernelPolicy::new("asset-policy")
            .allow("a2a.escrow.create", rule.clone())
            .allow("a2a.escrow.fund", rule);
        let executor = commerce.kernel_executor(policy).expect("kernel executor");
        let principal = KernelPrincipal {
            id: "agent:test".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant:test".into()),
            delegated_by: Some("user:test".into()),
            capabilities: vec![],
        };
        let mut create = CommandEnvelope::preview(
            "a2a.escrow.create",
            "asset-create-1",
            principal.clone(),
            CreateA2AEscrow {
                quote_id: None,
                payment_id: None,
                buyer_address: "did:key:buyer".into(),
                seller_address: "did:key:seller".into(),
                amount: Decimal::from_str("25.125").expect("decimal"),
                asset: "usdc".into(),
                network: "eip155:8453".into(),
                release_conditions: vec![],
                expires_at: Utc::now() + chrono::Duration::hours(1),
                auto_release_after: None,
                metadata: None,
            },
        )
        .into_apply();
        create.store_id = Some("store:test".into());
        create.commitment = Some(EconomicCommitment::for_asset(
            Decimal::from_str("25.125").expect("decimal"),
            "USDC",
        ));
        let created = executor.execute_create_a2a_escrow(&create).expect("create escrow");
        assert_eq!(created.status, ExecutionStatus::Succeeded);
        let escrow = created.result.expect("created escrow");
        assert_eq!(escrow.asset, "USDC");

        let mut wrong_fund = CommandEnvelope::preview(
            "a2a.escrow.fund",
            "asset-fund-wrong",
            principal.clone(),
            FundA2AEscrow { escrow_id: escrow.id.clone() },
        )
        .into_apply();
        wrong_fund.store_id = Some("store:test".into());
        wrong_fund.commitment = Some(EconomicCommitment::for_asset(
            Decimal::from_str("25.124").expect("decimal"),
            "USDC",
        ));
        let rejected = executor.execute_fund_a2a_escrow(&wrong_fund).expect("reject mismatch");
        assert_eq!(rejected.status, ExecutionStatus::Rejected);
        assert_eq!(rejected.error_code.as_deref(), Some("kernel.commitment_asset_mismatch"));

        let mut fund = CommandEnvelope::preview(
            "a2a.escrow.fund",
            "asset-fund-correct",
            principal,
            FundA2AEscrow { escrow_id: escrow.id },
        )
        .into_apply();
        fund.store_id = Some("store:test".into());
        fund.commitment = Some(EconomicCommitment::for_asset(
            Decimal::from_str("25.125").expect("decimal"),
            "USDC",
        ));
        let funded = executor.execute_fund_a2a_escrow(&fund).expect("fund escrow");
        assert_eq!(funded.status, ExecutionStatus::Succeeded);
        assert_eq!(
            funded.result.expect("funded escrow").status,
            stateset_core::A2AEscrowStatus::Active
        );
    }

    #[test]
    fn economic_agent_runtime_compiles_authority_into_execution() {
        let commerce = Commerce::in_memory().expect("in-memory commerce");
        let agent =
            EconomicAgent::new("agent:test", "user:test", "payments", "tenant:test", "store:test")
                .with_capabilities(["payments.create"]);
        let authority = EconomicAuthority::new("agent-authority-v1").allow(
            "payments.create",
            stateset_core::EconomicAuthorityRule::money(
                "payments.create",
                Money::new(Decimal::from_str("20.00").expect("decimal"), CurrencyCode::USD),
                Money::new(Decimal::from_str("100.00").expect("decimal"), CurrencyCode::USD),
            ),
        );
        let runtime = commerce.agent(agent, &authority).expect("agent runtime");
        let mut command = runtime.command(
            "payments.create",
            "agent-runtime-payment",
            CreatePayment {
                amount: Decimal::from_str("25.00").expect("decimal"),
                payment_method: PaymentMethodType::CreditCard,
                ..Default::default()
            },
        );
        command.commitment = Some(EconomicCommitment::for_money(
            "budget:not-required",
            Money::new(Decimal::from_str("25.00").expect("decimal"), CurrencyCode::USD),
        ));

        let receipt = runtime.executor().execute_create_payment(&command).expect("execute");
        assert_eq!(receipt.status, ExecutionStatus::Rejected);
        assert!(
            receipt
                .policy
                .expect("policy")
                .reason_codes
                .contains(&"policy.approval_required".to_string())
        );
    }

    #[test]
    fn durable_budget_is_consumed_atomically_and_retries_do_not_double_debit() {
        let commerce = Commerce::in_memory().expect("in-memory commerce");
        let policy = KernelPolicy::new("budget-policy").allow(
            "payments.create",
            KernelCommandPolicy::requiring(["payments.create"]).with_budget().with_max_amount(
                Money::new(Decimal::from_str("100.00").expect("decimal"), CurrencyCode::USD),
            ),
        );
        let executor = commerce.kernel_executor(policy).expect("kernel executor");
        let now = Utc::now();
        let budget = stateset_core::EconomicBudget::new(
            "budget:daily",
            "agent:test",
            Money::new(Decimal::from_str("20.00").expect("decimal"), CurrencyCode::USD),
            now - chrono::Duration::minutes(1),
            now + chrono::Duration::days(1),
        )
        .for_scope("tenant:test", "store:test");
        executor.provision_economic_budget(&budget).expect("provision budget");
        let identical = executor
            .provision_economic_budget(&budget)
            .expect("identical provisioning is idempotent");
        assert_eq!(identical.available.amount, "20.00");
        let mut conflicting = budget;
        conflicting.limit =
            Money::new(Decimal::from_str("21.00").expect("decimal"), CurrencyCode::USD).to_wire();
        let conflict = executor
            .provision_economic_budget(&conflicting)
            .expect_err("budget definitions are immutable");
        assert!(matches!(conflict, CommerceError::Conflict(_)));

        let mut command = payment_command();
        command.commitment = Some(EconomicCommitment::for_money(
            "budget:daily",
            Money::new(Decimal::from_str("12.34").expect("decimal"), CurrencyCode::USD),
        ));
        let preview = executor.execute_create_payment(&command).expect("preview payment");
        assert_eq!(preview.status, ExecutionStatus::Previewed);
        let before_apply = executor
            .economic_budget_status("budget:daily")
            .expect("budget status")
            .expect("budget exists");
        assert_eq!(before_apply.committed.amount, "0");

        let command = command.into_apply();
        let receipt = executor.execute_create_payment(&command).expect("apply payment");
        assert_eq!(receipt.status, ExecutionStatus::Succeeded);

        let replay = executor.execute_create_payment(&command).expect("idempotent replay");
        assert_eq!(replay.receipt_id, receipt.receipt_id);
        let mut overrun = payment_command().into_apply();
        overrun.idempotency_key = "budget-overrun".into();
        overrun.payload.amount = Decimal::from_str("8.00").expect("decimal");
        overrun.commitment = Some(EconomicCommitment::for_money(
            "budget:daily",
            Money::new(Decimal::from_str("8.00").expect("decimal"), CurrencyCode::USD),
        ));
        let rejected = executor.execute_create_payment(&overrun).expect("budget rejection");
        assert_eq!(rejected.status, ExecutionStatus::Rejected);
        assert_eq!(rejected.error_code.as_deref(), Some("kernel.budget_exceeded"));
        let status = executor
            .economic_budget_status("budget:daily")
            .expect("budget status")
            .expect("budget exists");
        assert_eq!(status.committed.amount, "12.34");
        assert_eq!(status.available.amount, "7.66");
    }

    #[test]
    fn concurrent_commands_cannot_overspend_one_budget() {
        let commerce = Commerce::in_memory().expect("in-memory commerce");
        let policy = KernelPolicy::new("concurrent-budget-policy").allow(
            "payments.create",
            KernelCommandPolicy::requiring(["payments.create"]).with_budget(),
        );
        let executor = commerce.kernel_executor(policy).expect("kernel executor");
        let now = Utc::now();
        let budget = stateset_core::EconomicBudget::new(
            "budget:concurrent",
            "agent:test",
            Money::new(Decimal::from_str("20.00").expect("decimal"), CurrencyCode::USD),
            now - chrono::Duration::minutes(1),
            now + chrono::Duration::days(1),
        )
        .for_scope("tenant:test", "store:test");
        executor.provision_economic_budget(&budget).expect("provision budget");

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let handles: Vec<_> = ["concurrent-a", "concurrent-b"]
            .into_iter()
            .map(|key| {
                let executor = executor.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut command = payment_command().into_apply();
                    command.idempotency_key = key.into();
                    command.payload.amount = Decimal::from_str("12.00").expect("decimal");
                    command.commitment = Some(EconomicCommitment::for_money(
                        "budget:concurrent",
                        Money::new(Decimal::from_str("12.00").expect("decimal"), CurrencyCode::USD),
                    ));
                    barrier.wait();
                    executor.execute_create_payment(&command).expect("concurrent command")
                })
            })
            .collect();
        barrier.wait();
        let receipts: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().expect("worker did not panic"))
            .collect();
        assert_eq!(
            receipts.iter().filter(|receipt| receipt.status == ExecutionStatus::Succeeded).count(),
            1
        );
        assert_eq!(
            receipts
                .iter()
                .filter(|receipt| receipt.error_code.as_deref() == Some("kernel.budget_exceeded"))
                .count(),
            1
        );
        let status = executor
            .economic_budget_status("budget:concurrent")
            .expect("budget status")
            .expect("budget exists");
        assert_eq!(status.committed.amount, "12.00");
        assert_eq!(status.available.amount, "8.00");
    }

    #[test]
    fn subscription_charge_binds_cycle_amount_customer_and_budget_atomically() {
        let commerce = Commerce::in_memory().expect("in-memory commerce");
        let customer = commerce
            .customers()
            .create(CreateCustomer {
                email: "subscriber@example.com".into(),
                first_name: "Ada".into(),
                last_name: "Buyer".into(),
                ..Default::default()
            })
            .expect("create customer");
        let plan = commerce
            .subscriptions()
            .create_plan(CreateSubscriptionPlan {
                name: "Kernel plan".into(),
                billing_interval: BillingInterval::Monthly,
                price: Decimal::from_str("12.34").expect("decimal"),
                currency: Some(CurrencyCode::USD),
                ..Default::default()
            })
            .expect("create plan");
        commerce.subscriptions().activate_plan(plan.id).expect("activate plan");
        let subscription = commerce
            .subscriptions()
            .subscribe(CreateSubscription {
                customer_id: customer.id,
                plan_id: plan.id,
                skip_trial: Some(true),
                ..Default::default()
            })
            .expect("create subscription");
        let now = Utc::now();
        let cycle = commerce
            .subscriptions()
            .create_billing_cycle(subscription.id, 2, now, now + chrono::Duration::days(30))
            .expect("create billing cycle");
        assert_eq!(cycle.total, Decimal::from_str("12.34").expect("decimal"));

        let counterparty = format!("customer:{}", customer.id);
        let wrong_counterparty = format!("customer:{}", uuid::Uuid::new_v4());
        let policy = KernelPolicy::new("subscription-budget-policy").allow(
            "subscriptions.charge",
            KernelCommandPolicy::requiring(["subscriptions.charge"])
                .with_budget()
                .with_max_amount(Money::new(
                    Decimal::from_str("20.00").expect("decimal"),
                    CurrencyCode::USD,
                ))
                .for_counterparties([counterparty.clone(), wrong_counterparty.clone()]),
        );
        let executor = commerce.kernel_executor(policy).expect("kernel executor");
        let budget = stateset_core::EconomicBudget::new(
            "budget:subscriptions",
            "agent:test",
            Money::new(Decimal::from_str("20.00").expect("decimal"), CurrencyCode::USD),
            now - chrono::Duration::minutes(1),
            now + chrono::Duration::days(1),
        )
        .for_scope("tenant:test", "store:test");
        executor.provision_economic_budget(&budget).expect("provision budget");

        let mut command = CommandEnvelope::preview(
            "subscriptions.charge",
            "subscription-charge-1",
            KernelPrincipal {
                id: "agent:test".into(),
                kind: PrincipalKind::Agent,
                tenant_id: Some("tenant:test".into()),
                delegated_by: Some("user:test".into()),
                capabilities: vec!["subscriptions.charge".into()],
            },
            ChargeSubscription {
                billing_cycle_id: cycle.id,
                payment_method: PaymentMethodType::CreditCard,
                processor: Some("test".into()),
            },
        );
        command.store_id = Some("store:test".into());
        command.commitment = Some(
            EconomicCommitment::for_money(
                "budget:subscriptions",
                Money::new(Decimal::from_str("12.34").expect("decimal"), CurrencyCode::USD),
            )
            .with_counterparty(counterparty),
        );

        let mut wrong_target = command.clone().into_apply();
        wrong_target.command_id = uuid::Uuid::new_v4();
        wrong_target.idempotency_key = "subscription-charge-wrong-customer".into();
        wrong_target.commitment.as_mut().expect("commitment").counterparty_id =
            Some(wrong_counterparty);
        let rejected =
            executor.execute_charge_subscription(&wrong_target).expect("reject wrong counterparty");
        assert_eq!(rejected.status, ExecutionStatus::Rejected);
        assert_eq!(rejected.error_code.as_deref(), Some("kernel.commitment_counterparty_mismatch"));

        let preview = executor.execute_charge_subscription(&command).expect("preview charge");
        assert_eq!(preview.status, ExecutionStatus::Previewed);
        let status = executor
            .economic_budget_status("budget:subscriptions")
            .expect("budget status")
            .expect("budget exists");
        assert_eq!(status.committed.amount, "0");

        let receipt =
            executor.execute_charge_subscription(&command.into_apply()).expect("apply charge");
        assert_eq!(receipt.status, ExecutionStatus::Succeeded);
        let status = executor
            .economic_budget_status("budget:subscriptions")
            .expect("budget status")
            .expect("budget exists");
        assert_eq!(status.committed.amount, "12.34");
        assert_eq!(status.available.amount, "7.66");
    }

    #[test]
    fn checkout_binds_repriced_total_and_rolls_back_order_on_commitment_mismatch() {
        let commerce = Commerce::in_memory().expect("in-memory commerce");
        let cart = commerce
            .carts()
            .create(CreateCart {
                customer_email: Some("checkout@example.com".into()),
                customer_name: Some("Kernel Buyer".into()),
                items: Some(vec![AddCartItem {
                    sku: "KERNEL-CHECKOUT".into(),
                    name: "Governed item".into(),
                    quantity: 2,
                    unit_price: Decimal::from_str("12.50").expect("decimal"),
                    ..Default::default()
                }]),
                shipping_address: Some(CartAddress {
                    first_name: "Kernel".into(),
                    last_name: "Buyer".into(),
                    line1: "1 Economic Way".into(),
                    city: "Vancouver".into(),
                    postal_code: "V5K 0A1".into(),
                    country: "CA".into(),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .expect("create checkoutable cart");
        assert_eq!(cart.grand_total, Decimal::from_str("25.00").expect("decimal"));

        let policy = KernelPolicy::new("checkout-budget-policy").allow(
            "checkout.commit",
            KernelCommandPolicy::requiring(["checkout.commit"]).with_budget().with_max_amount(
                Money::new(Decimal::from_str("30.00").expect("decimal"), CurrencyCode::USD),
            ),
        );
        let executor = commerce.kernel_executor(policy).expect("kernel executor");
        let now = Utc::now();
        executor
            .provision_economic_budget(
                &stateset_core::EconomicBudget::new(
                    "budget:checkout",
                    "agent:test",
                    Money::new(Decimal::from_str("30.00").expect("decimal"), CurrencyCode::USD),
                    now - chrono::Duration::minutes(1),
                    now + chrono::Duration::days(1),
                )
                .for_scope("tenant:test", "store:test"),
            )
            .expect("provision budget");
        let mut command = CommandEnvelope::preview(
            "checkout.commit",
            "checkout-commit-1",
            KernelPrincipal {
                id: "agent:test".into(),
                kind: PrincipalKind::Agent,
                tenant_id: Some("tenant:test".into()),
                delegated_by: Some("user:test".into()),
                capabilities: vec!["checkout.commit".into()],
            },
            CommitCheckout::new(cart.id),
        );
        command.store_id = Some("store:test".into());
        command.commitment = Some(EconomicCommitment::for_money(
            "budget:checkout",
            Money::new(Decimal::from_str("25.00").expect("decimal"), CurrencyCode::USD),
        ));

        let preview = executor.execute_commit_checkout(&command).expect("preview checkout");
        assert_eq!(preview.status, ExecutionStatus::Previewed);
        assert_eq!(
            executor
                .economic_budget_status("budget:checkout")
                .expect("budget status")
                .expect("budget exists")
                .committed
                .amount,
            "0"
        );

        let mut mismatch = command.clone().into_apply();
        mismatch.command_id = uuid::Uuid::new_v4();
        mismatch.idempotency_key = "checkout-commit-mismatch".into();
        mismatch.commitment = Some(EconomicCommitment::for_money(
            "budget:checkout",
            Money::new(Decimal::from_str("24.00").expect("decimal"), CurrencyCode::USD),
        ));
        let rejected = executor.execute_commit_checkout(&mismatch).expect("reject mismatch");
        assert_eq!(rejected.status, ExecutionStatus::Rejected);
        assert_eq!(rejected.error_code.as_deref(), Some("kernel.commitment_amount_mismatch"));
        assert!(
            commerce.carts().get(cart.id).expect("cart lookup").expect("cart").order_id.is_none()
        );

        let applied = executor
            .execute_commit_checkout(&command.clone().into_apply())
            .expect("apply checkout");
        assert_eq!(applied.status, ExecutionStatus::Succeeded);
        assert_eq!(
            executor
                .economic_budget_status("budget:checkout")
                .expect("budget status")
                .expect("budget exists")
                .committed
                .amount,
            "25.00"
        );

        let mut second_command = command.into_apply();
        second_command.command_id = uuid::Uuid::new_v4();
        second_command.idempotency_key = "checkout-commit-second-command".into();
        let second = executor
            .execute_commit_checkout(&second_command)
            .expect("reject second economic command");
        assert_eq!(second.status, ExecutionStatus::Rejected);
        assert_eq!(second.error_code.as_deref(), Some("commerce.checkout.conflict"));
        assert_eq!(
            executor
                .economic_budget_status("budget:checkout")
                .expect("budget status")
                .expect("budget exists")
                .committed
                .amount,
            "25.00"
        );
    }
}
