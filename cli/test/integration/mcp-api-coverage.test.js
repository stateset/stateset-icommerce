/**
 * Integration test for shared MCP API coverage.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  AUDITED_CLASS_METHOD_TOOL_COVERAGE,
  buildMcpApiCoverage,
} from '../../src/coverage/mcp-api-coverage.js';

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

  it('does not credit a tool with a method it never calls', () => {
    const coverage = buildMcpApiCoverage();
    const payments = AUDITED_CLASS_METHOD_TOOL_COVERAGE.Payments;
    // create_refund records a pending refund; nothing in the tool surface settles it.
    for (const method of ['completeRefund', 'failRefund']) {
      assert.equal(payments.methodToTools[method], undefined, method);
      assert.ok(
        coverage.unexposedAuditedMethods.some(
          (entry) => entry.method === `Payments.${method}` && entry.reason.length > 0,
        ),
        method,
      );
    }
  });
});
