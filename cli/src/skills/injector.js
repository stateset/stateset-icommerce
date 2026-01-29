/**
 * Skill Context Injector for StateSet iCommerce
 *
 * Hooks into the HookRunner's `before_agent_start` sequential hook
 * to inject relevant skill knowledge into agent prompts.
 */

import { getPluginRegistry } from '../channels/plugin-api.js';

// ============================================================================
// Agent-to-Skill Mapping
// ============================================================================

const AGENT_SKILL_MAP = {
  'orders': ['commerce-orders'],
  'checkout': ['commerce-checkout', 'commerce-payments'],
  'inventory': ['commerce-inventory'],
  'returns': ['commerce-returns'],
  'analytics': ['commerce-analytics'],
  'promotions': ['commerce-promotions'],
  'subscriptions': ['commerce-subscriptions'],
  'customer-service': ['commerce-customer-service', 'commerce-orders', 'commerce-returns'],
  'storefront': ['commerce-storefront'],
  'manufacturing': ['commerce-manufacturing'],
  'payments': ['commerce-payments'],
  'shipments': ['commerce-shipments'],
  'suppliers': ['commerce-suppliers'],
  'invoices': ['commerce-invoices'],
  'warranties': ['commerce-warranties'],
  'currency': ['commerce-currency'],
  'tax': ['commerce-tax'],
  'sync': ['commerce-sync'],
  'autonomous': ['commerce-autonomous-engine', 'commerce-autonomous-runbook'],
};

// ============================================================================
// SkillInjector
// ============================================================================

export class SkillInjector {
  /**
   * @param {import('./registry.js').SkillRegistry} registry
   */
  constructor(registry) {
    this._registry = registry;
    this._maxSkills = 3;
    this._maxBodyLength = 2000;
  }

  /**
   * Hook handler for `before_agent_start`.
   * Finds relevant skills and prepends their content to the prompt.
   *
   * @param {Object} data - { text, session }
   * @returns {Object} Modified data with skill context
   */
  async injectSkillContext(data) {
    if (!data.text) return data;

    const agentName = data.session?.agent || null;
    const selectedNames = this.selectSkills(data.text, agentName);

    if (selectedNames.length === 0) return data;

    const contextBlock = this.formatSkillsForPrompt(selectedNames);
    if (!contextBlock) return data;

    return {
      ...data,
      text: `${contextBlock}\n\n${data.text}`,
    };
  }

  /**
   * Select skills relevant to the user's message.
   *
   * @param {string} text - User message
   * @param {string} [agentName] - Current agent name
   * @returns {string[]} Skill names to inject
   */
  selectSkills(text, agentName) {
    const selected = new Set();

    // 1. Agent-mapped skills (always include if agent is known)
    if (agentName && AGENT_SKILL_MAP[agentName]) {
      for (const name of AGENT_SKILL_MAP[agentName]) {
        if (this._registry.has(name)) {
          selected.add(name);
        }
      }
    }

    // 2. Text-based matching (fill remaining slots)
    if (selected.size < this._maxSkills) {
      const relevant = this._registry.findRelevantSkills(text, this._maxSkills);
      for (const skill of relevant) {
        if (selected.size >= this._maxSkills) break;
        selected.add(skill.name);
      }
    }

    return [...selected].slice(0, this._maxSkills);
  }

  /**
   * Format selected skills into a prompt context block.
   *
   * @param {string[]} names - Skill names
   * @returns {string|null}
   */
  formatSkillsForPrompt(names) {
    const blocks = [];

    for (const name of names) {
      let body = this._registry.getSkillBody(name);
      if (!body) continue;

      // Strip ## References section (contains file paths irrelevant to agent)
      body = body.replace(/## References[\s\S]*$/m, '').trim();

      // Truncate if too long
      if (body.length > this._maxBodyLength) {
        body = body.slice(0, this._maxBodyLength) + '\n...(truncated)';
      }

      blocks.push(`## Skill: ${name}\n${body}`);
    }

    if (blocks.length === 0) return null;

    return `<skills-context>\n${blocks.join('\n\n')}\n</skills-context>`;
  }

  /**
   * Set maximum number of skills to inject per request.
   *
   * @param {number} n
   */
  setMaxSkills(n) {
    this._maxSkills = Math.max(1, Math.min(n, 10));
  }

  /**
   * Set maximum body length per skill.
   *
   * @param {number} n
   */
  setMaxBodyLength(n) {
    this._maxBodyLength = Math.max(200, n);
  }
}

// ============================================================================
// Factory & Hook Registration
// ============================================================================

/**
 * Create a SkillInjector bound to a registry.
 *
 * @param {import('./registry.js').SkillRegistry} registry
 * @returns {SkillInjector}
 */
export function createSkillInjector(registry) {
  return new SkillInjector(registry);
}

/**
 * Register skill injection hooks with the HookRunner.
 *
 * @param {import('./registry.js').SkillRegistry} registry
 * @returns {SkillInjector}
 */
export function registerSkillHooks(registry) {
  const injector = createSkillInjector(registry);
  const hookRunner = getPluginRegistry().getHookRunner();

  hookRunner.add('before_agent_start', (data) => injector.injectSkillContext(data), {
    priority: 10, // Run early so skills context is available to other hooks
    pluginId: '__skill-injector',
  });

  return injector;
}

export { AGENT_SKILL_MAP };
