use std::collections::{HashMap, VecDeque};

use serde::Serialize;
use serde_json::Value;
use smallvec::SmallVec;
use uuid::Uuid;

use crate::action::{ActionType, PolicyAction};
use crate::explanation::PolicyExplanation;
use crate::policy_set::{PolicySet, PolicySetEvaluation};

/// Maximum number of evaluation records to keep in history.
const MAX_HISTORY_SIZE: usize = 1000;

/// Behavior when evaluating a domain with no registered policy sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UnknownDomainMode {
    /// Fail-open: allow when no policy sets are registered for the domain.
    Allow,
    /// Fail-closed: deny when no policy sets are registered for the domain.
    Deny,
}

impl Default for UnknownDomainMode {
    fn default() -> Self {
        Self::Deny
    }
}

/// The main policy engine — manages policy sets and evaluates contexts.
///
/// # Deny-overrides
///
/// When evaluating a domain, if **any** matched action across **all** policy sets
/// is a `Deny`, the overall result is `should_deny = true`, regardless of any
/// `Allow` actions.
///
/// ## Unknown domains
///
/// By default, domains with no registered policy sets are denied
/// ([`UnknownDomainMode::Deny`]). Use [`PolicyEngine::with_unknown_domain_mode`]
/// or [`PolicyEngine::set_unknown_domain_mode`] to opt into fail-open behavior.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use stateset_policy::{
///     PolicyEngine, PolicySet, PolicyRule, PolicyAction,
///     ConditionGroup, ConditionNode, Condition, Operator, Logic,
/// };
///
/// let mut engine = PolicyEngine::new();
///
/// let rule = PolicyRule::new("high-value", "Flag high-value orders")
///     .with_priority(10)
///     .with_conditions(ConditionGroup::new(Logic::And, vec![
///         ConditionNode::Leaf(Condition::new("order.total", Operator::Gt, json!(10000))),
///     ]))
///     .with_action(PolicyAction::deny("Exceeds limit", "Get approval"));
///
/// engine.register_policy_set(
///     PolicySet::new("order-limits", "orders").with_rule(rule),
/// );
///
/// let result = engine.evaluate("orders", &json!({"order": {"total": 15000}}));
/// assert!(result.should_deny);
/// assert_eq!(result.explanations.len(), 1);
/// ```
#[derive(Debug)]
pub struct PolicyEngine {
    /// All registered policy sets, keyed by their UUID.
    policy_sets: HashMap<Uuid, PolicySet>,
    /// Index from domain name to the UUIDs of its policy sets.
    domain_index: HashMap<String, Vec<Uuid>>,
    /// Behavior for domains with no registered policy sets.
    unknown_domain_mode: UnknownDomainMode,
    /// Evaluation history (capped at [`MAX_HISTORY_SIZE`]).
    history: VecDeque<EvaluationRecord>,
}

impl PolicyEngine {
    /// Create a new, empty policy engine.
    #[must_use]
    pub fn new() -> Self {
        Self {
            policy_sets: HashMap::new(),
            domain_index: HashMap::new(),
            unknown_domain_mode: UnknownDomainMode::default(),
            history: VecDeque::new(),
        }
    }

    /// Builder: set unknown-domain behavior.
    #[must_use]
    pub const fn with_unknown_domain_mode(mut self, mode: UnknownDomainMode) -> Self {
        self.unknown_domain_mode = mode;
        self
    }

    /// Set unknown-domain behavior.
    pub const fn set_unknown_domain_mode(&mut self, mode: UnknownDomainMode) {
        self.unknown_domain_mode = mode;
    }

    /// Get the currently configured unknown-domain behavior.
    #[must_use]
    pub const fn unknown_domain_mode(&self) -> UnknownDomainMode {
        self.unknown_domain_mode
    }

    /// Register a policy set. If a set with the same ID already exists, it is replaced.
    pub fn register_policy_set(&mut self, set: PolicySet) {
        let id = set.id;
        let domain = set.domain.clone();

        if let Some(previous) = self.policy_sets.insert(id, set) {
            // If an existing set is being replaced, remove stale domain index entries.
            if let Some(ids) = self.domain_index.get_mut(&previous.domain) {
                ids.retain(|existing_id| existing_id != &id);
                if ids.is_empty() {
                    self.domain_index.remove(&previous.domain);
                }
            }
        }

        let domain_ids = self.domain_index.entry(domain).or_default();
        if !domain_ids.contains(&id) {
            domain_ids.push(id);
        }
    }

