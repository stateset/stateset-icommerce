//! Agent Card domain models for AI agent identity and capabilities
//!
//! Agent cards are used to advertise an AI agent's commerce capabilities,
//! payment methods, and trust level for agent-to-agent (A2A) commerce.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::x402::{X402Asset, X402Network};

// =============================================================================
// Agent Card
// =============================================================================

/// Agent Card - Identity and capability advertisement for AI agents
///
/// Agent cards enable discovery and verification of AI agents in the
/// agent-to-agent commerce ecosystem. They contain:
/// - Identity: wallet address, public key for verification
/// - Capabilities: supported payment networks, assets, A2A skills
/// - Trust: verification level and limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// Unique agent card ID
    pub id: Uuid,

    /// Human-readable agent name
    pub name: String,

    /// Description of the agent's purpose/capabilities
    pub description: Option<String>,

    // =========================================================================
    // Identity & Authentication
    // =========================================================================
    /// Wallet address for receiving/sending payments
    pub wallet_address: String,

    /// Ed25519 public key for signature verification (hex-encoded)
    pub public_key: String,

    // =========================================================================
    // Payment Capabilities
    // =========================================================================
    /// Supported blockchain networks
    pub supported_networks: Vec<X402Network>,

    /// Supported payment assets (stablecoins)
    pub supported_assets: Vec<X402Asset>,

    // =========================================================================
    // A2A Commerce Capabilities
    // =========================================================================
    /// List of A2A skills this agent supports
    pub a2a_skills: Vec<A2ASkill>,

    // =========================================================================
    // Trust & Verification
    // =========================================================================
    /// Trust level determines transaction limits and capabilities
    pub trust_level: TrustLevel,

    /// When the agent was verified (if applicable)
    pub verified_at: Option<DateTime<Utc>>,

    /// Method used for verification
    pub verification_method: Option<String>,

    // =========================================================================
    // Endpoint
    // =========================================================================
    /// URL for A2A communication
    pub endpoint_url: Option<String>,

    /// Protocol for endpoint (https, grpc, websocket)
    pub endpoint_protocol: Option<String>,

    // =========================================================================
    // Merchant/Business Info
    // =========================================================================
    /// Associated merchant ID
    pub merchant_id: Option<String>,

    /// Merchant/business name
    pub merchant_name: Option<String>,

    /// Business category
    pub business_category: Option<String>,

    // =========================================================================
    // Limits & Policies
    // =========================================================================
    /// Maximum single transaction amount (in smallest unit)
    pub max_transaction_amount: Option<u64>,

    /// Daily volume limit (in smallest unit)
    pub daily_volume_limit: Option<u64>,

    /// Whether KYC is required for transactions
    pub requires_kyc: bool,

    // =========================================================================
    // Status
    // =========================================================================
    /// Whether the agent card is active
    pub active: bool,

    /// When the agent was suspended (if applicable)
    pub suspended_at: Option<DateTime<Utc>>,

    /// Reason for suspension
    pub suspension_reason: Option<String>,

    // =========================================================================
    // Metadata
    // =========================================================================
    /// Additional metadata (JSON)
    pub metadata: Option<String>,

    /// When the agent card was created
    pub created_at: DateTime<Utc>,

    /// When the agent card was last updated
    pub updated_at: DateTime<Utc>,
}

