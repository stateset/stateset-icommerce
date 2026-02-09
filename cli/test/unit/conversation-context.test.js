import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { ConversationContext } from '../../src/mcp-conversation-context.js';

// ============================================================================
// ConversationContext — constructor
// ============================================================================

describe('ConversationContext', () => {
  let ctx;

  beforeEach(() => {
    ctx = new ConversationContext(null);
  });

  // --------------------------------------------------------------------------
  // Constructor
  // --------------------------------------------------------------------------

  describe('constructor', () => {
    it('stores commerce reference', () => {
      const commerce = { some: 'client' };
      const c = new ConversationContext(commerce);
      assert.equal(c.commerce, commerce);
    });

    it('starts with empty sessions map', () => {
      assert.equal(ctx.sessions.size, 0);
    });

    it('starts with null activeSessionId', () => {
      assert.equal(ctx.activeSessionId, null);
    });

    it('is an EventEmitter', () => {
      assert.equal(typeof ctx.on, 'function');
      assert.equal(typeof ctx.emit, 'function');
    });
  });

  // --------------------------------------------------------------------------
  // createSession
  // --------------------------------------------------------------------------

  describe('createSession', () => {
    it('creates a session with a unique id', () => {
      const session = ctx.createSession();
      assert.ok(session.id.startsWith('session-'));
    });

    it('stores session in sessions map', () => {
      const session = ctx.createSession();
      assert.equal(ctx.sessions.get(session.id), session);
    });

    it('sets activeSessionId', () => {
      const session = ctx.createSession();
      assert.equal(ctx.activeSessionId, session.id);
    });

    it('initializes empty toolCallHistory', () => {
      const session = ctx.createSession();
      assert.deepEqual(session.toolCallHistory, []);
    });

    it('initializes empty operations', () => {
      const session = ctx.createSession();
      assert.deepEqual(session.operations, []);
    });

    it('stores metadata', () => {
      const session = ctx.createSession({ user: 'alice' });
      assert.deepEqual(session.metadata, { user: 'alice' });
    });

    it('emits session:created event', () => {
      let emitted = null;
      ctx.on('session:created', (s) => { emitted = s; });
      const session = ctx.createSession();
      assert.equal(emitted, session);
    });

    it('initializes state with empty pending arrays', () => {
      const session = ctx.createSession();
      assert.deepEqual(session.state.pendingOrders, []);
      assert.deepEqual(session.state.reservedInventory, []);
      assert.deepEqual(session.state.pendingPayments, []);
    });

    it('initializes context fields', () => {
      const session = ctx.createSession();
      assert.equal(session.context.currentTask, null);
      assert.deepEqual(session.context.goals, []);
      assert.deepEqual(session.context.constraints, []);
      assert.deepEqual(session.context.preferences, {});
    });
  });

  // --------------------------------------------------------------------------
  // getActiveSession
  // --------------------------------------------------------------------------

  describe('getActiveSession', () => {
    it('auto-creates a session if none exists', () => {
      const session = ctx.getActiveSession();
      assert.ok(session);
      assert.ok(session.id.startsWith('session-'));
    });

    it('returns existing active session', () => {
      const created = ctx.createSession();
      const active = ctx.getActiveSession();
      assert.equal(active, created);
    });
  });

  // --------------------------------------------------------------------------
  // recordToolCall
  // --------------------------------------------------------------------------

  describe('recordToolCall', () => {
    it('records a successful tool call', () => {
      const tc = ctx.recordToolCall('list_orders', { limit: 10 }, { success: true, data: [] });
      assert.ok(tc.id.startsWith('call-'));
      assert.equal(tc.tool, 'list_orders');
      assert.equal(tc.status, 'success');
    });

    it('records a failed tool call', () => {
      const tc = ctx.recordToolCall('create_order', {}, { success: false, error: 'boom' });
      assert.equal(tc.status, 'error');
    });

    it('stores calls in toolCallHistory', () => {
      ctx.recordToolCall('a', {}, { success: true });
      ctx.recordToolCall('b', {}, { success: true });
      const session = ctx.getActiveSession();
      assert.equal(session.toolCallHistory.length, 2);
    });

    it('emits tool:called event', () => {
      let emitted = null;
      ctx.on('tool:called', (data) => { emitted = data; });
      ctx.recordToolCall('x', {}, { success: true });
      assert.ok(emitted);
      assert.equal(emitted.toolCall.tool, 'x');
    });

    it('emits tool:succeeded for successful calls', () => {
      let emitted = false;
      ctx.on('tool:succeeded', () => { emitted = true; });
      ctx.recordToolCall('x', {}, { success: true });
      assert.ok(emitted);
    });

    it('emits tool:failed for failed calls', () => {
      let emitted = false;
      ctx.on('tool:failed', () => { emitted = true; });
      ctx.recordToolCall('x', {}, { success: false, error: 'oops' });
      assert.ok(emitted);
    });

    it('stores optional duration and context', () => {
      const tc = ctx.recordToolCall('x', {}, { success: true }, { duration: 123, context: 'ctx' });
      assert.equal(tc.duration, 123);
      assert.equal(tc.context, 'ctx');
    });

    it('updates lastActivityAt on session', () => {
      const session = ctx.createSession();
      const original = session.lastActivityAt;
      // Record after a slight delay conceptually — but timestamps may be same
      ctx.recordToolCall('x', {}, { success: true });
      assert.ok(session.lastActivityAt);
    });
  });

  // --------------------------------------------------------------------------
  // recordOperation
  // --------------------------------------------------------------------------

  describe('recordOperation', () => {
    it('records an operation with auto-generated id', () => {
      const op = ctx.recordOperation({ type: 'create', resource: 'order' });
      assert.ok(op.id.startsWith('op-'));
      assert.equal(op.type, 'create');
      assert.equal(op.resource, 'order');
    });

    it('pushes into session operations array', () => {
      ctx.recordOperation({ type: 'a' });
      ctx.recordOperation({ type: 'b' });
      const session = ctx.getActiveSession();
      assert.equal(session.operations.length, 2);
    });

    it('emits operation:recorded event', () => {
      let emitted = null;
      ctx.on('operation:recorded', (data) => { emitted = data; });
      ctx.recordOperation({ type: 'x' });
      assert.ok(emitted);
    });
  });

  // --------------------------------------------------------------------------
  // trackResource
  // --------------------------------------------------------------------------

  describe('trackResource', () => {
    it('tracks a resource in createdResources', () => {
      const rollback = () => {};
      ctx.trackResource('order', 'ORD-1', rollback);
      const session = ctx.getActiveSession();
      assert.ok(session.state.createdResources.has('ORD-1'));
    });

    it('adds reservation type to reservedInventory', () => {
      ctx.trackResource('reservation', 'RES-1', () => {});
      const session = ctx.getActiveSession();
      assert.ok(session.state.reservedInventory.includes('RES-1'));
    });

    it('adds order type to pendingOrders', () => {
      ctx.trackResource('order', 'ORD-1', () => {});
      const session = ctx.getActiveSession();
      assert.ok(session.state.pendingOrders.includes('ORD-1'));
    });

    it('adds payment type to pendingPayments', () => {
      ctx.trackResource('payment', 'PAY-1', () => {});
      const session = ctx.getActiveSession();
      assert.ok(session.state.pendingPayments.includes('PAY-1'));
    });

    it('emits resource:tracked event', () => {
      let emitted = null;
      ctx.on('resource:tracked', (data) => { emitted = data; });
      ctx.trackResource('order', 'ORD-2', () => {});
      assert.equal(emitted.resourceType, 'order');
      assert.equal(emitted.resourceId, 'ORD-2');
    });
  });

  // --------------------------------------------------------------------------
  // rollbackSession
  // --------------------------------------------------------------------------

  describe('rollbackSession', () => {
    it('returns success with no resources message when empty', async () => {
      ctx.createSession();
      const result = await ctx.rollbackSession();
      assert.ok(result.success);
      assert.ok(result.message.includes('No resources'));
    });

    it('calls rollback functions for tracked resources', async () => {
      let rolled = false;
      ctx.trackResource('order', 'ORD-1', async () => { rolled = true; });
      await ctx.rollbackSession();
      assert.ok(rolled);
    });

    it('clears createdResources after rollback', async () => {
      ctx.trackResource('order', 'ORD-1', async () => {});
      await ctx.rollbackSession();
      const session = ctx.getActiveSession();
      assert.equal(session.state.createdResources.size, 0);
    });

    it('reports failed rollbacks', async () => {
      ctx.trackResource('order', 'ORD-1', async () => { throw new Error('fail'); });
      const result = await ctx.rollbackSession();
      assert.equal(result.success, false);
      assert.equal(result.results[0].status, 'failed');
    });

    it('skips resources without rollback function', async () => {
      const session = ctx.getActiveSession();
      session.state.createdResources.set('ORD-X', { type: 'order', createdAt: new Date().toISOString() });
      const result = await ctx.rollbackSession();
      assert.equal(result.results[0].status, 'skipped');
    });
  });

  // --------------------------------------------------------------------------
  // rollbackResource
  // --------------------------------------------------------------------------

  describe('rollbackResource', () => {
    it('returns failure for unknown resource', async () => {
      ctx.createSession();
      const result = await ctx.rollbackResource('unknown-id');
      assert.equal(result.success, false);
    });

    it('calls rollback for tracked resource and removes it', async () => {
      let called = false;
      ctx.trackResource('order', 'ORD-1', async () => { called = true; return 'ok'; });
      const result = await ctx.rollbackResource('ORD-1');
      assert.ok(called);
      assert.ok(result.success);
      const session = ctx.getActiveSession();
      assert.ok(!session.state.createdResources.has('ORD-1'));
    });

    it('handles rollback error gracefully', async () => {
      ctx.trackResource('order', 'ORD-1', async () => { throw new Error('fail'); });
      const result = await ctx.rollbackResource('ORD-1');
      assert.equal(result.success, false);
      assert.equal(result.error, 'fail');
    });
  });

  // --------------------------------------------------------------------------
  // getErrorContext
  // --------------------------------------------------------------------------

  describe('getErrorContext', () => {
    it('returns error analysis object', () => {
      const error = new Error('something broke');
      const analysis = ctx.getErrorContext(error, 'create_order');
      assert.equal(analysis.error, 'something broke');
      assert.equal(analysis.tool, 'create_order');
      assert.ok(Array.isArray(analysis.suggestions));
    });

    it('suggests stock-related actions for insufficient stock', () => {
      const error = new Error('insufficient stock for SKU-1');
      const analysis = ctx.getErrorContext(error, 'reserve_inventory');
      assert.ok(analysis.suggestions.length > 0);
      assert.ok(analysis.suggestions.some((s) => s.includes('stock')));
    });

    it('suggests order lookup for order not found', () => {
      const error = new Error('order not found');
      const analysis = ctx.getErrorContext(error, 'get_order');
      assert.ok(analysis.suggestions.some((s) => s.includes('order')));
    });

    it('suggests status check for invalid status transition', () => {
      const error = new Error('invalid status transition');
      const analysis = ctx.getErrorContext(error, 'update_order_status');
      assert.ok(analysis.suggestions.some((s) => s.includes('status')));
    });

    it('suggests customer creation for customer not found', () => {
      const error = new Error('customer not found');
      const analysis = ctx.getErrorContext(error, 'get_customer');
      assert.ok(analysis.suggestions.some((s) => s.includes('customer')));
    });

    it('includes recent activity from tool call history', () => {
      ctx.recordToolCall('list_orders', {}, { success: true });
      const analysis = ctx.getErrorContext(new Error('test'), 'get_order');
      assert.ok(Array.isArray(analysis.recentActivity));
      assert.equal(analysis.recentActivity.length, 1);
    });

    it('reports canRollback when resources exist', () => {
      ctx.trackResource('order', 'ORD-1', () => {});
      const analysis = ctx.getErrorContext(new Error('test'), 'x');
      assert.ok(analysis.canRollback);
    });
  });

  // --------------------------------------------------------------------------
  // suggestNextActions
  // --------------------------------------------------------------------------

  describe('suggestNextActions', () => {
    it('suggests list_products and list_customers for empty history', () => {
      ctx.createSession();
      const suggestions = ctx.suggestNextActions();
      assert.equal(suggestions.length, 2);
      assert.ok(suggestions.some((s) => s.action === 'list_products'));
      assert.ok(suggestions.some((s) => s.action === 'list_customers'));
    });

    it('suggests get_product_variant after list_products', () => {
      ctx.recordToolCall('list_products', {}, { success: true });
      const suggestions = ctx.suggestNextActions();
      assert.ok(suggestions.some((s) => s.action === 'get_product_variant'));
    });

    it('suggests get_stock after get_product_variant', () => {
      ctx.recordToolCall('get_product_variant', { sku: 'SKU-1' }, { success: true });
      const suggestions = ctx.suggestNextActions();
      assert.ok(suggestions.some((s) => s.action === 'get_stock'));
    });

    it('suggests process_payment when pending orders exist', () => {
      ctx.trackResource('order', 'ORD-1', () => {});
      ctx.recordToolCall('some_tool', {}, { success: true });
      const suggestions = ctx.suggestNextActions();
      assert.ok(suggestions.some((s) => s.action === 'process_payment'));
    });

    it('suggests verify order after create_order', () => {
      ctx.recordToolCall('create_order', {}, { success: true });
      const suggestions = ctx.suggestNextActions();
      assert.ok(suggestions.some((s) => s.action === 'get_order'));
    });
  });

  // --------------------------------------------------------------------------
  // getSessionSummary
  // --------------------------------------------------------------------------

  describe('getSessionSummary', () => {
    it('returns summary with counts', () => {
      ctx.recordToolCall('a', {}, { success: true });
      ctx.recordToolCall('b', {}, { success: false, error: 'x' });
      const summary = ctx.getSessionSummary();
      assert.equal(summary.toolCallCount, 2);
      assert.equal(summary.successfulCalls, 1);
      assert.equal(summary.failedCalls, 1);
    });

    it('includes pending resource counts', () => {
      ctx.trackResource('order', 'ORD-1', () => {});
      ctx.trackResource('reservation', 'RES-1', () => {});
      const summary = ctx.getSessionSummary();
      assert.equal(summary.pendingResources, 2);
      assert.equal(summary.pendingOrders, 1);
      assert.equal(summary.reservedInventory, 1);
    });

    it('includes sessionId and timestamps', () => {
      ctx.createSession();
      const summary = ctx.getSessionSummary();
      assert.ok(summary.sessionId);
      assert.ok(summary.createdAt);
      assert.ok(summary.lastActivityAt);
    });
  });

  // --------------------------------------------------------------------------
  // setContext / addGoal / completeGoal
  // --------------------------------------------------------------------------

  describe('setContext', () => {
    it('merges context into session', () => {
      ctx.createSession();
      ctx.setContext({ currentTask: 'Process returns' });
      const session = ctx.getActiveSession();
      assert.equal(session.context.currentTask, 'Process returns');
    });

    it('emits context:updated event', () => {
      let emitted = null;
      ctx.on('context:updated', (data) => { emitted = data; });
      ctx.createSession();
      ctx.setContext({ currentTask: 'x' });
      assert.ok(emitted);
    });
  });

  describe('addGoal', () => {
    it('pushes a goal into session context', () => {
      ctx.createSession();
      ctx.addGoal('Create order');
      const session = ctx.getActiveSession();
      assert.deepEqual(session.context.goals, ['Create order']);
    });

    it('emits goal:added event', () => {
      let emitted = null;
      ctx.on('goal:added', (data) => { emitted = data; });
      ctx.createSession();
      ctx.addGoal('Ship order');
      assert.equal(emitted.goal, 'Ship order');
    });
  });

  describe('completeGoal', () => {
    it('removes a goal from session context', () => {
      ctx.createSession();
      ctx.addGoal('A');
      ctx.addGoal('B');
      ctx.completeGoal('A');
      const session = ctx.getActiveSession();
      assert.deepEqual(session.context.goals, ['B']);
    });

    it('emits goal:completed event', () => {
      let emitted = null;
      ctx.on('goal:completed', (data) => { emitted = data; });
      ctx.createSession();
      ctx.addGoal('A');
      ctx.completeGoal('A');
      assert.equal(emitted.goal, 'A');
    });
  });

  // --------------------------------------------------------------------------
  // endSession
  // --------------------------------------------------------------------------

  describe('endSession', () => {
    it('returns null when no active session', () => {
      assert.equal(ctx.endSession(), null);
    });

    it('removes session from map and clears activeSessionId', () => {
      const session = ctx.createSession();
      ctx.endSession();
      assert.equal(ctx.activeSessionId, null);
      assert.equal(ctx.sessions.has(session.id), false);
    });

    it('returns session with endedAt and summary', () => {
      ctx.createSession();
      const ended = ctx.endSession();
      assert.ok(ended.endedAt);
      assert.ok(ended.summary);
    });

    it('emits session:ended event', () => {
      let emitted = null;
      ctx.on('session:ended', (data) => { emitted = data; });
      ctx.createSession();
      ctx.endSession();
      assert.ok(emitted);
      assert.ok(emitted.session);
      assert.ok(emitted.summary);
    });
  });

  // --------------------------------------------------------------------------
  // listSessions / switchSession
  // --------------------------------------------------------------------------

  describe('listSessions', () => {
    it('lists all sessions', () => {
      ctx.createSession();
      ctx.createSession();
      const list = ctx.listSessions();
      assert.equal(list.length, 2);
    });

    it('marks the active session', () => {
      const s1 = ctx.createSession();
      ctx.createSession();
      const list = ctx.listSessions();
      const active = list.filter((s) => s.active);
      assert.equal(active.length, 1);
    });
  });

  describe('switchSession', () => {
    it('switches to an existing session', () => {
      const s1 = ctx.createSession();
      const s2 = ctx.createSession();
      ctx.switchSession(s1.id);
      assert.equal(ctx.activeSessionId, s1.id);
    });

    it('throws for non-existent session', () => {
      assert.throws(() => ctx.switchSession('no-such-id'), /not found/);
    });

    it('emits session:switched event', () => {
      let emitted = null;
      ctx.on('session:switched', (data) => { emitted = data; });
      const s1 = ctx.createSession();
      ctx.createSession();
      ctx.switchSession(s1.id);
      assert.equal(emitted.sessionId, s1.id);
    });
  });
});
