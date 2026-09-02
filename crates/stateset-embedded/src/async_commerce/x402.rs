//! Agentic commerce accessors: x402 payment intents, A2A quotes and purchases, agent cards.

use super::*;
use stateset_core::CommerceError;

/// Async x402 and A2A operations.
pub struct AsyncX402 {
    db: Arc<PostgresDatabase>,
}

impl AsyncX402 {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Payment Intent Operations
    // ========================================================================

    /// Create a new x402 payment intent.
    pub async fn create_intent(&self, input: CreateX402PaymentIntent) -> Result<X402PaymentIntent> {
        self.reconcile_with_source(&input).await?;
        self.db.x402_payment_intents().create_async(input).await
    }

    /// Mirror of the sync accessor: an intent for a cart/order must be for
    /// exactly the cart `grand_total` / order `total_amount` in that currency.
    async fn reconcile_with_source(&self, input: &CreateX402PaymentIntent) -> Result<()> {
        if let Some(cart_id) = input.cart_id {
            let cart = self.db.carts().get_async(cart_id).await?.ok_or_else(|| {
                CommerceError::ValidationError(format!(
                    "cart {cart_id} not found; cannot create an x402 intent for it"
                ))
            })?;
            crate::x402::reconcile_intent_amount(
                input,
                "cart",
                cart_id,
                cart.grand_total,
                cart.currency,
            )?;
        }
        if let Some(order_id) = input.order_id {
            let order = self.db.orders().get_async(order_id).await?.ok_or_else(|| {
                CommerceError::ValidationError(format!(
                    "order {order_id} not found; cannot create an x402 intent for it"
                ))
            })?;
            crate::x402::reconcile_intent_amount(
                input,
                "order",
                order_id,
                order.total_amount,
                order.currency,
            )?;
        }
        Ok(())
    }

    /// Get a payment intent by ID.
    pub async fn get_intent(&self, id: Uuid) -> Result<Option<X402PaymentIntent>> {
        self.db.x402_payment_intents().get_async(id).await
    }

