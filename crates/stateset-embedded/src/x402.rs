//! x402 Payment Protocol operations for AI agent commerce
//!
//! This module provides high-level operations for x402 stablecoin payments,
//! enabling AI agents to transact with USDC, ssUSD, and other stablecoins
//! on Set Chain L2 and other supported networks.
//!
//! # Overview
//!
//! The x402 protocol enables instant, low-cost stablecoin payments for AI agents:
//! - Hybrid Ed25519 + ML-DSA-65 payment intents by default
//! - Merkle proof verification for settlement confirmation
//! - Multi-network support (Set Chain, Base, Ethereum, Arbitrum)
//! - Multi-asset support (USDC, ssUSD, USDT, DAI)
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateX402PaymentIntent, X402Network, X402Asset};
//! use rust_decimal_macros::dec;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a payment intent
//! let intent = commerce.x402().create_intent(CreateX402PaymentIntent {
//!     payer_address: "0xBuyer...".into(),
//!     payee_address: "0xSeller...".into(),
//!     amount: dec!(100.00),
//!     asset: X402Asset::Usdc,
//!     network: X402Network::SetChain,
//!     ..Default::default()
//! })?;
//!
//! // Sign the intent with its configured scheme. New intents default to hybrid
//! // Ed25519 + ML-DSA-65 signatures.
//! let signed = commerce.x402().sign_intent(intent.id, SignX402PaymentIntent {
//!     intent_id: intent.id,
//!     signature_scheme: None,
//!     signature: "0x<ed25519_signature_component>".into(),
//!     public_key: "0x<ed25519_public_key_component>".into(),
//!     signature_bundle: Some(x402_signature_bundle),
//!     public_key_bundle: Some(x402_public_key_bundle),
//! })?;
//!
//! // After on-chain settlement, mark as settled
//! let settled = commerce.x402().mark_settled(
//!     intent.id,
//!     "0xTxHash...",
//!     12345678,
//! )?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{
    A2APurchase, A2APurchaseFilter, AgentCard, AgentCardFilter, CreateA2APurchase, CreateA2AQuote,
    CreateAgentCard, CreateX402PaymentIntent, PurchaseStatus, QuoteStatus, Result,
    SignX402PaymentIntent, SkillQuote, SkillQuoteFilter, TrustLevel, UpdateAgentCard, X402Asset,
    X402CreditAccount, X402CreditAdjustment, X402CreditDirection, X402CreditTransaction,
    X402CreditTransactionFilter, X402IntentStatus, X402Network, X402PaymentIntent,
    X402PaymentIntentFilter,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// x402 payment protocol operations
///
/// Provides methods for creating, signing, and managing x402 payment intents,
/// as well as agent card management for AI agent commerce.
pub struct X402 {
    db: Arc<dyn Database>,
}

/// Fiat currency a stablecoin asset is pegged to; `None` for volatile assets
/// that cannot be reconciled against a fiat-priced cart or order.
const fn asset_fiat_currency(asset: X402Asset) -> Option<stateset_core::CurrencyCode> {
    match asset {
        X402Asset::Usdc
        | X402Asset::Usdt
        | X402Asset::Dai
        | X402Asset::SsUsd
        | X402Asset::WssUsd => Some(stateset_core::CurrencyCode::USD),
        _ => None,
    }
}

/// Statuses under which an intent still claims (or has already collected)
/// the cart/order it is linked to. A second intent in one of these states
/// for the same cart/order would be a double charge.
pub(crate) const CLAIMING_STATUSES: [X402IntentStatus; 5] = [
    X402IntentStatus::Created,
    X402IntentStatus::Signed,
    X402IntentStatus::Sequenced,
    X402IntentStatus::Batched,
    X402IntentStatus::Settled,
];

/// Refuse a new cart/order-linked intent while one of `existing` still
/// claims (or has settled) the same source. The error names the existing
/// intent so a caller can reuse or cancel it (idempotent-style).
pub(crate) fn refuse_duplicate_claim(
    source: &str,
    source_id: Uuid,
    existing: &[X402PaymentIntent],
) -> Result<()> {
    use stateset_core::CommerceError;
    if let Some(open) = existing.iter().find(|i| CLAIMING_STATUSES.contains(&i.status)) {
        let verb = if open.status == X402IntentStatus::Settled {
            "was already paid by"
        } else {
            "already has an open"
        };
        return Err(CommerceError::Conflict(format!(
            "{source} {source_id} {verb} x402 intent {} ({}); reuse or cancel it instead of creating another",
            open.id, open.status
        )));
    }
    Ok(())
}

/// Check that `input.amount` (in the asset's smallest unit) equals `expected`
/// in `currency`, the amount the referenced cart/order actually charges.
///
/// Reconciliation contract for cart/order-linked intents:
/// * the intent must be for **exactly** the cart `grand_total` / order
///   `total_amount` — no partial or over-payments;
/// * only USD-pegged stablecoins (`USDC`, `USDT`, `DAI`, `ssUSD`, `wssUSD`)
///   can be reconciled, and only against a source priced in `USD`. A cart or
///   order in any other currency, or a volatile asset such as ETH, is refused
///   with a [`CommerceError::ValidationError`](stateset_core::CommerceError)
///   naming both currencies.
pub(crate) fn reconcile_intent_amount(
    input: &CreateX402PaymentIntent,
    source: &str,
    source_id: Uuid,
    expected: rust_decimal::Decimal,
    currency: stateset_core::CurrencyCode,
) -> Result<()> {
    use stateset_core::{CommerceError, from_smallest_unit};

    let asset_currency = asset_fiat_currency(input.asset).ok_or_else(|| {
        CommerceError::ValidationError(format!(
            "x402 asset {} has no fiat peg; cannot reconcile it against {source} {source_id} priced in {}",
            input.asset,
            currency.as_str()
        ))
    })?;
    if asset_currency != currency {
        return Err(CommerceError::ValidationError(format!(
            "x402 asset {} settles in {} but {source} {source_id} is priced in {}",
            input.asset,
            asset_currency.as_str(),
            currency.as_str()
        )));
    }
    let amount = from_smallest_unit(input.amount, input.asset);
    if amount != expected {
        return Err(CommerceError::ValidationError(format!(
            "x402 intent amount {amount} {} does not match {source} {source_id} total {expected} {}",
            input.asset,
            currency.as_str()
        )));
    }
    Ok(())
}

impl std::fmt::Debug for X402 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("X402").finish_non_exhaustive()
    }
}

impl X402 {
    /// Create a new X402 operations instance
    pub fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Payment Intent Operations
    // ========================================================================

    /// Create a new x402 payment intent
    ///
    /// Creates an unsigned payment intent that must be signed by the payer
    /// before it can be submitted for settlement.
    ///
    /// # Cart / order reconciliation
    ///
    /// An intent carrying `cart_id` / `order_id` is checked against the
    /// referenced cart or order before it exists:
    /// * the amount must equal the cart `grand_total` / order `total_amount`
    ///   **exactly** (no partial or over-payments);
    /// * the asset must be a USD-pegged stablecoin and the cart/order must be
    ///   priced in `USD` — any other currency pairing is refused with a
    ///   `ValidationError` naming both currencies;
    /// * at most one intent may claim a cart/order at a time: if an intent in
    ///   `Created`/`Signed`/`Sequenced`/`Batched` status is still open, or a
    ///   `Settled` intent already paid it, the call fails with a `Conflict`
    ///   naming that intent so the caller can reuse or cancel it.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let intent = commerce.x402().create_intent(CreateX402PaymentIntent {
    ///     payer_address: "0xBuyer...".into(),
    ///     payee_address: "0xSeller...".into(),
    ///     amount: dec!(50.00),
    ///     asset: X402Asset::Usdc,
    ///     network: X402Network::SetChain,
    ///     cart_id: Some(cart.id),
    ///     ..Default::default()
    /// })?;
    /// ```
    pub fn create_intent(&self, input: CreateX402PaymentIntent) -> Result<X402PaymentIntent> {
        self.reconcile_with_source(&input)?;
        self.db.x402_payment_intents().create(input)
    }

    /// An intent created for a cart or order must be for exactly what that
    /// cart/order charges. The repository only rejects a zero amount, so the
    /// accessor reconciles the caller's amount against the cart's
    /// `grand_total` / the order's `total_amount` (same currency) before the
    /// intent exists.
    fn reconcile_with_source(&self, input: &CreateX402PaymentIntent) -> Result<()> {
        if let Some(cart_id) = input.cart_id {
            let cart = self.db.carts().get(cart_id.into())?.ok_or_else(|| {
                stateset_core::CommerceError::ValidationError(format!(
                    "cart {cart_id} not found; cannot create an x402 intent for it"
                ))
            })?;
            reconcile_intent_amount(input, "cart", cart_id, cart.grand_total, cart.currency)?;
            refuse_duplicate_claim("cart", cart_id, &self.intents_for_cart(cart_id)?)?;
        }
        if let Some(order_id) = input.order_id {
            let order = self.db.orders().get(order_id.into())?.ok_or_else(|| {
                stateset_core::CommerceError::ValidationError(format!(
                    "order {order_id} not found; cannot create an x402 intent for it"
                ))
            })?;
            reconcile_intent_amount(input, "order", order_id, order.total_amount, order.currency)?;
            refuse_duplicate_claim("order", order_id, &self.intents_for_order(order_id)?)?;
        }
        Ok(())
    }

