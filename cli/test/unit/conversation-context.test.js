/**
 * Tests for cli/src/mcp-conversation-context.js — ConversationContext
 *
 * Comprehensive coverage: constructor, session lifecycle, tool call recording,
 * operation recording, resource tracking, rollback (session + individual),
 * error context analysis, action suggestions, context/goals, multi-session.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { ConversationContext } from '../../src/mcp-conversation-context.js';

describe('ConversationContext', () => {
  /** @type {ConversationContext} */
  let ctx;

  beforeEach(() => {
    ctx = new ConversationContext(null);
  });

  // ==========================================================================
  // Constructor
  // ==========================================================================

  describe('constructor', () => {
    it('stores the commerce reference', () => {
      const commerce = { fake: true };
      const c = new ConversationContext(commerce);
      assert.strictEqual(c.commerce, commerce);
    });

    it('accepts null commerce for testing', () => {
      assert.strictEqual(ctx.commerce, null);
    });

    it('initialises sessions as empty Map', () => {
      assert.ok(ctx.sessions instanceof Map);
      assert.strictEqual(ctx.sessions.size, 0);
    });

    it('starts with null activeSessionId', () => {
      assert.strictEqual(ctx.activeSessionId, null);
    });

    it('is an EventEmitter', () => {
      assert.strictEqual(typeof ctx.on, 'function');
      assert.strictEqual(typeof ctx.emit, 'function');
    });
  });

  // ==========================================================================
  // createSession
  // ==========================================================================

  describe('createSession', () => {
    it('returns a session object with expected top-level fields', () => {
      const session = ctx.createSession();
      assert.ok(session.id);
      assert.ok(session.createdAt);
      assert.ok(session.lastActivityAt);
      assert.ok(Array.isArray(session.toolCallHistory));
      assert.ok(Array.isArray(session.operations));
      assert.ok(Array.isArray(session.rollbacks));
    });

    it('generates an ID matching session-{timestamp}-{uuid9chars}', () => {
      const session = ctx.createSession();
      assert.match(session.id, /^session-\d+-[a-f0-9-]{9}$/);
    });

    it('defaults metadata to empty object', () => {
      const session = ctx.createSession();
      assert.deepStrictEqual(session.metadata, {});
    });

    it('stores provided metadata', () => {
      const meta = { user: 'alice', intent: 'purchase' };
      const session = ctx.createSession(meta);
      assert.deepStrictEqual(session.metadata, meta);
    });

    it('sets activeSessionId to new session', () => {
      const session = ctx.createSession();
      assert.strictEqual(ctx.activeSessionId, session.id);
    });

    it('adds session to sessions map', () => {
      const session = ctx.createSession();
      assert.strictEqual(ctx.sessions.get(session.id), session);
    });

    it('emits session:created event with the session', () => {
      let emitted = null;
      ctx.on('session:created', (s) => { emitted = s; });
      const session = ctx.createSession();
      assert.strictEqual(emitted, session);
    });

    it('initialises empty state arrays and createdResources map', () => {
      const session = ctx.createSession();
      assert.deepStrictEqual(session.state.pendingOrders, []);
      assert.deepStrictEqual(session.state.reservedInventory, []);
      assert.deepStrictEqual(session.state.pendingPayments, []);
      assert.ok(session.state.createdResources instanceof Map);
      assert.strictEqual(session.state.createdResources.size, 0);
    });

    it('initialises context with null task and empty goals/constraints/preferences', () => {
      const session = ctx.createSession();
      assert.strictEqual(session.context.currentTask, null);
      assert.deepStrictEqual(session.context.goals, []);
      assert.deepStrictEqual(session.context.constraints, []);
      assert.deepStrictEqual(session.context.preferences, {});
    });
  });

  // ==========================================================================
  // getActiveSession
  // ==========================================================================

  describe('getActiveSession', () => {
    it('auto-creates a session when none is active', () => {
      const session = ctx.getActiveSession();
      assert.ok(session);
      assert.ok(session.id.startsWith('session-'));
      assert.strictEqual(ctx.activeSessionId, session.id);
    });

    it('returns the existing active session', () => {
      const created = ctx.createSession();
      const fetched = ctx.getActiveSession();
      assert.strictEqual(fetched, created);
    });
  });

  // ==========================================================================
  // recordToolCall
  // ==========================================================================

  describe('recordToolCall', () => {
    it('returns a tool call object with expected fields', () => {
      const result = { success: true, data: 'ok' };
      const tc = ctx.recordToolCall('list_products', { limit: 10 }, result);
      assert.match(tc.id, /^call-\d+-[a-f0-9-]{9}$/);
      assert.ok(tc.timestamp);
      assert.strictEqual(tc.tool, 'list_products');
      assert.deepStrictEqual(tc.params, { limit: 10 });
      assert.deepStrictEqual(tc.result, result);
      assert.strictEqual(tc.status, 'success');
    });

    it('marks status "error" when result.success is false', () => {
      const tc = ctx.recordToolCall('create_order', {}, { success: false, error: 'boom' });
      assert.strictEqual(tc.status, 'error');
    });

    it('marks status "success" when result.success is undefined', () => {
      const tc = ctx.recordToolCall('list_customers', {}, { data: [] });
      assert.strictEqual(tc.status, 'success');
    });

    it('stores optional duration, context, enrollmentId, and rollbackFn', () => {
      const rollbackFn = () => {};
      const tc = ctx.recordToolCall('get_stock', {}, { success: true }, {
        duration: 42,
        context: 'inventory check',
        enrollmentId: 'ENR-1',
        rollbackFn,
      });
      assert.strictEqual(tc.duration, 42);
      assert.strictEqual(tc.context, 'inventory check');
      assert.strictEqual(tc.enrollmentId, 'ENR-1');
      assert.strictEqual(tc.rollbackFn, rollbackFn);
    });

    it('pushes into session toolCallHistory', () => {
      ctx.createSession();
      ctx.recordToolCall('a', {}, { success: true });
      ctx.recordToolCall('b', {}, { success: true });
      assert.strictEqual(ctx.getActiveSession().toolCallHistory.length, 2);
    });

    it('updates lastActivityAt', () => {
      const session = ctx.createSession();
      const before = session.lastActivityAt;
      ctx.recordToolCall('list_products', {}, { success: true });
      assert.ok(session.lastActivityAt >= before);
    });

    it('emits tool:called for every call', () => {
      const events = [];
      ctx.on('tool:called', (e) => events.push(e));
      ctx.recordToolCall('list_products', {}, { success: true });
      ctx.recordToolCall('create_order', {}, { success: false, error: 'x' });
      assert.strictEqual(events.length, 2);
    });

    it('emits tool:succeeded for successful calls', () => {
      let emitted = null;
      ctx.on('tool:succeeded', (e) => { emitted = e; });
      ctx.recordToolCall('get_stock', {}, { success: true });
      assert.ok(emitted);
      assert.strictEqual(emitted.toolCall.tool, 'get_stock');
    });

    it('emits tool:failed with error for failed calls', () => {
      let emitted = null;
      ctx.on('tool:failed', (e) => { emitted = e; });
      ctx.recordToolCall('create_order', {}, { success: false, error: 'oops' });
      assert.ok(emitted);
      assert.strictEqual(emitted.error, 'oops');
    });

    it('auto-creates session if none exists', () => {
      assert.strictEqual(ctx.activeSessionId, null);
      ctx.recordToolCall('list_products', {}, { success: true });
      assert.ok(ctx.activeSessionId);
    });
  });

  // ==========================================================================
  // recordOperation
  // ==========================================================================

  describe('recordOperation', () => {
    it('returns an operation record with auto-generated id and timestamp', () => {
      const op = ctx.recordOperation({ type: 'create', resource: 'order' });
      assert.match(op.id, /^op-\d+-[a-f0-9-]{9}$/);
      assert.ok(op.timestamp);
      assert.strictEqual(op.type, 'create');
      assert.strictEqual(op.resource, 'order');
    });

    it('pushes into session operations list', () => {
      ctx.createSession();
      ctx.recordOperation({ type: 'create', resource: 'order' });
      ctx.recordOperation({ type: 'update', resource: 'inventory' });
      assert.strictEqual(ctx.getActiveSession().operations.length, 2);
    });

    it('emits operation:recorded with the operation', () => {
      let emitted = null;
      ctx.on('operation:recorded', (e) => { emitted = e; });
      ctx.recordOperation({ type: 'delete', resource: 'cart' });
      assert.ok(emitted);
      assert.strictEqual(emitted.operation.type, 'delete');
    });

    it('updates lastActivityAt', () => {
      const session = ctx.createSession();
      const before = session.lastActivityAt;
      ctx.recordOperation({ type: 'create', resource: 'product' });
      assert.ok(session.lastActivityAt >= before);
    });
  });

  // ==========================================================================
  // trackResource
  // ==========================================================================

  describe('trackResource', () => {
    it('adds resource to createdResources map with type, createdAt, rollback', () => {
      ctx.createSession();
      const rollback = () => {};
      ctx.trackResource('order', 'ORD-1', rollback);
      const tracked = ctx.getActiveSession().state.createdResources.get('ORD-1');
      assert.ok(tracked);
      assert.strictEqual(tracked.type, 'order');
      assert.strictEqual(tracked.rollback, rollback);
      assert.ok(tracked.createdAt);
    });

    it('adds reservation type to reservedInventory', () => {
      ctx.createSession();
      ctx.trackResource('reservation', 'RES-1', () => {});
      assert.ok(ctx.getActiveSession().state.reservedInventory.includes('RES-1'));
    });

    it('adds order type to pendingOrders', () => {
      ctx.createSession();
      ctx.trackResource('order', 'ORD-2', () => {});
      assert.ok(ctx.getActiveSession().state.pendingOrders.includes('ORD-2'));
    });

    it('adds payment type to pendingPayments', () => {
      ctx.createSession();
      ctx.trackResource('payment', 'PAY-1', () => {});
      assert.ok(ctx.getActiveSession().state.pendingPayments.includes('PAY-1'));
    });

    it('does not add unknown types to any specialised array', () => {
      ctx.createSession();
      ctx.trackResource('customer', 'CUST-1', () => {});
      const s = ctx.getActiveSession().state;
      assert.strictEqual(s.pendingOrders.length, 0);
      assert.strictEqual(s.reservedInventory.length, 0);
      assert.strictEqual(s.pendingPayments.length, 0);
    });

    it('emits resource:tracked with type and id', () => {
      let emitted = null;
      ctx.on('resource:tracked', (e) => { emitted = e; });
      ctx.trackResource('product', 'PROD-1', () => {});
      assert.ok(emitted);
      assert.strictEqual(emitted.resourceType, 'product');
      assert.strictEqual(emitted.resourceId, 'PROD-1');
    });
  });

  // ==========================================================================
  // rollbackSession
  // ==========================================================================

  describe('rollbackSession', () => {
    it('returns success with empty results when no resources to rollback', async () => {
      ctx.createSession();
      const result = await ctx.rollbackSession();
      assert.strictEqual(result.success, true);
      assert.strictEqual(result.message, 'No resources to rollback');
      assert.deepStrictEqual(result.results, []);
    });

    it('calls rollback functions and reports success results', async () => {
      ctx.createSession();
      let called = false;
      ctx.trackResource('order', 'ORD-1', async () => { called = true; return 'cancelled'; });
      const result = await ctx.rollbackSession();
      assert.strictEqual(called, true);
      assert.strictEqual(result.success, true);
      assert.strictEqual(result.results.length, 1);
      assert.strictEqual(result.results[0].status, 'success');
      assert.strictEqual(result.results[0].result, 'cancelled');
      assert.strictEqual(result.results[0].resourceId, 'ORD-1');
      assert.strictEqual(result.results[0].resourceType, 'order');
    });

    it('skips resources without a rollback function', async () => {
      ctx.createSession();
      ctx.getActiveSession().state.createdResources.set('RES-X', {
        type: 'misc',
        createdAt: new Date().toISOString(),
        rollback: null,
      });
      const result = await ctx.rollbackSession();
      assert.strictEqual(result.success, true);
      assert.strictEqual(result.results[0].status, 'skipped');
      assert.ok(result.results[0].reason);
    });

    it('reports failed rollbacks and sets success to false', async () => {
      ctx.createSession();
      ctx.trackResource('order', 'ORD-BAD', async () => { throw new Error('cannot cancel'); });
      const result = await ctx.rollbackSession();
      assert.strictEqual(result.success, false);
      assert.strictEqual(result.results[0].status, 'failed');
      assert.strictEqual(result.results[0].error, 'cannot cancel');
    });

    it('handles mix of successful and failed rollbacks', async () => {
      ctx.createSession();
      ctx.trackResource('order', 'ORD-OK', async () => 'done');
      ctx.trackResource('payment', 'PAY-BAD', async () => { throw new Error('fail'); });
      const result = await ctx.rollbackSession();
      assert.strictEqual(result.success, false);
      assert.strictEqual(result.results.length, 2);
      const statuses = result.results.map((r) => r.status);
      assert.ok(statuses.includes('success'));
      assert.ok(statuses.includes('failed'));
    });

    it('clears all state arrays after rollback', async () => {
      ctx.createSession();
      ctx.trackResource('reservation', 'RES-1', async () => {});
      ctx.trackResource('payment', 'PAY-1', async () => {});
      ctx.trackResource('order', 'ORD-1', async () => {});
      await ctx.rollbackSession();
      const s = ctx.getActiveSession().state;
      assert.strictEqual(s.createdResources.size, 0);
      assert.deepStrictEqual(s.pendingOrders, []);
      assert.deepStrictEqual(s.reservedInventory, []);
      assert.deepStrictEqual(s.pendingPayments, []);
    });

    it('emits rollback:started, rollback:success, and rollback:completed', async () => {
      const events = [];
      ctx.on('rollback:started', () => events.push('started'));
      ctx.on('rollback:success', () => events.push('success'));
      ctx.on('rollback:completed', () => events.push('completed'));
      ctx.createSession();
      ctx.trackResource('order', 'ORD-1', async () => {});
      await ctx.rollbackSession();
      assert.deepStrictEqual(events, ['started', 'success', 'completed']);
    });

    it('emits rollback:failed when a rollback throws', async () => {
      let failEvent = null;
      ctx.on('rollback:failed', (e) => { failEvent = e; });
      ctx.createSession();
      ctx.trackResource('order', 'ORD-BAD', async () => { throw new Error('boom'); });
      await ctx.rollbackSession();
      assert.ok(failEvent);
      assert.strictEqual(failEvent.resourceId, 'ORD-BAD');
    });

    it('includes resourceCount in rollback:started event', async () => {
      let startEvent = null;
      ctx.on('rollback:started', (e) => { startEvent = e; });
      ctx.createSession();
      ctx.trackResource('order', 'ORD-1', async () => {});
      ctx.trackResource('payment', 'PAY-1', async () => {});
      await ctx.rollbackSession();
      assert.strictEqual(startEvent.resourceCount, 2);
    });

    it('message includes resource count', async () => {
      ctx.createSession();
      ctx.trackResource('order', 'ORD-1', async () => {});
      ctx.trackResource('payment', 'PAY-1', async () => {});
      const result = await ctx.rollbackSession();
      assert.ok(result.message.includes('2'));
    });
  });

  // ==========================================================================
  // rollbackResource
  // ==========================================================================

  describe('rollbackResource', () => {
    it('returns failure for unknown resource', async () => {
      ctx.createSession();
      const result = await ctx.rollbackResource('NOPE');
      assert.strictEqual(result.success, false);
      assert.ok(result.error.includes('NOPE'));
    });

    it('calls rollback, removes from map, and returns success', async () => {
      ctx.createSession();
      let called = false;
      ctx.trackResource('order', 'ORD-1', async () => { called = true; return 'rolled-back'; });
      const result = await ctx.rollbackResource('ORD-1');
      assert.strictEqual(called, true);
      assert.strictEqual(result.success, true);
      assert.strictEqual(result.resourceType, 'order');
      assert.strictEqual(result.result, 'rolled-back');
      assert.strictEqual(ctx.getActiveSession().state.createdResources.has('ORD-1'), false);
    });

    it('removes reservation from reservedInventory', async () => {
      ctx.createSession();
      ctx.trackResource('reservation', 'RES-1', async () => {});
      await ctx.rollbackResource('RES-1');
      assert.strictEqual(ctx.getActiveSession().state.reservedInventory.length, 0);
    });

    it('removes payment from pendingPayments', async () => {
      ctx.createSession();
      ctx.trackResource('payment', 'PAY-1', async () => {});
      await ctx.rollbackResource('PAY-1');
      assert.strictEqual(ctx.getActiveSession().state.pendingPayments.length, 0);
    });

    it('removes order from pendingOrders', async () => {
      ctx.createSession();
      ctx.trackResource('order', 'ORD-1', async () => {});
      await ctx.rollbackResource('ORD-1');
      assert.strictEqual(ctx.getActiveSession().state.pendingOrders.length, 0);
    });

    it('returns failure with error message when rollback throws', async () => {
      ctx.createSession();
      ctx.trackResource('order', 'ORD-BAD', async () => { throw new Error('cannot undo'); });
      const result = await ctx.rollbackResource('ORD-BAD');
      assert.strictEqual(result.success, false);
      assert.strictEqual(result.error, 'cannot undo');
      assert.strictEqual(result.resourceType, 'order');
    });

    it('emits rollback:success on success', async () => {
      let emitted = null;
      ctx.on('rollback:success', (e) => { emitted = e; });
      ctx.createSession();
      ctx.trackResource('order', 'ORD-1', async () => {});
      await ctx.rollbackResource('ORD-1');
      assert.ok(emitted);
      assert.strictEqual(emitted.resourceId, 'ORD-1');
    });

    it('emits rollback:failed on error', async () => {
      let emitted = null;
      ctx.on('rollback:failed', (e) => { emitted = e; });
      ctx.createSession();
      ctx.trackResource('order', 'ORD-BAD', async () => { throw new Error('x'); });
      await ctx.rollbackResource('ORD-BAD');
      assert.ok(emitted);
      assert.strictEqual(emitted.resourceId, 'ORD-BAD');
    });
  });

  // ==========================================================================
  // getErrorContext
  // ==========================================================================

  describe('getErrorContext', () => {
    it('returns analysis with base fields', () => {
      const analysis = ctx.getErrorContext(new Error('something broke'), 'get_stock');
      assert.strictEqual(analysis.error, 'something broke');
      assert.strictEqual(analysis.tool, 'get_stock');
      assert.ok(analysis.timestamp);
      assert.ok(Array.isArray(analysis.recentActivity));
      assert.ok(Array.isArray(analysis.pendingResources));
      assert.ok(Array.isArray(analysis.suggestions));
    });

    it('suggests stock actions for "insufficient stock" error', () => {
      const analysis = ctx.getErrorContext(
        new Error('insufficient stock for SKU-123'),
        'reserve_inventory',
      );
      assert.ok(analysis.suggestions.some((s) => s.includes('get_stock')));
      assert.ok(analysis.suggestions.some((s) => s.includes('adjust_inventory')));
    });

    it('adds context when recent get_stock call exists for insufficient stock', () => {
      ctx.recordToolCall('get_stock', { sku: 'SKU-1' }, { success: true });
      const analysis = ctx.getErrorContext(
        new Error('insufficient stock'),
        'reserve_inventory',
      );
      assert.ok(analysis.context);
      assert.ok(analysis.context.includes('stock') || analysis.context.includes('inventory'));
    });

    it('suggests order lookup for "order not found" error', () => {
      const analysis = ctx.getErrorContext(new Error('order not found'), 'get_order');
      assert.ok(analysis.suggestions.some((s) => s.includes('list_orders')));
      assert.ok(analysis.suggestions.some((s) => s.includes('order ID')));
    });

    it('adds context mentioning recent order IDs for "order not found"', () => {
      ctx.recordToolCall('create_order', { orderId: 'ORD-1' }, { success: true });
      const analysis = ctx.getErrorContext(
        new Error('order not found'),
        'update_order_status',
      );
      assert.ok(analysis.context);
      assert.ok(analysis.context.includes('ORD-1'));
    });

    it('suggests status transitions for "invalid status transition" error', () => {
      const analysis = ctx.getErrorContext(
        new Error('invalid status transition'),
        'update_order_status',
      );
      assert.ok(analysis.suggestions.length >= 3);
      assert.ok(analysis.suggestions.some((s) => s.includes('get_order')));
      assert.ok(analysis.suggestions.some((s) => s.includes('prerequisites')));
    });

    it('suggests customer lookup for "customer not found" error', () => {
      const analysis = ctx.getErrorContext(
        new Error('customer not found'),
        'create_order',
      );
      assert.ok(analysis.suggestions.some((s) => s.includes('get_customer') || s.includes('list_customers')));
    });

    it('adds recommendation when "Create order" goal is set and customer not found', () => {
      ctx.addGoal('Create order');
      const analysis = ctx.getErrorContext(
        new Error('customer not found'),
        'create_order',
      );
      assert.ok(analysis.recommendation);
      assert.ok(analysis.recommendation.includes('create_customer'));
    });

    it('sets canRollback true when resources are tracked', () => {
      ctx.createSession();
      ctx.trackResource('order', 'ORD-1', () => {});
      const analysis = ctx.getErrorContext(new Error('fail'), 'get_order');
      assert.strictEqual(analysis.canRollback, true);
    });

    it('sets canRollback false when no resources are tracked', () => {
      ctx.createSession();
      const analysis = ctx.getErrorContext(new Error('fail'), 'get_order');
      assert.strictEqual(analysis.canRollback, false);
    });

    it('includes at most 5 recent activities', () => {
      ctx.createSession();
      for (let i = 0; i < 8; i++) {
        ctx.recordToolCall(`tool_${i}`, {}, { success: true });
      }
      const analysis = ctx.getErrorContext(new Error('x'), 'test');
      assert.strictEqual(analysis.recentActivity.length, 5);
    });

    it('maps pending resources to id/type/createdAt objects', () => {
      ctx.createSession();
      ctx.trackResource('order', 'ORD-1', () => {});
      const analysis = ctx.getErrorContext(new Error('test'), 'x');
      assert.strictEqual(analysis.pendingResources.length, 1);
      assert.strictEqual(analysis.pendingResources[0].id, 'ORD-1');
      assert.strictEqual(analysis.pendingResources[0].type, 'order');
      assert.ok(analysis.pendingResources[0].createdAt);
    });

    it('returns empty suggestions for unrecognised error patterns', () => {
      const analysis = ctx.getErrorContext(
        new Error('something completely unexpected'),
        'unknown_tool',
      );
      assert.deepStrictEqual(analysis.suggestions, []);
    });
  });

  // ==========================================================================
  // suggestNextActions
  // ==========================================================================

  describe('suggestNextActions', () => {
    it('returns default suggestions for empty history', () => {
      const suggestions = ctx.suggestNextActions();
      assert.strictEqual(suggestions.length, 2);
      assert.strictEqual(suggestions[0].action, 'list_products');
      assert.strictEqual(suggestions[0].priority, 'high');
      assert.strictEqual(suggestions[1].action, 'list_customers');
      assert.strictEqual(suggestions[1].priority, 'medium');
    });

    it('suggests get_product_variant after list_products', () => {
      ctx.recordToolCall('list_products', {}, { success: true });
      const suggestions = ctx.suggestNextActions();
      assert.ok(suggestions.some((s) => s.action === 'get_product_variant'));
    });

    it('suggests get_stock after get_product_variant', () => {
      ctx.recordToolCall('get_product_variant', { sku: 'SKU-1' }, { success: true });
      const suggestions = ctx.suggestNextActions();
      const stockSuggestion = suggestions.find((s) => s.action === 'get_stock');
      assert.ok(stockSuggestion);
      assert.strictEqual(stockSuggestion.params.sku, 'SKU-1');
    });

    it('suggests reserve_inventory after get_stock', () => {
      ctx.recordToolCall('get_stock', { sku: 'SKU-1' }, { success: true });
      const suggestions = ctx.suggestNextActions();
      assert.ok(suggestions.some((s) => s.action === 'reserve_inventory'));
    });

    it('suggests create_payment for pending orders', () => {
      ctx.createSession();
      ctx.trackResource('order', 'ORD-1', () => {});
      ctx.recordToolCall('create_order', {}, { success: true });
      const suggestions = ctx.suggestNextActions();
      assert.ok(suggestions.some((s) => s.action === 'create_payment'));
    });

    it('suggests confirm_reservation for reserved inventory', () => {
      ctx.createSession();
      ctx.trackResource('reservation', 'RES-1', () => {});
      ctx.recordToolCall('reserve_inventory', {}, { success: true });
      const suggestions = ctx.suggestNextActions();
      assert.ok(suggestions.some((s) => s.action === 'confirm_reservation'));
    });

    it('suggests get_order after create_order', () => {
      ctx.recordToolCall('create_order', {}, { success: true });
      const suggestions = ctx.suggestNextActions();
      assert.ok(suggestions.some((s) => s.action === 'get_order'));
    });

    it('suggests create_payment after create_order without payment', () => {
      ctx.recordToolCall('create_order', {}, { success: true });
      const suggestions = ctx.suggestNextActions();
      assert.ok(suggestions.some((s) => s.action === 'create_payment' && s.reason.includes('payment')));
    });
  });

  // ==========================================================================
  // getSessionSummary
  // ==========================================================================

  describe('getSessionSummary', () => {
    it('returns summary with correct counts', () => {
      ctx.createSession();
      ctx.recordToolCall('list_products', {}, { success: true });
      ctx.recordToolCall('create_order', {}, { success: false, error: 'fail' });
      ctx.recordOperation({ type: 'create', resource: 'order' });
      ctx.trackResource('payment', 'PAY-1', () => {});

      const summary = ctx.getSessionSummary();
      assert.strictEqual(summary.toolCallCount, 2);
      assert.strictEqual(summary.successfulCalls, 1);
      assert.strictEqual(summary.failedCalls, 1);
      assert.strictEqual(summary.operationCount, 1);
      assert.strictEqual(summary.pendingResources, 1);
      assert.strictEqual(summary.pendingPayments, 1);
    });

    it('includes sessionId and timestamps', () => {
      const session = ctx.createSession();
      const summary = ctx.getSessionSummary();
      assert.strictEqual(summary.sessionId, session.id);
      assert.ok(summary.createdAt);
      assert.ok(summary.lastActivityAt);
    });

    it('includes current context goals and currentTask', () => {
      ctx.createSession();
      ctx.addGoal('Purchase items');
      ctx.setContext({ currentTask: 'checkout' });
      const summary = ctx.getSessionSummary();
      assert.deepStrictEqual(summary.goals, ['Purchase items']);
      assert.strictEqual(summary.currentTask, 'checkout');
    });

    it('includes reservation inventory count', () => {
      ctx.createSession();
      ctx.trackResource('reservation', 'RES-1', () => {});
      ctx.trackResource('reservation', 'RES-2', () => {});
      const summary = ctx.getSessionSummary();
      assert.strictEqual(summary.reservedInventory, 2);
    });
  });

  // ==========================================================================
  // setContext
  // ==========================================================================

  describe('setContext', () => {
    it('merges context into session', () => {
      ctx.createSession();
      ctx.setContext({ currentTask: 'checkout', preferences: { currency: 'USD' } });
      const session = ctx.getActiveSession();
      assert.strictEqual(session.context.currentTask, 'checkout');
      assert.deepStrictEqual(session.context.preferences, { currency: 'USD' });
    });

    it('emits context:updated event', () => {
      let emitted = null;
      ctx.on('context:updated', (e) => { emitted = e; });
      ctx.createSession();
      ctx.setContext({ currentTask: 'browse' });
      assert.ok(emitted);
      assert.strictEqual(emitted.context.currentTask, 'browse');
    });

    it('preserves existing context fields not being overwritten', () => {
      ctx.createSession();
      ctx.addGoal('Buy stuff');
      ctx.setContext({ currentTask: 'checkout' });
      const session = ctx.getActiveSession();
      assert.deepStrictEqual(session.context.goals, ['Buy stuff']);
      assert.strictEqual(session.context.currentTask, 'checkout');
    });
  });

  // ==========================================================================
  // addGoal / completeGoal
  // ==========================================================================

  describe('addGoal', () => {
    it('adds goal to session context goals', () => {
      ctx.createSession();
      ctx.addGoal('Find product');
      assert.deepStrictEqual(ctx.getActiveSession().context.goals, ['Find product']);
    });

    it('can add multiple goals', () => {
      ctx.createSession();
      ctx.addGoal('A');
      ctx.addGoal('B');
      assert.deepStrictEqual(ctx.getActiveSession().context.goals, ['A', 'B']);
    });

    it('emits goal:added with sessionId and goal', () => {
      let emitted = null;
      ctx.on('goal:added', (e) => { emitted = e; });
      ctx.createSession();
      ctx.addGoal('Create order');
      assert.strictEqual(emitted.goal, 'Create order');
      assert.ok(emitted.sessionId);
    });
  });

  describe('completeGoal', () => {
    it('removes completed goal from list', () => {
      ctx.createSession();
      ctx.addGoal('A');
      ctx.addGoal('B');
      ctx.completeGoal('A');
      assert.deepStrictEqual(ctx.getActiveSession().context.goals, ['B']);
    });

    it('emits goal:completed with sessionId and goal', () => {
      let emitted = null;
      ctx.on('goal:completed', (e) => { emitted = e; });
      ctx.createSession();
      ctx.addGoal('Ship order');
      ctx.completeGoal('Ship order');
      assert.strictEqual(emitted.goal, 'Ship order');
      assert.ok(emitted.sessionId);
    });

    it('is a no-op for non-existent goal', () => {
      ctx.createSession();
      ctx.addGoal('A');
      ctx.completeGoal('Z');
      assert.deepStrictEqual(ctx.getActiveSession().context.goals, ['A']);
    });
  });

  // ==========================================================================
  // endSession
  // ==========================================================================

  describe('endSession', () => {
    it('returns null when no active session', () => {
      assert.strictEqual(ctx.endSession(), null);
    });

    it('returns saved session with endedAt and summary', () => {
      ctx.createSession();
      const saved = ctx.endSession();
      assert.ok(saved.endedAt);
      assert.ok(saved.summary);
      assert.ok(saved.summary.sessionId);
    });

    it('removes session from sessions map', () => {
      const session = ctx.createSession();
      ctx.endSession();
      assert.strictEqual(ctx.sessions.has(session.id), false);
    });

    it('clears activeSessionId', () => {
      ctx.createSession();
      ctx.endSession();
      assert.strictEqual(ctx.activeSessionId, null);
    });

    it('emits session:ended with session and summary', () => {
      let emitted = null;
      ctx.on('session:ended', (e) => { emitted = e; });
      ctx.createSession();
      ctx.endSession();
      assert.ok(emitted);
      assert.ok(emitted.session);
      assert.ok(emitted.summary);
    });

    it('saved session retains toolCallHistory data', () => {
      ctx.createSession();
      ctx.recordToolCall('list_products', {}, { success: true });
      const saved = ctx.endSession();
      assert.strictEqual(saved.toolCallHistory.length, 1);
    });
  });

  // ==========================================================================
  // listSessions
  // ==========================================================================

  describe('listSessions', () => {
    it('returns empty array when no sessions exist', () => {
      assert.deepStrictEqual(ctx.listSessions(), []);
    });

    it('returns session summaries with correct fields', () => {
      ctx.createSession({ name: 'first' });
      const list = ctx.listSessions();
      assert.strictEqual(list.length, 1);
      assert.ok(list[0].id);
      assert.ok(list[0].createdAt);
      assert.ok(list[0].lastActivityAt);
      assert.strictEqual(list[0].toolCallCount, 0);
      assert.strictEqual(list[0].active, true);
    });

    it('marks only the active session as active', () => {
      const s1 = ctx.createSession();
      const s2 = ctx.createSession();
      const list = ctx.listSessions();
      const entry1 = list.find((l) => l.id === s1.id);
      const entry2 = list.find((l) => l.id === s2.id);
      assert.strictEqual(entry1.active, false);
      assert.strictEqual(entry2.active, true);
    });

    it('reflects toolCallCount per session', () => {
      ctx.createSession();
      ctx.recordToolCall('a', {}, { success: true });
      ctx.recordToolCall('b', {}, { success: true });
      ctx.createSession();
      ctx.recordToolCall('c', {}, { success: true });
      const list = ctx.listSessions();
      const counts = list.map((l) => l.toolCallCount).sort();
      assert.deepStrictEqual(counts, [1, 2]);
    });
  });

  // ==========================================================================
  // switchSession
  // ==========================================================================

  describe('switchSession', () => {
    it('switches to an existing session', () => {
      const s1 = ctx.createSession();
      ctx.createSession();
      const switched = ctx.switchSession(s1.id);
      assert.strictEqual(ctx.activeSessionId, s1.id);
      assert.strictEqual(switched, s1);
    });

    it('throws for non-existent session', () => {
      ctx.createSession();
      assert.throws(
        () => ctx.switchSession('session-nonexistent'),
        { message: /session-nonexistent not found/ },
      );
    });

    it('emits session:switched with sessionId', () => {
      let emitted = null;
      ctx.on('session:switched', (e) => { emitted = e; });
      const s1 = ctx.createSession();
      ctx.createSession();
      ctx.switchSession(s1.id);
      assert.strictEqual(emitted.sessionId, s1.id);
    });
  });

  // ==========================================================================
  // Multi-session integration
  // ==========================================================================

  describe('multi-session integration', () => {
    it('maintains independent tool histories per session', () => {
      const s1 = ctx.createSession();
      ctx.recordToolCall('list_products', {}, { success: true });

      const s2 = ctx.createSession();
      ctx.recordToolCall('list_customers', {}, { success: true });
      ctx.recordToolCall('get_customer', { id: '1' }, { success: true });

      assert.strictEqual(s1.toolCallHistory.length, 1);
      assert.strictEqual(s2.toolCallHistory.length, 2);
    });

    it('maintains independent resources per session', () => {
      const s1 = ctx.createSession();
      ctx.trackResource('order', 'ORD-S1', () => {});

      ctx.createSession();
      ctx.trackResource('payment', 'PAY-S2', () => {});

      assert.strictEqual(s1.state.createdResources.size, 1);
      assert.ok(s1.state.createdResources.has('ORD-S1'));
    });

    it('endSession then getActiveSession creates a fresh session', () => {
      ctx.createSession();
      ctx.recordToolCall('list_products', {}, { success: true });
      ctx.endSession();

      const fresh = ctx.getActiveSession();
      assert.strictEqual(fresh.toolCallHistory.length, 0);
    });

    it('switching sessions allows recording to different sessions', () => {
      const s1 = ctx.createSession();
      const s2 = ctx.createSession();

      ctx.switchSession(s1.id);
      ctx.recordToolCall('list_products', {}, { success: true });

      ctx.switchSession(s2.id);
      ctx.recordToolCall('list_customers', {}, { success: true });
      ctx.recordToolCall('get_customer', {}, { success: true });

      assert.strictEqual(s1.toolCallHistory.length, 1);
      assert.strictEqual(s1.toolCallHistory[0].tool, 'list_products');
      assert.strictEqual(s2.toolCallHistory.length, 2);
    });
  });
});
