/**
 * Tool Composition Engine for StateSet MCP Server
 * Enables agents to orchestrate multi-step commerce operations with compensating actions
 */

import { EventEmitter } from 'events';
import crypto from 'node:crypto';

export class ToolComposer extends EventEmitter {
  constructor(commerce, options = {}) {
    super();
    this.commerce = commerce;
    this.allowApply = options.allowApply ?? true;
    this.dbPath = options.dbPath || './store.db';
    this.resolveTreasuryAgentId = options.resolveTreasuryAgentId || (async () => 'default');
    this.treasuryContextOptions = options.treasuryContextOptions || {};
    this.buildAuditContext = options.buildAuditContext || (() => ({}));
    this.buildTreasuryIdentityMetadata =
      options.buildTreasuryIdentityMetadata || (async () => ({}));
    this.extra = options.extra || {};
    this._toolRegistryPromise = null;
    this.orchestrations = new Map();
    this.activeTransactions = new Map();
  }

  /**
   * Create a multi-step orchestration with best-effort compensating rollback
   * @param {string} name - Orchestration name
   * @param {Array} steps - Array of steps { tool, params, validate }
   * @returns {Promise<Object>} - Orchestration result with rollbacks on failure
   */
  async orchestrate(name, steps) {
    const orchestrationId = `orch-${Date.now()}-${crypto.randomUUID().slice(0, 9)}`;
    const results = [];
    const rollbackStack = [];

    this.emit('orchestration:started', { orchestrationId, name, stepCount: steps.length });

    try {
      for (let i = 0; i < steps.length; i++) {
        const step = steps[i];
        const stepId = `${orchestrationId}-step-${i}`;
        const resolvedParams =
          typeof step.params === 'function' ? step.params(results) : (step.params ?? {});

        this.emit('step:started', {
          orchestrationId,
          stepId,
          step: i + 1,
          total: steps.length,
          tool: step.tool,
        });

        const result = await this.executeTool(step.tool, resolvedParams);
        results.push({ step: i, tool: step.tool, result });

        // Validation step after each execution
        if (step.validate) {
          const validation = await step.validate(result);
          if (!validation.valid) {
            throw new Error(`Validation failed at step ${i + 1}: ${validation.error}`);
          }
        }

        // Add rollback function to stack if provided
        if (step.rollback) {
          rollbackStack.unshift({
            step: i,
            tool: step.tool,
            rollbackFn: () => step.rollback(result),
          });
        }

        this.emit('step:completed', { orchestrationId, stepId, step: i + 1, result });
      }

      this.emit('orchestration:completed', { orchestrationId, name, results });
      return {
        success: true,
        orchestrationId,
        name,
        results,
        completedAt: new Date().toISOString(),
      };
    } catch (error) {
      this.emit('orchestration:failed', { orchestrationId, name, error, progress: results.length });

      // Execute rollbacks in reverse order
      const rollbackResults = [];
      for (const rollback of rollbackStack) {
        try {
          const rollbackPlan = await rollback.rollbackFn();
          if (
            rollbackPlan &&
            typeof rollbackPlan === 'object' &&
            Object.hasOwn(rollbackPlan, 'tool')
          ) {
            if (!rollbackPlan.tool) {
              rollbackResults.push({
                step: rollback.step,
                tool: rollback.tool,
                status: 'skipped',
                reason: 'No compensating tool is available',
              });
              this.emit('rollback:skipped', { orchestrationId, step: rollback.step });
              continue;
            }
          }
          const result = rollbackPlan?.tool
            ? await this.executeTool(rollbackPlan.tool, rollbackPlan.params ?? {})
            : rollbackPlan;
          rollbackResults.push({
            step: rollback.step,
            tool: rollback.tool,
            status: 'success',
            result,
          });
          this.emit('rollback:success', { orchestrationId, step: rollback.step });
        } catch (rollbackError) {
          rollbackResults.push({
            step: rollback.step,
            tool: rollback.tool,
            status: 'failed',
            error: rollbackError.message,
          });
          this.emit('rollback:failed', {
            orchestrationId,
            step: rollback.step,
            error: rollbackError,
          });
        }
      }

      return {
        success: false,
        orchestrationId,
        name,
        error: error.message,
        progress: results.length,
        totalSteps: steps.length,
        completedSteps: results,
        rollbacks: rollbackResults,
      };
    }
  }

