/**
 * Declarative Policy Engine for StateSet Commerce
 *
 * Enables AI agents to follow business rules without hardcoding:
 * - YAML/JSON policy definitions
 * - Condition evaluation with operators
 * - Action triggers based on rules
 * - Policy versioning and audit
 */

import { EventEmitter } from 'events';
import { randomUUID } from 'crypto';
import fs from 'fs';
import path from 'path';
import { parse as parseYAML } from 'yaml';

/**
 * Supported comparison operators
 */
export const Operators = {
  // Comparison
  eq: (a, b) => a === b,
  neq: (a, b) => a !== b,
  gt: (a, b) => a > b,
  gte: (a, b) => a >= b,
  lt: (a, b) => a < b,
  lte: (a, b) => a <= b,

  // String
  contains: (a, b) => String(a).includes(String(b)),
  startsWith: (a, b) => String(a).startsWith(String(b)),
  endsWith: (a, b) => String(a).endsWith(String(b)),
  matches: (a, b) => {
    const pattern = String(b);
    if (pattern.length > 200) return false;
    try {
      return new RegExp(pattern).test(String(a));
    } catch (err) {
      console.debug('[policy-engine] Regex match failed:', err.message || err);
      return false;
    }
  },

  // Collection
  in: (a, b) => (Array.isArray(b) ? b.includes(a) : false),
  notIn: (a, b) => (Array.isArray(b) ? !b.includes(a) : true),
  isEmpty: (a) => !a || (Array.isArray(a) ? a.length === 0 : Object.keys(a).length === 0),
  isNotEmpty: (a) => a && (Array.isArray(a) ? a.length > 0 : Object.keys(a).length > 0),

  // Type
  isNull: (a) => a === null || a === undefined,
  isNotNull: (a) => a !== null && a !== undefined,
  isTrue: (a) => a === true,
  isFalse: (a) => a === false,

  // Numeric
  between: (a, [min, max]) => a >= min && a <= max,
  divisibleBy: (a, b) => a % b === 0,
};

/**
 * Get nested value from object using dot notation
 */
function getNestedValue(obj, path) {
  if (!path) return obj;

  const parts = path.split('.');
  let current = obj;

  for (const part of parts) {
    if (current === null || current === undefined) return undefined;

    // Handle array access: items[0]
    const arrayMatch = part.match(/^(\w+)\[(\d+)\]$/);
    if (arrayMatch) {
      current = current[arrayMatch[1]];
      if (Array.isArray(current)) {
        current = current[parseInt(arrayMatch[2], 10)];
      } else {
        return undefined;
      }
    } else {
      current = current[part];
    }
  }

  return current;
}

/**
 * Condition operators that ignore the provided comparison value.
 */
const UnaryOperators = new Set([
  'isEmpty',
  'isNotEmpty',
  'isNull',
  'isNotNull',
  'isTrue',
  'isFalse',
]);

/**
 * Resolve a condition's comparison value when it is a dynamic reference like:
 *   "${order.billingAddress.country}"
 *
 * If the reference path does not exist in context, `resolved` will be undefined.
 */
function resolveConditionValue(value, context) {
  if (typeof value !== 'string') {
    return { resolved: value, isDynamicRef: false };
  }

  const match = value.match(/^\$\{([^}]+)\}$/);
  if (!match) {
    return { resolved: value, isDynamicRef: false };
  }

  const refPath = match[1].trim();
  return { resolved: getNestedValue(context, refPath), isDynamicRef: true };
}

/**
 * Condition definition
 */
export class Condition {
  constructor({
    field, // Path to field in context (dot notation)
    operator, // Operator name
    value = null, // Value to compare against
    negate = false, // Negate the result
  }) {
    this.field = field;
    this.operator = operator;
    this.value = value;
    this.negate = negate;
  }

