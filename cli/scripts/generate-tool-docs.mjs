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

const __dirname = dirname(fileURLToPath(import.meta.url));
export const TOOLS_DOC_PATH = resolve(__dirname, '..', 'docs', 'TOOLS.md');

function escapeCell(text) {
  return String(text ?? '')
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

  const lines = [
    '# StateSet CLI Tool Catalog',
    '',
    '<!-- GENERATED FILE — do not edit by hand. -->',
    '<!-- Regenerate with: npm run docs:tools (from cli/) -->',
    '',
    `Source of truth: \`cli/src/tools/domain-registry.js\`.`,
    '',
    `**${totalTools} tools** across **${domains.length} domains**.`,
    '',
    '## Domains',
    '',
    '| Domain | Tools |',
    '| --- | ---: |',
    ...domains.map(([name, tools]) => `| [${name}](#${name.replace(/[^a-z0-9-]/g, '')}) | ${tools.length} |`),
    '',
  ];

  for (const [name, tools] of domains) {
    lines.push(`## ${name}`, '', '| Tool | Permission | Description |', '| --- | --- | --- |');
    for (const tool of tools) {
      lines.push(
        `| \`${escapeCell(tool.name)}\` | ${escapeCell(tool.permission ?? '—')} | ${escapeCell(tool.description)} |`,
      );
    }
    lines.push('');
  }

  return `${lines.join('\n').trimEnd()}\n`;
}

const invokedDirectly = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
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
