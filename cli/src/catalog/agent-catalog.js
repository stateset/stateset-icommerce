/**
 * Machine-Readable Agent Catalog
 *
 * SQLite-backed catalog of products and services discoverable by AI agents.
 * Products include machine-readable specs, capability requirements, trust
 * levels, fulfillment chains, and pricing — enabling autonomous agent
 * discovery, matching, and procurement.
 *
 * Factory function pattern: createAgentCatalog(store) → catalog API object.
 */

import { randomUUID } from 'node:crypto';

// ============================================================================
// Trust Level Hierarchy
// ============================================================================

const TRUST_LEVELS = ['sandbox', 'verified', 'enterprise', 'admin'];

/**
 * Returns true if the agent's trust level meets or exceeds the required level.
 * @param {string} agentLevel
 * @param {string} requiredLevel
 * @returns {boolean}
 */
function trustMeetsMinimum(agentLevel, requiredLevel) {
  const agentIdx = TRUST_LEVELS.indexOf(agentLevel);
  const requiredIdx = TRUST_LEVELS.indexOf(requiredLevel);
  if (agentIdx === -1 || requiredIdx === -1) return false;
  return agentIdx >= requiredIdx;
}

// ============================================================================
// Schema & Column Whitelist
// ============================================================================

const CATALOG_SCHEMA = `
CREATE TABLE IF NOT EXISTS agent_catalog (
  id TEXT PRIMARY KEY,
  product_id TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  capabilities TEXT NOT NULL DEFAULT '[]',
  agent_requirements TEXT DEFAULT '{}',
  fulfillment_agents TEXT DEFAULT '[]',
  fulfillment_chains TEXT DEFAULT '[]',
  min_trust_level TEXT DEFAULT 'sandbox',
  max_price REAL,
  currency TEXT DEFAULT 'USD',
  machine_spec TEXT DEFAULT '{}',
  tags TEXT DEFAULT '[]',
  category TEXT,
  status TEXT NOT NULL DEFAULT 'active',
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_catalog_product ON agent_catalog(product_id);
CREATE INDEX IF NOT EXISTS idx_catalog_status ON agent_catalog(status);
CREATE INDEX IF NOT EXISTS idx_catalog_category ON agent_catalog(category);
CREATE INDEX IF NOT EXISTS idx_catalog_trust ON agent_catalog(min_trust_level);
`;

const UPDATABLE_COLUMNS = new Set([
  'name',
  'description',
  'capabilities',
  'agent_requirements',
  'fulfillment_agents',
  'fulfillment_chains',
  'min_trust_level',
  'max_price',
  'currency',
  'machine_spec',
  'tags',
  'category',
  'status',
  'version',
  'updated_at',
]);

const JSON_FIELDS = new Set([
  'capabilities',
  'agent_requirements',
  'fulfillment_agents',
  'fulfillment_chains',
  'machine_spec',
  'tags',
]);

// ============================================================================
// Helpers
// ============================================================================

/**
 * Parse JSON columns back to native objects.
 * @param {object} row  Raw SQLite row
 * @returns {object}    Row with parsed JSON fields
 */
function parseRow(row) {
  if (!row) return null;
  const parsed = { ...row };
  for (const field of JSON_FIELDS) {
    if (typeof parsed[field] === 'string') {
      try {
        parsed[field] = JSON.parse(parsed[field]);
      } catch (parseErr) {
        console.debug(`agent-catalog field "${field}" kept as string:`, parseErr.message);
      }
    }
  }
  return parsed;
}

/**
 * Stringify value if it is an array or non-null object.
 * @param {*} value
 * @returns {*}
 */
function maybeStringify(value) {
  if (Array.isArray(value) || (typeof value === 'object' && value !== null)) {
    return JSON.stringify(value);
  }
  return value;
}

/**
 * Validate that all update keys are in the column whitelist.
 * @param {string[]} keys
 */