    /// Remove a policy set by its UUID. Returns the removed set, if any.
    pub fn unregister_policy_set(&mut self, id: &Uuid) -> Option<PolicySet> {
        let set = self.policy_sets.remove(id)?;

        // Remove from domain index
        if let Some(ids) = self.domain_index.get_mut(&set.domain) {
            ids.retain(|i| i != id);
            if ids.is_empty() {
                self.domain_index.remove(&set.domain);
            }
        }

        Some(set)
    }

    /// Get a policy set by its UUID.
    #[must_use]
    pub fn get_policy_set(&self, id: &Uuid) -> Option<&PolicySet> {
        self.policy_sets.get(id)
    }

    /// Get all policy sets for a domain.
    #[must_use]
    pub fn get_policies_for_domain(&self, domain: &str) -> Vec<&PolicySet> {
        self.domain_index
            .get(domain)
            .map(|ids| ids.iter().filter_map(|id| self.policy_sets.get(id)).collect())
            .unwrap_or_default()
    }

    /// List all registered policy sets.
    #[must_use]
    pub fn list_policy_sets(&self) -> Vec<&PolicySet> {
        self.policy_sets.values().collect()
    }

    /// Total number of registered policy sets.
    #[must_use]
    pub fn policy_set_count(&self) -> usize {
        self.policy_sets.len()
    }

    /// Total number of rules across all policy sets.
    #[must_use]
    pub fn total_rule_count(&self) -> usize {
        self.policy_sets.values().map(|ps| ps.rules.len()).sum()
    }

    /// Evaluate all policy sets for the given domain.
    ///
    /// Records the evaluation in history. For evaluation without history
    /// recording, use [`evaluate_dry_run`](Self::evaluate_dry_run).
    ///
    /// # Deny-overrides
    ///
    /// If **any** action across all matched rules is `Deny`, `should_deny` is `true`
    /// and `should_allow` is `false`, regardless of any `Allow` actions.
    ///
    /// If no policy sets are registered for `domain`, behavior is controlled by
    /// [`UnknownDomainMode`] (default: deny).
    pub fn evaluate(&mut self, domain: &str, context: &Value) -> PolicyEvaluation {
        let eval = self.evaluate_inner(domain, context, false);

        // Record in history
        self.history.push_back(EvaluationRecord {
            id: Uuid::new_v4(),
            domain: domain.to_owned(),
            timestamp: chrono::Utc::now(),
            should_allow: eval.should_allow,
            should_deny: eval.should_deny,
            matched_rule_count: eval.results.iter().map(|r| r.matched_rules.len()).sum(),
        });

        // Keep only the last MAX_HISTORY_SIZE entries
        while self.history.len() > MAX_HISTORY_SIZE {
            self.history.pop_front();
        }

        eval
    }

    /// Evaluate without recording history (dry-run mode).
    #[must_use]
    pub fn evaluate_dry_run(&self, domain: &str, context: &Value) -> PolicyEvaluation {
        self.evaluate_inner(domain, context, true)
    }

    /// Internal evaluation logic shared by `evaluate` and `evaluate_dry_run`.
    fn evaluate_inner(&self, domain: &str, context: &Value, dry_run: bool) -> PolicyEvaluation {
        // Look up domain index directly to avoid allocating a Vec<&PolicySet>.
        let set_ids = self.domain_index.get(domain);
        let is_empty = set_ids.is_none_or(Vec::is_empty);

        if is_empty {
            let unknown_domain_action = match self.unknown_domain_mode {
                UnknownDomainMode::Allow => PolicyAction::allow(),
                UnknownDomainMode::Deny => PolicyAction::deny_simple(format!(
                    "No policy sets registered for domain \"{domain}\"",
                )),
            };

            let should_allow = matches!(self.unknown_domain_mode, UnknownDomainMode::Allow);
            return PolicyEvaluation {
                domain: domain.to_owned(),
                results: Vec::new(),
                actions: vec![unknown_domain_action],
                explanations: Vec::new(),
                should_allow,
                should_deny: !should_allow,
                dry_run,
            };
        }

        let ids = set_ids.expect("checked non-empty above");

        // SmallVec avoids heap allocation for typical policy counts (<=4 sets).
        let mut all_results: SmallVec<[PolicySetEvaluation; 4]> = SmallVec::new();
        let mut all_actions: SmallVec<[PolicyAction; 8]> = SmallVec::new();
        let mut all_explanations: SmallVec<[PolicyExplanation; 4]> = SmallVec::new();

        // Track deny/allow during iteration to avoid a second scan.
        let mut has_deny = false;
        let mut has_allow = false;

        for id in ids {
            let Some(ps) = self.policy_sets.get(id) else {
                continue;
            };
            let eval_result = ps.evaluate(context);

            if eval_result.matched {
                for action in &eval_result.actions {
                    match action.action_type {
                        ActionType::Deny => has_deny = true,
                        ActionType::Allow => has_allow = true,
                        _ => {}
                    }
                }
                all_actions.extend(eval_result.actions.iter().cloned());
                all_explanations.extend(eval_result.explanations.iter().cloned());
            } else if eval_result.default_applied {
                match ps.default_action.action_type {
                    ActionType::Deny => has_deny = true,
                    ActionType::Allow => has_allow = true,
                    _ => {}
                }
                all_actions.push(ps.default_action.clone());
            }

            all_results.push(eval_result);
        }

        PolicyEvaluation {
            domain: domain.to_owned(),
            results: all_results.into_vec(),
            actions: all_actions.into_vec(),
            explanations: all_explanations.into_vec(),
            should_allow: !has_deny && has_allow,
            should_deny: has_deny,
            dry_run,
        }
    }

