/**
 * A2A Declarative Rules Engine — Programmable Agent Guardrails
 *
 * Provides "if X then Y" rules that automatically trigger without manual
 * intervention. Agents register rules with conditions and actions; the engine
 * evaluates context objects against all matching rules and returns an
 * aggregate decision.
 *
 * Rule evaluation order: higher priority first, first `block` action wins.
 *
 * @example
 * ```javascript
 * const engine = createRulesEngine();
 *
 * engine.addRule({
 *   name: 'High-value guard',
 *   description: 'Block transactions over $1000 without escrow',
 *   agentAddress: '0xAgent1',
 *   condition: { field: 'amount', operator: 'gt', value: 1000 },
 *   action: { type: 'require_escrow', params: { reason: 'high value' } },
 *   priority: 90,
 *   enabled: true,
 *   tags: ['financial', 'safety'],
 * });
 *
 * const result = engine.evaluate({ amount: 5000, counterparty: '0xNew' });
 * // { allowed: true/false, appliedRules: [...], explanation: '...' }
 * ```
 */

import { randomUUID } from 'node:crypto';

// ── Condition operators ────────────────────────────────────────────────────

const OPERATORS = {
  eq: (a, b) => a === b,
  neq: (a, b) => a !== b,
  gt: (a, b) => a > b,
  gte: (a, b) => a >= b,
  lt: (a, b) => a < b,
  lte: (a, b) => a <= b,
  in: (a, b) => Array.isArray(b) && b.includes(a),
  not_in: (a, b) => Array.isArray(b) && !b.includes(a),
  contains: (a, b) => typeof a === 'string' && a.includes(b),
  matches: (a, b) => {
    try {
      return typeof a === 'string' && new RegExp(b).test(a);
    } catch {
      return false;
    }
  },
};

// ── Condition evaluation ───────────────────────────────────────────────────

/**
 * Resolve a dot-separated field path on an object.
 * e.g. resolveField({ a: { b: 3 } }, 'a.b') → 3
 *
 * @param {Object} ctx
 * @param {string} fieldPath
 * @returns {*}
 */
function resolveField(ctx, fieldPath) {
  if (!ctx || typeof fieldPath !== 'string') return undefined;
  const parts = fieldPath.split('.');
  let value = ctx;
  for (const part of parts) {
    if (value === null || value === undefined) return undefined;
    value = value[part];
  }
  return value;
}

/**
 * Evaluate a single condition (simple or compound) against a context.
 *
 * @param {Object} condition
 * @param {Object} ctx
 * @returns {boolean}
 */
function evaluateCondition(condition, ctx) {
  // Compound: all (AND)
  if (Array.isArray(condition.all)) {
    return condition.all.every((sub) => evaluateCondition(sub, ctx));
  }

  // Compound: any (OR)
  if (Array.isArray(condition.any)) {
    return condition.any.some((sub) => evaluateCondition(sub, ctx));
  }

  // Simple condition
  const { field, operator, value } = condition;
  if (!field || !operator) return false;

  const fn = OPERATORS[operator];
  if (!fn) return false;

  const fieldValue = resolveField(ctx, field);
  return fn(fieldValue, value);
}

// ── Built-in rule templates ────────────────────────────────────────────────

const TEMPLATES = {
  HIGH_VALUE_GUARD: (threshold = 1000) => ({
    name: 'High-value guard',
    description: `Block transactions above $${threshold} without escrow`,
    condition: { field: 'amount', operator: 'gt', value: threshold },
    action: { type: 'require_escrow', params: { reason: 'high value' } },
    priority: 90,
    enabled: true,
    tags: ['financial', 'safety'],
  }),

  LOW_REPUTATION_FILTER: (minReputation = 3.0) => ({
    name: 'Low-reputation filter',
    description: `Decline counterparties with reputation below ${minReputation}`,
    condition: { field: 'counterpartyReputation', operator: 'lt', value: minReputation },
    action: { type: 'block', params: { reason: 'low reputation' } },
    priority: 85,
    enabled: true,
    tags: ['trust', 'safety'],
  }),

  DAILY_SPEND_LIMIT: (limit = 5000) => ({
    name: 'Daily spend limit',
    description: `Block if daily spend exceeds $${limit}`,
    condition: { field: 'dailySpend', operator: 'gt', value: limit },
    action: { type: 'block', params: { reason: 'daily spend limit exceeded' } },
    priority: 95,
    enabled: true,
    tags: ['financial', 'budget'],
  }),

  FIRST_TIME_BUYER_ESCROW: () => ({
    name: 'First-time buyer escrow',
    description: 'Require escrow for first-time counterparties',
    condition: { field: 'isFirstTimeBuyer', operator: 'eq', value: true },
    action: { type: 'require_escrow', params: { reason: 'first-time buyer' } },
    priority: 70,
    enabled: true,
    tags: ['trust', 'onboarding'],
  }),

  DISPUTE_RATE_BLACKLIST: (maxDisputeRate = 10) => ({
    name: 'Dispute rate blacklist',
    description: `Auto-blacklist agents with dispute rate above ${maxDisputeRate}%`,
    condition: { field: 'disputeRatePercent', operator: 'gt', value: maxDisputeRate },
    action: { type: 'block', params: { reason: 'excessive dispute rate' } },
    priority: 95,
    enabled: true,
    tags: ['trust', 'safety'],
  }),
};

