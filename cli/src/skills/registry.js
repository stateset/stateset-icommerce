/**
 * Skill Registry for StateSet iCommerce
 *
 * Central registry of all loaded skills with search, category filtering,
 * and context retrieval for agent prompt injection.
 */

// ============================================================================
// Category Map
// ============================================================================

const CATEGORY_MAP = {
  // Core Commerce
  'commerce-engine-setup': 'core',
  'commerce-embedded-sdk': 'core',
  'commerce-customers': 'core',
  'commerce-products': 'core',
  'commerce-orders': 'core',
  'commerce-checkout': 'core',
  'commerce-payments': 'core',
  'commerce-storefront': 'core',
  'commerce-customer-service': 'core',
  // Inventory & Fulfillment
  'commerce-inventory': 'fulfillment',
  'commerce-shipments': 'fulfillment',
  'commerce-returns': 'fulfillment',
  'commerce-backorders': 'fulfillment',
  'commerce-fulfillment': 'fulfillment',
  'commerce-receiving': 'fulfillment',
  'commerce-warehouse': 'fulfillment',
  'commerce-lots-and-serials': 'fulfillment',
  // Financial
  'commerce-invoices': 'financial',
  'commerce-tax': 'financial',
  'commerce-currency': 'financial',
  'commerce-accounts-payable': 'financial',
  'commerce-accounts-receivable': 'financial',
  'commerce-cost-accounting': 'financial',
  'commerce-credit': 'financial',
  'commerce-general-ledger': 'financial',
  // Marketing & Subscriptions
  'commerce-promotions': 'marketing',
  'commerce-subscriptions': 'marketing',
  // Supply Chain & Manufacturing
  'commerce-suppliers': 'supply-chain',
  'commerce-manufacturing': 'supply-chain',
  'commerce-warranties': 'supply-chain',
  'commerce-quality': 'supply-chain',
  // Analytics & Search
  'commerce-analytics': 'analytics',
  'commerce-vector-search': 'analytics',
  'commerce-events': 'analytics',
  // Platform & Automation
  'commerce-sync': 'platform',
  'commerce-autonomous-engine': 'platform',
  'commerce-autonomous-runbook': 'platform',
  'commerce-mcp-tools': 'platform',
};

// ============================================================================
// Types
// ============================================================================

/**
 * @typedef {Object} SkillEntry
 * @property {string} name
 * @property {string} description
 * @property {string} category
 * @property {string[]} tags
 * @property {string} origin
 * @property {string} dirPath
 * @property {import('./parser.js').ParsedSkill} parsed
 * @property {boolean} hasReferences
 * @property {boolean} hasScripts
 */

// ============================================================================
// Tag Generation
// ============================================================================

/**
 * Generate tags from a skill's metadata.
 *
 * @param {import('./loader.js').DiscoveredSkill} discovered
 * @returns {string[]}
 */
function generateTags(discovered) {
  const tags = new Set();
  const { parsed } = discovered;

  // From skill name (strip commerce- prefix, split on hyphens)
  const nameParts = parsed.name.replace(/^commerce-/, '').split('-');
  for (const p of nameParts) tags.add(p);

  // From description keywords
  const descWords = parsed.description.toLowerCase().split(/\s+/);
  const stopWords = new Set([
    'and',
    'the',
    'for',
    'use',
    'when',
    'or',
    'a',
    'an',
    'in',
    'to',
    'of',
    'is',
    'on',
    'at',
  ]);
  for (const w of descWords) {
    const clean = w.replace(/[^a-z0-9-]/g, '');
    if (clean.length > 2 && !stopWords.has(clean)) {
      tags.add(clean);
    }
  }

  // From MCP tool prefixes
  for (const tool of parsed.mcpTools) {
    const verb = tool.split('_')[0];
    tags.add(verb);
  }

  // From CLI commands
  for (const cmd of parsed.cliCommands) {
    tags.add(cmd.replace('stateset-', ''));
  }

  return [...tags].sort();
}

// ============================================================================
// SkillRegistry
// ============================================================================

export class SkillRegistry {
  constructor() {
    /** @type {Map<string, SkillEntry>} */
    this._skills = new Map();
  }

  /**
   * Load skills from an array of discovered skills.
   *
   * @param {import('./loader.js').DiscoveredSkill[]} discovered
   */
  loadFromDiscovered(discovered) {
    for (const d of discovered) {
      this.addSkill({
        name: d.name,
        description: d.parsed.description,
        category: CATEGORY_MAP[d.name] || 'other',
        tags: generateTags(d),
        origin: d.origin,
        dirPath: d.dirPath,
        parsed: d.parsed,
        hasReferences: d.hasReferences,
        hasScripts: d.hasScripts,
      });
    }
  }

  /**
   * Add a single skill entry.
   *
   * @param {SkillEntry} entry
   */
  addSkill(entry) {
    this._skills.set(entry.name, entry);
  }

  /**
   * Remove a skill by name.
   *
   * @param {string} name
   * @returns {boolean}
   */
  removeSkill(name) {
    return this._skills.delete(name);
  }

