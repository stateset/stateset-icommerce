/**
 * Unit tests for the RFQ Competition demo scenario
 *
 * Tests cli/src/a2a/demo-scenarios.js:
 *   - DEMO_SCENARIOS registry includes 'rfq-competition'
 *   - runDemoScenario('rfq-competition', ...) routing
 *   - runRFQCompetition() return shape and field types
 *   - Seller contact and quote provision
 *   - Winner selection and loser declination
 *   - Score positivity
 *   - Custom log function (silencing output)
 *   - Edge cases (unknown scenario)
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { A2AStore } from '../../src/a2a/store.js';
import { makeCommerceProxy } from '../../src/a2a/agent-runtime.js';
import {
  runRFQCompetition,
  runDemoScenario,
  DEMO_SCENARIOS,
} from '../../src/a2a/demo-scenarios.js';

// ===========================================================================
// Helpers
// ===========================================================================

/** Silent logger — swallows all output */
const noop = () => {};

function setup() {
  const store = new A2AStore({ dbPath: ':memory:' });
  store.init();
  const commerce = makeCommerceProxy(store);
  return { store, commerce };
}

// ===========================================================================
// Tests
// ===========================================================================

describe('RFQ Competition demo scenario', () => {
  let store;
  let commerce;
  let result;

  beforeEach(async () => {
    ({ store, commerce } = setup());
    result = await runRFQCompetition(commerce, { log: noop });
  });

  afterEach(() => {
    try {
      store.close();
    } catch {
      // Already closed or never opened
    }
  });

  // -----------------------------------------------------------------------
  // 1. DEMO_SCENARIOS includes 'rfq-competition'
  // -----------------------------------------------------------------------

  it('DEMO_SCENARIOS includes rfq-competition', () => {
    assert.ok(
      Array.isArray(DEMO_SCENARIOS),
      'DEMO_SCENARIOS should be an array',
    );
    assert.ok(
      DEMO_SCENARIOS.includes('rfq-competition'),
      'DEMO_SCENARIOS should include rfq-competition',
    );
  });

  // -----------------------------------------------------------------------
  // 2. runDemoScenario routes correctly
  // -----------------------------------------------------------------------

  it('runDemoScenario routes rfq-competition to runRFQCompetition', async () => {
    const s = setup();
    try {
      const routed = await runDemoScenario('rfq-competition', s.commerce, {
        log: noop,
      });
      assert.strictEqual(routed.scenario, 'rfq-competition');
      assert.ok(routed.rfqId, 'routed result should have rfqId');
    } finally {
      s.store.close();
    }
  });

  // -----------------------------------------------------------------------
  // 3. Return shape
  // -----------------------------------------------------------------------

  it('returns an object with scenario field set to rfq-competition', () => {
    assert.strictEqual(result.scenario, 'rfq-competition');
  });

  it('returns a non-empty rfqId string', () => {
    assert.strictEqual(typeof result.rfqId, 'string');
    assert.ok(result.rfqId.length > 0, 'rfqId should not be empty');
  });

  it('returns sellersContacted as a positive number', () => {
    assert.strictEqual(typeof result.sellersContacted, 'number');
    assert.ok(result.sellersContacted > 0, 'sellersContacted should be > 0');
  });

  it('returns scoredCount as a non-negative number', () => {
    assert.strictEqual(typeof result.scoredCount, 'number');
    assert.ok(result.scoredCount >= 0, 'scoredCount should be >= 0');
  });

  it('returns winnerId as a string or null', () => {
    assert.ok(
      result.winnerId === null || typeof result.winnerId === 'string',
      'winnerId should be string or null',
    );
  });

  it('returns winnerAddress as a string or null', () => {
    assert.ok(
      result.winnerAddress === null || typeof result.winnerAddress === 'string',
      'winnerAddress should be string or null',
    );
  });

  it('returns winnerScore as a number or null', () => {
    assert.ok(
      result.winnerScore === null || typeof result.winnerScore === 'number',
      'winnerScore should be number or null',
    );
  });

  it('returns losersDeclined as a number', () => {
    assert.strictEqual(typeof result.losersDeclined, 'number');
  });

  // -----------------------------------------------------------------------
  // 4. Sellers are contacted and provide quotes
  // -----------------------------------------------------------------------

  it('contacts exactly 5 sellers (matching the config)', () => {
    assert.strictEqual(result.sellersContacted, 5);
  });

  it('scores all contacted sellers after tick()', () => {
    assert.strictEqual(
      result.scoredCount,
      result.sellersContacted,
      'scoredCount should equal sellersContacted after all sellers tick',
    );
  });

  // -----------------------------------------------------------------------
  // 5. A winner is selected
  // -----------------------------------------------------------------------

  it('selects a winner with a non-null winnerId', () => {
    assert.ok(result.winnerId !== null, 'winnerId should be non-null');
    assert.strictEqual(typeof result.winnerId, 'string');
  });

  it('selects a winner with a non-null winnerAddress', () => {
    assert.ok(result.winnerAddress !== null, 'winnerAddress should be non-null');
    assert.ok(
      result.winnerAddress.startsWith('0x'),
      'winnerAddress should be a hex wallet address',
    );
  });

  // -----------------------------------------------------------------------
  // 6. Multiple sellers are declined
  // -----------------------------------------------------------------------

  it('declines at least 1 loser', () => {
    assert.ok(result.losersDeclined >= 1, 'at least 1 loser should be declined');
  });

  it('declines exactly sellersContacted - 1 losers', () => {
    assert.strictEqual(
      result.losersDeclined,
      result.sellersContacted - 1,
      'losersDeclined should equal sellersContacted - 1',
    );
  });

  // -----------------------------------------------------------------------
  // 7. Scores are positive
  // -----------------------------------------------------------------------

  it('winner has a positive score', () => {
    assert.ok(result.winnerScore > 0, 'winnerScore should be positive');
  });

  // -----------------------------------------------------------------------
  // 8. Custom log function
  // -----------------------------------------------------------------------

  it('works with a custom log function that captures messages', async () => {
    const messages = [];
    const s = setup();
    try {
      await runRFQCompetition(s.commerce, {
        log: (msg) => messages.push(msg),
      });
      assert.ok(messages.length > 0, 'should have captured log messages');
      assert.ok(
        messages.some((m) => typeof m === 'string' && m.includes('[demo]')),
        'at least one message should contain [demo] prefix',
      );
    } finally {
      s.store.close();
    }
  });

  it('captures seller registration log messages', async () => {
    const messages = [];
    const s = setup();
    try {
      await runRFQCompetition(s.commerce, {
        log: (msg) => messages.push(String(msg)),
      });
      const registrationMsgs = messages.filter((m) => m.includes('registered service'));
      assert.strictEqual(
        registrationMsgs.length,
        5,
        'should log 5 service registrations',
      );
    } finally {
      s.store.close();
    }
  });

  it('captures the RFQ broadcast log message', async () => {
    const messages = [];
    const s = setup();
    try {
      await runRFQCompetition(s.commerce, {
        log: (msg) => messages.push(String(msg)),
      });
      assert.ok(
        messages.some((m) => m.includes('RFQ broadcast')),
        'should log the RFQ broadcast event',
      );
    } finally {
      s.store.close();
    }
  });

  it('captures the award log message', async () => {
    const messages = [];
    const s = setup();
    try {
      await runRFQCompetition(s.commerce, {
        log: (msg) => messages.push(String(msg)),
      });
      assert.ok(
        messages.some((m) => m.includes('Awarded to')),
        'should log the award event',
      );
    } finally {
      s.store.close();
    }
  });

  // -----------------------------------------------------------------------
  // 9. Edge cases — unknown scenario throws
  // -----------------------------------------------------------------------

  it('runDemoScenario throws on unknown scenario name', async () => {
    await assert.rejects(
      () => runDemoScenario('nonexistent-scenario', commerce, { log: noop }),
      (err) => {
        assert.ok(err instanceof Error);
        assert.ok(
          err.message.includes('Unknown demo scenario'),
          `Error message should mention unknown scenario, got: ${err.message}`,
        );
        assert.ok(
          err.message.includes('nonexistent-scenario'),
          'Error message should include the bad scenario name',
        );
        return true;
      },
    );
  });

  it('runDemoScenario error message lists available scenarios', async () => {
    await assert.rejects(
      () => runDemoScenario('bad-name', commerce, { log: noop }),
      (err) => {
        for (const name of DEMO_SCENARIOS) {
          assert.ok(
            err.message.includes(name),
            `Error message should list available scenario: ${name}`,
          );
        }
        return true;
      },
    );
  });
});
