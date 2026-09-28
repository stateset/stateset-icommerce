//! Agent-card governance for A2A commerce participants.
//!
//! A quote or purchase may only be created between registered agents: the
//! buyer must resolve to an agent card that can buy
//! ([`AgentCard::can_buy`]) and the seller to one that can sell
//! ([`AgentCard::can_sell`]). Both backends load the cards inside the write
//! transaction that inserts the quote/purchase and run this check on them.

use stateset_core::{AgentCard, CommerceError, Result};
use uuid::Uuid;

/// Which side of an A2A trade an agent is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum A2ASide {
    Buyer,
    Seller,
}

impl A2ASide {
    const fn field(self) -> &'static str {
        match self {
            Self::Buyer => "buyer_agent_id",
            Self::Seller => "seller_agent_id",
        }
    }

    const fn role(self) -> &'static str {
        match self {
            Self::Buyer => "buyer",
            Self::Seller => "seller",
        }
    }

    const fn skill(self) -> &'static str {
        match self {
            Self::Buyer => "buy",
            Self::Seller => "sell",
        }
    }
}

/// Refuse unless `card` (the card loaded for `agent_id`) exists, is active,
/// is not suspended, and advertises the skill `side` needs.
pub(crate) fn ensure_participant(
    card: Option<&AgentCard>,
    agent_id: Uuid,
    side: A2ASide,
) -> Result<()> {
    let Some(card) = card else {
        return Err(CommerceError::ValidationError(format!(
            "{} {agent_id} has no registered agent card; register an agent card before trading",
            side.field()
        )));
    };
    if !card.active || card.suspended_at.is_some() {
        return Err(CommerceError::ValidationError(format!(
            "{} {agent_id} agent card is not active; a suspended or deactivated agent cannot act as {}",
            side.field(),
            side.role()
        )));
    }
    let permitted = match side {
        A2ASide::Buyer => card.can_buy(),
        A2ASide::Seller => card.can_sell(),
    };
    if !permitted {
        return Err(CommerceError::ValidationError(format!(
            "{} {agent_id} is not permitted to {}: its agent card does not advertise a {} skill",
            side.field(),
            side.skill(),
            match side {
                A2ASide::Buyer => "buy / request_quote",
                A2ASide::Seller => "sell / quote",
            }
        )));
    }
    Ok(())
}
