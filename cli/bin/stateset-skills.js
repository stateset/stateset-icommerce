#!/usr/bin/env node

/**
 * stateset-skills — Browse and manage iCommerce skills marketplace.
 *
 * Usage:
 *   stateset-skills list [--category <cat>] [--origin <origin>] [--json]
 *   stateset-skills search <query> [--json]
 *   stateset-skills install <name> [--force]
 *   stateset-skills uninstall <name>
 *   stateset-skills info <name> [--json]
 *   stateset-skills categories [--json]
 *   stateset-skills marketplace [--json]
 *   stateset-skills doctor
 */

import { parseArgs } from 'node:util';
import { discoverSkills } from '../src/skills/loader.js';
import { SkillRegistry, getSkillRegistry, CATEGORY_MAP } from '../src/skills/registry.js';
import { MarketplaceClient, getMarketplaceClient } from '../src/skills/marketplace.js';

// ============================================================================
// CLI Parsing
// ============================================================================

const { values: flags, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    json: { type: 'boolean', default: false },
    force: { type: 'boolean', default: false },
    category: { type: 'string', short: 'c' },
    origin: { type: 'string', short: 'o' },
    help: { type: 'boolean', short: 'h', default: false },
    version: { type: 'boolean', short: 'v', default: false },
  },
});

const command = positionals[0] || 'list';
const arg = positionals[1] || '';

// ============================================================================
// Helpers
// ============================================================================

function printTable(rows, headers) {
  if (rows.length === 0) {
    console.log('  (none)');
    return;
  }

  const widths = headers.map((h, i) => {
    const max = Math.max(h.length, ...rows.map((r) => String(r[i] || '').length));
    return Math.min(max, 60);
  });

  const header = headers.map((h, i) => h.padEnd(widths[i])).join('  ');
  const sep = widths.map((w) => '-'.repeat(w)).join('  ');

  console.log(`  ${header}`);
  console.log(`  ${sep}`);

  for (const row of rows) {
    const line = row.map((cell, i) => {
      const s = String(cell || '');
      return s.length > widths[i] ? s.slice(0, widths[i] - 3) + '...' : s.padEnd(widths[i]);
    }).join('  ');
    console.log(`  ${line}`);
  }
}

// ============================================================================
// Initialize
// ============================================================================

function initRegistry() {
  const discovered = discoverSkills({ verbose: false });
  const registry = getSkillRegistry();
  registry.loadFromDiscovered(discovered);
  return registry;
}

// ============================================================================
// Commands
// ============================================================================

async function cmdList() {
  const registry = initRegistry();
  const opts = {};
  if (flags.category) opts.category = flags.category;
  if (flags.origin) opts.origin = flags.origin;

  const skills = registry.list(opts);

  if (flags.json) {
    console.log(JSON.stringify(skills.map((s) => ({
      name: s.name, description: s.description, category: s.category, origin: s.origin,
    })), null, 2));
    return;
  }

  const label = flags.category ? `Skills (${flags.category})` : 'Skills';
  console.log(`\n${label}: ${skills.length} loaded\n`);

  const rows = skills.map((s) => [s.name, s.category, s.origin, s.description]);
  printTable(rows, ['Name', 'Category', 'Origin', 'Description']);
  console.log();
}

async function cmdSearch() {
  if (!arg) {
    console.error('Usage: stateset-skills search <query>');
    process.exit(1);
  }

  const registry = initRegistry();
  const results = registry.search(arg);

  if (flags.json) {
    console.log(JSON.stringify(results.map((s) => ({
      name: s.name, description: s.description, category: s.category,
    })), null, 2));
    return;
  }

  console.log(`\nSearch results for "${arg}": ${results.length} found\n`);

  const rows = results.map((s) => [s.name, s.category, s.description]);
  printTable(rows, ['Name', 'Category', 'Description']);
  console.log();
}

async function cmdInstall() {
  if (!arg) {
    console.error('Usage: stateset-skills install <name> [--force]');
    process.exit(1);
  }

  const marketplace = getMarketplaceClient();
  const result = await marketplace.install(arg, { force: flags.force });

  if (flags.json) {
    console.log(JSON.stringify(result, null, 2));
    return;
  }

  if (result.installed) {
    console.log(`Installed "${arg}" to ${result.path}`);
  } else {
    console.error(`Failed to install "${arg}": ${result.error}`);
    process.exit(1);
  }
}

async function cmdUninstall() {
  if (!arg) {
    console.error('Usage: stateset-skills uninstall <name>');
    process.exit(1);
  }

  const marketplace = getMarketplaceClient();
  const result = marketplace.uninstall(arg);

  if (flags.json) {
    console.log(JSON.stringify(result, null, 2));
    return;
  }

  if (result.removed) {
    console.log(`Uninstalled "${arg}"`);
  } else {
    console.error(`Failed to uninstall "${arg}": ${result.error}`);
    process.exit(1);
  }
}