  /**
   * Execute a tool with parameters (delegates to MCP tools)
   */
  async executeTool(toolName, params) {
    if (!this.commerce) {
      // Preserve legacy test behavior when no commerce client is provided.
      return {
        tool: toolName,
        params,
        executedAt: new Date().toISOString(),
      };
    }

    const registry = await this._loadToolRegistry();
    const tool = registry.get(toolName);
    if (!tool) {
      throw new Error(`Unknown tool: ${toolName}`);
    }

    return tool.handler({
      commerce: this.commerce,
      params,
      allowApply: this.allowApply,
      dbPath: this.dbPath,
      resolveTreasuryAgentId: this.resolveTreasuryAgentId,
      treasuryContextOptions: this.treasuryContextOptions,
      buildAuditContext: this.buildAuditContext,
      buildTreasuryIdentityMetadata: this.buildTreasuryIdentityMetadata,
      extra: this.extra,
    });
  }

  async _loadToolRegistry() {
    if (!this._toolRegistryPromise) {
      this._toolRegistryPromise = import('./tools/index.js').then(
        async ({ createToolRegistry }) => {
          const registry = createToolRegistry();
          await registry.loadAll();
          return registry;
        },
      );
    }
    return this._toolRegistryPromise;
  }

  /**
   * Create order with inventory reservation (best-effort compensation)
   * Example: Reserve inventory → Create order → Confirm reservation
   */
  async createOrderWithReservation(params) {
    return this.orchestrate('create-order-with-reservation', [
      {
        tool: 'reserve_inventory',
        params: {
          sku: params.items[0].sku,
          quantity: params.items[0].quantity,
          referenceType: 'order',
          referenceId: params.orderId || `pending-${Date.now()}`,
          expiresInSeconds: 3600,
        },
        validate: (result) => ({ valid: !!result.reservation?.id }),
        rollback: async (result) => ({
          tool: 'release_reservation',
          params: { reservationId: result.reservation.id },
        }),
      },
      {
        tool: 'create_order',
        params: {
          customerId: params.customerId,
          items: params.items,
          currency: params.currency || 'USD',
          notes: params.notes,
        },
        validate: (result) => ({ valid: !!result.order?.id }),
        rollback: async (result) => ({
          tool: 'cancel_order',
          params: { orderId: result.order.id },
        }),
      },
      {
        tool: 'confirm_reservation',
        params: (previousResults) => ({
          reservationId: previousResults[0].result.reservation.id,
        }),
        validate: () => ({ valid: true }),
      },
    ]);
  }

  /**
   * Process return with restock (best-effort compensation)
   * Example: Approve return → Credit payment → Restock inventory
   */
  async processReturnWithRestock(params) {
    return this.orchestrate('process-return-with-restock', [
      {
        tool: 'approve_return',
        params: { returnId: params.returnId },
        validate: (result) => ({ valid: result.return?.status === 'approved' }),
        rollback: async () => ({
          tool: void 0, // No rollback for approve
        }),
      },
      {
        tool: 'adjust_inventory',
        params: {
          sku: params.sku,
          quantity: Math.abs(params.quantity), // Positive for restock
          reason: 'Return processed',
        },
        validate: (result) => ({ valid: !!result.stock }),
      },
      {
        tool: 'create_refund',
        params: {
          paymentId: params.paymentId || params.orderId,
          amount: params.amount,
        },
        rollback: async (result) => ({
          tool: void 0, // No rollback path for refunds in current payment toolset
          params: { paymentId: result.paymentId },
        }),
      },
    ]);
  }

