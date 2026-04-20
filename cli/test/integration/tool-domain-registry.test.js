/**
 * Integration test for shared domain tool registry coverage.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  DOMAIN_TOOL_ARRAYS,
  TOOL_MODULE_NAMES,
  ALL_DOMAIN_TOOLS,
  TOOL_POLICY_DOMAIN_BY_NAME,
} from '../../src/tools/domain-registry.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const toolsDir = path.resolve(__dirname, '../../src/tools');

function listToolModulesOnDisk() {
  return fs
    .readdirSync(toolsDir)
    .filter((entry) => entry.endsWith('.js') && !['index.js', 'domain-registry.js'].includes(entry))
    .map((entry) => entry.replace(/\.js$/, ''))
    .sort();
}

describe('tool domain registry', () => {
  it('should cover every top-level tool module file on disk', () => {
    assert.deepStrictEqual([...TOOL_MODULE_NAMES].sort(), listToolModulesOnDisk());
  });

  it('should expose every registry module through DOMAIN_TOOL_ARRAYS', () => {
    assert.deepStrictEqual(Object.keys(DOMAIN_TOOL_ARRAYS).sort(), [...TOOL_MODULE_NAMES].sort());
  });

  it('should provide a policy domain for every domain tool', () => {
    const uncoveredTools = ALL_DOMAIN_TOOLS.filter((tool) => !TOOL_POLICY_DOMAIN_BY_NAME[tool.name]).map(
      (tool) => tool.name,
    );
    assert.deepStrictEqual(uncoveredTools, []);
  });
});
