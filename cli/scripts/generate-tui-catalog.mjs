#!/usr/bin/env node
// The workbench loads metadata only: never import executable tool handlers there.
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { DOMAIN_TOOL_ENTRIES } from '../src/tools/domain-registry.js';
import { domainTierLabel, toolTier, EXPERIMENTAL_REASONS } from '../src/tools/tool-tiers.js';
import { AGENTIC_RUNTIME_TOOLS } from '../src/mcp/agentic-runtime-tools.js';

export const CATALOG_PATH = fileURLToPath(new URL('../src/tui/catalog.json', import.meta.url));
export function buildTuiCatalog() {
  return (
    JSON.stringify(
      [...DOMAIN_TOOL_ENTRIES, ['agentic-runtime', AGENTIC_RUNTIME_TOOLS]].map(([id, tools]) => ({
        id,
        tier: id === 'agentic-runtime' ? 'mixed' : domainTierLabel(id),
        reason: EXPERIMENTAL_REASONS[id] || '',
        tools: tools.map((tool) => ({
          name: tool.name,
          tier: toolTier(tool.name),
          permission: tool.permission || 'unspecified',
          description: tool.description,
          reason: EXPERIMENTAL_REASONS[tool.name] || '',
        })),
      })),
      null,
      2,
    ) + '\n'
  );
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const content = buildTuiCatalog();
  if (process.argv.includes('--check')) {
    if (readFileSync(CATALOG_PATH, 'utf8') !== content)
      throw new Error('TUI catalog is stale; run node cli/scripts/generate-tui-catalog.mjs');
  } else writeFileSync(CATALOG_PATH, content);
}