  /**
   * Evaluate the condition against a context
   */
  evaluate(context) {
    const fieldValue = getNestedValue(context, this.field);
    const operatorFn = Operators[this.operator];

    if (!operatorFn) {
      throw new Error(`Unknown operator: ${this.operator}`);
    }

    const isUnary = UnaryOperators.has(this.operator);
    let compareValue = this.value;

    if (!isUnary) {
      const { resolved, isDynamicRef } = resolveConditionValue(this.value, context);
      compareValue = resolved;

      // Missing dynamic references are treated as non-matches (safe default).
      // This prevents false positives (e.g. neq against an undefined compare value).
      if (isDynamicRef && compareValue === undefined) {
        return false;
      }
    }

    let result = isUnary ? operatorFn(fieldValue) : operatorFn(fieldValue, compareValue);

    if (this.negate) {
      result = !result;
    }

    return result;
  }

  /**
   * Evaluate and return detailed result for explainable policy decisions.
   * @param {Object} context
   * @returns {{ matched: boolean, field: string, operator: string, expectedValue: *, actualValue: * }}
   */
  evaluateWithDetail(context) {
    const fieldValue = getNestedValue(context, this.field);
    const operatorFn = Operators[this.operator];

    if (!operatorFn) {
      throw new Error(`Unknown operator: ${this.operator}`);
    }

    const isUnary = UnaryOperators.has(this.operator);
    let compareValue = this.value;

    if (!isUnary) {
      const { resolved, isDynamicRef } = resolveConditionValue(this.value, context);
      compareValue = resolved;

      if (isDynamicRef && compareValue === undefined) {
        return {
          matched: false,
          field: this.field,
          operator: this.operator,
          expectedValue: this.value,
          actualValue: fieldValue,
        };
      }
    }

    let result = isUnary ? operatorFn(fieldValue) : operatorFn(fieldValue, compareValue);
    if (this.negate) result = !result;

    return {
      matched: result,
      field: this.field,
      operator: this.operator,
      expectedValue: isUnary ? null : compareValue,
      actualValue: fieldValue,
    };
  }

  toJSON() {
    return {
      field: this.field,
      operator: this.operator,
      value: this.value,
      negate: this.negate,
    };
  }
}

/**
 * Condition group with AND/OR logic
 */
export class ConditionGroup {
  constructor({
    logic = 'and', // 'and' or 'or'
    conditions = [], // Array of Condition or ConditionGroup
  }) {
    this.logic = logic;
    this.conditions = conditions.map((c) => {
      if (c.conditions) {
        return new ConditionGroup(c);
      }
      return new Condition(c);
    });
  }

  /**
   * Evaluate the condition group
   */
  evaluate(context) {
    if (this.conditions.length === 0) return true;

    if (this.logic === 'and') {
      return this.conditions.every((c) => c.evaluate(context));
    } else {
      return this.conditions.some((c) => c.evaluate(context));
    }
  }

  /**
   * Evaluate all conditions and return detail for each.
   * @param {Object} context
   * @returns {{ matched: boolean, details: Array }}
   */
  evaluateWithDetail(context) {
    if (this.conditions.length === 0) return { matched: true, details: [] };

    const details = this.conditions.map((c) => {
      if (typeof c.evaluateWithDetail === 'function') {
        return c.evaluateWithDetail(context);
      }
      return { matched: c.evaluate(context) };
    });

    const matched =
      this.logic === 'and' ? details.every((d) => d.matched) : details.some((d) => d.matched);

    return { matched, details };
  }

  toJSON() {
    return {
      logic: this.logic,
      conditions: this.conditions.map((c) => c.toJSON()),
    };
  }
}

/**
 * Policy action definition
 */
export class PolicyAction {
  constructor({
    type, // 'allow', 'deny', 'agent', 'workflow', 'notify', 'transform'
    agent = null, // Agent to invoke
    request = null, // Request to send to agent
    workflow = null, // Workflow to start
    notification = null, // Notification to send
    transform = null, // Data transformation
    reason = null, // Human-readable reason for this action
    remediation = null, // Suggested fix/workaround when denied
    metadata = {},
  }) {
    this.type = type;
    this.agent = agent;
    this.request = request;
    this.workflow = workflow;
    this.notification = notification;
    this.transform = transform;
    this.reason = reason;
    this.remediation = remediation;
    this.metadata = metadata;
  }