function validateUpdateKeys(keys) {
  for (const key of keys) {
    if (!UPDATABLE_COLUMNS.has(key)) {
      throw new Error(`Column '${key}' is not allowed for update on agent_catalog`);
    }
  }
}

// ============================================================================
// Factory
// ============================================================================

/**
 * Create an agent catalog service backed by the given store's SQLite database.
 *
 * @param {{ db: import('better-sqlite3').Database }} store
 * @returns {object} Catalog API
 */
export function createAgentCatalog(store) {
  const { db } = store;
  db.exec(CATALOG_SCHEMA);

  // --------------------------------------------------------------------------
  // publishProduct
  // --------------------------------------------------------------------------

  function publishProduct({
    productId,
    name,
    description,
    capabilities,
    agentRequirements,
    fulfillmentAgents,
    fulfillmentChains,
    minTrustLevel,
    maxPrice,
    currency,
    machineSpec,
    tags,
    category,
  }) {
    if (!productId) throw new Error('productId is required');
    if (!name) throw new Error('name is required');
    if (!Array.isArray(capabilities) || capabilities.length === 0) {
      throw new Error('capabilities must be a non-empty array');
    }

    const id = randomUUID();
    const now = new Date().toISOString();

    db.prepare(
      `
      INSERT INTO agent_catalog
        (id, product_id, name, description, capabilities, agent_requirements,
         fulfillment_agents, fulfillment_chains, min_trust_level, max_price,
         currency, machine_spec, tags, category, status, version, created_at, updated_at)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', 1, ?, ?)
    `,
    ).run(
      id,
      productId,
      name,
      description || null,
      JSON.stringify(capabilities),
      JSON.stringify(agentRequirements || {}),
      JSON.stringify(fulfillmentAgents || []),
      JSON.stringify(fulfillmentChains || []),
      minTrustLevel || 'sandbox',
      maxPrice ?? null,
      currency || 'USD',
      JSON.stringify(machineSpec || {}),
      JSON.stringify(tags || []),
      category || null,
      now,
      now,
    );

    return { catalogEntryId: id, productId, status: 'active' };
  }

  // --------------------------------------------------------------------------
  // queryProducts
  // --------------------------------------------------------------------------

  function queryProducts({
    capability,
    agentTrustLevel,
    maxPrice,
    fulfillmentChain,
    category,
    status,
    limit,
    offset,
  } = {}) {
    const conditions = [];
    const params = [];

    if (status) {
      conditions.push('status = ?');
      params.push(status);
    } else {
      conditions.push("status = 'active'");
    }

    if (capability) {
      conditions.push('capabilities LIKE ?');
      params.push(`%${capability}%`);
    }

    if (agentTrustLevel) {
      // Include products whose min_trust_level the agent meets
      const allowedLevels = TRUST_LEVELS.filter(
        (_, i) => i <= TRUST_LEVELS.indexOf(agentTrustLevel),
      );
      if (allowedLevels.length > 0) {
        const placeholders = allowedLevels.map(() => '?').join(', ');
        conditions.push(`min_trust_level IN (${placeholders})`);
        params.push(...allowedLevels);
      }
    }

    if (maxPrice !== undefined && maxPrice !== null) {
      conditions.push('(max_price <= ? OR max_price IS NULL)');
      params.push(maxPrice);
    }

    if (fulfillmentChain) {
      conditions.push('fulfillment_chains LIKE ?');
      params.push(`%${fulfillmentChain}%`);
    }

    if (category) {
      conditions.push('category = ?');
      params.push(category);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';

    const totalRow = db
      .prepare(`SELECT COUNT(*) AS cnt FROM agent_catalog ${where}`)
      .get(...params);
    const total = totalRow ? totalRow.cnt : 0;

    const effectiveLimit = limit && Number.isFinite(limit) ? Math.min(limit, 1000) : 100;
    const effectiveOffset = offset && Number.isFinite(offset) ? offset : 0;

    const rows = db
      .prepare(`SELECT * FROM agent_catalog ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`)
      .all(...params, effectiveLimit, effectiveOffset);

    return { products: rows.map(parseRow), total };
  }

  // --------------------------------------------------------------------------
  // getProductSpec
  // --------------------------------------------------------------------------

  function getProductSpec(productIdOrCatalogId) {
    if (!productIdOrCatalogId) return null;

    let row = db.prepare('SELECT * FROM agent_catalog WHERE id = ?').get(productIdOrCatalogId);
    if (!row) {
      row = db
        .prepare('SELECT * FROM agent_catalog WHERE product_id = ?')
        .get(productIdOrCatalogId);
    }
    if (!row) return null;

    const entry = parseRow(row);

    // Build a JSON Schema fragment from agent_requirements
    const spec = {
      ...entry,
      schema: {
        type: 'object',
        properties: {},
        required: [],
      },
    };

    if (entry.agent_requirements && typeof entry.agent_requirements === 'object') {
      for (const [key, value] of Object.entries(entry.agent_requirements)) {
        spec.schema.properties[key] = typeof value === 'object' ? value : { const: value };
        spec.schema.required.push(key);
      }
    }

    return { entry, spec };
  }

  // --------------------------------------------------------------------------
  // updateProduct
  // --------------------------------------------------------------------------

  function updateProduct(catalogId, updates) {
    if (!catalogId) throw new Error('catalogId is required');
    if (!updates || typeof updates !== 'object') throw new Error('updates must be an object');

    const keys = Object.keys(updates).filter((k) => k !== 'version' && k !== 'updated_at');
    validateUpdateKeys(keys);

    const fields = [];
    const values = [];

    for (const [key, value] of Object.entries(updates)) {
      if (key === 'version' || key === 'updated_at') continue;
      if (value !== undefined) {
        fields.push(`${key} = ?`);
        values.push(JSON_FIELDS.has(key) ? maybeStringify(value) : value);
      }
    }

    // Always increment version and update timestamp
    fields.push('version = version + 1');
    fields.push('updated_at = ?');
    values.push(new Date().toISOString());

    if (fields.length === 2) {
      // Only version + updated_at, no real updates
      const existing = db.prepare('SELECT * FROM agent_catalog WHERE id = ?').get(catalogId);
      return { entry: parseRow(existing) };
    }

    values.push(catalogId);
    db.prepare(`UPDATE agent_catalog SET ${fields.join(', ')} WHERE id = ?`).run(...values);

    const row = db.prepare('SELECT * FROM agent_catalog WHERE id = ?').get(catalogId);
    return { entry: parseRow(row) };
  }

  // --------------------------------------------------------------------------
  // matchAgentToProducts
  // --------------------------------------------------------------------------

  function matchAgentToProducts(agentCapabilities, agentTrustLevel) {
    if (!Array.isArray(agentCapabilities) || agentCapabilities.length === 0) {
      return { compatibleProducts: [] };
    }

    // Get all active products the agent's trust level can access
    const allowedLevels = TRUST_LEVELS.filter(
      (_, i) => i <= TRUST_LEVELS.indexOf(agentTrustLevel || 'sandbox'),
    );
    if (allowedLevels.length === 0) return { compatibleProducts: [] };

    const placeholders = allowedLevels.map(() => '?').join(', ');
    const rows = db
      .prepare(
        `SELECT * FROM agent_catalog WHERE status = 'active' AND min_trust_level IN (${placeholders})`,
      )
      .all(...allowedLevels);

    // Score by capability overlap
    const scored = [];
    for (const row of rows) {
      const parsed = parseRow(row);
      const productCaps = parsed.capabilities || [];
      const matchCount = agentCapabilities.filter((c) => productCaps.includes(c)).length;
      if (matchCount > 0) {
        scored.push({ ...parsed, matchScore: matchCount });
      }
    }

    // Sort by match score descending
    scored.sort((a, b) => b.matchScore - a.matchScore);

    return { compatibleProducts: scored };
  }

  // --------------------------------------------------------------------------
  // matchProductToAgents
  // --------------------------------------------------------------------------

  function matchProductToAgents(productId, availableAgents) {
    if (!Array.isArray(availableAgents)) return { compatibleAgents: [] };

    const specResult = getProductSpec(productId);
    if (!specResult) return { compatibleAgents: [] };

    const { entry } = specResult;
    const requiredCaps = entry.capabilities || [];
    const requiredTrust = entry.min_trust_level || 'sandbox';

    const compatible = availableAgents.filter((agent) => {
      // Check trust level
      const agentTrust = agent.trustLevel || agent.trust_level || 'sandbox';
      if (!trustMeetsMinimum(agentTrust, requiredTrust)) return false;

      // Check capability overlap
      const agentCaps = agent.capabilities || [];
      const hasCapability = requiredCaps.some((c) => agentCaps.includes(c));
      return hasCapability;
    });

    return { compatibleAgents: compatible };
  }

  // --------------------------------------------------------------------------
  // exportCatalog
  // --------------------------------------------------------------------------

  function exportCatalog({ format, category, status } = {}) {
    const conditions = [];
    const params = [];

    if (status) {
      conditions.push('status = ?');
      params.push(status);
    }
    if (category) {
      conditions.push('category = ?');
      params.push(category);
    }

    const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
    const rows = db
      .prepare(`SELECT * FROM agent_catalog ${where} ORDER BY created_at DESC`)
      .all(...params);
    const entries = rows.map(parseRow);
    const exportedAt = new Date().toISOString();

    if (format === 'openapi') {
      const paths = {};
      for (const entry of entries) {
        const pathKey = `/products/${entry.product_id}`;
        paths[pathKey] = {
          get: {
            summary: entry.name,
            description: entry.description || '',
            operationId: `get_${entry.product_id.replace(/[^a-zA-Z0-9_]/g, '_')}`,
            tags: entry.tags || [],
            parameters: [
              {
                name: 'agentTrustLevel',
                in: 'header',
                required: true,
                schema: { type: 'string', enum: TRUST_LEVELS },
                description: `Minimum trust: ${entry.min_trust_level}`,
              },
            ],
            responses: {
              200: {
                description: 'Product spec',
                content: {
                  'application/json': {
                    schema: {
                      type: 'object',
                      properties: {
                        id: { type: 'string' },
                        name: { type: 'string', example: entry.name },
                        capabilities: { type: 'array', items: { type: 'string' } },
                        maxPrice: { type: 'number', example: entry.max_price },
                        currency: { type: 'string', example: entry.currency },
                      },
                    },
                  },
                },
              },
            },
          },
        };
      }

      return {
        entries,
        format: 'openapi',
        exportedAt,
        openapi: {
          openapi: '3.0.3',
          info: { title: 'Agent Catalog', version: '1.0.0' },
          paths,
        },
      };
    }

    return { entries, format: format || 'json', exportedAt };
  }

  // --------------------------------------------------------------------------
  // delistProduct
  // --------------------------------------------------------------------------

  function delistProduct(catalogId) {
    if (!catalogId) throw new Error('catalogId is required');

    db.prepare("UPDATE agent_catalog SET status = 'delisted', updated_at = ? WHERE id = ?").run(
      new Date().toISOString(),
      catalogId,
    );

    const row = db.prepare('SELECT * FROM agent_catalog WHERE id = ?').get(catalogId);
    return { entry: parseRow(row) };
  }

  // --------------------------------------------------------------------------
  // Public API
  // --------------------------------------------------------------------------

  return {
    publishProduct,
    queryProducts,
    getProductSpec,
    updateProduct,
    matchAgentToProducts,
    matchProductToAgents,
    exportCatalog,
    delistProduct,
  };
}