  /**
   * Get a skill by name.
   *
   * @param {string} name
   * @returns {SkillEntry|null}
   */
  get(name) {
    return this._skills.get(name) || null;
  }

  /**
   * Check if a skill exists.
   *
   * @param {string} name
   * @returns {boolean}
   */
  has(name) {
    return this._skills.has(name);
  }

  /**
   * List all skills, optionally filtered.
   *
   * @param {Object} [opts]
   * @param {string} [opts.category]
   * @param {string} [opts.origin]
   * @returns {SkillEntry[]}
   */
  list(opts = {}) {
    let skills = [...this._skills.values()];

    if (opts.category) {
      skills = skills.filter((s) => s.category === opts.category);
    }
    if (opts.origin) {
      skills = skills.filter((s) => s.origin === opts.origin);
    }

    return skills.sort((a, b) => a.name.localeCompare(b.name));
  }

  /**
   * Search skills by query string.
   * Scores results by relevance: exact name > name contains > description contains > tag match.
   *
   * @param {string} query
   * @returns {SkillEntry[]}
   */
  search(query) {
    if (!query || !query.trim()) return this.list();

    const q = query.toLowerCase().trim();
    const terms = q.split(/\s+/);

    const scored = [];

    for (const skill of this._skills.values()) {
      let score = 0;
      const nameLower = skill.name.toLowerCase();
      const descLower = skill.description.toLowerCase();

      // Exact name match
      if (nameLower === q || nameLower === `commerce-${q}`) {
        score += 100;
      }

      // Name contains query
      if (nameLower.includes(q)) {
        score += 50;
      }

      // Description contains query
      if (descLower.includes(q)) {
        score += 30;
      }

      // Individual term matches
      for (const term of terms) {
        if (nameLower.includes(term)) score += 10;
        if (descLower.includes(term)) score += 5;
        if (skill.category === term) score += 15;
        if (skill.tags.some((t) => t.includes(term))) score += 8;
        if (skill.parsed.mcpTools.some((t) => t.includes(term))) score += 12;
      }

      if (score > 0) {
        scored.push({ skill, score });
      }
    }

    scored.sort((a, b) => b.score - a.score);
    return scored.map((s) => s.skill);
  }

  /**
   * Get unique categories.
   *
   * @returns {string[]}
   */
  getCategories() {
    const cats = new Set();
    for (const skill of this._skills.values()) {
      cats.add(skill.category);
    }
    return [...cats].sort();
  }

  /**
   * List skills in a specific category.
   *
   * @param {string} category
   * @returns {SkillEntry[]}
   */
  listByCategory(category) {
    return this.list({ category });
  }

  /**
   * Get the markdown body of a skill for prompt injection.
   *
   * @param {string} name
   * @returns {string|null}
   */
  getSkillBody(name) {
    const skill = this._skills.get(name);
    return skill ? skill.parsed.body : null;
  }

  /**
   * Get multiple skill bodies.
   *
   * @param {string[]} names
   * @returns {Map<string, string>}
   */
  getSkillBodies(names) {
    const result = new Map();
    for (const name of names) {
      const body = this.getSkillBody(name);
      if (body) result.set(name, body);
    }
    return result;
  }

  /**
   * Find skills relevant to a user message.
   * Matches against descriptions, tool names, and CLI commands.
   *
   * @param {string} text
   * @param {number} [limit=3]
   * @returns {SkillEntry[]}
   */
  findRelevantSkills(text, limit = 3) {
    if (!text) return [];
    return this.search(text).slice(0, limit);
  }

  /**
   * Get total skill count.
   *
   * @returns {number}
   */
  count() {
    return this._skills.size;
  }

  /**
   * Get registry statistics.
   *
   * @returns {{ total: number, bundled: number, installed: number, workspace: number, categories: Object<string, number> }}
   */
  getStats() {
    const stats = {
      total: this._skills.size,
      bundled: 0,
      installed: 0,
      workspace: 0,
      categories: {},
    };

    for (const skill of this._skills.values()) {
      if (skill.origin === 'bundled') stats.bundled++;
      else if (skill.origin === 'installed') stats.installed++;
      else if (skill.origin === 'workspace') stats.workspace++;

      stats.categories[skill.category] = (stats.categories[skill.category] || 0) + 1;
    }

    return stats;
  }

  /**
   * Export registry as a serializable catalog.
   *
   * @returns {Object}
   */
  toJSON() {
    return {
      version: '1.0.0',
      generatedAt: new Date().toISOString(),
      skills: this.list().map((s) => ({
        name: s.name,
        description: s.description,
        category: s.category,
        tags: s.tags,
        origin: s.origin,
        hasReferences: s.hasReferences,
        hasScripts: s.hasScripts,
        sections: s.parsed.sections,
        mcpTools: s.parsed.mcpTools,
        cliCommands: s.parsed.cliCommands,
      })),
    };
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the shared SkillRegistry singleton.
 *
 * @returns {SkillRegistry}
 */
export function getSkillRegistry() {
  if (!_instance) {
    _instance = new SkillRegistry();
  }
  return _instance;
}

/**
 * Reset the singleton (for testing).
 */
export function resetSkillRegistry() {
  _instance = null;
}

export { CATEGORY_MAP };