  toJSON() {
    return {
      type: this.type,
      agent: this.agent,
      request: this.request,
      workflow: this.workflow,
      notification: this.notification,
      transform: this.transform,
      reason: this.reason,
      remediation: this.remediation,
      metadata: this.metadata,
    };
  }
}

/**
 * Policy rule definition
 */
export class PolicyRule {
  constructor({
    id = randomUUID(),
    name,
    description = '',
    enabled = true,
    priority = 0, // Higher priority rules are evaluated first
    conditions, // ConditionGroup or simple condition
    action, // PolicyAction or action config
    stopOnMatch = false, // Stop evaluating further rules if matched
    metadata = {},
  }) {
    this.id = id;
    this.name = name;
    this.description = description;
    this.enabled = enabled;
    this.priority = priority;
    this.stopOnMatch = stopOnMatch;
    this.metadata = metadata;

    // Parse conditions
    if (conditions.logic) {
      this.conditions = new ConditionGroup(conditions);
    } else if (conditions.field) {
      this.conditions = new ConditionGroup({ logic: 'and', conditions: [conditions] });
    } else if (Array.isArray(conditions)) {
      this.conditions = new ConditionGroup({ logic: 'and', conditions });
    } else {
      this.conditions = new ConditionGroup(conditions);
    }

    // Parse action
    this.action = action instanceof PolicyAction ? action : new PolicyAction(action);
  }

  /**
   * Evaluate if the rule matches
   */
  matches(context) {
    if (!this.enabled) return false;
    return this.conditions.evaluate(context);
  }

  /**
   * Evaluate if the rule matches and return condition details for explanations.
   * @param {Object} context
   * @returns {{ matched: boolean, conditionDetails: Array }}
   */
  matchesWithDetail(context) {
    if (!this.enabled) return { matched: false, conditionDetails: [] };
    const { matched, details } = this.conditions.evaluateWithDetail(context);
    return { matched, conditionDetails: details };
  }

  toJSON() {
    return {
      id: this.id,
      name: this.name,
      description: this.description,
      enabled: this.enabled,
      priority: this.priority,
      conditions: this.conditions.toJSON(),
      action: this.action.toJSON(),
      stopOnMatch: this.stopOnMatch,
      metadata: this.metadata,
    };
  }
}

/**
 * Policy set - collection of rules for a domain
 */
export class PolicySet {
  constructor({
    id = randomUUID(),
    name,
    description = '',
    domain, // 'orders', 'returns', 'inventory', etc.
    version = '1.0.0',
    rules = [],
    defaultAction = { type: 'allow' },
    metadata = {},
  }) {
    this.id = id;
    this.name = name;
    this.description = description;
    this.domain = domain;
    this.version = version;
    this.metadata = metadata;

    this.rules = rules.map((r) => (r instanceof PolicyRule ? r : new PolicyRule(r)));
    this.rules.sort((a, b) => b.priority - a.priority);

    this.defaultAction =
      defaultAction instanceof PolicyAction ? defaultAction : new PolicyAction(defaultAction);
  }

  /**
   * Evaluate all rules and return matching actions with explanations.
   */
  evaluate(context) {
    const matchedRules = [];
    const explanations = [];

    for (const rule of this.rules) {
      const { matched, conditionDetails } = rule.matchesWithDetail(context);

      if (matched) {
        matchedRules.push(rule);

        explanations.push(
          new PolicyExplanation({
            policySetId: this.id,
            policySetName: this.name,
            ruleId: rule.id,
            ruleName: rule.name,
            ruleDescription: rule.description,
            actionType: rule.action.type,
            reason: rule.action.reason || rule.action.metadata?.reason || rule.description || '',
            remediation: rule.action.remediation || rule.action.metadata?.remediation || null,
            conditions: conditionDetails.flatMap((d) => (d.details ? d.details : [d])),
          }),
        );

        if (rule.stopOnMatch) {
          break;
        }
      }
    }

    return {
      matched: matchedRules.length > 0,
      rules: matchedRules,
      actions: matchedRules.map((r) => r.action),
      explanations,
      defaultApplied: matchedRules.length === 0,
    };
  }