    /// Load policy sets from a directory of YAML/JSON files (strict mode).
    ///
    /// Scans for `*.yaml`, `*.yml`, and `*.json` files. Any invalid policy file
    /// returns an error.
    #[cfg(feature = "yaml")]
    pub fn load_from_dir(&mut self, dir: &std::path::Path) -> crate::Result<usize> {
        let sets = crate::loader::load_policies_from_dir(dir)?;
        let count = sets.len();
        for set in sets {
            self.register_policy_set(set);
        }
        Ok(count)
    }

    /// Load policy sets from a directory in permissive mode.
    ///
    /// Invalid policy files are skipped.
    #[cfg(feature = "yaml")]
    pub fn load_from_dir_permissive(&mut self, dir: &std::path::Path) -> crate::Result<usize> {
        let sets = crate::loader::load_policies_from_dir_permissive(dir)?;
        let count = sets.len();
        for set in sets {
            self.register_policy_set(set);
        }
        Ok(count)
    }

    /// Get the evaluation history.
    #[must_use]
    pub const fn get_history(&self) -> &VecDeque<EvaluationRecord> {
        &self.history
    }

    /// Get the last `n` history entries.
    #[must_use]
    pub fn get_recent_history(&self, n: usize) -> Vec<&EvaluationRecord> {
        self.history.iter().rev().take(n).collect()
    }

    /// Get history entries filtered by domain.
    #[must_use]
    pub fn get_history_for_domain(&self, domain: &str) -> Vec<&EvaluationRecord> {
        self.history.iter().filter(|r| r.domain == domain).collect()
    }

    /// Clear all history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Get a summary of the engine status.
    #[must_use]
    pub fn get_status(&self) -> EngineStatus {
        let mut by_domain: HashMap<String, usize> = HashMap::new();
        for ps in self.policy_sets.values() {
            *by_domain.entry(ps.domain.clone()).or_default() += 1;
        }

        EngineStatus {
            total_policy_sets: self.policy_sets.len(),
            total_rules: self.total_rule_count(),
            by_domain,
            recent_evaluations: self.history.iter().rev().take(10).cloned().collect(),
        }
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// The overall result of evaluating all policy sets for a domain.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyEvaluation {
    /// The domain that was evaluated.
    pub domain: String,
    /// Per-policy-set evaluation results.
    pub results: Vec<PolicySetEvaluation>,
    /// All collected actions from matched rules + defaults.
    pub actions: Vec<PolicyAction>,
    /// All explanations from matched rules.
    pub explanations: Vec<PolicyExplanation>,
    /// Whether the overall evaluation allows the operation.
    pub should_allow: bool,
    /// Whether the overall evaluation denies the operation.
    pub should_deny: bool,
    /// Whether this was a dry-run evaluation.
    pub dry_run: bool,
}

/// A record of a past evaluation (stored in history).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationRecord {
    /// Unique ID for this evaluation.
    pub id: Uuid,
    /// The domain that was evaluated.
    pub domain: String,
    /// When the evaluation occurred.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Whether the result was "allow".
    pub should_allow: bool,
    /// Whether the result was "deny".
    pub should_deny: bool,
    /// How many rules matched across all policy sets.
    pub matched_rule_count: usize,
}