async function cmdInfo() {
  if (!arg) {
    console.error('Usage: stateset-skills info <name>');
    process.exit(1);
  }

  const registry = initRegistry();
  const skill = registry.get(arg);

  if (!skill) {
    // Check marketplace catalog
    const marketplace = getMarketplaceClient();
    const entry = marketplace.getCatalogEntry(arg);
    if (entry) {
      if (flags.json) {
        console.log(JSON.stringify({ ...entry, installed: false, source: 'marketplace' }, null, 2));
        return;
      }
      console.log(`\nSkill: ${entry.name} (not installed)`);
      console.log(`Description: ${entry.description}`);
      console.log(`Category: ${entry.category}`);
      console.log(`Tags: ${entry.tags.join(', ')}`);
      console.log(`Public: ${entry.isPublic ? 'yes' : 'no'}`);
      console.log(`\nInstall with: stateset-skills install ${entry.name}\n`);
      return;
    }

    console.error(`Skill "${arg}" not found.`);
    process.exit(1);
  }

  if (flags.json) {
    console.log(JSON.stringify({
      name: skill.name,
      description: skill.description,
      category: skill.category,
      tags: skill.tags,
      origin: skill.origin,
      dirPath: skill.dirPath,
      hasReferences: skill.hasReferences,
      hasScripts: skill.hasScripts,
      sections: skill.parsed.sections,
      mcpTools: skill.parsed.mcpTools,
      cliCommands: skill.parsed.cliCommands,
    }, null, 2));
    return;
  }

  console.log(`\nSkill: ${skill.name}`);
  console.log(`Description: ${skill.description}`);
  console.log(`Category: ${skill.category}`);
  console.log(`Origin: ${skill.origin}`);
  console.log(`Path: ${skill.dirPath}`);
  console.log(`Tags: ${skill.tags.join(', ')}`);
  console.log(`References: ${skill.hasReferences ? 'yes' : 'no'}`);
  console.log(`Scripts: ${skill.hasScripts ? 'yes' : 'no'}`);
  if (skill.parsed.sections.length > 0) {
    console.log(`Sections: ${skill.parsed.sections.join(', ')}`);
  }
  if (skill.parsed.mcpTools.length > 0) {
    console.log(`MCP Tools: ${skill.parsed.mcpTools.join(', ')}`);
  }
  if (skill.parsed.cliCommands.length > 0) {
    console.log(`CLI Commands: ${skill.parsed.cliCommands.join(', ')}`);
  }
  console.log();
}

async function cmdCategories() {
  const registry = initRegistry();
  const stats = registry.getStats();

  if (flags.json) {
    console.log(JSON.stringify(stats.categories, null, 2));
    return;
  }

  console.log(`\nSkill Categories (${Object.keys(stats.categories).length}):\n`);

  const rows = Object.entries(stats.categories)
    .sort((a, b) => b[1] - a[1])
    .map(([cat, count]) => [cat, String(count)]);

  printTable(rows, ['Category', 'Skills']);
  console.log(`\nTotal: ${stats.total} skills\n`);
}

async function cmdMarketplace() {
  const marketplace = getMarketplaceClient();
  const stats = marketplace.getCatalogStats();

  if (flags.json) {
    console.log(JSON.stringify(stats, null, 2));
    return;
  }

  console.log('\nStateSet iCommerce Skills Marketplace\n');
  console.log(`  Total skills:    ${stats.total}`);
  console.log(`  Public:          ${stats.public}`);
  console.log(`  Internal:        ${stats.internal}`);
  console.log(`  Installed:       ${marketplace.listInstalled().length}`);
  console.log('\n  Categories:');

  for (const [cat, count] of Object.entries(stats.categories).sort((a, b) => b[1] - a[1])) {
    console.log(`    ${cat}: ${count}`);
  }

  console.log(`\n  Use "stateset-skills search <query>" to find skills.`);
  console.log(`  Use "stateset-skills install <name>" to install.\n`);
}

async function cmdDoctor() {
  const registry = initRegistry();
  const skills = registry.list();
  let issues = 0;

  console.log(`\nChecking ${skills.length} skills...\n`);

  for (const skill of skills) {
    const problems = [];

    if (!skill.parsed.body || skill.parsed.body.length < 10) {
      problems.push('empty or very short body');
    }
    if (!skill.parsed.title) {
      problems.push('missing title (# heading)');
    }
    if (skill.parsed.sections.length === 0) {
      problems.push('no sections (## headings)');
    }

    if (problems.length > 0) {
      console.log(`  ${skill.name}: ${problems.join(', ')}`);
      issues++;
    }
  }

  if (issues === 0) {
    console.log('  All skills are healthy.');
  } else {
    console.log(`\n  ${issues} skill(s) have issues.`);
  }

  console.log();
}

function printHelp() {
  console.log(`
stateset-skills — Browse and manage iCommerce skills

COMMANDS:
  list                    List all loaded skills
  search <query>          Search skills by keyword
  install <name>          Install a skill from marketplace
  uninstall <name>        Remove an installed skill
  info <name>             Show skill details
  categories              List skill categories
  marketplace             Show marketplace overview
  doctor                  Check skill health

OPTIONS:
  --json                  Output as JSON
  --category, -c <cat>    Filter by category
  --origin, -o <origin>   Filter by origin (bundled, installed, workspace)
  --force                 Overwrite on install
  --help, -h              Show this help
  --version, -v           Show version
`);
}

// ============================================================================
// Main
// ============================================================================

if (flags.help) {
  printHelp();
  process.exit(0);
}

if (flags.version) {
  console.log('0.2.7');
  process.exit(0);
}

const commands = {
  list: cmdList,
  search: cmdSearch,
  install: cmdInstall,
  uninstall: cmdUninstall,
  info: cmdInfo,
  categories: cmdCategories,
  marketplace: cmdMarketplace,
  doctor: cmdDoctor,
  help: () => { printHelp(); },
};

const handler = commands[command];
if (!handler) {
  console.error(`Unknown command: ${command}`);
  printHelp();
  process.exit(1);
}

handler().catch((err) => {
  console.error(`Error: ${err.message}`);
  process.exit(1);
});
