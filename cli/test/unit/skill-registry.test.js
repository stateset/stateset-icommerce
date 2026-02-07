/**
 * Unit tests for skills/registry.js — SkillRegistry
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import {
  SkillRegistry,
  CATEGORY_MAP,
  getSkillRegistry,
  resetSkillRegistry,
} from '../../src/skills/registry.js';

// ---------------------------------------------------------------------------
// Helpers — create mock skill entries
// ---------------------------------------------------------------------------

function mockSkill(name, overrides = {}) {
  return {
    name,
    description: overrides.description || `Skill for ${name}`,
    category: CATEGORY_MAP[name] || overrides.category || 'other',
    tags: overrides.tags || [name.replace('commerce-', '')],
    origin: overrides.origin || 'bundled',
    dirPath: `/skills/${name}`,
    parsed: {
      name,
      description: overrides.description || `Skill for ${name}`,
      body: overrides.body || `# ${name}\nDocumentation here.`,
      sections: ['Overview'],
      mcpTools: overrides.mcpTools || [],
      cliCommands: overrides.cliCommands || [],
    },
    hasReferences: false,
    hasScripts: false,
    ...overrides,
  };
}

function populatedRegistry() {
  const reg = new SkillRegistry();
  reg.addSkill(mockSkill('commerce-orders', { mcpTools: ['list_orders', 'get_order'] }));
  reg.addSkill(
    mockSkill('commerce-checkout', {
      mcpTools: ['create_cart', 'complete_checkout'],
      cliCommands: ['stateset-checkout'],
    }),
  );
  reg.addSkill(mockSkill('commerce-inventory', { mcpTools: ['get_stock'] }));
  reg.addSkill(
    mockSkill('commerce-analytics', {
      mcpTools: ['get_sales_summary'],
      description: 'Sales metrics and forecasting',
    }),
  );
  reg.addSkill(
    mockSkill('commerce-promotions', {
      mcpTools: ['list_promotions'],
      description: 'Promotions and discounts',
    }),
  );
  return reg;
}

// ===========================================================================
// CATEGORY_MAP
// ===========================================================================

describe('CATEGORY_MAP', () => {
  it('maps core commerce skills', () => {
    assert.strictEqual(CATEGORY_MAP['commerce-orders'], 'core');
    assert.strictEqual(CATEGORY_MAP['commerce-customers'], 'core');
    assert.strictEqual(CATEGORY_MAP['commerce-checkout'], 'core');
  });

  it('maps fulfillment skills', () => {
    assert.strictEqual(CATEGORY_MAP['commerce-inventory'], 'fulfillment');
    assert.strictEqual(CATEGORY_MAP['commerce-returns'], 'fulfillment');
    assert.strictEqual(CATEGORY_MAP['commerce-shipments'], 'fulfillment');
  });

  it('maps financial skills', () => {
    assert.strictEqual(CATEGORY_MAP['commerce-invoices'], 'financial');
    assert.strictEqual(CATEGORY_MAP['commerce-tax'], 'financial');
    assert.strictEqual(CATEGORY_MAP['commerce-currency'], 'financial');
  });

  it('maps analytics skills', () => {
    assert.strictEqual(CATEGORY_MAP['commerce-analytics'], 'analytics');
  });

  it('maps marketing skills', () => {
    assert.strictEqual(CATEGORY_MAP['commerce-promotions'], 'marketing');
    assert.strictEqual(CATEGORY_MAP['commerce-subscriptions'], 'marketing');
  });

  it('maps supply-chain skills', () => {
    assert.strictEqual(CATEGORY_MAP['commerce-suppliers'], 'supply-chain');
    assert.strictEqual(CATEGORY_MAP['commerce-manufacturing'], 'supply-chain');
  });

  it('maps platform skills', () => {
    assert.strictEqual(CATEGORY_MAP['commerce-sync'], 'platform');
  });
});

// ===========================================================================
// SkillRegistry CRUD
// ===========================================================================

describe('SkillRegistry CRUD', () => {
  it('addSkill and get round-trip', () => {
    const reg = new SkillRegistry();
    reg.addSkill(mockSkill('commerce-orders'));
    const s = reg.get('commerce-orders');
    assert.ok(s);
    assert.strictEqual(s.name, 'commerce-orders');
  });

  it('get returns null for missing skill', () => {
    const reg = new SkillRegistry();
    assert.strictEqual(reg.get('nonexistent'), null);
  });

  it('has returns true/false', () => {
    const reg = new SkillRegistry();
    reg.addSkill(mockSkill('commerce-orders'));
    assert.strictEqual(reg.has('commerce-orders'), true);
    assert.strictEqual(reg.has('commerce-nope'), false);
  });

  it('removeSkill deletes a skill', () => {
    const reg = new SkillRegistry();
    reg.addSkill(mockSkill('commerce-orders'));
    assert.strictEqual(reg.removeSkill('commerce-orders'), true);
    assert.strictEqual(reg.has('commerce-orders'), false);
  });

  it('removeSkill returns false for missing', () => {
    const reg = new SkillRegistry();
    assert.strictEqual(reg.removeSkill('nope'), false);
  });

  it('count returns skill count', () => {
    const reg = populatedRegistry();
    assert.strictEqual(reg.count(), 5);
  });
});

// ===========================================================================
// list and filtering
// ===========================================================================

describe('SkillRegistry list', () => {
  it('returns all skills sorted by name', () => {
    const reg = populatedRegistry();
    const all = reg.list();
    assert.strictEqual(all.length, 5);
    assert.strictEqual(all[0].name, 'commerce-analytics');
    assert.strictEqual(all[4].name, 'commerce-promotions');
  });

  it('filters by category', () => {
    const reg = populatedRegistry();
    const core = reg.list({ category: 'core' });
    assert.ok(core.length >= 2);
    assert.ok(core.every((s) => s.category === 'core'));
  });

  it('filters by origin', () => {
    const reg = new SkillRegistry();
    reg.addSkill(mockSkill('commerce-orders', { origin: 'bundled' }));
    reg.addSkill(mockSkill('commerce-custom', { origin: 'workspace', category: 'other' }));
    const ws = reg.list({ origin: 'workspace' });
    assert.strictEqual(ws.length, 1);
    assert.strictEqual(ws[0].name, 'commerce-custom');
  });

  it('listByCategory delegates to list', () => {
    const reg = populatedRegistry();
    const fulfillment = reg.listByCategory('fulfillment');
    assert.ok(fulfillment.every((s) => s.category === 'fulfillment'));
  });
});

// ===========================================================================
// search
// ===========================================================================

describe('SkillRegistry search', () => {
  it('exact name match scores highest', () => {
    const reg = populatedRegistry();
    const results = reg.search('commerce-orders');
    assert.strictEqual(results[0].name, 'commerce-orders');
  });

  it('search with commerce- prefix stripped', () => {
    const reg = populatedRegistry();
    const results = reg.search('orders');
    assert.ok(results.length > 0);
    assert.strictEqual(results[0].name, 'commerce-orders');
  });

  it('description search finds skills', () => {
    const reg = populatedRegistry();
    const results = reg.search('metrics');
    assert.ok(results.some((s) => s.name === 'commerce-analytics'));
  });

  it('category search works', () => {
    const reg = populatedRegistry();
    const results = reg.search('marketing');
    assert.ok(results.some((s) => s.name === 'commerce-promotions'));
  });

  it('empty query returns all skills', () => {
    const reg = populatedRegistry();
    assert.strictEqual(reg.search('').length, 5);
    assert.strictEqual(reg.search('  ').length, 5);
  });

  it('no results for garbage query', () => {
    const reg = populatedRegistry();
    const results = reg.search('zzzznonexistent');
    assert.strictEqual(results.length, 0);
  });
});

// ===========================================================================
// getSkillBody / getSkillBodies
// ===========================================================================

describe('SkillRegistry skill bodies', () => {
  it('getSkillBody returns parsed body', () => {
    const reg = new SkillRegistry();
    reg.addSkill(mockSkill('commerce-orders', { body: '# Orders\nFull docs.' }));
    assert.strictEqual(reg.getSkillBody('commerce-orders'), '# Orders\nFull docs.');
  });

  it('getSkillBody returns null for missing skill', () => {
    const reg = new SkillRegistry();
    assert.strictEqual(reg.getSkillBody('nope'), null);
  });

  it('getSkillBodies returns map', () => {
    const reg = populatedRegistry();
    const bodies = reg.getSkillBodies(['commerce-orders', 'commerce-checkout', 'missing']);
    assert.strictEqual(bodies.size, 2);
    assert.ok(bodies.has('commerce-orders'));
    assert.ok(bodies.has('commerce-checkout'));
    assert.ok(!bodies.has('missing'));
  });
});

// ===========================================================================
// getCategories
// ===========================================================================

describe('SkillRegistry getCategories', () => {
  it('returns unique sorted categories', () => {
    const reg = populatedRegistry();
    const cats = reg.getCategories();
    assert.ok(cats.includes('core'));
    assert.ok(cats.includes('fulfillment'));
    assert.ok(cats.includes('analytics'));
    assert.ok(cats.includes('marketing'));
    // Should be sorted
    const sorted = [...cats].sort();
    assert.deepStrictEqual(cats, sorted);
  });
});

// ===========================================================================
// findRelevantSkills
// ===========================================================================

describe('SkillRegistry findRelevantSkills', () => {
  it('returns up to limit results', () => {
    const reg = populatedRegistry();
    const results = reg.findRelevantSkills('commerce', 2);
    assert.ok(results.length <= 2);
  });

  it('returns empty for null/empty text', () => {
    const reg = populatedRegistry();
    assert.deepStrictEqual(reg.findRelevantSkills(''), []);
    assert.deepStrictEqual(reg.findRelevantSkills(null), []);
  });
});

// ===========================================================================
// getStats
// ===========================================================================

describe('SkillRegistry getStats', () => {
  it('counts by origin and category', () => {
    const reg = new SkillRegistry();
    reg.addSkill(mockSkill('commerce-orders', { origin: 'bundled' }));
    reg.addSkill(mockSkill('commerce-checkout', { origin: 'bundled' }));
    reg.addSkill(mockSkill('commerce-custom', { origin: 'installed', category: 'other' }));
    reg.addSkill(mockSkill('commerce-local', { origin: 'workspace', category: 'other' }));

    const stats = reg.getStats();
    assert.strictEqual(stats.total, 4);
    assert.strictEqual(stats.bundled, 2);
    assert.strictEqual(stats.installed, 1);
    assert.strictEqual(stats.workspace, 1);
    assert.strictEqual(stats.categories['core'], 2);
    assert.strictEqual(stats.categories['other'], 2);
  });
});

// ===========================================================================
// toJSON
// ===========================================================================

describe('SkillRegistry toJSON', () => {
  it('exports catalog with version and skills', () => {
    const reg = populatedRegistry();
    const json = reg.toJSON();
    assert.strictEqual(json.version, '1.0.0');
    assert.ok(json.generatedAt);
    assert.strictEqual(json.skills.length, 5);
    const first = json.skills[0];
    assert.ok(first.name);
    assert.ok(first.description);
    assert.ok(first.category);
    assert.ok(Array.isArray(first.tags));
    assert.ok(Array.isArray(first.mcpTools));
  });
});

// ===========================================================================
// loadFromDiscovered
// ===========================================================================

describe('SkillRegistry loadFromDiscovered', () => {
  it('loads discovered skills with tags', () => {
    const reg = new SkillRegistry();
    const discovered = [
      {
        name: 'commerce-orders',
        origin: 'bundled',
        dirPath: '/skills/commerce-orders',
        hasReferences: false,
        hasScripts: false,
        parsed: {
          name: 'commerce-orders',
          description: 'Order lifecycle management',
          body: '# Orders',
          sections: ['Overview'],
          mcpTools: ['list_orders', 'get_order'],
          cliCommands: ['stateset-orders'],
        },
      },
    ];

    reg.loadFromDiscovered(discovered);
    assert.strictEqual(reg.count(), 1);
    const skill = reg.get('commerce-orders');
    assert.ok(skill);
    assert.strictEqual(skill.category, 'core');
    assert.ok(skill.tags.length > 0);
    assert.ok(skill.tags.includes('orders'));
  });
});

// ===========================================================================
// Singleton
// ===========================================================================

describe('SkillRegistry singleton', () => {
  beforeEach(() => resetSkillRegistry());

  it('getSkillRegistry returns same instance', () => {
    const a = getSkillRegistry();
    const b = getSkillRegistry();
    assert.strictEqual(a, b);
  });

  it('resetSkillRegistry clears singleton', () => {
    const a = getSkillRegistry();
    resetSkillRegistry();
    const b = getSkillRegistry();
    assert.notStrictEqual(a, b);
  });
});