impl AgentCard {
    /// Create a new agent card
    pub fn new(
        name: impl Into<String>,
        wallet_address: impl Into<String>,
        public_key: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            wallet_address: wallet_address.into(),
            public_key: public_key.into(),
            supported_networks: vec![X402Network::SetChain],
            supported_assets: vec![X402Asset::Usdc],
            a2a_skills: Vec::new(),
            trust_level: TrustLevel::Standard,
            verified_at: None,
            verification_method: None,
            endpoint_url: None,
            endpoint_protocol: None,
            merchant_id: None,
            merchant_name: None,
            business_category: None,
            max_transaction_amount: None,
            daily_volume_limit: None,
            requires_kyc: false,
            active: true,
            suspended_at: None,
            suspension_reason: None,
            metadata: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add supported networks
    pub fn with_networks(mut self, networks: Vec<X402Network>) -> Self {
        self.supported_networks = networks;
        self
    }

    /// Add supported assets
    pub fn with_assets(mut self, assets: Vec<X402Asset>) -> Self {
        self.supported_assets = assets;
        self
    }

    /// Add A2A skills
    pub fn with_skills(mut self, skills: Vec<A2ASkill>) -> Self {
        self.a2a_skills = skills;
        self
    }

    /// Set trust level
    pub const fn with_trust_level(mut self, level: TrustLevel) -> Self {
        self.trust_level = level;
        self
    }

    /// Set endpoint
    pub fn with_endpoint(mut self, url: impl Into<String>, protocol: impl Into<String>) -> Self {
        self.endpoint_url = Some(url.into());
        self.endpoint_protocol = Some(protocol.into());
        self
    }

    /// Check if agent supports a specific network
    pub fn supports_network(&self, network: X402Network) -> bool {
        self.supported_networks.contains(&network)
    }

    /// Check if agent supports a specific asset
    pub fn supports_asset(&self, asset: X402Asset) -> bool {
        self.supported_assets.contains(&asset)
    }

    /// Check if agent has a specific skill
    pub fn has_skill(&self, skill: &A2ASkill) -> bool {
        self.a2a_skills.contains(skill)
    }

    /// Check if agent can sell
    pub fn can_sell(&self) -> bool {
        self.a2a_skills.iter().any(|s| matches!(s, A2ASkill::Sell | A2ASkill::Quote))
    }

    /// Check if agent can buy
    pub fn can_buy(&self) -> bool {
        self.a2a_skills.iter().any(|s| matches!(s, A2ASkill::Buy | A2ASkill::RequestQuote))
    }
}

// =============================================================================
// Trust Level
// =============================================================================

/// Trust level for agent cards
///
/// Higher trust levels enable higher transaction limits and more capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TrustLevel {
    /// Sandbox - for testing only, no real transactions
    Sandbox,
    /// Standard - default level for new agents
    #[default]
    Standard,
    /// Verified - identity verified, higher limits
    Verified,
    /// Enterprise - business verified, highest limits
    Enterprise,
}

impl TrustLevel {
    /// Numeric rank for trust comparison (higher is more trusted).
    pub const fn rank(&self) -> u8 {
        match self {
            Self::Sandbox => 0,
            Self::Standard => 1,
            Self::Verified => 2,
            Self::Enterprise => 3,
        }
    }

    /// Get the default transaction limit for this trust level (in USDC cents)
    pub const fn default_transaction_limit(&self) -> u64 {
        match self {
            Self::Sandbox => 100_000_000,        // $100 (for testing)
            Self::Standard => 1_000_000_000,     // $1,000
            Self::Verified => 10_000_000_000,    // $10,000
            Self::Enterprise => 100_000_000_000, // $100,000
        }
    }

    /// Get the default daily volume limit for this trust level
    pub const fn default_daily_limit(&self) -> u64 {
        match self {
            Self::Sandbox => 1_000_000_000,        // $1,000/day
            Self::Standard => 10_000_000_000,      // $10,000/day
            Self::Verified => 100_000_000_000,     // $100,000/day
            Self::Enterprise => 1_000_000_000_000, // $1,000,000/day
        }
    }
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sandbox => write!(f, "sandbox"),
            Self::Standard => write!(f, "standard"),
            Self::Verified => write!(f, "verified"),
            Self::Enterprise => write!(f, "enterprise"),
        }
    }
}

impl std::str::FromStr for TrustLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "sandbox" | "test" => Ok(Self::Sandbox),
            "standard" | "default" => Ok(Self::Standard),
            "verified" => Ok(Self::Verified),
            "enterprise" | "business" => Ok(Self::Enterprise),
            _ => Err(format!("Unknown trust level: {}", s)),
        }
    }
}

// =============================================================================
// A2A Skills
// =============================================================================

/// A2A commerce skills that an agent can advertise
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum A2ASkill {
    /// Can sell products/services
    Sell,
    /// Can buy products/services
    Buy,
    /// Can provide price quotes
    Quote,
    /// Can request price quotes
    RequestQuote,
    /// Can fulfill orders
    Fulfill,
    /// Can ship physical goods
    Ship,
    /// Can provide digital delivery
    DigitalDeliver,
    /// Can process returns
    ProcessReturn,
    /// Can issue refunds
    Refund,
    /// Can provide customer support
    Support,
}

impl std::fmt::Display for A2ASkill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sell => write!(f, "commerce.sell"),
            Self::Buy => write!(f, "commerce.buy"),
            Self::Quote => write!(f, "commerce.quote"),
            Self::RequestQuote => write!(f, "commerce.request_quote"),
            Self::Fulfill => write!(f, "commerce.fulfill"),
            Self::Ship => write!(f, "commerce.ship"),
            Self::DigitalDeliver => write!(f, "commerce.digital_deliver"),
            Self::ProcessReturn => write!(f, "commerce.process_return"),
            Self::Refund => write!(f, "commerce.refund"),
            Self::Support => write!(f, "commerce.support"),
        }
    }
}

