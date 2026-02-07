/**
 * Conversation Context Manager for StateSet MCP Server
 * Tracks all tool calls in a session, maintains operation timeline,
 * enables rollback of operations, and provides context-aware assistance
 */

import { EventEmitter } from 'events';
import crypto from 'node:crypto';

export class ConversationContext extends EventEmitter {
  constructor(commerce) {
    super();
    this.commerce = commerce;
    this.sessions = new Map();
    this.activeSessionId = null;
  }

  /**
   * Create a new conversation session
   * @param {Object} metadata - Optional metadata for the session
   * @returns {Object} The new session object
   */
  createSession(metadata = {}) {
    const sessionId = `session-${Date.now()}-${crypto.randomUUID().slice(0, 9)}`;
    const session = {
      id: sessionId,
      createdAt: new Date().toISOString(),
      lastActivityAt: new Date().toISOString(),
      metadata,
      toolCallHistory: [],
      operations: [],
      rollbacks: [],
      state: {
        pendingOrders: [],
        reservedInventory: [],
        pendingPayments: [],
        createdResources: new Map(),
      },
      context: {
        currentTask: null,
        goals: [],
        constraints: [],
        preferences: {},
      },
    };

    this.sessions.set(sessionId, session);
    this.activeSessionId = sessionId;
    this.emit('session:created', session);
    return session;
  }

  /**
   * Get active session or create one if none exists
   * @returns {Object} The active session
   */
  getActiveSession() {
    if (!this.activeSessionId) {
      return this.createSession();
    }
    return this.sessions.get(this.activeSessionId);
  }

  /**
   * Record a tool call in the conversation
   * @param {string} toolName - Name of the tool called
   * @param {Object} params - Parameters passed to the tool
   * @param {Object} result - Result from the tool
   * @param {Object} options - Optional metadata
   * @returns {Object} The recorded tool call
   */
  recordToolCall(toolName, params, result, options = {}) {
    const session = this.getActiveSession();
    const toolCall = {
      id: `call-${Date.now()}-${crypto.randomUUID().slice(0, 9)}`,
      timestamp: new Date().toISOString(),
      tool: toolName,
      params,
      result,
      status: result.success !== false ? 'success' : 'error',
      duration: options.duration,
      context: options.context,
      rollbackFn: options.rollbackFn,
      enrollmentId: options.enrollmentId,
    };

    session.toolCallHistory.push(toolCall);
    session.lastActivityAt = new Date().toISOString();

    this.emit('tool:called', { sessionId: session.id, toolCall });

    if (result.success === false) {
      this.emit('tool:failed', { sessionId: session.id, toolCall, error: result.error });
    } else {
      this.emit('tool:succeeded', { sessionId: session.id, toolCall });
    }

    return toolCall;
  }

  /**
   * Record an operation that creates or modifies state
   * @param {Object} operation - Operation details
   */
  recordOperation(operation) {
    const session = this.getActiveSession();
    const operationRecord = {
      id: `op-${Date.now()}-${crypto.randomUUID().slice(0, 9)}`,
      timestamp: new Date().toISOString(),
      ...operation,
    };

    session.operations.push(operationRecord);
    session.lastActivityAt = new Date().toISOString();

    this.emit('operation:recorded', { sessionId: session.id, operation: operationRecord });

    return operationRecord;
  }

  /**
   * Track created resources for potential rollback
   * @param {string} resourceType - Type of resource (order, customer, etc.)
   * @param {string} resourceId - ID of the resource
   * @param {Function} rollbackFn - Function to rollback this resource
   */
  trackResource(resourceType, resourceId, rollbackFn) {
    const session = this.getActiveSession();
    session.state.createdResources.set(resourceId, {
      type: resourceType,
      createdAt: new Date().toISOString(),
      rollback: rollbackFn,
    });

    if (resourceType === 'reservation') {
      session.state.reservedInventory.push(resourceId);
    } else if (resourceType === 'order' && isPendingStatus(resourceId)) {
      session.state.pendingOrders.push(resourceId);
    } else if (resourceType === 'payment') {
      session.state.pendingPayments.push(resourceId);
    }

    this.emit('resource:tracked', { sessionId: session.id, resourceType, resourceId });
  }

