/**
 * Unit tests for agent-router.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { routeToAgent, routeToAgentWithConfidence } from '../../src/agent-router.js';

describe('routeToAgent', () => {
  it('routes tax-specific requests to tax agent', () => {
    const agent = routeToAgent('Calculate sales tax and nexus implications for this checkout');
    assert.strictEqual(agent, 'tax');
  });

  it('routes analytics-heavy requests to analytics agent', () => {
    const agent = routeToAgent(
      'Build a sales report with revenue forecast and top products for last quarter',
    );
    assert.strictEqual(agent, 'analytics');
  });

  it('defaults to customer-service when no keywords match', () => {
    const agent = routeToAgent('hello there');
    assert.strictEqual(agent, 'customer-service');
  });
});

describe('routeToAgentWithConfidence', () => {
  it('returns score metadata and alternatives', () => {
    const result = routeToAgentWithConfidence(
      'Create a shipment and track package delivery with carrier updates',
    );

    assert.ok(result.primary);
    assert.ok(typeof result.primary.score === 'number');
    assert.ok(typeof result.primary.confidence === 'number');
    assert.ok(Array.isArray(result.alternatives));
    assert.ok(result.thresholds && typeof result.thresholds.MIN_SCORE === 'number');
  });

  it('flags ambiguous routing when competing intents are close', () => {
    const result = routeToAgentWithConfidence('I need a refund for this order');
    assert.strictEqual(result.ambiguous, true);
    assert.strictEqual(result.primary.agent, 'returns');
  });

  it('returns default-level primary result when no meaningful match exists', () => {
    const result = routeToAgentWithConfidence('just saying hi');
    assert.strictEqual(result.primary.agent, 'customer-service');
    assert.strictEqual(result.primary.level, 'default');
    assert.ok(result.primary.reason.includes('No specific agent matched'));
  });
});