  toJSON() {
    return {
      id: this.id,
      name: this.name,
      description: this.description,
      domain: this.domain,
      version: this.version,
      rules: this.rules.map((r) => r.toJSON()),
      defaultAction: this.defaultAction.toJSON(),
      metadata: this.metadata,
    };
  }
}

/**
 * Policy evaluation result
 */
export class PolicyResult {
  constructor({
    policySetId,
    policySetName,
    domain,
    context,
    matched,
    rules,
    actions,
    defaultApplied,
    evaluatedAt = new Date().toISOString(),
  }) {
    this.policySetId = policySetId;
    this.policySetName = policySetName;
    this.domain = domain;
    this.context = context;
    this.matched = matched;
    this.rules = rules;
    this.actions = actions;
    this.defaultApplied = defaultApplied;
    this.evaluatedAt = evaluatedAt;
  }
}

/**
 * Structured explanation of a policy evaluation outcome.
 * Provides the full "why" of a denial/allow/transform decision.
 */
export class PolicyExplanation {
  constructor({
    policySetId,
    policySetName,
    ruleId,
    ruleName,
    ruleDescription = '',
    actionType,
    reason = '',
    remediation = null,
    conditions = [],
  }) {
    this.policySetId = policySetId;
    this.policySetName = policySetName;
    this.ruleId = ruleId;
    this.ruleName = ruleName;
    this.ruleDescription = ruleDescription;
    this.actionType = actionType;
    this.reason = reason;
    this.remediation = remediation;
    this.conditions = conditions;
  }

  /** Human-readable summary string */
  toString() {
    const parts = [`Policy "${this.policySetName}" / Rule "${this.ruleName}": ${this.actionType}`];
    if (this.reason) parts.push(`  Reason: ${this.reason}`);
    for (const c of this.conditions) {
      parts.push(
        `  - ${c.field} ${c.operator} ${JSON.stringify(c.expectedValue)} (actual: ${JSON.stringify(c.actualValue)}, matched: ${c.matched})`,
      );
    }
    if (this.remediation) parts.push(`  Remediation: ${this.remediation}`);
    return parts.join('\n');
  }

  toJSON() {
    return {
      policySetId: this.policySetId,
      policySetName: this.policySetName,
      ruleId: this.ruleId,
      ruleName: this.ruleName,
      ruleDescription: this.ruleDescription,
      actionType: this.actionType,
      reason: this.reason,
      remediation: this.remediation,
      conditions: this.conditions,
    };
  }
}

/**
 * Audit entry for a policy transform — records before/after values.
 */
export class TransformAuditEntry {
  constructor({ ruleId = null, ruleName = null, policySetId = null, field, before, after }) {
    this.ruleId = ruleId;
    this.ruleName = ruleName;
    this.policySetId = policySetId;
    this.field = field;
    this.before = before;
    this.after = after;
    this.timestamp = new Date().toISOString();
  }

  toJSON() {
    return {
      ruleId: this.ruleId,
      ruleName: this.ruleName,
      policySetId: this.policySetId,
      field: this.field,
      before: this.before,
      after: this.after,
      timestamp: this.timestamp,
    };
  }
}

/**
 * Policy Engine
 */
export class PolicyEngine extends EventEmitter {
  /**
   * @param {Object} options
   * @param {string|null} [options.storePath=null]
   * @param {Function|null} [options.executor=null] - Function to execute actions
   * @param {'allow'|'deny'} [options.unknownDomainMode='deny'] - Behavior when no policies match a domain
   */
  constructor({ storePath = null, executor = null, unknownDomainMode = 'deny' } = {}) {
    super();

    if (unknownDomainMode !== 'allow' && unknownDomainMode !== 'deny') {
      throw new Error(`unknownDomainMode must be 'allow' or 'deny', got '${unknownDomainMode}'`);
    }

    this.storePath = storePath;
    this.executor = executor;
    this.unknownDomainMode = unknownDomainMode;
    this.policySets = new Map();
    this.evaluationHistory = [];
  }

