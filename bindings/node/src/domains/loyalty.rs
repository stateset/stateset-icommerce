//! Loyalty  (points are integers; reward `value` is an exact decimal string).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Loyalty  (points are integers; reward `value` is an exact decimal string)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LoyaltyTierInput {
    pub name: String,
    pub min_points: i64,
    pub multiplier: f64,
    pub perks: Vec<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LoyaltyTierOutput {
    pub name: String,
    pub min_points: i64,
    pub multiplier: f64,
    pub perks: Vec<String>,
}

impl From<stateset_core::LoyaltyTier> for LoyaltyTierOutput {
    fn from(t: stateset_core::LoyaltyTier) -> Self {
        Self {
            name: t.name,
            min_points: t.min_points as i64,
            multiplier: t.multiplier,
            perks: t.perks,
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateLoyaltyProgramInput {
    pub name: String,
    pub description: Option<String>,
    pub points_per_dollar: u32,
    pub tiers: Option<Vec<LoyaltyTierInput>>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LoyaltyProgramOutput {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub points_per_dollar: u32,
    pub tiers: Vec<LoyaltyTierOutput>,
    #[napi(ts_type = "LoyaltyProgramStatus")]
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::LoyaltyProgram> for LoyaltyProgramOutput {
    fn from(p: stateset_core::LoyaltyProgram) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name,
            description: p.description,
            points_per_dollar: p.points_per_dollar,
            tiers: p.tiers.into_iter().map(Into::into).collect(),
            status: format!("{}", p.status),
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct EnrollCustomerInput {
    pub customer_id: String,
    pub program_id: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LoyaltyAccountOutput {
    pub id: String,
    pub customer_id: String,
    pub program_id: String,
    pub points_balance: i64,
    pub lifetime_points: i64,
    pub tier: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::LoyaltyAccount> for LoyaltyAccountOutput {
    fn from(a: stateset_core::LoyaltyAccount) -> Self {
        Self {
            id: a.id.to_string(),
            customer_id: a.customer_id.to_string(),
            program_id: a.program_id.to_string(),
            points_balance: a.points_balance,
            lifetime_points: a.lifetime_points as i64,
            tier: a.tier,
            created_at: a.created_at.to_rfc3339(),
            updated_at: a.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct AdjustPointsInput {
    pub account_id: String,
    pub points: i64,
    /// Transaction type, e.g. "earn", "redeem", "adjust", "expire"
    #[napi(ts_type = "LoyaltyTransactionType")]
    pub transaction_type: String,
    pub reference_id: Option<String>,
    pub description: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LoyaltyTransactionOutput {
    pub id: String,
    pub account_id: String,
    pub points: i64,
    #[napi(ts_type = "LoyaltyTransactionType")]
    pub transaction_type: String,
    pub reference_id: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
}

impl From<stateset_core::LoyaltyTransaction> for LoyaltyTransactionOutput {
    fn from(t: stateset_core::LoyaltyTransaction) -> Self {
        Self {
            id: t.id.to_string(),
            account_id: t.account_id.to_string(),
            points: t.points,
            transaction_type: format!("{}", t.transaction_type),
            reference_id: t.reference_id,
            description: t.description,
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreateRewardInput {
    pub program_id: String,
    pub name: String,
    pub description: Option<String>,
    pub points_cost: i64,
    /// Reward type, e.g. "discount", "free_product", "free_shipping"
    #[napi(ts_type = "LoyaltyRewardType")]
    pub reward_type: String,
    /// Monetary value as an exact decimal string (optional)
    pub value: Option<String>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RewardOutput {
    pub id: String,
    pub program_id: String,
    pub name: String,
    pub description: Option<String>,
    pub points_cost: i64,
    #[napi(ts_type = "LoyaltyRewardType")]
    pub reward_type: String,
    pub value: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::Reward> for RewardOutput {
    fn from(r: stateset_core::Reward) -> Self {
        Self {
            id: r.id.to_string(),
            program_id: r.program_id.to_string(),
            name: r.name,
            description: r.description,
            points_cost: r.points_cost as i64,
            reward_type: format!("{}", r.reward_type),
            value: r.value.map(|v| v.to_string()),
            is_active: r.is_active,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct LoyaltyAccountFilterInput {
    pub customer_id: Option<String>,
    pub program_id: Option<String>,
    pub tier: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct RewardFilterInput {
    pub program_id: Option<String>,
    #[napi(ts_type = "LoyaltyRewardType")]
    pub reward_type: Option<String>,
    pub is_active: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi]
pub struct Loyalty {
    pub(crate) commerce: Handle,
}

#[napi]
impl Loyalty {
    /// Whether the loyalty backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.loyalty().is_supported())
    }

    #[napi]
    pub async fn create_program(
        &self,
        input: CreateLoyaltyProgramInput,
    ) -> Result<LoyaltyProgramOutput> {
        let commerce = self.commerce.get()?;
        let tiers = input
            .tiers
            .unwrap_or_default()
            .into_iter()
            .map(|t| stateset_core::LoyaltyTier {
                name: t.name,
                min_points: t.min_points.max(0) as u64,
                multiplier: t.multiplier,
                perks: t.perks,
            })
            .collect();
        let program = commerce
            .loyalty()
            .create_program(stateset_core::CreateLoyaltyProgram {
                name: input.name,
                description: input.description,
                points_per_dollar: input.points_per_dollar,
                tiers,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create loyalty program", e))?;
        Ok(program.into())
    }

    #[napi]
    pub async fn get_program(&self, id: String) -> Result<Option<LoyaltyProgramOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let program = commerce
            .loyalty()
            .get_program(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get loyalty program", e))?;
        Ok(program.map(Into::into))
    }

    #[napi]
    pub async fn list_programs(&self) -> Result<Vec<LoyaltyProgramOutput>> {
        let commerce = self.commerce.get()?;
        let programs = commerce
            .loyalty()
            .list_programs()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list loyalty programs", e))?;
        Ok(programs.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn enroll(&self, input: EnrollCustomerInput) -> Result<LoyaltyAccountOutput> {
        let commerce = self.commerce.get()?;
        let customer_id: uuid::Uuid = input
            .customer_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let program_id: uuid::Uuid = input
            .program_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid program UUID"))?;
        let account = commerce
            .loyalty()
            .enroll(stateset_core::EnrollCustomer {
                customer_id: customer_id.into(),
                program_id: program_id.into(),
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to enroll customer", e))?;
        Ok(account.into())
    }

    #[napi]
    pub async fn get_account(&self, id: String) -> Result<Option<LoyaltyAccountOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let account = commerce
            .loyalty()
            .get_account(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get loyalty account", e))?;
        Ok(account.map(Into::into))
    }

    #[napi]
    pub async fn get_account_by_customer(
        &self,
        customer_id: String,
        program_id: String,
    ) -> Result<Option<LoyaltyAccountOutput>> {
        let commerce = self.commerce.get()?;
        let customer_id: uuid::Uuid =
            customer_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?;
        let program_id: uuid::Uuid =
            program_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid program UUID"))?;
        let account = commerce
            .loyalty()
            .get_account_by_customer(customer_id.into(), program_id.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get loyalty account", e))?;
        Ok(account.map(Into::into))
    }

    #[napi]
    pub async fn list_accounts(
        &self,
        filter: Option<LoyaltyAccountFilterInput>,
    ) -> Result<Vec<LoyaltyAccountOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or(LoyaltyAccountFilterInput {
            customer_id: None,
            program_id: None,
            tier: None,
            limit: None,
            offset: None,
        });
        let customer_id = match filter.customer_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid customer UUID"))?
                    .into(),
            ),
            None => None,
        };
        let program_id = match filter.program_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid program UUID"))?
                    .into(),
            ),
            None => None,
        };
        let accounts = commerce
            .loyalty()
            .list_accounts(stateset_core::LoyaltyAccountFilter {
                customer_id,
                program_id,
                tier: filter.tier,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list loyalty accounts", e))?;
        Ok(accounts.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn adjust_points(
        &self,
        input: AdjustPointsInput,
    ) -> Result<LoyaltyTransactionOutput> {
        let commerce = self.commerce.get()?;
        let account_id: uuid::Uuid = input
            .account_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid account UUID"))?;
        let transaction_type = input
            .transaction_type
            .parse::<stateset_core::LoyaltyTransactionType>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid transaction_type"))?;
        let txn = commerce
            .loyalty()
            .adjust_points(stateset_core::AdjustPoints {
                account_id: account_id.into(),
                points: input.points,
                transaction_type,
                reference_id: input.reference_id,
                description: input.description,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to adjust points", e))?;
        Ok(txn.into())
    }

    #[napi]
    pub async fn get_transactions(
        &self,
        account_id: String,
        limit: Option<u32>,
    ) -> Result<Vec<LoyaltyTransactionOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            account_id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let txns = commerce
            .loyalty()
            .get_transactions(uuid.into(), limit)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get transactions", e))?;
        Ok(txns.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn create_reward(&self, input: CreateRewardInput) -> Result<RewardOutput> {
        let commerce = self.commerce.get()?;
        let program_id: uuid::Uuid = input
            .program_id
            .parse()
            .map_err(|_| coded(ErrCode::Validation, "Invalid program UUID"))?;
        let reward_type = input
            .reward_type
            .parse::<stateset_core::RewardType>()
            .map_err(|_| coded(ErrCode::Validation, "Invalid reward_type"))?;
        let value = match input.value.as_deref() {
            Some(s) => Some(
                s.parse::<Decimal>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid value decimal"))?,
            ),
            None => None,
        };
        let reward = commerce
            .loyalty()
            .create_reward(stateset_core::CreateReward {
                program_id: program_id.into(),
                name: input.name,
                description: input.description,
                points_cost: input.points_cost.max(0) as u64,
                reward_type,
                value,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to create reward", e))?;
        Ok(reward.into())
    }

    #[napi]
    pub async fn get_reward(&self, id: String) -> Result<Option<RewardOutput>> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        let reward = commerce
            .loyalty()
            .get_reward(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get reward", e))?;
        Ok(reward.map(Into::into))
    }

    #[napi]
    pub async fn list_rewards(
        &self,
        filter: Option<RewardFilterInput>,
    ) -> Result<Vec<RewardOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or(RewardFilterInput {
            program_id: None,
            reward_type: None,
            is_active: None,
            limit: None,
            offset: None,
        });
        let program_id = match filter.program_id.as_deref() {
            Some(s) => Some(
                s.parse::<uuid::Uuid>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid program UUID"))?
                    .into(),
            ),
            None => None,
        };
        let reward_type = match filter.reward_type.as_deref() {
            Some(s) => Some(
                s.parse::<stateset_core::RewardType>()
                    .map_err(|_| coded(ErrCode::Validation, "Invalid reward_type"))?,
            ),
            None => None,
        };
        let rewards = commerce
            .loyalty()
            .list_rewards(stateset_core::RewardFilter {
                program_id,
                reward_type,
                is_active: filter.is_active,
                limit: filter.limit,
                offset: filter.offset,
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list rewards", e))?;
        Ok(rewards.into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn delete_reward(&self, id: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        let uuid: uuid::Uuid =
            id.parse().map_err(|_| coded(ErrCode::Validation, "Invalid UUID"))?;
        commerce
            .loyalty()
            .delete_reward(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to delete reward", e))?;
        Ok(())
    }
}