  /**
   * Rollback all operations in current session
   * @param {Object} options - Rollback options
   * @returns {Promise<Object>} Rollback results
   */
  async rollbackSession(_options = {}) {
    const session = this.getActiveSession();
    const results = [];

    const resources = Array.from(session.state.createdResources.entries());

    if (resources.length === 0) {
      return { success: true, message: 'No resources to rollback', results };
    }

    this.emit('rollback:started', { sessionId: session.id, resourceCount: resources.length });

    for (const [resourceId, resource] of resources) {
      try {
        if (typeof resource.rollback === 'function') {
          const result = await resource.rollback();
          results.push({
            resourceId,
            resourceType: resource.type,
            status: 'success',
            result,
          });
          this.emit('rollback:success', { sessionId: session.id, resourceId });
        } else {
          results.push({
            resourceId,
            resourceType: resource.type,
            status: 'skipped',
            reason: 'No rollback function available',
          });
        }
      } catch (error) {
        results.push({
          resourceId,
          resourceType: resource.type,
          status: 'failed',
          error: error.message,
        });
        this.emit('rollback:failed', { sessionId: session.id, resourceId, error });
      }
    }

    session.state.createdResources.clear();
    session.state.pendingOrders = [];
    session.state.reservedInventory = [];
    session.state.pendingPayments = [];

    this.emit('rollback:completed', { sessionId: session.id, results });

    return {
      success: results.every((r) => r.status === 'success' || r.status === 'skipped'),
      message: `Rolled back ${results.length} resources`,
      results,
    };
  }

  /**
   * Rollback specific resource
   * @param {string} resourceId - ID of resource to rollback
   * @returns {Promise<Object>} Rollback result
   */
  async rollbackResource(resourceId) {
    const session = this.getActiveSession();
    const resource = session.state.createdResources.get(resourceId);

    if (!resource) {
      return {
        success: false,
        error: `Resource ${resourceId} not found in session`,
      };
    }

    try {
      const result = await resource.rollback();
      session.state.createdResources.delete(resourceId);

      session.state.pendingOrders = session.state.pendingOrders.filter((id) => id !== resourceId);
      session.state.reservedInventory = session.state.reservedInventory.filter(
        (id) => id !== resourceId,
      );
      session.state.pendingPayments = session.state.pendingPayments.filter(
        (id) => id !== resourceId,
      );

      this.emit('rollback:success', { sessionId: session.id, resourceId });

      return {
        success: true,
        resourceType: resource.type,
        result,
      };
    } catch (error) {
      this.emit('rollback:failed', { sessionId: session.id, resourceId, error });

      return {
        success: false,
        resourceType: resource.type,
        error: error.message,
      };
    }
  }

  /**
   * Get context-aware error message
   * @param {Error} error - The error that occurred
   * @param {string} toolName - Tool that threw the error
   * @returns {Object} Context-aware error analysis
   */
  getErrorContext(error, toolName) {
    const session = this.getActiveSession();
    const recentCalls = session.toolCallHistory.slice(-5);
    const pendingResources = Array.from(session.state.createdResources.entries());

    const analysis = {
      error: error.message,
      tool: toolName,
      timestamp: new Date().toISOString(),
      recentActivity: recentCalls.map((c) => ({
        tool: c.tool,
        status: c.status,
        timestamp: c.timestamp,
      })),
      pendingResources: pendingResources.map(([id, r]) => ({
        id,
        type: r.type,
        createdAt: r.createdAt,
      })),
      suggestions: [],
      canRollback: pendingResources.length > 0,
    };

    if (error.message.includes('insufficient stock')) {
      analysis.suggestions.push('Check available stock with get_stock before reserving');
      analysis.suggestions.push('Adjust inventory with adjust_inventory or create a backorder');

      const recentStockCheck = recentCalls.find(
        (c) => c.tool === 'get_stock' || c.tool === 'reserve_inventory',
      );
      if (recentStockCheck) {
        analysis.context = 'You recently checked stock or attempted to reserve inventory';
      }
    }

    if (error.message.includes('order not found')) {
      analysis.suggestions.push('Verify order ID is correct');
      analysis.suggestions.push('Use list_orders to find valid order IDs');

      const recentOrderCalls = recentCalls.filter((c) => c.tool.includes('order'));
      if (recentOrderCalls.length > 0) {
        analysis.context = `You recently worked with orders: ${recentOrderCalls.map((c) => c.params.orderId).join(', ')}`;
      }
    }

    if (error.message.includes('invalid status transition')) {
      analysis.suggestions.push('Check current order status with get_order');
      analysis.suggestions.push('Review valid status transitions for current state');
      analysis.suggestions.push('Verify all prerequisites are met before transitioning');
    }

    if (error.message.includes('customer not found')) {
      analysis.suggestions.push('Verify customer ID or email exists');
      analysis.suggestions.push('Use get_customer or list_customers to find valid customer');

      if (session.context.goals.includes('Create order')) {
        analysis.recommendation = 'Create customer first with create_customer, then create order';
      }
    }

    return analysis;
  }