  /**
   * Load policies from storage
   */
  async load() {
    if (!this.storePath) return;

    try {
      const policiesDir = path.join(this.storePath, 'policies');
      if (!fs.existsSync(policiesDir)) return;

      const files = fs.readdirSync(policiesDir);

      for (const file of files) {
        if (!file.endsWith('.yaml') && !file.endsWith('.yml') && !file.endsWith('.json')) {
          continue;
        }

        const filePath = path.join(policiesDir, file);
        const content = fs.readFileSync(filePath, 'utf-8');

        let data;
        if (file.endsWith('.json')) {
          data = JSON.parse(content);
        } else {
          data = parseYAML(content);
        }

        const policySet = new PolicySet(data);
        this.policySets.set(policySet.id, policySet);

        // Also index by domain
        if (!this.policySets.has(`domain:${policySet.domain}`)) {
          this.policySets.set(`domain:${policySet.domain}`, []);
        }
        this.policySets.get(`domain:${policySet.domain}`).push(policySet);
      }

      this.emit('loaded', { policySetCount: this.policySets.size });
    } catch (error) {
      this.emit('error', { type: 'load', error });
    }
  }

  /**
   * Save policies to storage
   */
  async save() {
    if (!this.storePath) return;

    try {
      const policiesDir = path.join(this.storePath, 'policies');
      fs.mkdirSync(policiesDir, { recursive: true });

      for (const [id, policySet] of this.policySets) {
        if (id.startsWith('domain:')) continue; // Skip domain indexes

        const filePath = path.join(
          policiesDir,
          `${policySet.domain}-${policySet.id.slice(0, 8)}.json`,
        );
        fs.writeFileSync(filePath, JSON.stringify(policySet.toJSON(), null, 2));
      }

      this.emit('saved');
    } catch (error) {
      this.emit('error', { type: 'save', error });
    }
  }

  /**
   * Register a policy set
   */
  registerPolicySet(config) {
    const policySet = config instanceof PolicySet ? config : new PolicySet(config);

    this.policySets.set(policySet.id, policySet);

    // Index by domain
    const domainKey = `domain:${policySet.domain}`;
    if (!this.policySets.has(domainKey)) {
      this.policySets.set(domainKey, []);
    }
    const domainPolicies = this.policySets.get(domainKey);
    const existingIndex = domainPolicies.findIndex((p) => p.id === policySet.id);
    if (existingIndex >= 0) {
      domainPolicies[existingIndex] = policySet;
    } else {
      domainPolicies.push(policySet);
    }

    this.emit('policySet:registered', { policySet: policySet.toJSON() });
    this.save();

    return policySet;
  }

  /**
   * Get a policy set by ID
   */
  getPolicySet(policySetId) {
    return this.policySets.get(policySetId);
  }

  /**
   * Get policy sets for a domain
   */
  getPoliciesForDomain(domain) {
    return this.policySets.get(`domain:${domain}`) || [];
  }

  /**
   * List all policy sets
   */
  listPolicySets() {
    return Array.from(this.policySets.values())
      .filter((p) => p instanceof PolicySet)
      .map((p) => p.toJSON());
  }

