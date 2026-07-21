// Verifies the committed docs/TOOLS.md matches a fresh regeneration from the
// domain registry (regenerate-and-diff, like a lockfile check).
//
// If this fails, run: npm run docs:tools (from cli/) and commit the result.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { buildToolDocs, TOOLS_DOC_PATH } from '../../scripts/generate-tool-docs.mjs';
import { DOMAIN_TOOL_ENTRIES, ALL_DOMAIN_TOOLS } from '../../src/tools/domain-registry.js';

test('docs/TOOLS.md is up to date with the domain registry', () => {
  let committed;
  try {
    committed = readFileSync(TOOLS_DOC_PATH, 'utf8');
  } catch {
    assert.fail(`docs/TOOLS.md is missing — run "npm run docs:tools" and commit it (${TOOLS_DOC_PATH})`);
  }
  assert.equal(
    committed,
    buildToolDocs(),
    'docs/TOOLS.md is stale — run "npm run docs:tools" (from cli/) and commit the result',
  );
});

test('generated catalog states the real registry totals', () => {
  const markdown = buildToolDocs();
  assert.ok(
    markdown.includes(`**${ALL_DOMAIN_TOOLS.length} tools** across **${DOMAIN_TOOL_ENTRIES.length} domains**`),
    'catalog header must state the registry tool/domain counts',
  );
  for (const [domain] of DOMAIN_TOOL_ENTRIES) {
    assert.ok(markdown.includes(`## ${domain}\n`), `missing domain section: ${domain}`);
  }
});
