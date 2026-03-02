//! Agent card types, validation, and discovery filtering.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{A2AError, A2AResult};
use crate::reputation::TrustTier;

/// Standard agent skills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AgentSkill {
    /// Can buy goods/services.
    Buy,
    /// Can sell goods/services.
    Sell,
    /// Can provide price quotes.
    Quote,
    /// Can negotiate terms.
    Negotiate,
    /// Can fulfill orders.
    Fulfill,
    /// Can provide analytics.
    Analytics,
}

impl std::fmt::Display for AgentSkill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Buy => write!(f, "buy"),
            Self::Sell => write!(f, "sell"),
            Self::Quote => write!(f, "quote"),
            Self::Negotiate => write!(f, "negotiate"),
            Self::Fulfill => write!(f, "fulfill"),
            Self::Analytics => write!(f, "analytics"),
        }
    }
}

/// An agent's identity card in the A2A registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// Unique agent ID.
    pub id: Uuid,
    /// Display name.
    pub name: String,
    /// Wallet address (0x... format).
    pub wallet_address: String,
    /// Public key for x402 signing (hex, optional).
    pub public_key: Option<String>,
    /// Supported blockchain networks.
    pub supported_networks: Vec<String>,
    /// Supported payment assets.
    pub supported_assets: Vec<String>,
    /// Agent capabilities.
    pub skills: Vec<AgentSkill>,
    /// Webhook endpoint URL (optional).
    pub endpoint_url: Option<String>,
    /// Agent description.
    pub description: String,
    /// Trust tier.
    pub trust_tier: TrustTier,
    /// Whether the agent is active.
    pub active: bool,
    /// Suspension timestamp (if suspended).
    pub suspended_at: Option<DateTime<Utc>>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Updated timestamp.
    pub updated_at: DateTime<Utc>,
}

impl AgentCard {
    /// Create a new agent card.
    pub fn new(
        name: impl Into<String>,
        wallet_address: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            wallet_address: wallet_address.into(),
            public_key: None,
            supported_networks: vec!["set_chain".into()],
            supported_assets: vec!["USDC".into()],
            skills: vec![AgentSkill::Buy, AgentSkill::Sell, AgentSkill::Quote],
            endpoint_url: None,
            description: description.into(),
            trust_tier: TrustTier::Sandbox,
            active: true,
            suspended_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Set supported networks.
    #[allow(clippy::missing_const_for_fn)] // Vec destructor cannot be evaluated at compile-time
    pub fn with_networks(mut self, networks: Vec<String>) -> Self {
        self.supported_networks = networks;
        self
    }

    /// Set supported assets.
    #[allow(clippy::missing_const_for_fn)] // Vec destructor cannot be evaluated at compile-time
    pub fn with_assets(mut self, assets: Vec<String>) -> Self {
        self.supported_assets = assets;
        self
    }

    /// Set skills.
    #[allow(clippy::missing_const_for_fn)] // Vec destructor cannot be evaluated at compile-time
    pub fn with_skills(mut self, skills: Vec<AgentSkill>) -> Self {
        self.skills = skills;
        self
    }

    /// Set trust tier.
    pub const fn with_trust_tier(mut self, tier: TrustTier) -> Self {
        self.trust_tier = tier;
        self
    }

    /// Set endpoint URL.
    pub fn with_endpoint(mut self, url: impl Into<String>) -> Self {
        self.endpoint_url = Some(url.into());
        self
    }

    /// Check if this agent is active (not suspended).
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active && self.suspended_at.is_none()
    }

    /// Suspend this agent.
    pub fn suspend(&mut self) {
        self.active = false;
        self.suspended_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Reactivate this agent.
    pub fn reactivate(&mut self) {
        self.active = true;
        self.suspended_at = None;
        self.updated_at = Utc::now();
    }

    /// Check if this agent supports a specific network.
    #[must_use]
    pub fn supports_network(&self, network: &str) -> bool {
        self.supported_networks.iter().any(|n| n == network)
    }

    /// Check if this agent supports a specific asset.
    #[must_use]
    pub fn supports_asset(&self, asset: &str) -> bool {
        self.supported_assets.iter().any(|a| a == asset)
    }

    /// Check if this agent has a specific skill.
    #[must_use]
    pub fn has_skill(&self, skill: AgentSkill) -> bool {
        self.skills.contains(&skill)
    }
}

/// Filters for agent discovery.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscoveryFilter {
    /// Filter by supported network.
    pub network: Option<String>,
    /// Filter by supported asset.
    pub asset: Option<String>,
    /// Filter by skill.
    pub skill: Option<AgentSkill>,
    /// Filter by minimum trust tier.
    pub min_trust_tier: Option<TrustTier>,
    /// Maximum number of results.
    pub limit: Option<u32>,
}

