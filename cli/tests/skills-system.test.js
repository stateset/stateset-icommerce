/**
 * Skills System Tests for StateSet iCommerce v0.2.7
 *
 * Tests: Parser, Loader, Registry, Marketplace, Injector.
 *
 * Run: node --test tests/skills-system.test.js
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';

import {
  parseSkillMd,
  parseSkillContent,
  extractFrontmatter,
} from '../src/skills/parser.js';

import {
  discoverSkills,
  discoverFromDirectory,
  SKILL_ORIGINS,
} from '../src/skills/loader.js';

import {
  SkillRegistry,
  getSkillRegistry,
  resetSkillRegistry,
  CATEGORY_MAP,
} from '../src/skills/registry.js';

import {
  MarketplaceClient,
} from '../src/skills/marketplace.js';

import {
  SkillInjector,
  AGENT_SKILL_MAP,
} from '../src/skills/injector.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SKILLS_DIR = path.resolve(__dirname, '..', 'skills');

// ============================================================================
// Parser
// ============================================================================

describe('SkillParser', () => {
  it('should extract YAML frontmatter and body', () => {
    const content = `---
name: test-skill
description: A test skill for unit tests.
---

# Test Skill

This is the body.

## How It Works

1. Step one.
2. Step two.

## Usage

- MCP tools: \`list_orders\`, \`get_order\`
- CLI: \`stateset-orders\`
`;
    const result = extractFrontmatter(content);
    assert.ok(result.frontmatter);
    assert.equal(result.frontmatter.name, 'test-skill');
    assert.ok(result.body.includes('# Test Skill'));
  });

  it('should parse a complete skill from string', () => {
    const content = `---
name: commerce-test
description: Test skill for parsing.
---

# Commerce Test

## How It Works

1. Check \`list_orders\`.
2. Use \`stateset-orders\`.

## Usage

MCP tools: \`create_order\`, \`get_order\`
`;
    const parsed = parseSkillContent(content);
    assert.ok(parsed);
    assert.equal(parsed.name, 'commerce-test');
    assert.equal(parsed.description, 'Test skill for parsing.');
    assert.equal(parsed.title, 'Commerce Test');
    assert.ok(parsed.sections.includes('How It Works'));
    assert.ok(parsed.sections.includes('Usage'));
    assert.ok(parsed.mcpTools.includes('list_orders'));
    assert.ok(parsed.mcpTools.includes('create_order'));
    assert.ok(parsed.cliCommands.includes('stateset-orders'));
  });

  it('should return null for content without frontmatter', () => {
    const result = parseSkillContent('# No Frontmatter\nJust markdown.');
    assert.equal(result, null);
  });

  it('should return null for missing name field', () => {
    const content = `---
description: No name field.
---

# Test
`;
    const result = parseSkillContent(content);
    assert.equal(result, null);
  });

  it('should return null for missing description field', () => {
    const content = `---
name: missing-desc
---

# Test
`;
    const result = parseSkillContent(content);
    assert.equal(result, null);
  });

  it('should return null for empty content', () => {
    const result = parseSkillContent('');
    assert.equal(result, null);
  });

  it('should handle invalid YAML gracefully', () => {
    const content = `---
name: [invalid yaml
---

# Test
`;
    const result = parseSkillContent(content);
    assert.equal(result, null);
  });

  it('should parse a real SKILL.md from disk', () => {
    const skillPath = path.join(SKILLS_DIR, 'commerce-orders', 'SKILL.md');
    if (!fs.existsSync(skillPath)) return; // Skip if not bundled

    const parsed = parseSkillMd(skillPath);
    assert.ok(parsed);
    assert.equal(parsed.name, 'commerce-orders');
    assert.ok(parsed.description.length > 10);
    assert.ok(parsed.body.length > 50);
    assert.ok(parsed.mcpTools.length > 0);
  });
});

// ============================================================================
// Loader
// ============================================================================

describe('SkillLoader', () => {
  it('should discover skills from the bundled directory', () => {
    const discovered = discoverFromDirectory(SKILLS_DIR, 'bundled');
    assert.ok(discovered.length >= 30, `Expected >= 30 skills, got ${discovered.length}`);

    // Check structure of first skill
    const first = discovered[0];
    assert.ok(first.name);
    assert.equal(first.origin, 'bundled');
    assert.ok(first.dirPath);
    assert.ok(first.skillMdPath);
    assert.ok(first.parsed);
    assert.equal(typeof first.hasReferences, 'boolean');
    assert.equal(typeof first.hasScripts, 'boolean');
  });

  it('should return empty for nonexistent directory', () => {
    const discovered = discoverFromDirectory('/nonexistent/path', 'bundled');
    assert.equal(discovered.length, 0);
  });

  it('should discover from multiple origins with deduplication', () => {
    // Discover twice from same dir with different origins
    const all = discoverSkills({
      bundledDir: SKILLS_DIR,
      installedDir: SKILLS_DIR, // Same dir = all names already seen
      workspaceDir: '/nonexistent',
    });

    // Should not have duplicates
    const names = all.map((s) => s.name);
    const unique = new Set(names);
    assert.equal(names.length, unique.size, 'No duplicate names');
  });

  it('should have correct origin constants', () => {
    assert.equal(SKILL_ORIGINS.BUNDLED, 'bundled');
    assert.equal(SKILL_ORIGINS.INSTALLED, 'installed');
    assert.equal(SKILL_ORIGINS.WORKSPACE, 'workspace');
  });

  it('should sort results by name', () => {
    const discovered = discoverSkills({ bundledDir: SKILLS_DIR, installedDir: '/x', workspaceDir: '/x' });
    for (let i = 1; i < discovered.length; i++) {
      assert.ok(discovered[i].name >= discovered[i - 1].name, 'Skills should be sorted by name');
    }
  });
});

// ============================================================================
// Registry
// ============================================================================

describe('SkillRegistry', () => {
  let registry;

  beforeEach(() => {
    resetSkillRegistry();
    registry = new SkillRegistry();
    const discovered = discoverFromDirectory(SKILLS_DIR, 'bundled');
    registry.loadFromDiscovered(discovered);
  });

  it('should load all bundled skills', () => {
    assert.ok(registry.count() >= 30, `Expected >= 30, got ${registry.count()}`);
  });

  it('should get a skill by name', () => {
    const skill = registry.get('commerce-orders');
    assert.ok(skill);
    assert.equal(skill.name, 'commerce-orders');
    assert.equal(skill.category, 'core');
    assert.equal(skill.origin, 'bundled');
    assert.ok(skill.tags.length > 0);
    assert.ok(skill.parsed.body.length > 0);
  });

  it('should return null for unknown skill', () => {
    assert.equal(registry.get('nonexistent'), null);
  });

  it('should check if a skill exists', () => {
    assert.ok(registry.has('commerce-inventory'));
    assert.ok(!registry.has('nonexistent'));
  });

  it('should list all skills sorted by name', () => {
    const all = registry.list();
    assert.ok(all.length > 0);
    for (let i = 1; i < all.length; i++) {
      assert.ok(all[i].name >= all[i - 1].name);
    }
  });

  it('should filter by category', () => {
    const core = registry.list({ category: 'core' });
    assert.ok(core.length > 0);
    assert.ok(core.every((s) => s.category === 'core'));
  });

  it('should filter by origin', () => {
    const bundled = registry.list({ origin: 'bundled' });
    assert.equal(bundled.length, registry.count());
  });

  it('should search skills by query', () => {
    const results = registry.search('inventory');
    assert.ok(results.length > 0);
    assert.equal(results[0].name, 'commerce-inventory');
  });

  it('should search by partial match', () => {
    const results = registry.search('fulfillment');
    assert.ok(results.length > 0);
    assert.ok(results.some((s) => s.name === 'commerce-fulfillment'));
  });

  it('should return all skills for empty search', () => {
    const results = registry.search('');
    assert.equal(results.length, registry.count());
  });

  it('should get categories', () => {
    const cats = registry.getCategories();
    assert.ok(cats.includes('core'));
    assert.ok(cats.includes('financial'));
    assert.ok(cats.includes('fulfillment'));
    assert.ok(cats.includes('analytics'));
    assert.ok(cats.includes('platform'));
    assert.ok(cats.includes('marketing'));
    assert.ok(cats.includes('supply-chain'));
  });

  it('should list by category', () => {
    const financial = registry.listByCategory('financial');
    assert.ok(financial.length > 0);
    assert.ok(financial.every((s) => s.category === 'financial'));
  });

  it('should get skill body for prompt injection', () => {
    const body = registry.getSkillBody('commerce-orders');
    assert.ok(body);
    assert.ok(body.length > 50);
    assert.ok(body.includes('orders') || body.includes('Orders'));
  });

  it('should return null body for unknown skill', () => {
    assert.equal(registry.getSkillBody('nonexistent'), null);
  });

  it('should get multiple skill bodies', () => {
    const bodies = registry.getSkillBodies(['commerce-orders', 'commerce-inventory']);
    assert.equal(bodies.size, 2);
    assert.ok(bodies.has('commerce-orders'));
    assert.ok(bodies.has('commerce-inventory'));
  });

  it('should find relevant skills from text', () => {
    const relevant = registry.findRelevantSkills('show me inventory levels', 3);
    assert.ok(relevant.length > 0);
    assert.ok(relevant.length <= 3);
  });

  it('should get stats', () => {
    const stats = registry.getStats();
    assert.ok(stats.total >= 30);
    assert.ok(stats.bundled >= 30);
    assert.equal(stats.installed, 0);
    assert.equal(stats.workspace, 0);
    assert.ok(stats.categories.core > 0);
    assert.ok(stats.categories.financial > 0);
  });

  it('should export to JSON', () => {
    const json = registry.toJSON();
    assert.equal(json.version, '1.0.0');
    assert.ok(json.generatedAt);
    assert.ok(json.skills.length > 0);
    assert.ok(json.skills[0].name);
    assert.ok(json.skills[0].category);
  });

  it('should add and remove skills', () => {
    registry.addSkill({
      name: 'test-skill',
      description: 'A test',
      category: 'other',
      tags: ['test'],
      origin: 'workspace',
      dirPath: '/tmp',
      parsed: { name: 'test-skill', description: 'A test', body: '', title: '', sections: [], mcpTools: [], cliCommands: [], raw: {} },
      hasReferences: false,
      hasScripts: false,
    });

    assert.ok(registry.has('test-skill'));
    assert.ok(registry.removeSkill('test-skill'));
    assert.ok(!registry.has('test-skill'));
  });
});

// ============================================================================
// Marketplace
// ============================================================================

describe('MarketplaceClient', () => {
  let client;

  beforeEach(() => {
    client = new MarketplaceClient({
      catalogPath: path.join(SKILLS_DIR, 'marketplace.json'),
      installDir: path.join(__dirname, '..', '.test-skills'),
      bundledDir: SKILLS_DIR,
    });
  });

  it('should load local catalog', () => {
    const catalog = client.loadLocalCatalog();
    assert.ok(catalog);
    assert.ok(catalog.skills.length > 0);
    assert.equal(catalog.version, '1.0.0');
  });

  it('should search catalog', () => {
    const results = client.searchCatalog('orders');
    assert.ok(results.length > 0);
    assert.ok(results.some((s) => s.name === 'commerce-orders'));
  });

  it('should get catalog entry', () => {
    const entry = client.getCatalogEntry('commerce-inventory');
    assert.ok(entry);
    assert.equal(entry.name, 'commerce-inventory');
    assert.ok(entry.isPublic);
  });

  it('should return null for unknown catalog entry', () => {
    const entry = client.getCatalogEntry('nonexistent');
    assert.equal(entry, null);
  });

  it('should list categories', () => {
    const cats = client.listCategories();
    assert.ok(cats.includes('core'));
    assert.ok(cats.includes('financial'));
  });

  it('should list by category', () => {
    const core = client.listByCategory('core');
    assert.ok(core.length > 0);
    assert.ok(core.every((s) => s.category === 'core'));
  });

  it('should get catalog stats', () => {
    const stats = client.getCatalogStats();
    assert.ok(stats.total > 0);
    assert.ok(stats.public > 0);
    assert.ok(Object.keys(stats.categories).length > 0);
  });

  it('should install a bundled skill', async () => {
    const testDir = path.join(__dirname, '..', '.test-skills');
    try {
      const result = await client.install('commerce-orders');
      assert.ok(result.installed);
      assert.ok(fs.existsSync(path.join(testDir, 'commerce-orders', 'SKILL.md')));
    } finally {
      // Cleanup
      if (fs.existsSync(testDir)) {
        fs.rmSync(testDir, { recursive: true, force: true });
      }
    }
  });

  it('should refuse to install already-installed skill', async () => {
    const testDir = path.join(__dirname, '..', '.test-skills');
    try {
      await client.install('commerce-orders');
      const result = await client.install('commerce-orders'); // second time
      assert.ok(!result.installed);
      assert.ok(result.error.includes('Already installed'));
    } finally {
      if (fs.existsSync(testDir)) {
        fs.rmSync(testDir, { recursive: true, force: true });
      }
    }
  });

  it('should force-install over existing', async () => {
    const testDir = path.join(__dirname, '..', '.test-skills');
    try {
      await client.install('commerce-orders');
      const result = await client.install('commerce-orders', { force: true });
      assert.ok(result.installed);
    } finally {
      if (fs.existsSync(testDir)) {
        fs.rmSync(testDir, { recursive: true, force: true });
      }
    }
  });

  it('should uninstall a skill', async () => {
    const testDir = path.join(__dirname, '..', '.test-skills');
    try {
      await client.install('commerce-orders');
      const result = client.uninstall('commerce-orders');
      assert.ok(result.removed);
      assert.ok(!fs.existsSync(path.join(testDir, 'commerce-orders')));
    } finally {
      if (fs.existsSync(testDir)) {
        fs.rmSync(testDir, { recursive: true, force: true });
      }
    }
  });

  it('should refuse to uninstall nonexistent skill', () => {
    const result = client.uninstall('nonexistent');
    assert.ok(!result.removed);
    assert.ok(result.error.includes('not installed'));
  });

  it('should list installed skills', async () => {
    const testDir = path.join(__dirname, '..', '.test-skills');
    try {
      await client.install('commerce-orders');
      const installed = client.listInstalled();
      assert.ok(installed.includes('commerce-orders'));
    } finally {
      if (fs.existsSync(testDir)) {
        fs.rmSync(testDir, { recursive: true, force: true });
      }
    }
  });
});

// ============================================================================
// Injector
// ============================================================================

describe('SkillInjector', () => {
  let registry;
  let injector;

  beforeEach(() => {
    resetSkillRegistry();
    registry = new SkillRegistry();
    const discovered = discoverFromDirectory(SKILLS_DIR, 'bundled');
    registry.loadFromDiscovered(discovered);
    injector = new SkillInjector(registry);
  });

  it('should select skills based on agent name', () => {
    const names = injector.selectSkills('show me orders', 'orders');
    assert.ok(names.includes('commerce-orders'));
  });

  it('should select skills based on text when no agent', () => {
    const names = injector.selectSkills('check inventory levels', null);
    assert.ok(names.length > 0);
    assert.ok(names.length <= 3);
  });

  it('should limit to maxSkills', () => {
    injector.setMaxSkills(1);
    const names = injector.selectSkills('show me orders and inventory', 'customer-service');
    assert.ok(names.length <= 1);
  });

  it('should format skills for prompt', () => {
    const formatted = injector.formatSkillsForPrompt(['commerce-orders']);
    assert.ok(formatted);
    assert.ok(formatted.includes('<skills-context>'));
    assert.ok(formatted.includes('## Skill: commerce-orders'));
    assert.ok(formatted.includes('</skills-context>'));
  });

  it('should return null for empty skill list', () => {
    const formatted = injector.formatSkillsForPrompt([]);
    assert.equal(formatted, null);
  });

  it('should truncate long skill bodies', () => {
    injector.setMaxBodyLength(200);
    const formatted = injector.formatSkillsForPrompt(['commerce-orders']);
    assert.ok(formatted);
    assert.ok(formatted.includes('(truncated)'));
  });

  it('should strip References section from injected content', () => {
    const formatted = injector.formatSkillsForPrompt(['commerce-orders']);
    assert.ok(formatted);
    assert.ok(!formatted.includes('## References'));
  });

  it('should inject context via hook handler', async () => {
    const data = { text: 'show me pending orders', session: { agent: 'orders' } };
    const result = await injector.injectSkillContext(data);
    assert.ok(result.text.includes('<skills-context>'));
    assert.ok(result.text.includes('show me pending orders'));
  });

  it('should pass through when text is empty', async () => {
    const data = { text: '', session: {} };
    const result = await injector.injectSkillContext(data);
    assert.equal(result.text, '');
  });

  it('should handle unknown agent gracefully', () => {
    const names = injector.selectSkills('do something', 'unknown-agent');
    assert.ok(Array.isArray(names));
  });

  it('should have agent-skill mappings for all major agents', () => {
    const expectedAgents = ['orders', 'checkout', 'inventory', 'returns', 'analytics'];
    for (const agent of expectedAgents) {
      assert.ok(AGENT_SKILL_MAP[agent], `Missing mapping for agent: ${agent}`);
      assert.ok(AGENT_SKILL_MAP[agent].length > 0);
    }
  });
});

// ============================================================================
// Category Map
// ============================================================================

describe('Category Map', () => {
  it('should have entries for all standard commerce skills', () => {
    const expectedNames = [
      'commerce-orders', 'commerce-checkout', 'commerce-inventory',
      'commerce-returns', 'commerce-analytics', 'commerce-payments',
      'commerce-customers', 'commerce-products', 'commerce-promotions',
    ];
    for (const name of expectedNames) {
      assert.ok(CATEGORY_MAP[name], `Missing category for: ${name}`);
    }
  });

  it('should map all categories correctly', () => {
    const validCategories = new Set(['core', 'fulfillment', 'financial', 'marketing', 'supply-chain', 'analytics', 'platform']);
    for (const [name, category] of Object.entries(CATEGORY_MAP)) {
      assert.ok(validCategories.has(category), `Invalid category "${category}" for skill "${name}"`);
    }
  });
});

// ============================================================================
// Integration
// ============================================================================

describe('Integration: Full Pipeline', () => {
  it('should discover, load, register, and search skills end-to-end', () => {
    resetSkillRegistry();

    const discovered = discoverSkills({
      bundledDir: SKILLS_DIR,
      installedDir: '/nonexistent',
      workspaceDir: '/nonexistent',
    });

    assert.ok(discovered.length >= 30);

    const registry = getSkillRegistry();
    registry.loadFromDiscovered(discovered);

    assert.ok(registry.count() >= 30);
    assert.ok(registry.has('commerce-orders'));
    assert.ok(registry.has('commerce-inventory'));

    const results = registry.search('payment');
    assert.ok(results.length > 0);

    const stats = registry.getStats();
    assert.ok(stats.total >= 30);
    assert.equal(Object.keys(stats.categories).length, 7);

    const body = registry.getSkillBody('commerce-orders');
    assert.ok(body.length > 50);
  });

  it('should inject skill context into agent prompt', async () => {
    resetSkillRegistry();

    const discovered = discoverSkills({
      bundledDir: SKILLS_DIR,
      installedDir: '/nonexistent',
      workspaceDir: '/nonexistent',
    });

    const registry = getSkillRegistry();
    registry.loadFromDiscovered(discovered);

    const injector = new SkillInjector(registry);
    const result = await injector.injectSkillContext({
      text: 'help me with returns',
      session: { agent: 'returns' },
    });

    assert.ok(result.text.includes('<skills-context>'));
    assert.ok(result.text.includes('commerce-returns'));
    assert.ok(result.text.includes('help me with returns'));
  });
});
