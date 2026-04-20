/**
 * Integration test for shared MCP API coverage.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { buildMcpApiCoverage } from '../../src/coverage/mcp-api-coverage.js';

describe('mcp api coverage', () => {
  it('should have full getter and audited binding coverage', () => {
    const coverage = buildMcpApiCoverage();

    assert.strictEqual(coverage.uncoveredCommerceGetters.length, 0);
    assert.strictEqual(coverage.staleGetterMappings.length, 0);
    assert.strictEqual(coverage.uncoveredAuditedMethods.length, 0);
    assert.strictEqual(coverage.staleAuditedMethodMappings.length, 0);
    assert.strictEqual(coverage.invalidAuditedToolReferences.length, 0);
    assert.strictEqual(coverage.fullyCovered, true);
  });
});
