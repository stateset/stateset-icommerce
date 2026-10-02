#!/usr/bin/env node
/**
 * Generate the CLI tool catalog (cli/docs/TOOLS.md) from the domain registry.
 *
 * Usage:
 *   node scripts/generate-tool-docs.mjs           # write cli/docs/TOOLS.md
 *   node scripts/generate-tool-docs.mjs --stdout  # print to stdout instead
 *
 * The committed TOOLS.md is checked for freshness by
 * test/unit/tool-docs-up-to-date.test.js (regenerate-and-diff).
 */

import { writeFileSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

import { DOMAIN_TOOL_ENTRIES } from '../src/tools/domain-registry.js';
import {
  EXPERIMENTAL_REASONS,
  TOOL_TIERS,
  domainTierLabel,
  toolTier,
} from '../src/tools/tool-tiers.js';
import { AGENTIC_RUNTIME_TOOLS } from '../src/mcp/agentic-runtime-tools.js';
import { MCP_TOOL_PROFILES, resolveMcpToolFilter } from '../src/mcp/tool-profiles.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
export const TOOLS_DOC_PATH = resolve(__dirname, '..', 'docs', 'TOOLS.md');

function escapeCell(text) {
  return String(text ?? '')
    .replace(/\\/g, '\\\\')
    .replace(/\|/g, '\\|')
    .replace(/\s+/g, ' ')
    .trim();
}

/**
 * Render the tool catalog markdown from the domain registry.
 * Deterministic: depends only on registry content (no timestamps).
 * @returns {string}
 */
export function buildToolDocs() {
  const domains = DOMAIN_TOOL_ENTRIES;
  const totalTools = domains.reduce((sum, [, tools]) => sum + tools.length, 0);
  const everyTool = [...domains.flatMap(([, tools]) => tools), ...AGENTIC_RUNTIME_TOOLS];
  const tierCount = (tier) => everyTool.filter((tool) => toolTier(tool.name) === tier).length;
  const profileCount = (profile) => {
    const inProfile = resolveMcpToolFilter({ profile });
    return everyTool.filter((tool) => inProfile(tool.name)).length;
  };
  const reason = (key) => (EXPERIMENTAL_REASONS[key] ? ` — ${EXPERIMENTAL_REASONS[key]}` : '');

  const lines = [
    '# StateSet CLI Tool Catalog',
    '',
    '<!-- GENERATED FILE — do not edit by hand. -->',
    '<!-- Regenerate with: npm run docs:tools (from cli/) -->',
    '',
    `Source of truth: \`cli/src/tools/domain-registry.js\` (tools) and \`cli/src/tools/tool-tiers.js\` (tiers).`,
    '',
    `**${totalTools} tools** across **${domains.length} domains**, plus ${AGENTIC_RUNTIME_TOOLS.length} agentic runtime tools.`,
    '',
    '## Stability tiers',
    '',
    '| Tier | Tools | Meaning |',
    '| --- | ---: | --- |',
    `| core | ${tierCount('core')} | The default MCP surface (no \`--profile\`). Smoke-gated: every tool works or refuses cleanly on a fresh store, with no backlog. |`,
    `| extended | ${tierCount('extended')} | Real, specialised domains (finance, manufacturing, WMS, B2B, engagement). Opt in with \`--profile\` or \`--domains\`. |`,
    `| experimental | ${tierCount('experimental')} | Demo, external-stack-dependent (wallet, chain, API key, demo stack) or known-incomplete. Only \`--profile all\`, a curated profile naming the domain, or \`--domains\`. |`,
    '',
    '## Profiles',
    '',
    '| Profile | Tools | Domains |',
    '| --- | ---: | --- |',
    ...Object.keys(MCP_TOOL_PROFILES).map(
      (profile) =>
        `| ${profile}${profile === 'core' ? ' (default)' : ''} | ${profileCount(profile)} | ${
          profile === 'all' ? 'every domain' : MCP_TOOL_PROFILES[profile].join(', ')
        } |`,
    ),
    '',
    '`core` is exactly the core tier, agentic runtime tools included. The curated profiles',
    'expose every tool in their domains, whatever its tier, plus the agentic runtime tools.',
    '',
    '## Domains',
    '',
    '| Domain | Tier | Tools |',
    '| --- | --- | ---: |',
    ...domains.map(
      ([name, tools]) =>
        `| [${name}](#${name.replace(/[^a-z0-9-]/g, '')}) | ${domainTierLabel(name)} | ${tools.length} |`,
    ),
    '',
  ];

  const toolTable = (tools) => {
    lines.push('| Tool | Tier | Permission | Description |', '| --- | --- | --- | --- |');
    for (const tool of tools) {
      lines.push(
        `| \`${escapeCell(tool.name)}\` | ${toolTier(tool.name)}${escapeCell(reason(tool.name))} | ${escapeCell(tool.permission ?? '—')} | ${escapeCell(tool.description)} |`,
      );
    }
    lines.push('');
  };

  for (const [name, tools] of domains) {
    lines.push(`## ${name}`, '', `Tier: **${domainTierLabel(name)}**${reason(name)}`, '');
    toolTable(tools);
  }

  lines.push(
    '## agentic-runtime',
    '',
    'Server-level tools that belong to no domain (planning, simulation, replay, events, discovery).',
    '',
  );
  toolTable(AGENTIC_RUNTIME_TOOLS);

  if (!TOOL_TIERS.every((tier) => tierCount(tier) > 0)) {
    throw new Error('generate-tool-docs: a tier has no tools; check src/tools/tool-tiers.js');
  }

  return `${lines.join('\n').trimEnd()}\n`;
}

const invokedDirectly =
  process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  const markdown = buildToolDocs();
  if (process.argv.includes('--stdout')) {
    process.stdout.write(markdown);
  } else {
    mkdirSync(dirname(TOOLS_DOC_PATH), { recursive: true });
    writeFileSync(TOOLS_DOC_PATH, markdown);
    console.log(`Wrote ${TOOLS_DOC_PATH}`);
  }
}