// ── Rules engine factory ───────────────────────────────────────────────────

/**
 * Create a declarative rules engine.
 *
 * @returns {Object} Rules engine API
 */
export function createRulesEngine() {
  /** @type {Map<string, Object>} */
  const _rules = new Map();

  /** @type {Array<Object>} */
  const _auditLog = [];

  const MAX_AUDIT_LOG = 1000;

  // ── Rule CRUD ──────────────────────────────────────────────────────────

  /**
   * Register a rule. Returns the rule ID.
   *
   * @param {Object} rule
   * @param {string} rule.name - Human-readable name
   * @param {string} [rule.description] - Description
   * @param {string} [rule.agentAddress] - Owning agent address
   * @param {Object} rule.condition - Condition tree (simple or compound)
   * @param {Object} rule.action - { type, params }
   * @param {number} [rule.priority=50] - 1–100, higher = evaluated first
   * @param {boolean} [rule.enabled=true]
   * @param {string[]} [rule.tags=[]]
   * @returns {string} Rule ID
   */
  function addRule(rule) {
    if (!rule || !rule.name) {
      throw new Error('Rule name is required');
    }
    if (!rule.condition) {
      throw new Error('Rule condition is required');
    }
    if (!rule.action || !rule.action.type) {
      throw new Error('Rule action with type is required');
    }

    const priority = rule.priority ?? 50;
    if (priority < 1 || priority > 100) {
      throw new Error('Priority must be between 1 and 100');
    }

    const id = randomUUID();
    const now = new Date().toISOString();

    _rules.set(id, {
      id,
      name: rule.name,
      description: rule.description || '',
      agentAddress: rule.agentAddress || null,
      condition: rule.condition,
      action: rule.action,
      priority,
      enabled: rule.enabled !== false,
      tags: Array.isArray(rule.tags) ? [...rule.tags] : [],
      createdAt: now,
      updatedAt: now,
    });

    return id;
  }

  /**
   * Remove a rule by ID.
   *
   * @param {string} ruleId
   * @returns {boolean} True if removed
   */
  function removeRule(ruleId) {
    return _rules.delete(ruleId);
  }

  /**
   * Get a rule by ID.
   *
   * @param {string} ruleId
   * @returns {Object|null}
   */
  function getRule(ruleId) {
    const rule = _rules.get(ruleId);
    return rule ? { ...rule } : null;
  }

  /**
   * List rules, optionally filtered.
   *
   * @param {Object} [filter]
   * @param {string} [filter.agentAddress]
   * @param {boolean} [filter.enabled]
   * @param {string[]} [filter.tags] - Matching any tag
   * @returns {Object[]}
   */
  function listRules(filter = {}) {
    let rules = [..._rules.values()];

    if (filter.agentAddress !== undefined) {
      rules = rules.filter((r) => r.agentAddress === filter.agentAddress);
    }
    if (filter.enabled !== undefined) {
      rules = rules.filter((r) => r.enabled === filter.enabled);
    }
    if (Array.isArray(filter.tags) && filter.tags.length > 0) {
      rules = rules.filter((r) => filter.tags.some((t) => r.tags.includes(t)));
    }

    return rules.sort((a, b) => b.priority - a.priority);
  }

  /**
   * Enable a rule.
   *
   * @param {string} ruleId
   * @returns {boolean} True if found and enabled
   */
  function enableRule(ruleId) {
    const rule = _rules.get(ruleId);
    if (!rule) return false;
    rule.enabled = true;
    rule.updatedAt = new Date().toISOString();
    return true;
  }

  /**
   * Disable a rule.
   *
   * @param {string} ruleId
   * @returns {boolean} True if found and disabled
   */
  function disableRule(ruleId) {
    const rule = _rules.get(ruleId);
    if (!rule) return false;
    rule.enabled = false;
    rule.updatedAt = new Date().toISOString();
    return true;
  }

  // ── Evaluation ─────────────────────────────────────────────────────────

  /**
   * Evaluate all enabled rules against a context object.
   *
   * Rules are processed in priority order (highest first). The first `block`
   * action encountered causes `allowed` to be false. Other action types are
   * accumulated but do not block by default.
   *
   * @param {Object} ctx - Context object with arbitrary fields
   * @returns {{ allowed: boolean, appliedRules: Object[], explanation: string }}
   */
  function evaluate(ctx) {
    const enabledRules = [..._rules.values()]
      .filter((r) => r.enabled)
      .sort((a, b) => b.priority - a.priority);

    const appliedRules = [];
    let allowed = true;
    let blocked = false;

    for (const rule of enabledRules) {
      const matched = evaluateCondition(rule.condition, ctx);
      const entry = {
        ruleId: rule.id,
        name: rule.name,
        matched,
        action: matched ? rule.action : null,
      };
      appliedRules.push(entry);

      if (matched && rule.action.type === 'block' && !blocked) {
        allowed = false;
        blocked = true;
      }
    }

    const matchedNames = appliedRules.filter((r) => r.matched).map((r) => r.name);

    let explanation;
    if (matchedNames.length === 0) {
      explanation = 'No rules matched. Operation allowed by default.';
    } else if (!allowed) {
      const blocker = appliedRules.find((r) => r.matched && r.action?.type === 'block');
      explanation = `Blocked by rule "${blocker.name}". Matched rules: ${matchedNames.join(', ')}.`;
    } else {
      explanation = `Operation allowed. Matched rules: ${matchedNames.join(', ')}.`;
    }

    // Record to audit log
    const auditEntry = {
      id: randomUUID(),
      timestamp: new Date().toISOString(),
      context: ctx,
      allowed,
      appliedRules,
      explanation,
    };

    _auditLog.push(auditEntry);
    if (_auditLog.length > MAX_AUDIT_LOG) {
      _auditLog.splice(0, _auditLog.length - MAX_AUDIT_LOG);
    }

    return { allowed, appliedRules, explanation };
  }

  /**
   * Test a single rule against a context without recording to the audit log
   * or executing the action.
   *
   * @param {string} ruleId
   * @param {Object} ctx
   * @returns {{ matched: boolean, condition: Object, action: Object }}
   */
  function testRule(ruleId, ctx) {
    const rule = _rules.get(ruleId);
    if (!rule) {
      throw new Error(`Rule not found: ${ruleId}`);
    }

    const matched = evaluateCondition(rule.condition, ctx);
    return {
      matched,
      condition: rule.condition,
      action: rule.action,
    };
  }

  /**
   * Get recent audit log entries.
   *
   * @param {number} [limit=50]
   * @returns {Object[]}
   */
  function getAuditLog(limit = 50) {
    const safeLimit = Math.max(1, Math.min(limit, MAX_AUDIT_LOG));
    return _auditLog.slice(-safeLimit).reverse();
  }

  // ── Templates ──────────────────────────────────────────────────────────

  /**
   * Create a rule from a built-in template.
   *
   * @param {string} templateName - One of the TEMPLATES keys
   * @param {Object} [overrides] - Override template defaults
   * @param {*} [param] - Template-specific parameter (threshold, limit, etc.)
   * @returns {string} Rule ID
   */
  function addFromTemplate(templateName, overrides = {}, param) {
    const factory = TEMPLATES[templateName];
    if (!factory) {
      throw new Error(
        `Unknown template: ${templateName}. Available: ${Object.keys(TEMPLATES).join(', ')}`,
      );
    }

    const template = factory(param);
    return addRule({ ...template, ...overrides });
  }

  return {
    addRule,
    removeRule,
    getRule,
    listRules,
    enableRule,
    disableRule,
    evaluate,
    testRule,
    getAuditLog,
    addFromTemplate,
    TEMPLATES: Object.keys(TEMPLATES),
  };
}

export default { createRulesEngine };