/// Summary status of the engine.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    /// Total number of registered policy sets.
    pub total_policy_sets: usize,
    /// Total number of rules across all sets.
    pub total_rules: usize,
    /// Count of policy sets per domain.
    pub by_domain: HashMap<String, usize>,
    /// The 10 most recent evaluation records.
    pub recent_evaluations: Vec<EvaluationRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Condition, ConditionGroup, ConditionNode, Logic, Operator};
    use serde_json::json;

    fn high_value_deny_set() -> PolicySet {
        PolicySet::new("order-limits", "orders").with_rule(
            PolicyRule::new("high-value", "Deny high-value orders")
                .with_priority(100)
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new(
                        "order.total",
                        Operator::Gt,
                        json!(10000),
                    ))],
                ))
                .with_action(PolicyAction::deny(
                    "Order exceeds $10,000 limit",
                    "Request manager approval",
                )),
        )
    }

    #[test]
    fn engine_basic_deny() {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(high_value_deny_set());

        let result = engine.evaluate("orders", &json!({"order": {"total": 15000}}));
        assert!(result.should_deny);
        assert!(!result.should_allow);
        assert_eq!(result.explanations.len(), 1);
    }

    #[test]
    fn engine_basic_allow() {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(high_value_deny_set());

        let result = engine.evaluate("orders", &json!({"order": {"total": 500}}));
        assert!(result.should_allow);
        assert!(!result.should_deny);
    }

    #[test]
    fn engine_no_policy_sets() {
        let mut engine = PolicyEngine::new();
        let result = engine.evaluate("orders", &json!({}));
        assert!(!result.should_allow);
        assert!(result.should_deny);
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].action_type, ActionType::Deny);
    }

    #[test]
    fn engine_unknown_domain_denies_by_default() {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(high_value_deny_set());

        // Evaluating a different domain that has no policy sets
        let result = engine.evaluate("returns", &json!({}));
        assert!(!result.should_allow);
        assert!(result.should_deny);
    }

    #[test]
    fn engine_unknown_domain_can_allow_when_configured() {
        let mut engine = PolicyEngine::new().with_unknown_domain_mode(UnknownDomainMode::Allow);
        engine.register_policy_set(high_value_deny_set());

        // Evaluating a different domain that has no policy sets
        let result = engine.evaluate("returns", &json!({}));
        assert!(result.should_allow);
        assert!(!result.should_deny);
    }

    #[test]
    fn engine_deny_overrides_across_sets() {
        let mut engine = PolicyEngine::new();

        // Set 1: allows
        let allow_set = PolicySet::new("allow-set", "orders").with_rule(
            PolicyRule::new("allow-all", "Allow everything")
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new(
                        "order.total",
                        Operator::Gt,
                        json!(0),
                    ))],
                ))
                .with_action(PolicyAction::allow()),
        );

        // Set 2: denies high value
        let deny_set = high_value_deny_set();

        engine.register_policy_set(allow_set);
        engine.register_policy_set(deny_set);

        // Deny should override the allow
        let result = engine.evaluate("orders", &json!({"order": {"total": 15000}}));
        assert!(result.should_deny);
        assert!(!result.should_allow);
    }

    #[test]
    fn engine_dry_run_no_history() {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(high_value_deny_set());

        let result = engine.evaluate_dry_run("orders", &json!({"order": {"total": 15000}}));
        assert!(result.should_deny);
        assert!(result.dry_run);

        // History should be empty
        assert!(engine.get_history().is_empty());
    }

    #[test]
    fn engine_evaluate_records_history() {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(high_value_deny_set());

        engine.evaluate("orders", &json!({"order": {"total": 15000}}));
        engine.evaluate("orders", &json!({"order": {"total": 500}}));

        assert_eq!(engine.get_history().len(), 2);

        let recent = engine.get_recent_history(1);
        assert_eq!(recent.len(), 1);
        assert!(recent[0].should_allow); // The second evaluation was allow
    }

    #[test]
    fn engine_history_capped() {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(
            PolicySet::new("test", "orders").with_rule(
                PolicyRule::new("always-match", "Always matches")
                    .with_conditions(ConditionGroup::new(
                        Logic::And,
                        vec![ConditionNode::Leaf(Condition::new(
                            "x",
                            Operator::IsNotNull,
                            json!(null),
                        ))],
                    ))
                    .with_action(PolicyAction::allow()),
            ),
        );

        for i in 0..1050 {
            engine.evaluate("orders", &json!({"x": i}));
        }

        assert_eq!(engine.get_history().len(), MAX_HISTORY_SIZE);
    }

    #[test]
    fn engine_history_by_domain() {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(PolicySet::new("o", "orders"));
        engine.register_policy_set(PolicySet::new("r", "returns"));

        engine.evaluate("orders", &json!({}));
        engine.evaluate("returns", &json!({}));
        engine.evaluate("orders", &json!({}));

        let orders_history = engine.get_history_for_domain("orders");
        assert_eq!(orders_history.len(), 2);
    }

    #[test]
    fn engine_clear_history() {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(PolicySet::new("o", "orders"));
        engine.evaluate("orders", &json!({}));
        assert!(!engine.get_history().is_empty());

        engine.clear_history();
        assert!(engine.get_history().is_empty());
    }

    #[test]
    fn engine_register_and_unregister() {
        let mut engine = PolicyEngine::new();
        let ps = high_value_deny_set();
        let id = ps.id;

        engine.register_policy_set(ps);
        assert_eq!(engine.policy_set_count(), 1);

        let removed = engine.unregister_policy_set(&id);
        assert!(removed.is_some());
        assert_eq!(engine.policy_set_count(), 0);
        assert!(engine.get_policies_for_domain("orders").is_empty());
    }

    #[test]
    fn engine_re_register_same_id_updates_domain_index() {
        let mut engine = PolicyEngine::new();
        let id = Uuid::new_v4();

        let orders_set = PolicySet::new("limits", "orders").with_id(id);
        engine.register_policy_set(orders_set);
        assert_eq!(engine.get_policies_for_domain("orders").len(), 1);

        let returns_set = PolicySet::new("limits-v2", "returns").with_id(id);
        engine.register_policy_set(returns_set);

        assert!(engine.get_policies_for_domain("orders").is_empty());
        assert_eq!(engine.get_policies_for_domain("returns").len(), 1);
        assert_eq!(engine.policy_set_count(), 1);
    }

    #[test]
    fn engine_status() {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(high_value_deny_set());

        let status = engine.get_status();
        assert_eq!(status.total_policy_sets, 1);
        assert_eq!(status.total_rules, 1);
        assert_eq!(status.by_domain.get("orders"), Some(&1));
    }

    #[test]
    fn engine_multiple_sets_same_domain() {
        let mut engine = PolicyEngine::new();

        let set1 = PolicySet::new("set1", "orders").with_rule(
            PolicyRule::new("r1", "Rule 1")
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new("x", Operator::Eq, json!(1)))],
                ))
                .with_action(PolicyAction::allow()),
        );

        let set2 = PolicySet::new("set2", "orders").with_rule(
            PolicyRule::new("r2", "Rule 2")
                .with_conditions(ConditionGroup::new(
                    Logic::And,
                    vec![ConditionNode::Leaf(Condition::new("x", Operator::Eq, json!(1)))],
                ))
                .with_action(PolicyAction::allow()),
        );

        engine.register_policy_set(set1);
        engine.register_policy_set(set2);

        assert_eq!(engine.get_policies_for_domain("orders").len(), 2);

        let result = engine.evaluate("orders", &json!({"x": 1}));
        assert!(result.should_allow);
        assert_eq!(result.results.len(), 2);
    }

    #[test]
    fn engine_default_action_when_no_match() {
        let mut engine = PolicyEngine::new();

        // Policy set with a rule that won't match, default is allow
        let ps = PolicySet::new("test", "orders")
            .with_rule(
                PolicyRule::new("never-match", "Never matches")
                    .with_conditions(ConditionGroup::new(
                        Logic::And,
                        vec![ConditionNode::Leaf(Condition::new(
                            "x",
                            Operator::Eq,
                            json!("impossible"),
                        ))],
                    ))
                    .with_action(PolicyAction::deny("denied", "fix")),
            )
            .with_default_action(PolicyAction::allow());

        engine.register_policy_set(ps);
        let result = engine.evaluate("orders", &json!({"x": 1}));

        // Default allow action should be in the actions list
        assert!(result.should_allow);
        assert_eq!(result.actions.len(), 1);
        assert_eq!(result.actions[0].action_type, ActionType::Allow);
    }

    #[test]
    fn evaluation_serializable() {
        let mut engine = PolicyEngine::new();
        engine.register_policy_set(high_value_deny_set());

        let result = engine.evaluate("orders", &json!({"order": {"total": 15000}}));
        let json_str = serde_json::to_string(&result).unwrap();
        assert!(json_str.contains("shouldDeny"));
        assert!(json_str.contains("shouldAllow"));
    }

    use crate::PolicyRule;
}
