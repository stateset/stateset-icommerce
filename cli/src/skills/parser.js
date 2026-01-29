/**
 * SKILL.md Parser for StateSet iCommerce
 *
 * Parses SKILL.md files with YAML frontmatter + markdown body.
 * Extracts metadata, sections, MCP tool references, and CLI commands.
 */

import fs from 'fs';
import { parse as parseYaml } from 'yaml';

// ============================================================================
// Types
// ============================================================================

/**
 * @typedef {Object} ParsedSkill
 * @property {string} name - From frontmatter
 * @property {string} description - From frontmatter
 * @property {string} body - Full markdown body after frontmatter
 * @property {string} title - First # heading in body
 * @property {string[]} sections - ## heading names
 * @property {string[]} mcpTools - MCP tool names found in body
 * @property {string[]} cliCommands - CLI commands found in body
 * @property {Object} raw - Raw frontmatter object
 */

// ============================================================================
// Frontmatter Extraction
// ============================================================================

const FRONTMATTER_RE = /^---\s*\n([\s\S]*?)\n---\s*\n?([\s\S]*)$/;

/**
 * Split content into frontmatter and body.
 *
 * @param {string} content - Raw SKILL.md content
 * @returns {{ frontmatter: Object|null, body: string, error?: string }}
 */
export function extractFrontmatter(content) {
  if (!content || typeof content !== 'string') {
    return { frontmatter: null, body: '', error: 'Empty or non-string content' };
  }

  const match = content.match(FRONTMATTER_RE);
  if (!match) {
    // No frontmatter — treat entire content as body
    return { frontmatter: null, body: content.trim(), error: 'No YAML frontmatter found' };
  }

  try {
    const frontmatter = parseYaml(match[1]);
    if (!frontmatter || typeof frontmatter !== 'object') {
      return { frontmatter: null, body: match[2].trim(), error: 'Frontmatter is not an object' };
    }
    return { frontmatter, body: match[2].trim() };
  } catch (err) {
    return { frontmatter: null, body: match[2].trim(), error: `YAML parse error: ${err.message}` };
  }
}

// ============================================================================
// Body Analysis
// ============================================================================

const TITLE_RE = /^#\s+(.+)$/m;
const SECTION_RE = /^##\s+(.+)$/gm;
const MCP_TOOL_RE = /`((?:list|get|create|update|delete|set|adjust|reserve|confirm|release|approve|reject|send|record|calculate|convert|format|enable|validate|apply|complete|cancel|abandon|pause|resume|skip|activate|deactivate|archive|deliver|ship|start|add|remove|vector_search|vector_index|vector_clear|vector_stats|sync_status|sync_pull|sync_outbox|sync_entity_history|sync_conflicts|get_exchange_rate|list_exchange_rates|get_currency_settings|set_base_currency|enable_currencies|format_currency|get_tax_rate|list_tax_jurisdictions|list_tax_rates|get_tax_settings|get_us_state_tax_info|get_customer_tax_exemptions|get_agent_wallet|get_wallet_balance|create_stablecoin_payment|list_supported_chains|get_sales_summary|get_top_products|get_customer_metrics|get_top_customers|get_inventory_health|get_low_stock_items|get_demand_forecast|get_revenue_forecast|get_order_status_breakdown|get_return_metrics|get_overdue_invoices|get_abandoned_carts|get_active_promotions|get_shipping_rates|get_stock|get_billing_cycle|get_subscription_events|list_billing_cycles)_?[a-z_]*)`/g;
const CLI_CMD_RE = /`(stateset(?:-[a-z-]+)?)`/g;

/**
 * Extract structured data from the markdown body.
 *
 * @param {string} body
 * @returns {{ title: string, sections: string[], mcpTools: string[], cliCommands: string[] }}
 */
function analyzeBody(body) {
  const titleMatch = body.match(TITLE_RE);
  const title = titleMatch ? titleMatch[1].trim() : '';

  const sections = [];
  let m;
  while ((m = SECTION_RE.exec(body)) !== null) {
    sections.push(m[1].trim());
  }

  // Extract MCP tool names from backtick-quoted references
  const mcpTools = new Set();
  const toolMatches = body.matchAll(/`([a-z][a-z0-9_]+)`/g);
  for (const tm of toolMatches) {
    const name = tm[1];
    // Heuristic: MCP tools have underscores and start with a known verb or noun
    if (name.includes('_') && /^(list|get|create|update|delete|set|adjust|reserve|confirm|release|approve|reject|send|record|calculate|convert|format|enable|validate|apply|complete|cancel|abandon|pause|resume|skip|activate|deactivate|archive|deliver|ship|start|add|remove|vector|sync)/.test(name)) {
      mcpTools.add(name);
    }
  }

  const cliCommands = new Set();
  const cliMatches = body.matchAll(CLI_CMD_RE);
  for (const cm of cliMatches) {
    cliCommands.add(cm[1]);
  }

  return {
    title,
    sections,
    mcpTools: [...mcpTools].sort(),
    cliCommands: [...cliCommands].sort(),
  };
}

// ============================================================================
// Public API
// ============================================================================

/**
 * Parse a SKILL.md file from disk.
 *
 * @param {string} filePath - Absolute path to SKILL.md
 * @returns {ParsedSkill|null}
 */
export function parseSkillMd(filePath) {
  try {
    const content = fs.readFileSync(filePath, 'utf-8');
    return parseSkillContent(content);
  } catch (err) {
    console.warn(`[SkillParser] Failed to read ${filePath}: ${err.message}`);
    return null;
  }
}

/**
 * Parse SKILL.md content from a string.
 *
 * @param {string} content
 * @returns {ParsedSkill|null}
 */
export function parseSkillContent(content) {
  const { frontmatter, body, error } = extractFrontmatter(content);

  if (!frontmatter) {
    console.warn(`[SkillParser] ${error || 'Missing frontmatter'}`);
    return null;
  }

  const name = frontmatter.name;
  const description = frontmatter.description;

  if (!name || typeof name !== 'string') {
    console.warn('[SkillParser] Missing or invalid "name" in frontmatter');
    return null;
  }

  if (!description || typeof description !== 'string') {
    console.warn(`[SkillParser] Missing "description" in frontmatter for skill "${name}"`);
    return null;
  }

  const { title, sections, mcpTools, cliCommands } = analyzeBody(body);

  return {
    name: name.trim(),
    description: description.trim(),
    body,
    title,
    sections,
    mcpTools,
    cliCommands,
    raw: frontmatter,
  };
}
