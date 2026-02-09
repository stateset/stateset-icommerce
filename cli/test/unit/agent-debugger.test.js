/**
 * Unit tests for agent-debugger.js — AgentDebugger
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { AgentDebugger } from '../../src/agent-debugger.js';

// ===========================================================================
// Constructor
// ===========================================================================

describe('AgentDebugger — constructor', () => {
  it('is an instance of EventEmitter', () => {
    const dbg = new AgentDebugger(null);
    assert.ok(dbg instanceof EventEmitter);
  });

  it('stores the commerce object', () => {
    const commerce = { inventory: {} };
    const dbg = new AgentDebugger(commerce);
    assert.strictEqual(dbg.commerce, commerce);
  });

  it('initializes debugSessions as empty Map', () => {
    const dbg = new AgentDebugger(null);
    assert.ok(dbg.debugSessions instanceof Map);
    assert.strictEqual(dbg.debugSessions.size, 0);
  });

  it('initializes errorPatterns Map with 8 entries', () => {
    const dbg = new AgentDebugger(null);
    assert.ok(dbg.errorPatterns instanceof Map);
    assert.strictEqual(dbg.errorPatterns.size, 8);
  });

  it('initializes solutions Map', () => {
    const dbg = new AgentDebugger(null);
    assert.ok(dbg.solutions instanceof Map);
    assert.ok(dbg.solutions.size > 0);
  });

  it('has the expected error pattern keys', () => {
    const dbg = new AgentDebugger(null);
    const expectedKeys = [
      'insufficient_stock',
      'order_not_found',
      'customer_not_found',
      'invalid_status_transition',
      'payment_failed',
      'duplicate_email',
      'validation_error',
      'reservation_expired',
    ];
    for (const key of expectedKeys) {
      assert.ok(dbg.errorPatterns.has(key), `Missing pattern: ${key}`);
    }
  });
});

// ===========================================================================
// analyzeError
// ===========================================================================

describe('AgentDebugger — analyzeError', () => {
  let dbg;

  beforeEach(() => {
    dbg = new AgentDebugger(null);
  });

  it('matches "insufficient stock" to insufficient_stock pattern', async () => {
    const result = await dbg.analyzeError(new Error('insufficient stock for SKU-001'));
    assert.strictEqual(result.pattern.name, 'insufficient_stock');
    assert.strictEqual(result.pattern.category, 'inventory');
  });

  it('matches "order not found" to order_not_found pattern', async () => {
    const result = await dbg.analyzeError(new Error('order not found'));
    assert.strictEqual(result.pattern.name, 'order_not_found');
    assert.strictEqual(result.pattern.category, 'orders');
  });

  it('matches "invalid transition" to invalid_status_transition pattern', async () => {
    const result = await dbg.analyzeError(new Error('invalid status transition'));
    assert.strictEqual(result.pattern.name, 'invalid_status_transition');
  });

  it('matches "cannot transition" to invalid_status_transition pattern', async () => {
    const result = await dbg.analyzeError(new Error('cannot transition from pending to shipped'));
    assert.strictEqual(result.pattern.name, 'invalid_status_transition');
  });

  it('matches "payment failed" to payment_failed pattern', async () => {
    const result = await dbg.analyzeError(new Error('payment processing failed'));
    assert.strictEqual(result.pattern.name, 'payment_failed');
  });

  it('matches "customer not found" to customer_not_found pattern', async () => {
    const result = await dbg.analyzeError(new Error('customer not found'));
    assert.strictEqual(result.pattern.name, 'customer_not_found');
  });

  it('matches "email already exists" to duplicate_email pattern', async () => {
    const result = await dbg.analyzeError(new Error('email already exists'));
    assert.strictEqual(result.pattern.name, 'duplicate_email');
  });

  it('matches "validation error" to validation_error pattern', async () => {
    const result = await dbg.analyzeError(new Error('validation error on field'));
    assert.strictEqual(result.pattern.name, 'validation_error');
  });

  it('matches "reservation expired" to reservation_expired pattern', async () => {
    const result = await dbg.analyzeError(new Error('reservation has expired'));
    assert.strictEqual(result.pattern.name, 'reservation_expired');
  });

  it('returns pattern:null for unrecognized errors', async () => {
    const result = await dbg.analyzeError(new Error('something random happened'));
    assert.strictEqual(result.pattern, null);
  });

  it('includes error message and type in analysis', async () => {
    const err = new TypeError('bad input');
    const result = await dbg.analyzeError(err);
    assert.strictEqual(result.error, 'bad input');
    assert.strictEqual(result.errorType, 'TypeError');
  });

  it('includes timestamp in analysis', async () => {
    const result = await dbg.analyzeError(new Error('test'));
    assert.ok(result.timestamp);
    assert.ok(/^\d{4}-\d{2}-\d{2}T/.test(result.timestamp));
  });

  it('emits error:analyzed event', async () => {
    let emitted = false;
    dbg.on('error:analyzed', () => {
      emitted = true;
    });
    await dbg.analyzeError(new Error('test'));
    assert.ok(emitted);
  });
});

// ===========================================================================
// generateTechnicalExplanation
// ===========================================================================

describe('AgentDebugger — generateTechnicalExplanation', () => {
  let dbg;

  beforeEach(() => {
    dbg = new AgentDebugger(null);
  });

  it('returns explanation for insufficient_stock', () => {
    const result = dbg.generateTechnicalExplanation(new Error('test'), {
      name: 'insufficient_stock',
    });
    assert.ok(result.includes('insufficient inventory'));
  });

  it('returns explanation for invalid_status_transition', () => {
    const result = dbg.generateTechnicalExplanation(new Error('test'), {
      name: 'invalid_status_transition',
    });
    assert.ok(result.includes('state machine'));
  });

  it('returns explanation for order_not_found', () => {
    const result = dbg.generateTechnicalExplanation(new Error('test'), { name: 'order_not_found' });
    assert.ok(result.includes('could not be found'));
  });

  it('returns explanation for payment_failed', () => {
    const result = dbg.generateTechnicalExplanation(new Error('test'), { name: 'payment_failed' });
    assert.ok(result.includes('payment'));
  });

  it('returns generic explanation for null pattern', () => {
    const result = dbg.generateTechnicalExplanation(new Error('something'), null);
    assert.ok(result.includes('Unknown error'));
    assert.ok(result.includes('something'));
  });

  it('returns fallback for unmatched pattern name', () => {
    const result = dbg.generateTechnicalExplanation(new Error('test'), {
      name: 'unknown_pattern',
      category: 'misc',
      context: 'test context',
    });
    assert.ok(result.includes('misc'));
  });
});

// ===========================================================================
// analyzeToolContext
// ===========================================================================

describe('AgentDebugger — analyzeToolContext', () => {
  let dbg;

  beforeEach(() => {
    dbg = new AgentDebugger(null);
  });

  it('returns context for create_order', () => {
    const ctx = dbg.analyzeToolContext('create_order', new Error('test'));
    assert.ok(ctx.preamble.includes('creating a new order'));
    assert.ok(Array.isArray(ctx.commonCauses));
    assert.ok(Array.isArray(ctx.nextSteps));
  });

  it('returns context for reserve_inventory', () => {
    const ctx = dbg.analyzeToolContext('reserve_inventory', new Error('test'));
    assert.ok(ctx.preamble.includes('reserving inventory'));
  });

  it('returns context for update_order_status', () => {
    const ctx = dbg.analyzeToolContext('update_order_status', new Error('test'));
    assert.ok(ctx.preamble.includes('updating order status'));
  });

  it('returns context for process_payment', () => {
    const ctx = dbg.analyzeToolContext('process_payment', new Error('test'));
    assert.ok(ctx.preamble.includes('processing payment'));
  });

  it('returns generic context for unknown tool', () => {
    const ctx = dbg.analyzeToolContext('unknown_tool', new Error('test'));
    assert.ok(ctx.preamble.includes('unknown_tool'));
    assert.ok(ctx.commonCauses.length > 0);
  });
});

// ===========================================================================
// generateRecoveryExamples
// ===========================================================================

describe('AgentDebugger — generateRecoveryExamples', () => {
  let dbg;

  beforeEach(() => {
    dbg = new AgentDebugger(null);
  });

  it('returns examples for insufficient_stock + create_order', () => {
    const examples = dbg.generateRecoveryExamples(
      new Error('insufficient stock'),
      { name: 'insufficient_stock' },
      { tool: 'create_order' },
    );
    assert.ok(examples.length > 0);
    assert.ok(examples[0].title.includes('Stock'));
  });

  it('returns examples for invalid_status_transition + update_order_status', () => {
    const examples = dbg.generateRecoveryExamples(
      new Error('invalid transition'),
      { name: 'invalid_status_transition' },
      { tool: 'update_order_status' },
    );
    assert.ok(examples.length > 0);
    assert.ok(examples[0].title.includes('Status'));
  });

  it('returns empty array when pattern and tool do not match any example', () => {
    const examples = dbg.generateRecoveryExamples(
      new Error('test'),
      { name: 'payment_failed' },
      { tool: 'create_order' },
    );
    assert.deepStrictEqual(examples, []);
  });

  it('returns empty array for null pattern', () => {
    const examples = dbg.generateRecoveryExamples(new Error('test'), null, {});
    assert.deepStrictEqual(examples, []);
  });
});

// ===========================================================================
// attemptAutoRecovery
// ===========================================================================

describe('AgentDebugger — attemptAutoRecovery', () => {
  let dbg;

  beforeEach(() => {
    dbg = new AgentDebugger(null);
  });

  it('insufficient_stock returns canAutoRecover:false', async () => {
    const result = await dbg.attemptAutoRecovery(new Error('insufficient stock'), {});
    assert.strictEqual(result.canAutoRecover, false);
    assert.ok(result.reason);
  });

  it('invalid_status_transition with currentStatus returns canAutoRecover:true', async () => {
    const result = await dbg.attemptAutoRecovery(new Error('invalid status transition'), {
      currentStatus: 'pending',
      orderId: 'ORD-123',
    });
    assert.strictEqual(result.canAutoRecover, true);
    assert.strictEqual(result.strategy, 'retry_with_valid_status');
  });

  it('invalid_status_transition without currentStatus returns canAutoRecover:false', async () => {
    const result = await dbg.attemptAutoRecovery(new Error('invalid status transition'), {});
    assert.strictEqual(result.canAutoRecover, false);
  });

  it('unknown pattern returns canAutoRecover:false', async () => {
    const result = await dbg.attemptAutoRecovery(new Error('something random'), {});
    assert.strictEqual(result.canAutoRecover, false);
    assert.ok(result.reason.includes('Unknown'));
  });

  it('order_not_found returns canAutoRecover:false with suggestion', async () => {
    const result = await dbg.attemptAutoRecovery(new Error('order not found'), {});
    assert.strictEqual(result.canAutoRecover, false);
    assert.ok(result.suggestion || result.reason);
  });
});

// ===========================================================================
// createDebugSession
// ===========================================================================

describe('AgentDebugger — createDebugSession', () => {
  let dbg;

  beforeEach(() => {
    dbg = new AgentDebugger(null);
  });

  it('creates a session with a string ID starting with "debug-"', () => {
    const session = dbg.createDebugSession();
    assert.ok(session.id.startsWith('debug-'));
  });

  it('stores session in debugSessions Map', () => {
    const session = dbg.createDebugSession();
    assert.ok(dbg.debugSessions.has(session.id));
    assert.strictEqual(dbg.debugSessions.get(session.id), session);
  });

  it('sets status to active', () => {
    const session = dbg.createDebugSession();
    assert.strictEqual(session.status, 'active');
  });

  it('stores provided context', () => {
    const ctx = { tool: 'create_order', orderId: '123' };
    const session = dbg.createDebugSession(ctx);
    assert.deepStrictEqual(session.context, ctx);
  });

  it('emits debug:session:created event', () => {
    let emitted = null;
    dbg.on('debug:session:created', (s) => {
      emitted = s;
    });
    const session = dbg.createDebugSession();
    assert.strictEqual(emitted, session);
  });

  it('initializes errors as empty array', () => {
    const session = dbg.createDebugSession();
    assert.deepStrictEqual(session.errors, []);
  });
});

// ===========================================================================
// addErrorToSession
// ===========================================================================

describe('AgentDebugger — addErrorToSession', () => {
  let dbg;

  beforeEach(() => {
    dbg = new AgentDebugger(null);
  });

  it('adds error to an existing session', () => {
    const session = dbg.createDebugSession();
    dbg.addErrorToSession(session.id, new Error('test error'), { tool: 'x' });
    assert.strictEqual(session.errors.length, 1);
    assert.strictEqual(session.errors[0].error, 'test error');
  });

  it('throws for nonexistent session', () => {
    assert.throws(() => dbg.addErrorToSession('nonexistent', new Error('test'), {}), /not found/);
  });

  it('emits debug:session:error_added event', () => {
    const session = dbg.createDebugSession();
    let emitted = null;
    dbg.on('debug:session:error_added', (data) => {
      emitted = data;
    });
    dbg.addErrorToSession(session.id, new Error('test'), {});
    assert.ok(emitted);
    assert.strictEqual(emitted.sessionId, session.id);
  });
});

// ===========================================================================
// diagnoseSession
// ===========================================================================

describe('AgentDebugger — diagnoseSession', () => {
  let dbg;

  beforeEach(() => {
    dbg = new AgentDebugger(null);
  });

  it('returns diagnoses for session errors', async () => {
    const session = dbg.createDebugSession();
    dbg.addErrorToSession(session.id, new Error('insufficient stock'), {});
    dbg.addErrorToSession(session.id, new Error('order not found'), {});
    const result = await dbg.diagnoseSession(session.id);
    assert.strictEqual(result.errorCount, 2);
    assert.strictEqual(result.diagnoses.length, 2);
  });

  it('throws for nonexistent session', async () => {
    await assert.rejects(() => dbg.diagnoseSession('nonexistent'), /not found/);
  });

  it('returns empty diagnoses for session with no errors', async () => {
    const session = dbg.createDebugSession();
    const result = await dbg.diagnoseSession(session.id);
    assert.strictEqual(result.errorCount, 0);
    assert.deepStrictEqual(result.diagnoses, []);
  });
});

// ===========================================================================
// generateSessionRecommendations
// ===========================================================================

describe('AgentDebugger — generateSessionRecommendations', () => {
  let dbg;

  beforeEach(() => {
    dbg = new AgentDebugger(null);
  });

  it('identifies repeated patterns', () => {
    const diagnoses = [
      { pattern: { name: 'insufficient_stock' } },
      { pattern: { name: 'insufficient_stock' } },
      { pattern: { name: 'order_not_found' } },
    ];
    const recs = dbg.generateSessionRecommendations(diagnoses);
    assert.strictEqual(recs.length, 1);
    assert.strictEqual(recs[0].issue, 'insufficient_stock');
    assert.strictEqual(recs[0].count, 2);
  });

  it('returns empty when no patterns repeat', () => {
    const diagnoses = [
      { pattern: { name: 'insufficient_stock' } },
      { pattern: { name: 'order_not_found' } },
    ];
    const recs = dbg.generateSessionRecommendations(diagnoses);
    assert.strictEqual(recs.length, 0);
  });

  it('handles diagnoses with null patterns', () => {
    const diagnoses = [{ pattern: null }, { pattern: null }];
    const recs = dbg.generateSessionRecommendations(diagnoses);
    assert.strictEqual(recs.length, 0);
  });

  it('includes severity from errorPatterns', () => {
    const diagnoses = [
      { pattern: { name: 'insufficient_stock' } },
      { pattern: { name: 'insufficient_stock' } },
    ];
    const recs = dbg.generateSessionRecommendations(diagnoses);
    assert.strictEqual(recs[0].severity, 'high');
  });
});