    /// Sign a payment intent with its configured signature scheme.
    pub async fn sign_intent(
        &self,
        id: Uuid,
        input: SignX402PaymentIntent,
    ) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().sign_async(id, input).await
    }

    /// Mark an intent as sequenced in a settlement batch.
    pub async fn mark_sequenced(
        &self,
        id: Uuid,
        sequence_number: u64,
        batch_id: Uuid,
    ) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_sequenced_async(id, sequence_number, batch_id).await
    }

    /// Mark an intent as settled on-chain.
    pub async fn mark_settled(
        &self,
        id: Uuid,
        tx_hash: &str,
        block_number: u64,
    ) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_settled_async(id, tx_hash, block_number).await
    }

    /// Mark an intent as failed.
    pub async fn mark_failed(&self, id: Uuid, reason: &str) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_failed_async(id, reason).await
    }

    /// Mark an intent as expired.
    pub async fn mark_expired(&self, id: Uuid) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_expired_async(id).await
    }

    /// Cancel a payment intent.
    pub async fn cancel_intent(&self, id: Uuid) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().cancel_async(id).await
    }

    /// Get all payment intents for a cart.
    pub async fn intents_for_cart(&self, cart_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        self.db.x402_payment_intents().for_cart_async(cart_id).await
    }

    /// Get all payment intents for an order.
    pub async fn intents_for_order(&self, order_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        self.db.x402_payment_intents().for_order_async(order_id).await
    }

    /// Get the next nonce for a payer address.
    pub async fn get_next_nonce(&self, payer_address: &str) -> Result<u64> {
        self.db.x402_payment_intents().get_next_nonce_async(payer_address).await
    }

    /// List payment intents with optional filtering.
    pub async fn list_intents(
        &self,
        filter: X402PaymentIntentFilter,
    ) -> Result<Vec<X402PaymentIntent>> {
        self.db.x402_payment_intents().list_async(filter).await
    }

    /// Count payment intents matching a filter.
    pub async fn count_intents(&self, filter: X402PaymentIntentFilter) -> Result<u64> {
        self.db.x402_payment_intents().count_async(filter).await
    }

    /// Expire all stale intents that have passed their validity period.
    pub async fn expire_stale_intents(&self) -> Result<u64> {
        self.db.x402_payment_intents().expire_stale_intents_async().await
    }

    /// Get intents by status.
    pub async fn intents_by_status(
        &self,
        status: X402IntentStatus,
    ) -> Result<Vec<X402PaymentIntent>> {
        self.list_intents(X402PaymentIntentFilter { status: Some(status), ..Default::default() })
            .await
    }

    /// Get pending intents.
    pub async fn pending_intents(&self) -> Result<Vec<X402PaymentIntent>> {
        self.intents_by_status(X402IntentStatus::Created).await
    }

    /// Get signed intents awaiting settlement.
    pub async fn signed_intents(&self) -> Result<Vec<X402PaymentIntent>> {
        self.intents_by_status(X402IntentStatus::Signed).await
    }

    /// Get settled intents.
    pub async fn settled_intents(&self) -> Result<Vec<X402PaymentIntent>> {
        self.intents_by_status(X402IntentStatus::Settled).await
    }

    // ========================================================================
    // Credit Ledger Operations
    // ========================================================================

    /// Get a credit account for a payer/asset/network.
    pub async fn get_credit_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<Option<X402CreditAccount>> {
        self.db.x402_credits().get_account_async(payer_address, asset, network).await
    }

    /// Get or create a credit account (balance default = 0).
    pub async fn get_or_create_credit_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<X402CreditAccount> {
        self.db.x402_credits().get_or_create_account_async(payer_address, asset, network).await
    }

    /// Get current credit balance for a payer/asset/network.
    pub async fn get_credit_balance(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<u64> {
        self.db.x402_credits().get_balance_async(payer_address, asset, network).await
    }

    /// Apply a credit or debit adjustment.
    pub async fn adjust_credit_balance(
        &self,
        input: X402CreditAdjustment,
    ) -> Result<X402CreditTransaction> {
        self.db.x402_credits().adjust_balance_async(input).await
    }

    /// Credit an account (increase balance).
    #[allow(clippy::too_many_arguments)]
    pub async fn credit_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
        amount: u64,
        reason: Option<String>,
        reference_id: Option<String>,
        metadata: Option<String>,
    ) -> Result<X402CreditTransaction> {
        self.adjust_credit_balance(X402CreditAdjustment {
            payer_address: payer_address.to_string(),
            asset,
            network,
            direction: X402CreditDirection::Credit,
            amount,
            reason,
            reference_id,
            metadata,
        })
        .await
    }

    /// Debit an account (decrease balance).
    #[allow(clippy::too_many_arguments)]
    pub async fn debit_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
        amount: u64,
        reason: Option<String>,
        reference_id: Option<String>,
        metadata: Option<String>,
    ) -> Result<X402CreditTransaction> {
        self.adjust_credit_balance(X402CreditAdjustment {
            payer_address: payer_address.to_string(),
            asset,
            network,
            direction: X402CreditDirection::Debit,
            amount,
            reason,
            reference_id,
            metadata,
        })
        .await
    }

    /// List credit ledger transactions.
    pub async fn list_credit_transactions(
        &self,
        filter: X402CreditTransactionFilter,
    ) -> Result<Vec<X402CreditTransaction>> {
        self.db.x402_credits().list_transactions_async(filter).await
    }

    // ========================================================================
    // Agent Card Operations
    // ========================================================================

    /// Register a new agent card.
    pub async fn register_agent(&self, input: CreateAgentCard) -> Result<AgentCard> {
        self.db.agent_cards().create_async(input).await
    }

    /// Get an agent card by ID.
    pub async fn get_agent(&self, id: Uuid) -> Result<Option<AgentCard>> {
        self.db.agent_cards().get_async(id).await
    }

    /// Get an agent card by wallet address.
    pub async fn get_agent_by_wallet(&self, wallet_address: &str) -> Result<Option<AgentCard>> {
        self.db.agent_cards().get_by_wallet_async(wallet_address).await
    }

    /// Update an agent card.
    pub async fn update_agent(&self, id: Uuid, input: UpdateAgentCard) -> Result<AgentCard> {
        self.db.agent_cards().update_async(id, input).await
    }

    /// Delete an agent card.
    pub async fn delete_agent(&self, id: Uuid) -> Result<()> {
        self.db.agent_cards().delete_async(id).await
    }

    /// List agent cards with optional filtering.
    pub async fn list_agents(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>> {
        self.db.agent_cards().list_async(filter).await
    }

    /// Count agent cards matching a filter.
    pub async fn count_agents(&self, filter: AgentCardFilter) -> Result<u64> {
        self.db.agent_cards().count_async(filter).await
    }

    /// Verify an agent card.
    pub async fn verify_agent(&self, id: Uuid) -> Result<AgentCard> {
        self.db.agent_cards().verify_async(id, TrustLevel::Verified, "system").await
    }

    /// Suspend an agent card.
    pub async fn suspend_agent(&self, id: Uuid, reason: &str) -> Result<AgentCard> {
        self.db.agent_cards().suspend_async(id, reason).await
    }

    /// Reactivate an agent card.
    pub async fn reactivate_agent(&self, id: Uuid) -> Result<AgentCard> {
        self.db.agent_cards().reactivate_async(id).await
    }

    /// Discover agents with specific capabilities.
    pub async fn discover_agents(
        &self,
        network: Option<X402Network>,
        asset: Option<X402Asset>,
        skill: Option<A2ASkill>,
        min_trust_level: Option<TrustLevel>,
    ) -> Result<Vec<AgentCard>> {
        self.db
            .agent_cards()
            .discover_async(AgentCardFilter {
                network,
                asset,
                skill,
                trust_level: None,
                min_trust_level,
                active: Some(true),
                ..Default::default()
            })
            .await
    }

    /// Get all active agents.
    pub async fn active_agents(&self) -> Result<Vec<AgentCard>> {
        self.list_agents(AgentCardFilter { active: Some(true), ..Default::default() }).await
    }

    /// Get agents by trust level.
    pub async fn agents_by_trust_level(&self, level: TrustLevel) -> Result<Vec<AgentCard>> {
        self.list_agents(AgentCardFilter {
            trust_level: Some(level),
            active: Some(true),
            ..Default::default()
        })
        .await
    }

    /// Get verified agents.
    pub async fn verified_agents(&self) -> Result<Vec<AgentCard>> {
        self.agents_by_trust_level(TrustLevel::Verified).await
    }

    /// Create a payment intent for a cart.
    pub async fn create_cart_payment(
        &self,
        cart_id: Uuid,
        payer_address: &str,
        payee_address: &str,
        amount: rust_decimal::Decimal,
        network: X402Network,
        asset: X402Asset,
    ) -> Result<X402PaymentIntent> {
        self.create_intent(CreateX402PaymentIntent {
            payer_address: payer_address.to_string(),
            payee_address: payee_address.to_string(),
            amount: to_smallest_unit(amount, asset),
            asset,
            network,
            cart_id: Some(cart_id),
            ..Default::default()
        })
        .await
    }

    /// Get the active payment intent for a cart.
    pub async fn active_intent_for_cart(&self, cart_id: Uuid) -> Result<Option<X402PaymentIntent>> {
        let intents = self.intents_for_cart(cart_id).await?;
        Ok(intents.into_iter().find(|intent| {
            matches!(
                intent.status,
                X402IntentStatus::Created
                    | X402IntentStatus::Signed
                    | X402IntentStatus::Sequenced
                    | X402IntentStatus::Settled
            )
        }))
    }

    /// Check if an intent is ready for settlement.
    pub async fn is_ready_for_settlement(&self, id: Uuid) -> Result<bool> {
        if let Some(intent) = self.get_intent(id).await? {
            let now = Utc::now().timestamp() as u64;
            Ok(intent.status == X402IntentStatus::Signed && intent.valid_until > now)
        } else {
            Ok(false)
        }
    }

    /// Verify an intent's configured signature against its canonical signing hash.
    pub async fn has_valid_signature(&self, id: Uuid) -> Result<bool> {
        if let Some(intent) = self.get_intent(id).await? {
            if !intent.is_signed() {
                return Ok(false);
            }
            Ok(intent.verify_signature().unwrap_or(false))
        } else {
            Ok(false)
        }
    }

    // ========================================================================
    // A2A Commerce Operations
    // ========================================================================

    /// Create a new A2A quote.
    pub async fn create_quote(&self, input: CreateA2AQuote) -> Result<SkillQuote> {
        self.db.a2a_quotes().create_quote_async(input).await
    }

    /// Get an A2A quote by ID.
    pub async fn get_quote(&self, id: Uuid) -> Result<Option<SkillQuote>> {
        self.db.a2a_quotes().get_quote_async(id).await
    }

    /// Get an A2A quote by quote number.
    pub async fn get_quote_by_number(&self, quote_number: &str) -> Result<Option<SkillQuote>> {
        self.db.a2a_quotes().get_quote_by_number_async(quote_number).await
    }

    /// Update A2A quote status.
    pub async fn update_quote_status(&self, id: Uuid, status: QuoteStatus) -> Result<SkillQuote> {
        self.db.a2a_quotes().update_quote_status_async(id, status).await
    }

    /// List A2A quotes with filter.
    pub async fn list_quotes(&self, filter: SkillQuoteFilter) -> Result<Vec<SkillQuote>> {
        self.db.a2a_quotes().list_quotes_async(filter).await
    }

    /// Count A2A quotes matching filter.
    pub async fn count_quotes(&self, filter: SkillQuoteFilter) -> Result<u64> {
        self.db.a2a_quotes().count_quotes_async(filter).await
    }

    /// Create a new A2A purchase.
    pub async fn create_purchase(&self, input: CreateA2APurchase) -> Result<A2APurchase> {
        self.db.a2a_purchases().create_purchase_async(input).await
    }

    /// Get an A2A purchase by ID.
    pub async fn get_purchase(&self, id: Uuid) -> Result<Option<A2APurchase>> {
        self.db.a2a_purchases().get_purchase_async(id).await
    }

    /// Get an A2A purchase by purchase number.
    pub async fn get_purchase_by_number(
        &self,
        purchase_number: &str,
    ) -> Result<Option<A2APurchase>> {
        self.db.a2a_purchases().get_purchase_by_number_async(purchase_number).await
    }

    /// Update A2A purchase status.
    pub async fn update_purchase_status(
        &self,
        id: Uuid,
        status: PurchaseStatus,
    ) -> Result<A2APurchase> {
        self.db.a2a_purchases().update_purchase_status_async(id, status).await
    }

    /// Link an A2A purchase to an order.
    pub async fn link_purchase_to_order(
        &self,
        purchase_id: Uuid,
        order_id: Uuid,
    ) -> Result<A2APurchase> {
        self.db.a2a_purchases().link_purchase_to_order_async(purchase_id, order_id).await
    }

    /// Confirm delivery for an A2A purchase.
    pub async fn confirm_delivery(
        &self,
        purchase_id: Uuid,
        signature: &str,
        rating: Option<u8>,
        feedback: Option<&str>,
    ) -> Result<A2APurchase> {
        self.db
            .a2a_purchases()
            .confirm_delivery_async(purchase_id, signature, rating, feedback)
            .await
    }

    /// List A2A purchases with filter.
    pub async fn list_purchases(&self, filter: A2APurchaseFilter) -> Result<Vec<A2APurchase>> {
        self.db.a2a_purchases().list_purchases_async(filter).await
    }

    /// Count A2A purchases matching filter.
    pub async fn count_purchases(&self, filter: A2APurchaseFilter) -> Result<u64> {
        self.db.a2a_purchases().count_purchases_async(filter).await
    }
}