    /// Get a payment intent by ID
    pub fn get_intent(&self, id: Uuid) -> Result<Option<X402PaymentIntent>> {
        self.db.x402_payment_intents().get(id)
    }

    /// Sign a payment intent with its configured signature scheme
    ///
    /// The payer agent signs the intent's signing hash with their private key.
    /// New intents default to hybrid Ed25519 + ML-DSA-65, and the signing request
    /// must match the intent's configured scheme. This transitions the intent
    /// from `Created` to `Signed` status.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let signed = commerce.x402().sign_intent(intent.id, SignX402PaymentIntent {
    ///     intent_id: intent.id,
    ///     signature_scheme: None,
    ///     signature: "0x<ed25519_signature_component>".into(),
    ///     public_key: "0x<ed25519_public_key_component>".into(),
    ///     signature_bundle: Some(x402_signature_bundle),
    ///     public_key_bundle: Some(x402_public_key_bundle),
    /// })?;
    /// ```
    pub fn sign_intent(&self, id: Uuid, input: SignX402PaymentIntent) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().sign(id, input)
    }

    /// Mark an intent as sequenced in a batch
    ///
    /// Called when the intent has been included in a settlement batch
    /// but not yet confirmed on-chain.
    pub fn mark_sequenced(
        &self,
        id: Uuid,
        sequence_number: u64,
        batch_id: Uuid,
    ) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_sequenced(id, sequence_number, batch_id)
    }

    /// Mark an intent as settled on-chain
    ///
    /// Called after the payment has been confirmed on the blockchain.
    /// This is the final successful state for an intent.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let settled = commerce.x402().mark_settled(
    ///     intent.id,
    ///     "0x1234...abcd",  // Transaction hash
    ///     12345678,         // Block number
    /// )?;
    /// ```
    pub fn mark_settled(
        &self,
        id: Uuid,
        tx_hash: &str,
        block_number: u64,
    ) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_settled(id, tx_hash, block_number)
    }

    /// Mark a sequenced intent as included in a published batch commitment
    ///
    /// Records the batch merkle root and this intent's inclusion proof and
    /// moves the intent `Sequenced -> Batched`. Batched intents are exempt
    /// from the validity sweeper: their outcome is decided by the batch's
    /// on-chain result (`mark_settled` / `mark_failed`).
    pub fn mark_batched(
        &self,
        id: Uuid,
        batch_merkle_root: &str,
        inclusion_proof: Vec<String>,
    ) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_batched(id, batch_merkle_root, inclusion_proof)
    }

    /// Mark an intent as failed
    ///
    /// Called when the payment could not be processed (e.g., insufficient funds,
    /// invalid signature, network error).
    pub fn mark_failed(&self, id: Uuid, reason: &str) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_failed(id, reason)
    }

    /// Mark an intent as expired
    ///
    /// Called when the intent's validity period has passed without settlement.
    pub fn mark_expired(&self, id: Uuid) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().mark_expired(id)
    }

    /// Cancel a payment intent
    ///
    /// Can only cancel intents that are in `Created` or `Signed` status.
    /// Once sequenced or settled, intents cannot be cancelled.
    pub fn cancel_intent(&self, id: Uuid) -> Result<X402PaymentIntent> {
        self.db.x402_payment_intents().cancel(id)
    }

    /// Get all payment intents for a cart
    pub fn intents_for_cart(&self, cart_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        self.db.x402_payment_intents().for_cart(cart_id)
    }

    /// Get all payment intents for an order
    pub fn intents_for_order(&self, order_id: Uuid) -> Result<Vec<X402PaymentIntent>> {
        self.db.x402_payment_intents().for_order(order_id)
    }

    /// Get the next nonce for a payer address
    ///
    /// Used to ensure payment intents are processed in order and to prevent
    /// replay attacks.
    pub fn get_next_nonce(&self, payer_address: &str) -> Result<u64> {
        self.db.x402_payment_intents().get_next_nonce(payer_address)
    }

    /// List payment intents with optional filtering
    pub fn list_intents(&self, filter: X402PaymentIntentFilter) -> Result<Vec<X402PaymentIntent>> {
        self.db.x402_payment_intents().list(filter)
    }

    /// Count payment intents matching a filter
    pub fn count_intents(&self, filter: X402PaymentIntentFilter) -> Result<u64> {
        self.db.x402_payment_intents().count(filter)
    }

    /// Expire all stale intents that have passed their validity period
    ///
    /// Returns the number of intents that were expired.
    pub fn expire_stale_intents(&self) -> Result<u64> {
        self.db.x402_payment_intents().expire_stale_intents()
    }

    /// Get intents by status
    pub fn intents_by_status(&self, status: X402IntentStatus) -> Result<Vec<X402PaymentIntent>> {
        self.list_intents(X402PaymentIntentFilter { status: Some(status), ..Default::default() })
    }

    /// Get pending intents (created but not yet signed)
    pub fn pending_intents(&self) -> Result<Vec<X402PaymentIntent>> {
        self.intents_by_status(X402IntentStatus::Created)
    }

    /// Get signed intents awaiting settlement
    pub fn signed_intents(&self) -> Result<Vec<X402PaymentIntent>> {
        self.intents_by_status(X402IntentStatus::Signed)
    }

    /// Get settled intents
    pub fn settled_intents(&self) -> Result<Vec<X402PaymentIntent>> {
        self.intents_by_status(X402IntentStatus::Settled)
    }

    // ========================================================================
    // A2A Commerce Operations
    // ========================================================================

    /// Create a new A2A quote
    pub fn create_quote(&self, input: CreateA2AQuote) -> Result<SkillQuote> {
        self.db.a2a_quotes().create_quote(input)
    }

    /// Get an A2A quote by ID
    pub fn get_quote(&self, id: Uuid) -> Result<Option<SkillQuote>> {
        self.db.a2a_quotes().get_quote(id)
    }

    /// Get an A2A quote by quote number
    pub fn get_quote_by_number(&self, quote_number: &str) -> Result<Option<SkillQuote>> {
        self.db.a2a_quotes().get_quote_by_number(quote_number)
    }

    /// Update A2A quote status
    pub fn update_quote_status(&self, id: Uuid, status: QuoteStatus) -> Result<SkillQuote> {
        self.db.a2a_quotes().update_quote_status(id, status)
    }

    /// List A2A quotes with filter
    pub fn list_quotes(&self, filter: SkillQuoteFilter) -> Result<Vec<SkillQuote>> {
        self.db.a2a_quotes().list_quotes(filter)
    }

    /// Count A2A quotes matching filter
    pub fn count_quotes(&self, filter: SkillQuoteFilter) -> Result<u64> {
        self.db.a2a_quotes().count_quotes(filter)
    }

    /// Create a new A2A purchase
    pub fn create_purchase(&self, input: CreateA2APurchase) -> Result<A2APurchase> {
        self.db.a2a_purchases().create_purchase(input)
    }

    /// Get an A2A purchase by ID
    pub fn get_purchase(&self, id: Uuid) -> Result<Option<A2APurchase>> {
        self.db.a2a_purchases().get_purchase(id)
    }

    /// Get an A2A purchase by purchase number
    pub fn get_purchase_by_number(&self, purchase_number: &str) -> Result<Option<A2APurchase>> {
        self.db.a2a_purchases().get_purchase_by_number(purchase_number)
    }

    /// Update A2A purchase status
    pub fn update_purchase_status(&self, id: Uuid, status: PurchaseStatus) -> Result<A2APurchase> {
        self.db.a2a_purchases().update_purchase_status(id, status)
    }

    /// Link A2A purchase to an order
    pub fn link_purchase_to_order(&self, purchase_id: Uuid, order_id: Uuid) -> Result<A2APurchase> {
        self.db.a2a_purchases().link_purchase_to_order(purchase_id, order_id)
    }

    /// Confirm delivery for an A2A purchase
    pub fn confirm_delivery(
        &self,
        purchase_id: Uuid,
        signature: &str,
        rating: Option<u8>,
        feedback: Option<&str>,
    ) -> Result<A2APurchase> {
        self.db.a2a_purchases().confirm_delivery(purchase_id, signature, rating, feedback)
    }

    /// List A2A purchases with filter
    pub fn list_purchases(&self, filter: A2APurchaseFilter) -> Result<Vec<A2APurchase>> {
        self.db.a2a_purchases().list_purchases(filter)
    }

    /// Count A2A purchases matching filter
    pub fn count_purchases(&self, filter: A2APurchaseFilter) -> Result<u64> {
        self.db.a2a_purchases().count_purchases(filter)
    }

    // ========================================================================
    // Credit Ledger Operations (Metered Billing)
    // ========================================================================

    /// Get a credit account for a payer/asset/network
    pub fn get_credit_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<Option<X402CreditAccount>> {
        self.db.x402_credits().get_account(payer_address, asset, network)
    }

    /// Get or create a credit account (balance default = 0)
    pub fn get_or_create_credit_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<X402CreditAccount> {
        self.db.x402_credits().get_or_create_account(payer_address, asset, network)
    }

    /// Get current credit balance for a payer/asset/network
    pub fn get_credit_balance(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<u64> {
        self.db.x402_credits().get_balance(payer_address, asset, network)
    }

    /// Apply a credit or debit adjustment
    pub fn adjust_credit_balance(
        &self,
        input: X402CreditAdjustment,
    ) -> Result<X402CreditTransaction> {
        self.db.x402_credits().adjust_balance(input)
    }

    /// Credit an account (increase balance)
    #[allow(clippy::too_many_arguments)]
    pub fn credit_account(
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
    }

    /// Debit an account (decrease balance)
    #[allow(clippy::too_many_arguments)]
    pub fn debit_account(
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
    }

    /// List credit ledger transactions
    pub fn list_credit_transactions(
        &self,
        filter: X402CreditTransactionFilter,
    ) -> Result<Vec<X402CreditTransaction>> {
        self.db.x402_credits().list_transactions(filter)
    }

    // ========================================================================
    // A2A Credit Terms (durable net-30/60/90 lines between agents)
    // ========================================================================

    /// Open a tenant-scoped credit line between two agents.
    pub fn create_credit_terms(
        &self,
        input: stateset_core::CreateA2ACreditTerms,
    ) -> Result<stateset_core::A2ACreditTerms> {
        self.db.a2a_credit_terms().create_terms(input)
    }

    /// Fetch a credit line by id within a tenant.
    pub fn get_credit_terms(
        &self,
        tenant_id: &str,
        id: Uuid,
    ) -> Result<Option<stateset_core::A2ACreditTerms>> {
        self.db.a2a_credit_terms().get_terms(tenant_id, id)
    }

    /// List credit lines within a tenant.
    pub fn list_credit_terms(
        &self,
        filter: stateset_core::A2ACreditTermsFilter,
    ) -> Result<Vec<stateset_core::A2ACreditTerms>> {
        self.db.a2a_credit_terms().list_terms(filter)
    }

    /// Draw on a credit line (atomic; refused past the available credit).
    pub fn charge_credit_terms(
        &self,
        input: stateset_core::A2ACreditMovement,
    ) -> Result<(stateset_core::A2ACreditTerms, stateset_core::A2ACreditEntry)> {
        self.db.a2a_credit_terms().charge(input)
    }

    /// Pay down a credit line.
    pub fn record_credit_terms_payment(
        &self,
        input: stateset_core::A2ACreditMovement,
    ) -> Result<(stateset_core::A2ACreditTerms, stateset_core::A2ACreditEntry)> {
        self.db.a2a_credit_terms().record_payment(input)
    }

    /// Journal entries for a credit line, oldest first.
    pub fn list_credit_terms_entries(
        &self,
        tenant_id: &str,
        terms_id: Uuid,
    ) -> Result<Vec<stateset_core::A2ACreditEntry>> {
        self.db.a2a_credit_terms().list_entries(tenant_id, terms_id)
    }

    // ========================================================================
    // A2A Agent Messaging (durable conversations)
    // ========================================================================

    /// Persist a message, allocating its sequence number in the conversation.
    pub fn send_agent_message(
        &self,
        input: stateset_core::SendA2AAgentMessage,
    ) -> Result<stateset_core::A2AAgentMessage> {
        self.db.a2a_messages().send_message(input)
    }

    /// Fetch a message by id within a tenant.
    pub fn get_agent_message(
        &self,
        tenant_id: &str,
        id: Uuid,
    ) -> Result<Option<stateset_core::A2AAgentMessage>> {
        self.db.a2a_messages().get_message(tenant_id, id)
    }

    /// List messages within a tenant.
    pub fn list_agent_messages(
        &self,
        filter: stateset_core::A2AAgentMessageFilter,
    ) -> Result<Vec<stateset_core::A2AAgentMessage>> {
        self.db.a2a_messages().list_messages(filter)
    }

    /// Acknowledge a pending/delivered message.
    pub fn acknowledge_agent_message(
        &self,
        tenant_id: &str,
        id: Uuid,
    ) -> Result<stateset_core::A2AAgentMessage> {
        self.db.a2a_messages().acknowledge_message(tenant_id, id)
    }

    /// Record a delivery failure for a message.
    pub fn fail_agent_message(
        &self,
        tenant_id: &str,
        id: Uuid,
        error: &str,
    ) -> Result<stateset_core::A2AAgentMessage> {
        self.db.a2a_messages().fail_message(tenant_id, id, error)
    }

    // ========================================================================
    // Agent Card Operations
    // ========================================================================

    /// Register a new agent card
    ///
    /// Agent cards advertise an AI agent's commerce capabilities, including
    /// supported payment networks, assets, and A2A skills.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_core::{CreateAgentCard, X402Network, X402Asset, A2ASkill, TrustLevel};
    ///
    /// let card = commerce.x402().register_agent(CreateAgentCard {
    ///     name: "Widget Seller Bot".into(),
    ///     wallet_address: "0xSeller...".into(),
    ///     public_key: "base64_ed25519_pubkey".into(),
    ///     supported_networks: vec![X402Network::SetChain, X402Network::Base],
    ///     supported_assets: vec![X402Asset::Usdc, X402Asset::SsUsd],
    ///     a2a_skills: Some(vec![A2ASkill::Sell, A2ASkill::Quote, A2ASkill::Fulfill]),
    ///     endpoint_url: Some("https://api.example.com/a2a".into()),
    ///     ..Default::default()
    /// })?;
    /// ```
    pub fn register_agent(&self, input: CreateAgentCard) -> Result<AgentCard> {
        self.db.agent_cards().create(input)
    }

    /// Get an agent card by ID
    pub fn get_agent(&self, id: Uuid) -> Result<Option<AgentCard>> {
        self.db.agent_cards().get(id)
    }

    /// Get an agent card by wallet address
    pub fn get_agent_by_wallet(&self, wallet_address: &str) -> Result<Option<AgentCard>> {
        self.db.agent_cards().get_by_wallet(wallet_address)
    }

    /// Update an agent card
    pub fn update_agent(&self, id: Uuid, input: UpdateAgentCard) -> Result<AgentCard> {
        self.db.agent_cards().update(id, input)
    }

    /// Delete an agent card
    pub fn delete_agent(&self, id: Uuid) -> Result<()> {
        self.db.agent_cards().delete(id)
    }

    /// List agent cards with optional filtering
    pub fn list_agents(&self, filter: AgentCardFilter) -> Result<Vec<AgentCard>> {
        self.db.agent_cards().list(filter)
    }

    /// Count agent cards matching a filter
    pub fn count_agents(&self, filter: AgentCardFilter) -> Result<u64> {
        self.db.agent_cards().count(filter)
    }

    /// Verify an agent card (admin operation)
    ///
    /// Upgrades the agent's trust level to `Verified`.
    pub fn verify_agent(&self, id: Uuid) -> Result<AgentCard> {
        self.db.agent_cards().verify(id, TrustLevel::Verified, "system")
    }

    /// Suspend an agent card
    ///
    /// Temporarily disables the agent from participating in commerce.
    pub fn suspend_agent(&self, id: Uuid, reason: &str) -> Result<AgentCard> {
        self.db.agent_cards().suspend(id, reason)
    }

    /// Reactivate a suspended agent card
    pub fn reactivate_agent(&self, id: Uuid) -> Result<AgentCard> {
        self.db.agent_cards().reactivate(id)
    }

    /// Discover agents with specific capabilities
    ///
    /// Finds agents that support the specified network, asset, and skill.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_core::{X402Network, X402Asset, A2ASkill};
    ///
    /// // Find all agents that can sell on Set Chain with USDC
    /// let sellers = commerce.x402().discover_agents(
    ///     Some(X402Network::SetChain),
    ///     Some(X402Asset::Usdc),
    ///     Some(A2ASkill::Sell),
    ///     None,
    /// )?;
    /// ```
    pub fn discover_agents(
        &self,
        network: Option<X402Network>,
        asset: Option<X402Asset>,
        skill: Option<stateset_core::A2ASkill>,
        min_trust_level: Option<TrustLevel>,
    ) -> Result<Vec<AgentCard>> {
        self.db.agent_cards().discover(AgentCardFilter {
            network,
            asset,
            skill,
            trust_level: None,
            min_trust_level,
            active: Some(true),
            ..Default::default()
        })
    }

    /// Get all active agents
    pub fn active_agents(&self) -> Result<Vec<AgentCard>> {
        self.list_agents(AgentCardFilter { active: Some(true), ..Default::default() })
    }

    /// Get agents by trust level
    pub fn agents_by_trust_level(&self, level: TrustLevel) -> Result<Vec<AgentCard>> {
        self.list_agents(AgentCardFilter {
            trust_level: Some(level),
            active: Some(true),
            ..Default::default()
        })
    }

    /// Get verified agents only
    pub fn verified_agents(&self) -> Result<Vec<AgentCard>> {
        self.agents_by_trust_level(TrustLevel::Verified)
    }

    // ========================================================================
    // Convenience Methods
    // ========================================================================

    /// Create a payment intent for a cart
    ///
    /// Convenience method that creates an intent linked to a specific cart.
    /// The amount should be in the asset's decimal units (e.g., 100.00 for $100 USDC).
    pub fn create_cart_payment(
        &self,
        cart_id: Uuid,
        payer_address: &str,
        payee_address: &str,
        amount: rust_decimal::Decimal,
        network: X402Network,
        asset: X402Asset,
    ) -> Result<X402PaymentIntent> {
        use stateset_core::to_smallest_unit;
        self.create_intent(CreateX402PaymentIntent {
            payer_address: payer_address.to_string(),
            payee_address: payee_address.to_string(),
            amount: to_smallest_unit(amount, asset),
            asset,
            network,
            cart_id: Some(cart_id),
            ..Default::default()
        })
    }

    /// Get the active payment intent for a cart (if any)
    ///
    /// Returns the most recent non-failed, non-expired intent for the cart.
    pub fn active_intent_for_cart(&self, cart_id: Uuid) -> Result<Option<X402PaymentIntent>> {
        let intents = self.intents_for_cart(cart_id)?;
        Ok(intents.into_iter().find(|i| CLAIMING_STATUSES.contains(&i.status)))
    }

    /// Check if an intent is ready for settlement
    ///
    /// An intent is ready when it has been signed and has not expired.
    pub fn is_ready_for_settlement(&self, id: Uuid) -> Result<bool> {
        if let Some(intent) = self.get_intent(id)? {
            let now = chrono::Utc::now().timestamp() as u64;
            Ok(intent.status == X402IntentStatus::Signed && intent.valid_until > now)
        } else {
            Ok(false)
        }
    }

    /// Verify an intent's configured signature against its canonical signing hash.
    ///
    /// Returns `false` for missing, malformed, or invalid cryptographic fields.
    pub fn has_valid_signature(&self, id: Uuid) -> Result<bool> {
        if let Some(intent) = self.get_intent(id)? {
            if !intent.is_signed() {
                return Ok(false);
            }
            Ok(intent.verify_signature().unwrap_or(false))
        } else {
            Ok(false)
        }
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use stateset_core::{
        A2ASkill, CurrencyCode, ItemAvailability, QuotedItem, X402_DEFAULT_SIGNATURE_SCHEME,
        X402SignatureScheme,
    };
    use stateset_crypto::pqc::generate_hybrid_signing_keypair;

    fn setup_commerce() -> crate::Commerce {
        crate::Commerce::in_memory().unwrap()
    }

    /// Drive an intent through the real `sign` + `mark_sequenced` path.
    fn advance_to_sequenced(commerce: &crate::Commerce, id: Uuid) -> X402PaymentIntent {
        let mut locally_signed = commerce.x402().get_intent(id).unwrap().unwrap();
        sign_locally_with_hybrid(&mut locally_signed);
        commerce
            .x402()
            .sign_intent(
                id,
                SignX402PaymentIntent {
                    intent_id: id,
                    signature_scheme: None,
                    signature: locally_signed.payer_signature.clone().unwrap(),
                    public_key: locally_signed.payer_public_key.clone().unwrap(),
                    signature_bundle: locally_signed.payer_signature_bundle.clone(),
                    public_key_bundle: locally_signed.payer_public_key_bundle.clone(),
                },
            )
            .unwrap();
        commerce.x402().mark_sequenced(id, 1, Uuid::new_v4()).unwrap()
    }

    fn sign_locally_with_hybrid(intent: &mut X402PaymentIntent) {
        let keypair = generate_hybrid_signing_keypair().unwrap();
        intent.sign_with_hybrid(&keypair).unwrap();
    }

    #[test]
    fn test_create_payment_intent() {
        let commerce = setup_commerce();

        // 100 USDC = 100_000_000 (6 decimals)
        let intent = commerce
            .x402()
            .create_intent(CreateX402PaymentIntent {
                payer_address: "0xPayer123".into(),
                payee_address: "0xPayee456".into(),
                amount: 100_000_000,
                asset: X402Asset::Usdc,
                network: X402Network::SetChain,
                ..Default::default()
            })
            .unwrap();

        assert_eq!(intent.payer_address, "0xPayer123");
        assert_eq!(intent.payee_address, "0xPayee456");
        assert_eq!(intent.amount, 100_000_000);
        assert_eq!(intent.asset, X402Asset::Usdc);
        assert_eq!(intent.network, X402Network::SetChain);
        assert_eq!(intent.status, X402IntentStatus::Created);
        assert_eq!(intent.payer_signature_scheme, Some(X402_DEFAULT_SIGNATURE_SCHEME));
        assert!(intent.signing_hash.is_some());
    }

    #[test]
    fn test_sign_payment_intent() {
        let commerce = setup_commerce();

        // 50 USDC = 50_000_000 (6 decimals)
        let intent = commerce
            .x402()
            .create_intent(CreateX402PaymentIntent {
                payer_address: "0xPayer123".into(),
                payee_address: "0xPayee456".into(),
                amount: 50_000_000,
                ..Default::default()
            })
            .unwrap();

        let mut locally_signed = commerce.x402().get_intent(intent.id).unwrap().unwrap();
        sign_locally_with_hybrid(&mut locally_signed);
        let signature = locally_signed.payer_signature.clone().unwrap();
        let public_key = locally_signed.payer_public_key.clone().unwrap();
        let signature_bundle = locally_signed.payer_signature_bundle.clone();
        let public_key_bundle = locally_signed.payer_public_key_bundle.clone();

        let signed = commerce
            .x402()
            .sign_intent(
                intent.id,
                SignX402PaymentIntent {
                    intent_id: intent.id,
                    signature_scheme: None,
                    signature: signature.clone(),
                    public_key: public_key.clone(),
                    signature_bundle,
                    public_key_bundle,
                },
            )
            .unwrap();

        assert_eq!(signed.status, X402IntentStatus::Signed);
        assert_eq!(signed.payer_signature_scheme, Some(X402_DEFAULT_SIGNATURE_SCHEME));
        assert_eq!(signed.payer_signature, Some(signature));
        assert_eq!(signed.payer_public_key, Some(public_key));
        assert!(signed.payer_signature_bundle.is_some());
        assert!(signed.payer_public_key_bundle.is_some());
    }

    #[test]
    fn test_sign_payment_intent_rejects_ed25519_downgrade_for_new_intents() {
        let commerce = setup_commerce();

        let intent = commerce
            .x402()
            .create_intent(CreateX402PaymentIntent {
                payer_address: "0xHybridSigner".into(),
                payee_address: "0xPayee456".into(),
                amount: 50_000_000,
                ..Default::default()
            })
            .unwrap();

        let mut locally_signed = commerce.x402().get_intent(intent.id).unwrap().unwrap();
        locally_signed.sign_with_ed25519(&[21u8; 32]).unwrap();

        let err = commerce
            .x402()
            .sign_intent(
                intent.id,
                SignX402PaymentIntent {
                    intent_id: intent.id,
                    signature_scheme: Some(X402SignatureScheme::Ed25519),
                    signature: locally_signed.payer_signature.unwrap(),
                    public_key: locally_signed.payer_public_key.unwrap(),
                    signature_bundle: None,
                    public_key_bundle: None,
                },
            )
            .unwrap_err();

        assert!(err.to_string().contains("ed25519_ml_dsa65"));
    }

    #[test]
    fn test_has_valid_signature_true_for_hybrid_signature() {
        let commerce = setup_commerce();

        let intent = commerce
            .x402()
            .create_intent(CreateX402PaymentIntent {
                payer_address: "0xSigner".into(),
                payee_address: "0xPayee".into(),
                amount: 1_000_000,
                ..Default::default()
            })
            .unwrap();

        let mut to_sign = commerce.x402().get_intent(intent.id).unwrap().unwrap();
        sign_locally_with_hybrid(&mut to_sign);

        let signed = commerce
            .x402()
            .sign_intent(
                intent.id,
                SignX402PaymentIntent {
                    intent_id: intent.id,
                    signature_scheme: None,
                    signature: to_sign.payer_signature.clone().unwrap(),
                    public_key: to_sign.payer_public_key.clone().unwrap(),
                    signature_bundle: to_sign.payer_signature_bundle.clone(),
                    public_key_bundle: to_sign.payer_public_key_bundle.clone(),
                },
            )
            .unwrap();

        assert_eq!(signed.status, X402IntentStatus::Signed);
        assert!(commerce.x402().has_valid_signature(intent.id).unwrap());
    }

    #[test]
    fn test_sign_intent_rejects_malformed_signature() {
        let commerce = setup_commerce();

        let intent = commerce
            .x402()
            .create_intent(CreateX402PaymentIntent {
                payer_address: "0xSigner".into(),
                payee_address: "0xPayee".into(),
                amount: 1_000_000,
                ..Default::default()
            })
            .unwrap();

        let result = commerce.x402().sign_intent(
            intent.id,
            SignX402PaymentIntent {
                intent_id: intent.id,
                signature_scheme: None,
                signature: "not-hex-signature".into(),
                public_key: "not-hex-public-key".into(),
                signature_bundle: None,
                public_key_bundle: None,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_sign_intent_rejects_mismatched_intent_id() {
        let commerce = setup_commerce();

        let intent = commerce
            .x402()
            .create_intent(CreateX402PaymentIntent {
                payer_address: "0xSigner".into(),
                payee_address: "0xPayee".into(),
                amount: 1_000_000,
                ..Default::default()
            })
            .unwrap();

        let mut locally_signed = commerce.x402().get_intent(intent.id).unwrap().unwrap();
        sign_locally_with_hybrid(&mut locally_signed);

        let result = commerce.x402().sign_intent(
            intent.id,
            SignX402PaymentIntent {
                intent_id: Uuid::new_v4(),
                signature_scheme: None,
                signature: locally_signed.payer_signature.clone().unwrap(),
                public_key: locally_signed.payer_public_key.clone().unwrap(),
                signature_bundle: locally_signed.payer_signature_bundle.clone(),
                public_key_bundle: locally_signed.payer_public_key_bundle.clone(),
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_mark_settled() {
        let commerce = setup_commerce();

        // 25 USDC = 25_000_000 (6 decimals)
        let intent = commerce
            .x402()
            .create_intent(CreateX402PaymentIntent {
                payer_address: "0xPayer123".into(),
                payee_address: "0xPayee456".into(),
                amount: 25_000_000,
                ..Default::default()
            })
            .unwrap();

        // Sign first
        let mut locally_signed = commerce.x402().get_intent(intent.id).unwrap().unwrap();
        sign_locally_with_hybrid(&mut locally_signed);

        commerce
            .x402()
            .sign_intent(
                intent.id,
                SignX402PaymentIntent {
                    intent_id: intent.id,
                    signature_scheme: None,
                    signature: locally_signed.payer_signature.clone().unwrap(),
                    public_key: locally_signed.payer_public_key.clone().unwrap(),
                    signature_bundle: locally_signed.payer_signature_bundle.clone(),
                    public_key_bundle: locally_signed.payer_public_key_bundle.clone(),
                },
            )
            .unwrap();

        let sequenced = commerce.x402().mark_sequenced(intent.id, 7, Uuid::new_v4()).unwrap();
        assert_eq!(sequenced.status, X402IntentStatus::Sequenced);

        // Then settle
        let settled = commerce.x402().mark_settled(intent.id, "0xTxHash123", 12345).unwrap();

        assert_eq!(settled.status, X402IntentStatus::Settled);
        assert_eq!(settled.tx_hash, Some("0xTxHash123".to_string()));
        assert_eq!(settled.block_number, Some(12345));
    }

    #[test]
    fn test_mark_settled_rejects_unsequenced_intent() {
        let commerce = setup_commerce();

        let intent = commerce
            .x402()
            .create_intent(CreateX402PaymentIntent {
                payer_address: "0xPayer123".into(),
                payee_address: "0xPayee456".into(),
                amount: 25_000_000,
                ..Default::default()
            })
            .unwrap();

        let mut locally_signed = commerce.x402().get_intent(intent.id).unwrap().unwrap();
        sign_locally_with_hybrid(&mut locally_signed);

        commerce
            .x402()
            .sign_intent(
                intent.id,
                SignX402PaymentIntent {
                    intent_id: intent.id,
                    signature_scheme: None,
                    signature: locally_signed.payer_signature.clone().unwrap(),
                    public_key: locally_signed.payer_public_key.clone().unwrap(),
                    signature_bundle: locally_signed.payer_signature_bundle.clone(),
                    public_key_bundle: locally_signed.payer_public_key_bundle.clone(),
                },
            )
            .unwrap();

        let err = commerce.x402().mark_settled(intent.id, "0xTxHash123", 12345).unwrap_err();
        assert!(err.to_string().contains("Cannot settle intent in signed status"));
    }

    #[test]
    fn test_register_agent_card() {
        let commerce = setup_commerce();

        let card = commerce
            .x402()
            .register_agent(CreateAgentCard {
                name: "Test Agent".into(),
                wallet_address: "0xAgent123".into(),
                public_key: "test_pubkey".into(),
                supported_networks: Some(vec![X402Network::SetChain]),
                supported_assets: Some(vec![X402Asset::Usdc]),
                a2a_skills: Some(vec![A2ASkill::Sell, A2ASkill::Quote]),
                endpoint_url: Some("https://api.example.com".into()),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(card.name, "Test Agent");
        assert_eq!(card.wallet_address, "0xAgent123");
        assert!(card.active);
        assert_eq!(card.trust_level, TrustLevel::Standard);
    }

    #[test]
    fn test_discover_agents() {
        let commerce = setup_commerce();

        // Register a few agents
        commerce
            .x402()
            .register_agent(CreateAgentCard {
                name: "Seller 1".into(),
                wallet_address: "0xSeller1".into(),
                public_key: "pk1".into(),
                supported_networks: Some(vec![X402Network::SetChain]),
                supported_assets: Some(vec![X402Asset::Usdc]),
                a2a_skills: Some(vec![A2ASkill::Sell]),
                ..Default::default()
            })
            .unwrap();

        commerce
            .x402()
            .register_agent(CreateAgentCard {
                name: "Buyer 1".into(),
                wallet_address: "0xBuyer1".into(),
                public_key: "pk2".into(),
                supported_networks: Some(vec![X402Network::Base]),
                supported_assets: Some(vec![X402Asset::Usdc]),
                a2a_skills: Some(vec![A2ASkill::Buy]),
                ..Default::default()
            })
            .unwrap();

        // Discover agents on SetChain
        let set_chain_agents =
            commerce.x402().discover_agents(Some(X402Network::SetChain), None, None, None).unwrap();

        assert_eq!(set_chain_agents.len(), 1);
        assert_eq!(set_chain_agents[0].name, "Seller 1");
    }

    #[test]
    fn test_verify_agent() {
        let commerce = setup_commerce();

        let card = commerce
            .x402()
            .register_agent(CreateAgentCard {
                name: "Verified Agent".into(),
                wallet_address: "0xVerified".into(),
                public_key: "pk".into(),
                supported_networks: Some(vec![X402Network::SetChain]),
                supported_assets: Some(vec![X402Asset::Usdc]),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(card.trust_level, TrustLevel::Standard);

        let verified = commerce.x402().verify_agent(card.id).unwrap();
        assert_eq!(verified.trust_level, TrustLevel::Verified);
    }

    fn cart_with_total(commerce: &crate::Commerce, total: rust_decimal::Decimal) -> Uuid {
        use stateset_core::{AddCartItem, CreateCart};
        let cart = commerce
            .carts()
            .create(CreateCart {
                currency: Some(CurrencyCode::USD),
                items: Some(vec![AddCartItem {
                    sku: "SKU-X402".to_string(),
                    name: "x402 test item".to_string(),
                    quantity: 1,
                    unit_price: total,
                    ..Default::default()
                }]),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(cart.grand_total, total);
        cart.id.into()
    }

    #[test]
    fn test_create_and_track_cart_payment() {
        let commerce = setup_commerce();
        let cart_id = cart_with_total(&commerce, rust_decimal_macros::dec!(12.50));

        let intent = commerce
            .x402()
            .create_cart_payment(
                cart_id,
                "0xPayerCart",
                "0xPayeeCart",
                rust_decimal_macros::dec!(12.50),
                X402Network::SetChain,
                X402Asset::Usdc,
            )
            .unwrap();

        assert_eq!(intent.cart_id, Some(cart_id));
        assert_eq!(intent.status, X402IntentStatus::Created);

        let intents = commerce.x402().intents_for_cart(cart_id).unwrap();
        assert_eq!(intents.len(), 1);
        assert_eq!(intents[0].id, intent.id);

        let active = commerce.x402().active_intent_for_cart(cart_id).unwrap();
        assert!(active.is_some());
        assert_eq!(active.expect("active intent").id, intent.id);
    }

    #[test]
    fn test_cart_payment_amount_must_match_cart_grand_total() {
        let commerce = setup_commerce();
        let cart_id = cart_with_total(&commerce, rust_decimal_macros::dec!(12.50));

        let err = commerce
            .x402()
            .create_cart_payment(
                cart_id,
                "0xPayerCart",
                "0xPayeeCart",
                rust_decimal_macros::dec!(1.00),
                X402Network::SetChain,
                X402Asset::Usdc,
            )
            .expect_err("amount below the cart total must be refused");
        match err {
            stateset_core::CommerceError::ValidationError(message) => {
                let lower = message.to_lowercase();
                assert!(lower.contains("1.00") || lower.contains("1 usdc"), "{message}");
                assert!(message.contains("12.50") || message.contains("12.5"), "{message}");
                assert!(message.contains(&cart_id.to_string()), "{message}");
            }
            other => panic!("expected ValidationError, got {other:?}"),
        }
        assert!(commerce.x402().intents_for_cart(cart_id).unwrap().is_empty());

        // A volatile asset cannot be reconciled against a USD cart at all.
        let err = commerce
            .x402()
            .create_intent(CreateX402PaymentIntent {
                payer_address: "0xPayerCart".into(),
                payee_address: "0xPayeeCart".into(),
                amount: 1,
                asset: X402Asset::Eth,
                network: X402Network::SetChain,
                cart_id: Some(cart_id),
                ..Default::default()
            })
            .expect_err("ETH has no fiat peg");
        assert!(matches!(err, stateset_core::CommerceError::ValidationError(_)), "{err:?}");
    }

    #[test]
    fn test_cart_payment_refuses_non_usd_cart_with_clear_message() {
        use stateset_core::{AddCartItem, CreateCart};
        let commerce = setup_commerce();
        let cart = commerce
            .carts()
            .create(CreateCart {
                currency: Some(CurrencyCode::EUR),
                items: Some(vec![AddCartItem {
                    sku: "SKU-EUR".to_string(),
                    name: "euro item".to_string(),
                    quantity: 1,
                    unit_price: rust_decimal_macros::dec!(12.50),
                    ..Default::default()
                }]),
                ..Default::default()
            })
            .unwrap();
        let cart_id: Uuid = cart.id.into();

        let err = commerce
            .x402()
            .create_cart_payment(
                cart_id,
                "0xPayerCart",
                "0xPayeeCart",
                rust_decimal_macros::dec!(12.50),
                X402Network::SetChain,
                X402Asset::Usdc,
            )
            .expect_err("a USD stablecoin cannot pay a EUR cart");
        match err {
            stateset_core::CommerceError::ValidationError(message) => {
                assert!(message.contains("USD"), "{message}");
                assert!(message.contains("EUR"), "{message}");
                assert!(message.contains(&cart_id.to_string()), "{message}");
            }
            other => panic!("expected ValidationError, got {other:?}"),
        }
        assert!(commerce.x402().intents_for_cart(cart_id).unwrap().is_empty());
    }

    #[test]
    fn test_second_intent_for_same_cart_is_refused_while_first_is_open_or_settled() {
        let commerce = setup_commerce();
        let cart_id = cart_with_total(&commerce, rust_decimal_macros::dec!(12.50));
        let pay = || {
            commerce.x402().create_cart_payment(
                cart_id,
                "0xPayerCart",
                "0xPayeeCart",
                rust_decimal_macros::dec!(12.50),
                X402Network::SetChain,
                X402Asset::Usdc,
            )
        };
        let first = pay().expect("first intent");

        // Open (Created) intent blocks a second full-amount intent and names it.
        let err = pay().expect_err("double-pay must be refused");
        match &err {
            stateset_core::CommerceError::Conflict(message) => {
                assert!(message.contains(&first.id.to_string()), "{message}");
                assert!(message.contains("created"), "{message}");
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
        assert_eq!(commerce.x402().intents_for_cart(cart_id).unwrap().len(), 1);

        // Cancelling the first frees the cart for a new intent.
        commerce.x402().cancel_intent(first.id).expect("cancel");
        let second = pay().expect("cart is free again after cancel");
        assert_ne!(second.id, first.id);

        // A settled intent blocks forever (the cart has been paid).
        advance_to_sequenced(&commerce, second.id);
        commerce.x402().mark_settled(second.id, "0xtx-double-pay", 1).expect("settle");
        let err = pay().expect_err("paid cart must not accept another intent");
        match &err {
            stateset_core::CommerceError::Conflict(message) => {
                assert!(message.contains(&second.id.to_string()), "{message}");
                assert!(message.contains("already paid"), "{message}");
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn test_second_intent_for_same_order_is_refused() {
        use stateset_core::{CreateCustomer, CreateOrder, CreateOrderItem, ProductId};
        let commerce = setup_commerce();
        let customer = commerce
            .customers()
            .create(CreateCustomer {
                email: "x402-double@example.com".into(),
                first_name: "X".into(),
                last_name: "Payer".into(),
                ..Default::default()
            })
            .expect("customer");
        let order = commerce
            .orders()
            .create(CreateOrder {
                customer_id: customer.id,
                currency: Some(CurrencyCode::USD),
                items: vec![CreateOrderItem {
                    product_id: ProductId::new(),
                    sku: "SKU-ORD".to_string(),
                    name: "order item".to_string(),
                    quantity: 1,
                    unit_price: rust_decimal_macros::dec!(20.00),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        let make = || CreateX402PaymentIntent {
            payer_address: "0xPayerOrder".into(),
            payee_address: "0xPayeeOrder".into(),
            amount: 20_000_000,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            order_id: Some(order.id.into()),
            ..Default::default()
        };
        let first = commerce.x402().create_intent(make()).expect("first");
        let err = commerce.x402().create_intent(make()).expect_err("second refused");
        match err {
            stateset_core::CommerceError::Conflict(message) => {
                assert!(message.contains(&first.id.to_string()), "{message}");
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
        assert_eq!(commerce.x402().intents_for_order(order.id.into()).unwrap().len(), 1);
    }

    #[test]
    fn test_mark_batched_transitions_sequenced_intent_and_settles_from_batched() {
        let commerce = setup_commerce();
        let intent = commerce
            .x402()
            .create_intent(CreateX402PaymentIntent {
                payer_address: "0xPayerBatch".into(),
                payee_address: "0xPayeeBatch".into(),
                amount: 1_000_000,
                asset: X402Asset::Usdc,
                network: X402Network::SetChain,
                ..Default::default()
            })
            .unwrap();

        // Only Sequenced intents can be batched.
        let err = commerce
            .x402()
            .mark_batched(intent.id, "0xroot", vec!["0xa".into()])
            .expect_err("created intent cannot be batched");
        assert!(matches!(err, stateset_core::CommerceError::ValidationError(_)), "{err:?}");

        advance_to_sequenced(&commerce, intent.id);
        let err = commerce
            .x402()
            .mark_batched(intent.id, "  ", vec![])
            .expect_err("merkle root required");
        assert!(matches!(err, stateset_core::CommerceError::ValidationError(_)), "{err:?}");

        let batched = commerce
            .x402()
            .mark_batched(intent.id, "0xroot", vec!["0xa".into(), "0xb".into()])
            .expect("batch");
        assert_eq!(batched.status, X402IntentStatus::Batched);
        assert_eq!(batched.batch_merkle_root.as_deref(), Some("0xroot"));
        assert_eq!(batched.inclusion_proof, Some(vec!["0xa".to_string(), "0xb".to_string()]));

        // Batching twice is refused; settlement from Batched succeeds.
        let err =
            commerce.x402().mark_batched(intent.id, "0xroot", vec![]).expect_err("already batched");
        assert!(matches!(err, stateset_core::CommerceError::ValidationError(_)), "{err:?}");
        let settled = commerce.x402().mark_settled(intent.id, "0xtx-batched", 7).expect("settle");
        assert_eq!(settled.status, X402IntentStatus::Settled);
    }

    #[test]
    fn test_cart_payment_for_unknown_cart_is_refused() {
        let commerce = setup_commerce();
        let err = commerce
            .x402()
            .create_cart_payment(
                Uuid::new_v4(),
                "0xPayerCart",
                "0xPayeeCart",
                rust_decimal_macros::dec!(12.50),
                X402Network::SetChain,
                X402Asset::Usdc,
            )
            .expect_err("unknown cart must be refused");
        assert!(matches!(err, stateset_core::CommerceError::ValidationError(_)), "{err:?}");
    }

    #[test]
    fn test_order_payment_amount_must_match_order_total() {
        use stateset_core::{CreateCustomer, CreateOrder, CreateOrderItem, ProductId};
        let commerce = setup_commerce();
        let customer = commerce
            .customers()
            .create(CreateCustomer {
                email: "x402-order@example.com".into(),
                first_name: "X".into(),
                last_name: "Payer".into(),
                ..Default::default()
            })
            .expect("customer");
        let order = commerce
            .orders()
            .create(CreateOrder {
                customer_id: customer.id,
                currency: Some(CurrencyCode::USD),
                items: vec![CreateOrderItem {
                    product_id: ProductId::new(),
                    sku: "SKU-ORD".to_string(),
                    name: "order item".to_string(),
                    quantity: 2,
                    unit_price: rust_decimal_macros::dec!(10.00),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap();
        assert_eq!(order.total_amount, rust_decimal_macros::dec!(20.00));

        let make = |amount: u64| CreateX402PaymentIntent {
            payer_address: "0xPayerOrder".into(),
            payee_address: "0xPayeeOrder".into(),
            amount,
            asset: X402Asset::Usdc,
            network: X402Network::SetChain,
            order_id: Some(order.id.into()),
            ..Default::default()
        };

        let err = commerce.x402().create_intent(make(19_990_000)).expect_err("short by a cent");
        assert!(matches!(err, stateset_core::CommerceError::ValidationError(_)), "{err:?}");
        assert!(commerce.x402().intents_for_order(order.id.into()).unwrap().is_empty());

        let intent = commerce.x402().create_intent(make(20_000_000)).expect("exact amount");
        assert_eq!(intent.order_id, Some(order.id.into()));
        assert_eq!(intent.amount_decimal, rust_decimal_macros::dec!(20));
    }

    #[test]
    fn test_a2a_quote_and_purchase_flow() {
        let commerce = setup_commerce();

        let seller = commerce
            .x402()
            .register_agent(CreateAgentCard {
                name: "A2A Seller".into(),
                wallet_address: "0xSellerA2A".into(),
                public_key: "seller_pub".into(),
                supported_networks: Some(vec![X402Network::SetChain]),
                supported_assets: Some(vec![X402Asset::Usdc]),
                a2a_skills: Some(vec![A2ASkill::Sell]),
                trust_level: Some(TrustLevel::Verified),
                endpoint_url: Some("https://agent.example.com/".into()),
                ..Default::default()
            })
            .unwrap();

        let buyer_id = Uuid::new_v4();
        let quote = commerce
            .x402()
            .create_quote(CreateA2AQuote {
                buyer_agent_id: buyer_id,
                seller_agent_id: seller.id,
                items: vec![QuotedItem {
                    line_number: 1,
                    sku: Some("SKU-1".to_string()),
                    name: "Service Plan".to_string(),
                    quantity: 1,
                    unit_price: rust_decimal_macros::dec!(19.99),
                    total: rust_decimal_macros::dec!(19.99),
                    availability: ItemAvailability::InStock,
                    lead_time_days: Some(1),
                }],
                subtotal: rust_decimal_macros::dec!(19.99),
                total: rust_decimal_macros::dec!(19.99),
                currency: Some(CurrencyCode::USD),
                tax_amount: Some(rust_decimal::Decimal::ZERO),
                shipping_amount: Some(rust_decimal::Decimal::ZERO),
                discount_amount: Some(rust_decimal::Decimal::ZERO),
                valid_until: chrono::Utc::now() + chrono::Duration::hours(1),
                payment_network: Some(X402Network::SetChain),
                payment_asset: Some(X402Asset::Usdc),
                notes: Some("unit test quote".into()),
                ..Default::default()
            })
            .unwrap();

        assert_eq!(quote.status, QuoteStatus::Pending);

        let quoted = commerce.x402().update_quote_status(quote.id, QuoteStatus::Quoted).unwrap();
        assert_eq!(quoted.status, QuoteStatus::Quoted);

        let no_op_quote =
            commerce.x402().update_quote_status(quote.id, QuoteStatus::Quoted).unwrap();
        assert_eq!(no_op_quote.status, QuoteStatus::Quoted);

        let purchase = commerce
            .x402()
            .create_purchase(CreateA2APurchase {
                buyer_agent_id: buyer_id,
                seller_agent_id: seller.id,
                quote_id: Some(quoted.id),
                items: quoted.items,
                total: quoted.total,
                currency: Some(quoted.currency),
                fulfillment_type: Some("digital".to_string()),
                notes: Some("unit test purchase".into()),
                metadata: None,
                payment_intent_id: None,
            })
            .unwrap();

        assert_eq!(purchase.status, PurchaseStatus::Initiated);
        assert_eq!(purchase.quote_id, Some(quoted.id));

        let payment_pending = commerce
            .x402()
            .update_purchase_status(purchase.id, PurchaseStatus::PaymentPending)
            .unwrap();
        assert_eq!(payment_pending.status, PurchaseStatus::PaymentPending);

        let shipped =
            commerce.x402().update_purchase_status(purchase.id, PurchaseStatus::Shipped).unwrap();
        assert_eq!(shipped.status, PurchaseStatus::Shipped);

        let refreshed_quote = commerce.x402().get_quote(quote.id).unwrap();
        assert!(
            matches!(refreshed_quote.as_ref(), Some(value) if value.status == QuoteStatus::Purchased)
        );

        let listed_quotes = commerce
            .x402()
            .list_quotes(SkillQuoteFilter { buyer_agent_id: Some(buyer_id), ..Default::default() })
            .unwrap();
        assert!(listed_quotes.iter().any(|item| item.id == quoted.id));

        let quote_by_number = commerce
            .x402()
            .get_quote_by_number(&quoted.quote_number)
            .unwrap()
            .expect("quote by number");
        assert_eq!(quote_by_number.id, quoted.id);

        let counted_quotes = commerce
            .x402()
            .count_quotes(SkillQuoteFilter {
                seller_agent_id: Some(seller.id),
                ..Default::default()
            })
            .unwrap();
        assert!(counted_quotes >= 1);

        let completed = commerce
            .x402()
            .confirm_delivery(purchase.id, "delivery_signature", Some(5), Some("good"))
            .unwrap();
        assert_eq!(completed.status, PurchaseStatus::Completed);
        assert_eq!(
            completed.delivery_confirmation_signature,
            Some("delivery_signature".to_string())
        );

        let no_op_purchase =
            commerce.x402().update_purchase_status(purchase.id, PurchaseStatus::Completed).unwrap();
        assert_eq!(no_op_purchase.status, PurchaseStatus::Completed);

        let listed_purchases = commerce
            .x402()
            .list_purchases(A2APurchaseFilter {
                buyer_agent_id: Some(buyer_id),
                ..Default::default()
            })
            .unwrap();
        assert!(listed_purchases.iter().any(|item| item.id == purchase.id));

        let counted_purchases = commerce
            .x402()
            .count_purchases(A2APurchaseFilter {
                buyer_agent_id: Some(buyer_id),
                ..Default::default()
            })
            .unwrap();
        assert!(counted_purchases >= 1);
    }

    #[test]
    fn test_a2a_quote_and_purchase_state_guards() {
        let commerce = setup_commerce();

        let seller = commerce
            .x402()
            .register_agent(CreateAgentCard {
                name: "A2A Guard Seller".into(),
                wallet_address: "0xSellerA2AGuard".into(),
                public_key: "seller_guard_pub".into(),
                supported_networks: Some(vec![X402Network::SetChain]),
                supported_assets: Some(vec![X402Asset::Usdc]),
                a2a_skills: Some(vec![A2ASkill::Sell]),
                trust_level: Some(TrustLevel::Verified),
                endpoint_url: Some("https://agent.example.com/guard".into()),
                ..Default::default()
            })
            .unwrap();

        let other_seller = commerce
            .x402()
            .register_agent(CreateAgentCard {
                name: "A2A Wrong Seller".into(),
                wallet_address: "0xSellerWrong".into(),
                public_key: "wrong_seller_pub".into(),
                supported_networks: Some(vec![X402Network::SetChain]),
                supported_assets: Some(vec![X402Asset::Usdc]),
                a2a_skills: Some(vec![A2ASkill::Sell]),
                ..Default::default()
            })
            .unwrap();

        let buyer_id = Uuid::new_v4();
        let quote = commerce
            .x402()
            .create_quote(CreateA2AQuote {
                buyer_agent_id: buyer_id,
                seller_agent_id: seller.id,
                items: vec![QuotedItem {
                    line_number: 1,
                    sku: Some("SKU-2".to_string()),
                    name: "Guarded service".to_string(),
                    quantity: 1,
                    unit_price: rust_decimal_macros::dec!(30.00),
                    total: rust_decimal_macros::dec!(30.00),
                    availability: ItemAvailability::InStock,
                    lead_time_days: Some(2),
                }],
                subtotal: rust_decimal_macros::dec!(30.00),
                total: rust_decimal_macros::dec!(30.00),
                currency: Some(CurrencyCode::USD),
                tax_amount: Some(rust_decimal::Decimal::ZERO),
                shipping_amount: Some(rust_decimal::Decimal::ZERO),
                discount_amount: Some(rust_decimal::Decimal::ZERO),
                valid_until: chrono::Utc::now() + chrono::Duration::hours(1),
                payment_network: Some(X402Network::SetChain),
                payment_asset: Some(X402Asset::Usdc),
                notes: Some("guard quote".into()),
                ..Default::default()
            })
            .unwrap();

        assert!(commerce.x402().update_quote_status(quote.id, QuoteStatus::Accepted).is_err());

        let quoted = commerce.x402().update_quote_status(quote.id, QuoteStatus::Quoted).unwrap();

        assert!(
            commerce
                .x402()
                .create_purchase(CreateA2APurchase {
                    buyer_agent_id: buyer_id,
                    seller_agent_id: other_seller.id,
                    quote_id: Some(quoted.id),
                    items: quoted.items.clone(),
                    total: quoted.total,
                    currency: Some(CurrencyCode::USD),
                    fulfillment_type: Some("digital".to_string()),
                    notes: Some("mismatched seller".into()),
                    metadata: None,
                    payment_intent_id: None,
                })
                .is_err()
        );

        assert!(
            commerce
                .x402()
                .create_purchase(CreateA2APurchase {
                    buyer_agent_id: buyer_id,
                    seller_agent_id: seller.id,
                    quote_id: Some(quoted.id),
                    items: quoted.items.clone(),
                    total: quoted.total,
                    currency: Some(CurrencyCode::EUR),
                    fulfillment_type: Some("digital".to_string()),
                    notes: Some("mismatched currency".into()),
                    metadata: None,
                    payment_intent_id: None,
                })
                .is_err()
        );

        assert!(
            commerce
                .x402()
                .create_purchase(CreateA2APurchase {
                    buyer_agent_id: buyer_id,
                    seller_agent_id: seller.id,
                    quote_id: Some(quoted.id),
                    items: quoted.items.clone(),
                    total: quoted.total + rust_decimal::Decimal::ONE,
                    currency: Some(CurrencyCode::USD),
                    fulfillment_type: Some("digital".to_string()),
                    notes: Some("mismatched total".into()),
                    metadata: None,
                    payment_intent_id: None,
                })
                .is_err()
        );

        let purchase = commerce
            .x402()
            .create_purchase(CreateA2APurchase {
                buyer_agent_id: buyer_id,
                seller_agent_id: seller.id,
                quote_id: Some(quoted.id),
                items: quoted.items,
                total: quoted.total,
                currency: Some(quoted.currency),
                fulfillment_type: Some("digital".to_string()),
                notes: Some("valid purchase".into()),
                metadata: None,
                payment_intent_id: None,
            })
            .unwrap();

        assert_eq!(purchase.status, PurchaseStatus::Initiated);

        assert!(
            commerce.x402().update_purchase_status(purchase.id, PurchaseStatus::Completed).is_err()
        );

        assert!(
            commerce
                .x402()
                .confirm_delivery(purchase.id, "delivery_signature", Some(5), Some("blocked"))
                .is_err()
        );
    }

    #[test]
    fn test_a2a_purchase_rejected_for_expired_quote() {
        let commerce = setup_commerce();

        let seller = commerce
            .x402()
            .register_agent(CreateAgentCard {
                name: "Expired Quote Seller".into(),
                wallet_address: "0xSellerA2AExpired".into(),
                public_key: "expired_seller_pub".into(),
                supported_networks: Some(vec![X402Network::SetChain]),
                supported_assets: Some(vec![X402Asset::Usdc]),
                a2a_skills: Some(vec![A2ASkill::Sell]),
                trust_level: Some(TrustLevel::Verified),
                endpoint_url: Some("https://agent.example.com/".into()),
                ..Default::default()
            })
            .unwrap();

        let buyer_id = Uuid::new_v4();
        assert!(
            commerce
                .x402()
                .create_quote(CreateA2AQuote {
                    buyer_agent_id: buyer_id,
                    seller_agent_id: seller.id,
                    items: vec![QuotedItem {
                        line_number: 1,
                        sku: Some("SKU-EX".to_string()),
                        name: "Expired service".to_string(),
                        quantity: 1,
                        unit_price: rust_decimal_macros::dec!(15.00),
                        total: rust_decimal_macros::dec!(15.00),
                        availability: ItemAvailability::InStock,
                        lead_time_days: Some(1),
                    }],
                    subtotal: rust_decimal_macros::dec!(15.00),
                    total: rust_decimal_macros::dec!(15.00),
                    currency: Some(CurrencyCode::USD),
                    tax_amount: Some(rust_decimal::Decimal::ZERO),
                    shipping_amount: Some(rust_decimal::Decimal::ZERO),
                    discount_amount: Some(rust_decimal::Decimal::ZERO),
                    valid_until: chrono::Utc::now() - chrono::Duration::hours(1),
                    payment_network: Some(X402Network::SetChain),
                    payment_asset: Some(X402Asset::Usdc),
                    notes: Some("expired quote".into()),
                    ..Default::default()
                })
                .is_err()
        );

        let quote = commerce
            .x402()
            .create_quote(CreateA2AQuote {
                buyer_agent_id: buyer_id,
                seller_agent_id: seller.id,
                items: vec![QuotedItem {
                    line_number: 1,
                    sku: Some("SKU-EX".to_string()),
                    name: "Expired service".to_string(),
                    quantity: 1,
                    unit_price: rust_decimal_macros::dec!(15.00),
                    total: rust_decimal_macros::dec!(15.00),
                    availability: ItemAvailability::InStock,
                    lead_time_days: Some(1),
                }],
                subtotal: rust_decimal_macros::dec!(15.00),
                total: rust_decimal_macros::dec!(15.00),
                currency: Some(CurrencyCode::USD),
                tax_amount: Some(rust_decimal::Decimal::ZERO),
                shipping_amount: Some(rust_decimal::Decimal::ZERO),
                discount_amount: Some(rust_decimal::Decimal::ZERO),
                valid_until: chrono::Utc::now() + chrono::Duration::hours(1),
                payment_network: Some(X402Network::SetChain),
                payment_asset: Some(X402Asset::Usdc),
                notes: Some("expired quote".into()),
                ..Default::default()
            })
            .unwrap();

        let quoted = commerce.x402().update_quote_status(quote.id, QuoteStatus::Quoted).unwrap();
        let expired = commerce.x402().update_quote_status(quoted.id, QuoteStatus::Expired).unwrap();
        assert_eq!(quoted.status, QuoteStatus::Quoted);
        assert_eq!(expired.status, QuoteStatus::Expired);

        assert!(
            commerce
                .x402()
                .create_purchase(CreateA2APurchase {
                    buyer_agent_id: buyer_id,
                    seller_agent_id: seller.id,
                    quote_id: Some(expired.id),
                    items: expired.items,
                    total: expired.total,
                    currency: Some(expired.currency),
                    fulfillment_type: Some("digital".to_string()),
                    notes: Some("expired quote blocked".into()),
                    metadata: None,
                    payment_intent_id: None,
                })
                .is_err()
        );
    }

    #[test]
    fn test_a2a_purchase_state_lifecycle_controls() {
        let commerce = setup_commerce();

        let seller = commerce
            .x402()
            .register_agent(CreateAgentCard {
                name: "A2A Lifecycle Seller".into(),
                wallet_address: "0xSellerLifecycle".into(),
                public_key: "lifecycle_pub".into(),
                supported_networks: Some(vec![X402Network::SetChain]),
                supported_assets: Some(vec![X402Asset::Usdc]),
                a2a_skills: Some(vec![A2ASkill::Sell]),
                trust_level: Some(TrustLevel::Verified),
                endpoint_url: Some("https://agent.example.com/lifecycle".into()),
                ..Default::default()
            })
            .unwrap();

        let buyer_id = Uuid::new_v4();
        let now = chrono::Utc::now();

        let make_quote = |buyer_id: Uuid, seller_id: Uuid| CreateA2AQuote {
            buyer_agent_id: buyer_id,
            seller_agent_id: seller_id,
            items: vec![QuotedItem {
                line_number: 1,
                sku: Some("SKU-LC-1".to_string()),
                name: "Lifecycle service".to_string(),
                quantity: 1,
                unit_price: rust_decimal_macros::dec!(12.00),
                total: rust_decimal_macros::dec!(12.00),
                availability: ItemAvailability::InStock,
                lead_time_days: Some(1),
            }],
            subtotal: rust_decimal_macros::dec!(12.00),
            tax_amount: Some(rust_decimal::Decimal::ZERO),
            shipping_amount: Some(rust_decimal::Decimal::ZERO),
            discount_amount: Some(rust_decimal::Decimal::ZERO),
            total: rust_decimal_macros::dec!(12.00),
            currency: Some(CurrencyCode::USD),
            payment_network: Some(X402Network::SetChain),
            payment_asset: Some(X402Asset::Usdc),
            shipping_address: None,
            valid_until: now + chrono::Duration::hours(1),
            notes: Some("lifecycle quote".to_string()),
            metadata: None,
        };

        let cancelled_quote =
            commerce.x402().create_quote(make_quote(buyer_id, seller.id)).unwrap();
        let cancelled_quote =
            commerce.x402().update_quote_status(cancelled_quote.id, QuoteStatus::Quoted).unwrap();

        let cancelled_purchase = commerce
            .x402()
            .create_purchase(CreateA2APurchase {
                buyer_agent_id: buyer_id,
                seller_agent_id: seller.id,
                quote_id: Some(cancelled_quote.id),
                items: cancelled_quote.items.clone(),
                total: cancelled_quote.total,
                currency: Some(cancelled_quote.currency),
                fulfillment_type: Some("digital".to_string()),
                notes: Some("cancel path".into()),
                metadata: None,
                payment_intent_id: None,
            })
            .unwrap();

        let cancelled = commerce
            .x402()
            .update_purchase_status(cancelled_purchase.id, PurchaseStatus::Cancelled)
            .unwrap();
        assert_eq!(cancelled.status, PurchaseStatus::Cancelled);

        let no_op = commerce
            .x402()
            .update_purchase_status(cancelled_purchase.id, PurchaseStatus::Cancelled)
            .unwrap();
        assert_eq!(no_op.status, PurchaseStatus::Cancelled);

        assert!(
            commerce
                .x402()
                .update_purchase_status(cancelled_purchase.id, PurchaseStatus::PaymentPending)
                .is_err()
        );
        assert!(
            commerce
                .x402()
                .confirm_delivery(cancelled_purchase.id, "sig", Some(4), Some("should fail"))
                .is_err()
        );

        let disputed_quote =
            commerce.x402().create_quote(make_quote(Uuid::new_v4(), seller.id)).unwrap();
        let disputed_quote =
            commerce.x402().update_quote_status(disputed_quote.id, QuoteStatus::Quoted).unwrap();

        let disputed_purchase = commerce
            .x402()
            .create_purchase(CreateA2APurchase {
                buyer_agent_id: disputed_quote.buyer_agent_id,
                seller_agent_id: seller.id,
                quote_id: Some(disputed_quote.id),
                items: disputed_quote.items.clone(),
                total: disputed_quote.total,
                currency: Some(disputed_quote.currency),
                fulfillment_type: Some("digital".to_string()),
                notes: Some("dispute path".into()),
                metadata: None,
                payment_intent_id: None,
            })
            .unwrap();

        let disputed = commerce
            .x402()
            .update_purchase_status(disputed_purchase.id, PurchaseStatus::Disputed)
            .unwrap();
        assert_eq!(disputed.status, PurchaseStatus::Disputed);

        assert!(
            commerce
                .x402()
                .update_purchase_status(disputed_purchase.id, PurchaseStatus::Shipped)
                .is_err()
        );
        assert!(
            commerce
                .x402()
                .confirm_delivery(disputed_purchase.id, "sig", Some(4), Some("blocked"))
                .is_err()
        );
        assert!(
            commerce
                .x402()
                .update_purchase_status(disputed_purchase.id, PurchaseStatus::Disputed)
                .is_ok()
        );
    }
}