impl DiscoveryFilter {
    /// Check if an agent card matches this filter.
    #[must_use]
    pub fn matches(&self, card: &AgentCard) -> bool {
        // Must be active
        if !card.is_active() {
            return false;
        }

        // Network filter
        if let Some(ref network) = self.network {
            if !card.supports_network(network) {
                return false;
            }
        }

        // Asset filter
        if let Some(ref asset) = self.asset {
            if !card.supports_asset(asset) {
                return false;
            }
        }

        // Skill filter
        if let Some(skill) = self.skill {
            if !card.has_skill(skill) {
                return false;
            }
        }

        // Trust tier filter
        if let Some(min_tier) = self.min_trust_tier {
            if !card.trust_tier.is_at_least(min_tier) {
                return false;
            }
        }

        true
    }
}

/// Filter a list of agent cards using a discovery filter.
///
/// Results are sorted by trust tier (highest first), then by name.
#[must_use]
pub fn filter_agents(cards: &[AgentCard], filter: &DiscoveryFilter) -> Vec<AgentCard> {
    let mut results: Vec<AgentCard> = cards
        .iter()
        .filter(|c| filter.matches(c))
        .cloned()
        .collect();

    // Sort by trust tier descending, then name ascending
    results.sort_by(|a, b| {
        b.trust_tier
            .cmp(&a.trust_tier)
            .then_with(|| a.name.cmp(&b.name))
    });

    // Apply limit
    if let Some(limit) = filter.limit {
        results.truncate(limit as usize);
    }

    results
}