impl std::str::FromStr for A2ASkill {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().replace('.', "_").as_str() {
            "commerce_sell" | "sell" => Ok(Self::Sell),
            "commerce_buy" | "buy" => Ok(Self::Buy),
            "commerce_quote" | "quote" => Ok(Self::Quote),
            "commerce_request_quote" | "request_quote" => Ok(Self::RequestQuote),
            "commerce_fulfill" | "fulfill" => Ok(Self::Fulfill),
            "commerce_ship" | "ship" => Ok(Self::Ship),
            "commerce_digital_deliver" | "digital_deliver" => Ok(Self::DigitalDeliver),
            "commerce_process_return" | "process_return" => Ok(Self::ProcessReturn),
            "commerce_refund" | "refund" => Ok(Self::Refund),
            "commerce_support" | "support" => Ok(Self::Support),
            _ => Err(format!("Unknown A2A skill: {}", s)),
        }
    }
}

// =============================================================================
// Input/Filter Types
// =============================================================================

/// Input for creating an agent card
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateAgentCard {
    pub name: String,
    pub description: Option<String>,
    pub wallet_address: String,
    pub public_key: String,
    pub supported_networks: Option<Vec<X402Network>>,
    pub supported_assets: Option<Vec<X402Asset>>,
    pub a2a_skills: Option<Vec<A2ASkill>>,
    pub trust_level: Option<TrustLevel>,
    pub endpoint_url: Option<String>,
    pub endpoint_protocol: Option<String>,
    pub merchant_id: Option<String>,
    pub merchant_name: Option<String>,
    pub business_category: Option<String>,
    pub max_transaction_amount: Option<u64>,
    pub daily_volume_limit: Option<u64>,
    pub requires_kyc: Option<bool>,
    pub metadata: Option<String>,
}

/// Input for updating an agent card
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateAgentCard {
    pub name: Option<String>,
    pub description: Option<String>,
    pub supported_networks: Option<Vec<X402Network>>,
    pub supported_assets: Option<Vec<X402Asset>>,
    pub a2a_skills: Option<Vec<A2ASkill>>,
    pub trust_level: Option<TrustLevel>,
    pub endpoint_url: Option<String>,
    pub endpoint_protocol: Option<String>,
    pub merchant_id: Option<String>,
    pub merchant_name: Option<String>,
    pub business_category: Option<String>,
    pub max_transaction_amount: Option<u64>,
    pub daily_volume_limit: Option<u64>,
    pub requires_kyc: Option<bool>,
    pub active: Option<bool>,
    pub metadata: Option<String>,
}

/// Filter for listing agent cards
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentCardFilter {
    /// Filter by wallet address
    pub wallet_address: Option<String>,
    /// Filter by trust level
    pub trust_level: Option<TrustLevel>,
    /// Filter by minimum trust level (inclusive)
    pub min_trust_level: Option<TrustLevel>,
    /// Filter by supported network
    pub network: Option<X402Network>,
    /// Filter by supported asset
    pub asset: Option<X402Asset>,
    /// Filter by skill
    pub skill: Option<A2ASkill>,
    /// Filter by active status
    pub active: Option<bool>,
    /// Filter by merchant ID
    pub merchant_id: Option<String>,
    /// Pagination
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_card_creation() {
        let card = AgentCard::new("TestAgent", "0x1234567890abcdef", "0xpubkey1234")
            .with_networks(vec![X402Network::SetChain, X402Network::Base])
            .with_assets(vec![X402Asset::Usdc, X402Asset::SsUsd])
            .with_skills(vec![A2ASkill::Sell, A2ASkill::Quote]);

        assert_eq!(card.name, "TestAgent");
        assert!(card.supports_network(X402Network::SetChain));
        assert!(card.supports_asset(X402Asset::Usdc));
        assert!(card.can_sell());
    }

    #[test]
    fn test_trust_level_limits() {
        assert!(
            TrustLevel::Enterprise.default_transaction_limit()
                > TrustLevel::Standard.default_transaction_limit()
        );
        assert!(
            TrustLevel::Verified.default_daily_limit() > TrustLevel::Standard.default_daily_limit()
        );
    }

    #[test]
    fn test_a2a_skill_parsing() {
        assert_eq!("commerce.sell".parse::<A2ASkill>().unwrap(), A2ASkill::Sell);
        assert_eq!("buy".parse::<A2ASkill>().unwrap(), A2ASkill::Buy);
    }
}