  /**
   * Evaluate policies for a domain.
   *
   * Precedence: explicit deny > explicit allow > default (deny-overrides).
   *
   * @param {string} domain
   * @param {Object} context
   * @param {Object} [options]
   * @param {boolean} [options.dryRun=false] - If true, skip history recording
   * @returns {Promise<Object>}
   */
  async evaluate(domain, context, options = {}) {
    const { dryRun = false } = options;
    const policySets = this.getPoliciesForDomain(domain);
    const allResults = [];
    const allActions = [];
    const allExplanations = [];

    // When no policies exist for this domain, consult unknownDomainMode
    if (policySets.length === 0) {
      const shouldAllow = this.unknownDomainMode === 'allow';

      if (!dryRun) {
        this.evaluationHistory.push({
          timestamp: new Date().toISOString(),
          domain,
          context,
          results: [],
          explanations: [],
          unknownDomain: true,
          mode: this.unknownDomainMode,
        });
        if (this.evaluationHistory.length > 1000) {
          this.evaluationHistory = this.evaluationHistory.slice(-1000);
        }
      }

      this.emit('evaluated', {
        domain,
        context,
        results: [],
        explanations: [],
        unknownDomain: true,
        mode: this.unknownDomainMode,
      });

      return {
        domain,
        context: dryRun ? context : undefined,
        results: [],
        actions: [],
        explanations: [],
        shouldAllow,
        shouldDeny: !shouldAllow,
        dryRun,
        unknownDomain: true,
        unknownDomainMode: this.unknownDomainMode,
        reason: shouldAllow
          ? `No policies for domain '${domain}'; unknownDomainMode='allow' — passing through`
          : `No policies for domain '${domain}'; unknownDomainMode='deny' — blocking`,
      };
    }

    for (const policySet of policySets) {
      const evalResult = policySet.evaluate(context);

      const result = new PolicyResult({
        policySetId: policySet.id,
        policySetName: policySet.name,
        domain,
        context: dryRun ? context : undefined,
        matched: evalResult.matched,
        rules: evalResult.rules.map((r) => ({ id: r.id, name: r.name })),
        actions: evalResult.actions.map((a) => a.toJSON()),
        defaultApplied: evalResult.defaultApplied,
      });

      allResults.push(result);

      if (evalResult.matched) {
        allActions.push(...evalResult.actions);
        allExplanations.push(...evalResult.explanations);
      } else if (evalResult.defaultApplied) {
        allActions.push(policySet.defaultAction);
      }
    }

    // Deny-overrides precedence
    const hasDeny = allActions.some((a) => a.type === 'deny');
    const hasAllow = allActions.some((a) => a.type === 'allow');

    // Store in history (skip for dry-run)
    if (!dryRun) {
      this.evaluationHistory.push({
        timestamp: new Date().toISOString(),
        domain,
        context,
        results: allResults,
        explanations: allExplanations.map((e) => e.toJSON()),
      });

      // Keep last 1000 evaluations
      if (this.evaluationHistory.length > 1000) {
        this.evaluationHistory = this.evaluationHistory.slice(-1000);
      }
    }

    this.emit('evaluated', { domain, context, results: allResults, explanations: allExplanations });

    return {
      domain,
      context: dryRun ? context : undefined,
      results: allResults,
      actions: allActions,
      explanations: allExplanations,
      shouldAllow: !hasDeny && (hasAllow || allActions.length === 0),
      shouldDeny: hasDeny,
      dryRun,
    };
  }

  /**
   * Dry-run: evaluate policies without recording history.
   * Returns the full evaluation result including explanations.
   * @param {string} domain
   * @param {Object} context
   * @returns {Promise<Object>}
   */
  async evaluateDryRun(domain, context) {
    return this.evaluate(domain, context, { dryRun: true });
  }

  /**
   * Evaluate and execute actions
   */
  async evaluateAndExecute(domain, context) {
    const evaluation = await this.evaluate(domain, context);

    if (evaluation.shouldDeny) {
      this.emit('denied', { domain, context, evaluation });
      return { allowed: false, evaluation, executed: [] };
    }

    const executed = [];

    for (const action of evaluation.actions) {
      if (action.type === 'allow' || action.type === 'deny') {
        continue; // These are decision actions, not execution actions
      }

      try {
        let result = null;

        if (this.executor) {
          result = await this.executor(action, context);
        }

        executed.push({ action: action.toJSON(), result, success: true });
        this.emit('action:executed', { action, context, result });
      } catch (error) {
        executed.push({ action: action.toJSON(), error: error.message, success: false });
        this.emit('action:failed', { action, context, error });
      }
    }

    return { allowed: true, evaluation, executed };
  }