  /**
   * Suggest next actions based on conversation history
   * @returns {Array<Object>} Suggested actions
   */
  suggestNextActions() {
    const session = this.getActiveSession();
    const suggestions = [];

    if (session.toolCallHistory.length === 0) {
      return [
        {
          priority: 'high',
          action: 'list_products',
          reason: 'Start by exploring available products',
        },
        {
          priority: 'medium',
          action: 'list_customers',
          reason: 'Check existing customers',
        },
      ];
    }

    const lastCall = session.toolCallHistory[session.toolCallHistory.length - 1];
    const recentTools = new Set(session.toolCallHistory.slice(-5).map((c) => c.tool));

    if (lastCall.tool === 'list_products' && !recentTools.has('get_product')) {
      suggestions.push({
        priority: 'high',
        action: 'get_product_variant',
        reason: 'Get detailed information about a specific product',
        params: { sku: '<choose a SKU from list>' },
      });
    }

    if (lastCall.tool === 'get_product_variant' && !recentTools.has('get_stock')) {
      suggestions.push({
        priority: 'high',
        action: 'get_stock',
        reason: 'Check stock availability for the product',
        params: { sku: lastCall.params.sku },
      });
    }

    if (lastCall.tool === 'get_stock' && !recentTools.has('reserve_inventory')) {
      suggestions.push({
        priority: 'high',
        action: 'reserve_inventory',
        reason: 'Reserve inventory for an order',
        params: { sku: lastCall.params.sku, quantity: 1, referenceType: 'order' },
      });
    }

    if (session.state.pendingOrders.length > 0) {
      suggestions.push({
        priority: 'high',
        action: 'process_payment',
        reason: `Complete payment for ${session.state.pendingOrders.length} pending order(s)`,
      });
    }

    if (session.state.reservedInventory.length > 0 && !recentTools.has('confirm_reservation')) {
      suggestions.push({
        priority: 'medium',
        action: 'confirm_reservation',
        reason: 'Confirm reserved inventory to prevent expiration',
      });
    }

    if (recentTools.has('create_order') && !recentTools.has('get_order')) {
      suggestions.push({
        priority: 'high',
        action: 'get_order',
        reason: 'Verify the created order details',
        params: { orderId: '<order ID from creation>' },
      });
    }

    if (recentTools.has('create_order') && !recentTools.has('process_payment')) {
      suggestions.push({
        priority: 'high',
        action: 'process_payment',
        reason: 'Process payment for the created order',
      });
    }

    return suggestions;
  }

  /**
   * Get summary of current session
   * @returns {Object} Session summary
   */
  getSessionSummary() {
    const session = this.getActiveSession();
    const toolCalls = session.toolCallHistory;
    const successfulCalls = toolCalls.filter((c) => c.status === 'success').length;
    const failedCalls = toolCalls.filter((c) => c.status === 'error').length;

    return {
      sessionId: session.id,
      createdAt: session.createdAt,
      lastActivityAt: session.lastActivityAt,
      toolCallCount: toolCalls.length,
      successfulCalls,
      failedCalls,
      operationCount: session.operations.length,
      pendingResources: session.state.createdResources.size,
      pendingOrders: session.state.pendingOrders.length,
      reservedInventory: session.state.reservedInventory.length,
      pendingPayments: session.state.pendingPayments.length,
      currentTask: session.context.currentTask,
      goals: session.context.goals,
    };
  }

  /**
   * Set context for current session
   * @param {Object} context - Context information
   */
  setContext(context) {
    const session = this.getActiveSession();
    Object.assign(session.context, context);
    this.emit('context:updated', { sessionId: session.id, context });
  }

  /**
   * Add goal to current session
   * @param {string} goal - Goal to add
   */
  addGoal(goal) {
    const session = this.getActiveSession();
    session.context.goals.push(goal);
    this.emit('goal:added', { sessionId: session.id, goal });
  }

  /**
   * Mark a goal as completed
   * @param {string} goal - Goal that was completed
   */
  completeGoal(goal) {
    const session = this.getActiveSession();
    session.context.goals = session.context.goals.filter((g) => g !== goal);
    this.emit('goal:completed', { sessionId: session.id, goal });
  }

  /**
   * End current session
   * @returns {Object} Final session summary
   */
  endSession() {
    if (!this.activeSessionId) {
      return null;
    }

    const session = this.sessions.get(this.activeSessionId);
    const summary = this.getSessionSummary();
    session.endedAt = new Date().toISOString();
    session.summary = summary;

    const savedSession = { ...session };
    this.sessions.delete(this.activeSessionId);
    this.activeSessionId = null;

    this.emit('session:ended', { session: savedSession, summary });

    return savedSession;
  }

  /**
   * List all sessions
   * @returns {Array<Object>} Array of sessions
   */
  listSessions() {
    return Array.from(this.sessions.values()).map((s) => ({
      id: s.id,
      createdAt: s.createdAt,
      lastActivityAt: s.lastActivityAt,
      toolCallCount: s.toolCallHistory.length,
      active: s.id === this.activeSessionId,
    }));
  }

  /**
   * Switch to a different session
   * @param {string} sessionId - Session ID to switch to
   * @returns {Object} The session
   */
  switchSession(sessionId) {
    const session = this.sessions.get(sessionId);
    if (!session) {
      throw new Error(`Session ${sessionId} not found`);
    }

    this.activeSessionId = sessionId;
    this.emit('session:switched', { sessionId });
    return session;
  }
}

function isPendingStatus(_resourceId) {
  return true;
}
