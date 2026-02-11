//! x402 Payment Protocol operations for AI agent commerce
//!
//! This module provides high-level operations for x402 stablecoin payments,
//! enabling AI agents to transact with USDC, ssUSD, and other stablecoins
//! on Set Chain L2 and other supported networks.
//!
//! # Overview
//!
//! The x402 protocol enables instant, low-cost stablecoin payments for AI agents:
//! - Ed25519 signature-based payment intents
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
//! // Sign the intent (agent signs with their Ed25519 key)
//! let signed = commerce.x402().sign_intent(intent.id, SignX402PaymentIntent {
//!     signature: "base64_signature".into(),
//!     public_key: "base64_public_key".into(),
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
    AgentCard, AgentCardFilter, CreateAgentCard, CreateX402PaymentIntent, Result,
    SignX402PaymentIntent, TrustLevel, UpdateAgentCard, X402Asset, X402CreditAccount,
    X402CreditAdjustment, X402CreditDirection, X402CreditTransaction, X402CreditTransactionFilter,
    X402IntentStatus, X402Network, X402PaymentIntent, X402PaymentIntentFilter,
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
        self.db.x402_payment_intents().create(input)
    }

    /// Get a payment intent by ID
    pub fn get_intent(&self, id: Uuid) -> Result<Option<X402PaymentIntent>> {
        self.db.x402_payment_intents().get(id)
    }

    /// Sign a payment intent with an Ed25519 signature
    ///
    /// The payer agent signs the intent's signing hash with their private key.
    /// This transitions the intent from `Created` to `Signed` status.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let signed = commerce.x402().sign_intent(intent.id, SignX402PaymentIntent {
    ///     signature: base64::encode(&signature_bytes),
    ///     public_key: base64::encode(&public_key_bytes),
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
        self.db
            .x402_payment_intents()
            .mark_sequenced(id, sequence_number, batch_id)
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
        self.db
            .x402_payment_intents()
            .mark_settled(id, tx_hash, block_number)
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
        self.list_intents(X402PaymentIntentFilter {
            status: Some(status),
            ..Default::default()
        })
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
    // Credit Ledger Operations (Metered Billing)
    // ========================================================================

    /// Get a credit account for a payer/asset/network
    pub fn get_credit_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<Option<X402CreditAccount>> {
        self.db
            .x402_credits()
            .get_account(payer_address, asset, network)
    }

    /// Get or create a credit account (balance default = 0)
    pub fn get_or_create_credit_account(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<X402CreditAccount> {
        self.db
            .x402_credits()
            .get_or_create_account(payer_address, asset, network)
    }

    /// Get current credit balance for a payer/asset/network
    pub fn get_credit_balance(
        &self,
        payer_address: &str,
        asset: X402Asset,
        network: X402Network,
    ) -> Result<u64> {
        self.db
            .x402_credits()
            .get_balance(payer_address, asset, network)
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
        self.db
            .agent_cards()
            .verify(id, TrustLevel::Verified, "system")
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
        self.list_agents(AgentCardFilter {
            active: Some(true),
            ..Default::default()
        })
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
        Ok(intents.into_iter().find(|i| {
            matches!(
                i.status,
                X402IntentStatus::Created
                    | X402IntentStatus::Signed
                    | X402IntentStatus::Sequenced
                    | X402IntentStatus::Settled
            )
        }))
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

    /// Verify an intent's Ed25519 signature against its canonical signing hash.
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

#[cfg(test)]
mod tests {
    use super::*;
    use stateset_core::A2ASkill;

    fn setup_commerce() -> crate::Commerce {
        crate::Commerce::in_memory().unwrap()
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
        locally_signed.sign_with_ed25519(&[3u8; 32]).unwrap();
        let signature = locally_signed.payer_signature.unwrap();
        let public_key = locally_signed.payer_public_key.unwrap();

        let signed = commerce
            .x402()
            .sign_intent(
                intent.id,
                SignX402PaymentIntent {
                    intent_id: intent.id,
                    signature: signature.clone(),
                    public_key: public_key.clone(),
                },
            )
            .unwrap();

        assert_eq!(signed.status, X402IntentStatus::Signed);
        assert_eq!(signed.payer_signature, Some(signature));
        assert_eq!(signed.payer_public_key, Some(public_key));
    }

    #[test]
    fn test_has_valid_signature_true_for_ed25519_signature() {
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
        to_sign.sign_with_ed25519(&[7u8; 32]).unwrap();

        let signed = commerce
            .x402()
            .sign_intent(
                intent.id,
                SignX402PaymentIntent {
                    intent_id: intent.id,
                    signature: to_sign.payer_signature.unwrap(),
                    public_key: to_sign.payer_public_key.unwrap(),
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
                signature: "not-hex-signature".into(),
                public_key: "not-hex-public-key".into(),
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
        locally_signed.sign_with_ed25519(&[15u8; 32]).unwrap();

        let result = commerce.x402().sign_intent(
            intent.id,
            SignX402PaymentIntent {
                intent_id: Uuid::new_v4(),
                signature: locally_signed.payer_signature.unwrap(),
                public_key: locally_signed.payer_public_key.unwrap(),
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
        locally_signed.sign_with_ed25519(&[9u8; 32]).unwrap();

        commerce
            .x402()
            .sign_intent(
                intent.id,
                SignX402PaymentIntent {
                    intent_id: intent.id,
                    signature: locally_signed.payer_signature.unwrap(),
                    public_key: locally_signed.payer_public_key.unwrap(),
                },
            )
            .unwrap();

        // Then settle
        let settled = commerce
            .x402()
            .mark_settled(intent.id, "0xTxHash123", 12345)
            .unwrap();

        assert_eq!(settled.status, X402IntentStatus::Settled);
        assert_eq!(settled.tx_hash, Some("0xTxHash123".to_string()));
        assert_eq!(settled.block_number, Some(12345));
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
        let set_chain_agents = commerce
            .x402()
            .discover_agents(Some(X402Network::SetChain), None, None, None)
            .unwrap();

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
}