  /**
   * Checkout flow (cart → order → payment → fulfillment)
   * Example: Get cart → Reserve inventory → Create order → Process payment → Ship
   */
  async completeCheckout(params) {
    return this.orchestrate('complete-checkout', [
      {
        tool: 'get_cart',
        params: { cartId: params.cartId },
        validate: (result) => ({ valid: result.cart?.itemCount > 0 }),
      },
      {
        tool: 'calculate_tax',
        params: (previousResults) => ({
          items: previousResults[0].result.cart.items,
          shippingAddress: previousResults[0].result.cart.shippingAddress,
        }),
      },
      {
        tool: 'reserve_inventory',
        params: (previousResults) => ({
          sku: previousResults[0].result.cart.items[0].sku,
          quantity: previousResults[0].result.cart.items[0].quantity,
          referenceType: 'order',
          referenceId: `pending-${Date.now()}`,
          expiresInSeconds: 3600,
        }),
        validate: (result) => ({ valid: !!result.reservation?.id }),
        rollback: async (result) => ({
          tool: 'release_reservation',
          params: { reservationId: result.reservation.id },
        }),
      },
      {
        tool: 'create_order',
        params: (previousResults) => ({
          customerId: previousResults[0].result.cart.customerId,
          items: previousResults[0].result.cart.items,
          currency: previousResults[0].result.cart.currency,
        }),
        validate: (result) => ({ valid: !!result.order?.id }),
        rollback: async (result) => ({
          tool: 'cancel_order',
          params: { orderId: result.order.id },
        }),
      },
      {
        tool: 'create_payment',
        params: (previousResults) => ({
          orderId: previousResults[3].result.order.id,
          amount: previousResults[3].result.order.totalAmount,
          method: params.paymentMethod,
        }),
        validate: (result) => ({ valid: !!result.payment?.id }),
        rollback: async (result) => ({
          tool: 'create_refund',
          params: { paymentId: result.payment.id },
        }),
      },
      {
        tool: 'update_order_status',
        params: (previousResults) => ({
          orderId: previousResults[3].result.order.id,
          status: 'confirmed',
        }),
      },
      {
        tool: 'confirm_reservation',
        params: (previousResults) => ({
          reservationId: previousResults[2].result.reservation.id,
        }),
      },
    ]);
  }

  /**
   * Get orchestration status by ID
   */
  getStatus(orchestrationId) {
    return this.orchestrations.get(orchestrationId);
  }

  /**
   * Get all active orchestrations
   */
  getActiveOrchestrations() {
    return Array.from(this.orchestrations.values());
  }

  /**
   * Cancel orchestration (if still running)
   */
  async cancel(orchestrationId) {
    const orchestration = this.orchestrations.get(orchestrationId);
    if (!orchestration) {
      throw new Error('Orchestration not found');
    }

    if (orchestration.status !== 'running') {
      throw new Error('Orchestration is not running');
    }

    // Trigger rollbacks
    orchestration.status = 'cancelled';
    this.emit('orchestration:cancelled', { orchestrationId });

    return orchestration;
  }
}

/**
 * Pre-defined orchestration templates for common workflows
 */
export const ORCHESTRATION_TEMPLATES = {
  checkout: {
    name: 'Complete Checkout',
    steps: [
      { tool: 'get_cart', validate: (r) => r.cart?.itemCount > 0 },
      { tool: 'calculate_tax' },
      { tool: 'reserve_inventory', rollback: 'release_reservation' },
      { tool: 'create_order', rollback: 'cancel_order' },
      { tool: 'create_payment', rollback: 'create_refund' },
      { tool: 'update_order_status', params: { status: 'confirmed' } },
      { tool: 'confirm_reservation' },
    ],
  },

  return: {
    name: 'Process Return',
    steps: [
      { tool: 'approve_return' },
      { tool: 'adjust_inventory', params: { reason: 'Return processed' } },
      { tool: 'create_refund' },
    ],
  },

  agentic_payment: {
    name: 'Agentic Payment',
    steps: [{ tool: 'x402_execute_agent_payment' }, { tool: 'x402_get_intent' }],
  },

  mpp_paid_tool_call: {
    name: 'MPP Paid Tool Call',
    steps: [{ tool: 'agentic_payment_discovery' }, { tool: 'agentic_prepare_payment' }],
  },

  fulfillment: {
    name: 'Order Fulfillment',
    steps: [
      { tool: 'get_order', validate: (r) => r.order?.status === 'confirmed' },
      { tool: 'pack_order' },
      { tool: 'ship_order', rollback: 'unship_order' },
    ],
  },
};