  /**
   * Get evaluation history
   */
  getHistory({ domain = null, limit = 100 } = {}) {
    let history = this.evaluationHistory;

    if (domain) {
      history = history.filter((h) => h.domain === domain);
    }

    return history.slice(-limit);
  }

  /**
   * Get engine status
   */
  getStatus() {
    const policySets = Array.from(this.policySets.values()).filter((p) => p instanceof PolicySet);

    const byDomain = {};
    for (const ps of policySets) {
      byDomain[ps.domain] = (byDomain[ps.domain] || 0) + 1;
    }

    return {
      totalPolicySets: policySets.length,
      totalRules: policySets.reduce((sum, ps) => sum + ps.rules.length, 0),
      byDomain,
      recentEvaluations: this.evaluationHistory.slice(-10),
    };
  }
}

/**
 * Pre-defined policy templates for common commerce scenarios
 */
export const PolicyTemplates = {
  // Auto-approve returns under $100 for VIP customers
  autoApproveReturns: {
    name: 'Auto-Approve Small Returns',
    description: 'Automatically approve returns under $100 for customers with high lifetime value',
    domain: 'returns',
    rules: [
      {
        name: 'auto_approve_small_vip_returns',
        description: 'Auto-approve returns < $100 for VIP customers',
        priority: 100,
        conditions: {
          logic: 'and',
          conditions: [
            { field: 'return.value', operator: 'lt', value: 100 },
            { field: 'customer.lifetimeValue', operator: 'gt', value: 500 },
            { field: 'customer.returnRate', operator: 'lt', value: 0.1 },
          ],
        },
        action: {
          type: 'agent',
          agent: 'returns',
          request: 'Approve return {return.id} - auto-approved per policy',
        },
        stopOnMatch: true,
      },
      {
        name: 'flag_high_value_returns',
        description: 'Flag high-value returns for manual review',
        priority: 50,
        conditions: {
          logic: 'or',
          conditions: [
            { field: 'return.value', operator: 'gte', value: 500 },
            { field: 'customer.returnRate', operator: 'gte', value: 0.2 },
          ],
        },
        action: {
          type: 'workflow',
          workflow: 'returnProcessing',
          metadata: { requiresApproval: true },
        },
      },
    ],
    defaultAction: { type: 'allow' },
  },

  // Inventory restock triggers
  inventoryRestock: {
    name: 'Inventory Restock Rules',
    description: 'Automatically trigger restock when inventory is low',
    domain: 'inventory',
    rules: [
      {
        name: 'critical_stock_alert',
        description: 'Create urgent PO when stock is critically low',
        priority: 100,
        conditions: {
          logic: 'and',
          conditions: [
            { field: 'inventory.quantity', operator: 'lte', value: 5 },
            { field: 'inventory.reorderPoint', operator: 'gt', value: 0 },
          ],
        },
        action: {
          type: 'agent',
          agent: 'suppliers',
          request: 'Create urgent purchase order for SKU {inventory.sku} - critical stock level',
        },
        stopOnMatch: true,
      },
      {
        name: 'low_stock_reorder',
        description: 'Create standard PO when below reorder point',
        priority: 50,
        conditions: {
          field: 'inventory.quantity',
          operator: 'lte',
          value: '${inventory.reorderPoint}', // Dynamic reference
        },
        action: {
          type: 'agent',
          agent: 'suppliers',
          request:
            'Create purchase order for SKU {inventory.sku} to restock to {inventory.targetQuantity}',
        },
      },
    ],
    defaultAction: { type: 'allow' },
  },

  // Order fraud detection
  orderFraudDetection: {
    name: 'Order Fraud Detection',
    description: 'Flag potentially fraudulent orders',
    domain: 'orders',
    rules: [
      {
        name: 'high_value_new_customer',
        description: 'Flag high-value orders from new customers',
        priority: 100,
        conditions: {
          logic: 'and',
          conditions: [
            { field: 'order.total', operator: 'gt', value: 1000 },
            { field: 'customer.orderCount', operator: 'lt', value: 2 },
          ],
        },
        action: {
          type: 'workflow',
          workflow: 'orderFulfillment',
          metadata: { requiresReview: true, riskLevel: 'high' },
        },
      },
      {
        name: 'velocity_check',
        description: 'Flag multiple orders in short time',
        priority: 90,
        conditions: {
          logic: 'and',
          conditions: [
            { field: 'customer.ordersLast24h', operator: 'gt', value: 3 },
            { field: 'order.total', operator: 'gt', value: 200 },
          ],
        },
        action: {
          type: 'notify',
          notification: {
            channel: 'slack',
            message:
              'Velocity alert: Customer {customer.id} placed {customer.ordersLast24h} orders in 24h',
          },
        },
      },
      {
        name: 'shipping_billing_mismatch',
        description: 'Flag orders with mismatched addresses',
        priority: 80,
        conditions: {
          logic: 'and',
          conditions: [
            {
              field: 'order.shippingAddress.country',
              operator: 'neq',
              value: '${order.billingAddress.country}',
            },
            { field: 'order.total', operator: 'gt', value: 500 },
          ],
        },
        action: {
          type: 'workflow',
          workflow: 'orderFulfillment',
          metadata: { requiresReview: true, riskLevel: 'medium' },
        },
      },
    ],
    defaultAction: { type: 'allow' },
  },

  // Promotion eligibility
  promotionEligibility: {
    name: 'Promotion Eligibility Rules',
    description: 'Determine promotion eligibility and stacking',
    domain: 'promotions',
    rules: [
      {
        name: 'vip_exclusive',
        description: 'Allow VIP-only promotions',
        priority: 100,
        conditions: {
          logic: 'and',
          conditions: [
            { field: 'promotion.vipOnly', operator: 'isTrue' },
            { field: 'customer.tier', operator: 'in', value: ['gold', 'platinum'] },
          ],
        },
        action: { type: 'allow' },
      },
      {
        name: 'block_vip_for_regular',
        description: 'Block VIP promotions for regular customers',
        priority: 99,
        conditions: {
          logic: 'and',
          conditions: [
            { field: 'promotion.vipOnly', operator: 'isTrue' },
            { field: 'customer.tier', operator: 'notIn', value: ['gold', 'platinum'] },
          ],
        },
        action: { type: 'deny' },
        stopOnMatch: true,
      },
      {
        name: 'no_double_discount',
        description: 'Prevent stacking percentage discounts',
        priority: 50,
        conditions: {
          logic: 'and',
          conditions: [
            { field: 'cart.hasPercentageDiscount', operator: 'isTrue' },
            { field: 'promotion.type', operator: 'eq', value: 'percentage' },
          ],
        },
        action: { type: 'deny' },
        stopOnMatch: true,
      },
    ],
    defaultAction: { type: 'allow' },
  },

  // Subscription management
  subscriptionRules: {
    name: 'Subscription Management Rules',
    description: 'Handle subscription lifecycle events',
    domain: 'subscriptions',
    rules: [
      {
        name: 'auto_cancel_failed_payments',
        description: 'Cancel subscription after 3 failed payments',
        priority: 100,
        conditions: {
          field: 'subscription.consecutiveFailedPayments',
          operator: 'gte',
          value: 3,
        },
        action: {
          type: 'agent',
          agent: 'subscriptions',
          request: 'Cancel subscription {subscription.id} due to payment failures',
        },
      },
      {
        name: 'offer_discount_on_cancel',
        description: 'Offer discount when long-term customer cancels',
        priority: 80,
        conditions: {
          logic: 'and',
          conditions: [
            { field: 'event', operator: 'eq', value: 'cancellation_requested' },
            { field: 'subscription.monthsActive', operator: 'gte', value: 6 },
          ],
        },
        action: {
          type: 'agent',
          agent: 'subscriptions',
          request:
            'Offer 20% retention discount to customer {customer.id} for subscription {subscription.id}',
        },
      },
    ],
    defaultAction: { type: 'allow' },
  },
};

export default PolicyEngine;