/// Validate an agent card for completeness.
///
/// # Errors
///
/// Returns [`A2AError::AgentCardError`] if required fields are missing.
pub fn validate_agent_card(card: &AgentCard) -> A2AResult<()> {
    if card.name.is_empty() {
        return Err(A2AError::agent_card("name is required"));
    }
    if card.wallet_address.is_empty() {
        return Err(A2AError::agent_card("wallet_address is required"));
    }
    if card.description.is_empty() {
        return Err(A2AError::agent_card("description is required"));
    }
    if card.supported_networks.is_empty() {
        return Err(A2AError::agent_card("at least one supported network is required"));
    }
    if card.supported_assets.is_empty() {
        return Err(A2AError::agent_card("at least one supported asset is required"));
    }
    if card.skills.is_empty() {
        return Err(A2AError::agent_card("at least one skill is required"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_card() -> AgentCard {
        AgentCard::new("TestAgent", "0xABCD1234", "A test agent")
    }

    // ===== AgentCard creation =====

    #[test]
    fn card_creation() {
        let card = sample_card();
        assert_eq!(card.name, "TestAgent");
        assert_eq!(card.wallet_address, "0xABCD1234");
        assert!(card.is_active());
        assert_eq!(card.trust_tier, TrustTier::Sandbox);
    }

    #[test]
    fn card_with_networks() {
        let card = sample_card().with_networks(vec!["set_chain".into(), "ethereum".into()]);
        assert!(card.supports_network("set_chain"));
        assert!(card.supports_network("ethereum"));
        assert!(!card.supports_network("solana"));
    }

    #[test]
    fn card_with_assets() {
        let card = sample_card().with_assets(vec!["USDC".into(), "ssUSD".into()]);
        assert!(card.supports_asset("USDC"));
        assert!(card.supports_asset("ssUSD"));
        assert!(!card.supports_asset("ETH"));
    }

    #[test]
    fn card_with_skills() {
        let card = sample_card().with_skills(vec![AgentSkill::Sell, AgentSkill::Quote]);
        assert!(card.has_skill(AgentSkill::Sell));
        assert!(card.has_skill(AgentSkill::Quote));
        assert!(!card.has_skill(AgentSkill::Buy));
    }

    #[test]
    fn card_suspension() {
        let mut card = sample_card();
        assert!(card.is_active());

        card.suspend();
        assert!(!card.is_active());
        assert!(card.suspended_at.is_some());

        card.reactivate();
        assert!(card.is_active());
        assert!(card.suspended_at.is_none());
    }

    // ===== Validation =====

    #[test]
    fn validate_valid_card() {
        assert!(validate_agent_card(&sample_card()).is_ok());
    }

    #[test]
    fn validate_empty_name() {
        let mut card = sample_card();
        card.name = String::new();
        let err = validate_agent_card(&card).unwrap_err();
        assert!(matches!(err, A2AError::AgentCardError(_)));
    }

    #[test]
    fn validate_empty_wallet() {
        let mut card = sample_card();
        card.wallet_address = String::new();
        let err = validate_agent_card(&card).unwrap_err();
        assert!(matches!(err, A2AError::AgentCardError(_)));
    }

    #[test]
    fn validate_empty_description() {
        let mut card = sample_card();
        card.description = String::new();
        let err = validate_agent_card(&card).unwrap_err();
        assert!(matches!(err, A2AError::AgentCardError(_)));
    }

    #[test]
    fn validate_empty_networks() {
        let mut card = sample_card();
        card.supported_networks.clear();
        let err = validate_agent_card(&card).unwrap_err();
        assert!(matches!(err, A2AError::AgentCardError(_)));
    }

    #[test]
    fn validate_empty_assets() {
        let mut card = sample_card();
        card.supported_assets.clear();
        let err = validate_agent_card(&card).unwrap_err();
        assert!(matches!(err, A2AError::AgentCardError(_)));
    }

    #[test]
    fn validate_empty_skills() {
        let mut card = sample_card();
        card.skills.clear();
        let err = validate_agent_card(&card).unwrap_err();
        assert!(matches!(err, A2AError::AgentCardError(_)));
    }

    // ===== Discovery filter =====

    #[test]
    fn filter_matches_active_card() {
        let card = sample_card();
        let filter = DiscoveryFilter::default();
        assert!(filter.matches(&card));
    }

    #[test]
    fn filter_rejects_inactive_card() {
        let mut card = sample_card();
        card.suspend();
        let filter = DiscoveryFilter::default();
        assert!(!filter.matches(&card));
    }

    #[test]
    fn filter_by_network() {
        let card = sample_card();
        let filter = DiscoveryFilter { network: Some("set_chain".into()), ..Default::default() };
        assert!(filter.matches(&card));

        let filter2 = DiscoveryFilter { network: Some("solana".into()), ..Default::default() };
        assert!(!filter2.matches(&card));
    }

    #[test]
    fn filter_by_asset() {
        let card = sample_card();
        let filter = DiscoveryFilter { asset: Some("USDC".into()), ..Default::default() };
        assert!(filter.matches(&card));

        let filter2 = DiscoveryFilter { asset: Some("ETH".into()), ..Default::default() };
        assert!(!filter2.matches(&card));
    }

    #[test]
    fn filter_by_skill() {
        let card = sample_card();
        let filter = DiscoveryFilter { skill: Some(AgentSkill::Buy), ..Default::default() };
        assert!(filter.matches(&card));

        let filter2 = DiscoveryFilter { skill: Some(AgentSkill::Analytics), ..Default::default() };
        assert!(!filter2.matches(&card));
    }

    #[test]
    fn filter_by_trust_tier() {
        let card = sample_card().with_trust_tier(TrustTier::Verified);
        let filter = DiscoveryFilter {
            min_trust_tier: Some(TrustTier::Standard),
            ..Default::default()
        };
        assert!(filter.matches(&card));

        let filter2 = DiscoveryFilter {
            min_trust_tier: Some(TrustTier::Enterprise),
            ..Default::default()
        };
        assert!(!filter2.matches(&card));
    }

    // ===== filter_agents =====

    #[test]
    fn filter_agents_sorted_by_tier_then_name() {
        let cards = vec![
            AgentCard::new("Charlie", "0x3", "desc").with_trust_tier(TrustTier::Standard),
            AgentCard::new("Alice", "0x1", "desc").with_trust_tier(TrustTier::Verified),
            AgentCard::new("Bob", "0x2", "desc").with_trust_tier(TrustTier::Verified),
        ];
        let results = filter_agents(&cards, &DiscoveryFilter::default());
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].name, "Alice");
        assert_eq!(results[1].name, "Bob");
        assert_eq!(results[2].name, "Charlie");
    }

    #[test]
    fn filter_agents_respects_limit() {
        let cards: Vec<AgentCard> = (0..10)
            .map(|i| AgentCard::new(format!("Agent{i}"), format!("0x{i}"), "desc"))
            .collect();
        let filter = DiscoveryFilter { limit: Some(3), ..Default::default() };
        let results = filter_agents(&cards, &filter);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn filter_agents_excludes_suspended() {
        let mut card = sample_card();
        card.suspend();
        let results = filter_agents(&[card], &DiscoveryFilter::default());
        assert!(results.is_empty());
    }

    // ===== AgentSkill =====

    #[test]
    fn skill_display() {
        assert_eq!(AgentSkill::Buy.to_string(), "buy");
        assert_eq!(AgentSkill::Sell.to_string(), "sell");
        assert_eq!(AgentSkill::Quote.to_string(), "quote");
        assert_eq!(AgentSkill::Negotiate.to_string(), "negotiate");
        assert_eq!(AgentSkill::Fulfill.to_string(), "fulfill");
        assert_eq!(AgentSkill::Analytics.to_string(), "analytics");
    }

    #[test]
    fn card_serde_roundtrip() {
        let card = sample_card();
        let json = serde_json::to_string(&card).unwrap();
        let parsed: AgentCard = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, card.name);
        assert_eq!(parsed.wallet_address, card.wallet_address);
    }
}
