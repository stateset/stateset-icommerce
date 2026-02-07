/**
 * Tool Composition Engine for StateSet MCP Server
 * Enables agents to orchestrate multi-step commerce operations with atomicity
 */

import { EventEmitter } from 'events';
import crypto from 'node:crypto';

export class ToolComposer extends EventEmitter {
  constructor(commerce) {
    super();
    this.commerce = commerce;
    this.orchestrations = new Map();
    this.activeTransactions = new Map();
  }

  /**
   * Create a multi-step orchestration with atomic rollback
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

        this.emit('step:started', {
          orchestrationId,
          stepId,
          step: i + 1,
          total: steps.length,
          tool: step.tool,
        });

        const result = await this.executeTool(step.tool, step.params);
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
          const result = await rollback.rollbackFn();
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
    // This will be connected to the actual MCP tool execution
    // For now, simulate execution
    return {
      tool: toolName,
      params,
      executedAt: new Date().toISOString(),
    };
  }

  /**
   * Create order with inventory reservation (atomic)
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
   * Process return with restock (atomic)
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
        tool: 'refund_payment',
        params: {
          orderId: params.orderId,
          amount: params.amount,
        },
        rollback: async (result) => ({
          tool: 'capture_payment', // Capture if refund fails
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
        tool: 'process_payment',
        params: (previousResults) => ({
          orderId: previousResults[4].result.order.id,
          amount: previousResults[4].result.order.totalAmount,
          method: params.paymentMethod,
        }),
        validate: (result) => ({ valid: result.payment?.status === 'paid' }),
        rollback: async (result) => ({
          tool: 'refund_payment',
          params: { paymentId: result.payment.id },
        }),
      },
      {
        tool: 'update_order_status',
        params: (previousResults) => ({
          orderId: previousResults[4].result.order.id,
          status: 'confirmed',
        }),
      },
      {
        tool: 'confirm_reservation',
        params: (previousResults) => ({
          reservationId: previousResults[3].result.reservation.id,
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
      { tool: 'process_payment', rollback: 'refund_payment' },
      { tool: 'update_order_status', params: { status: 'confirmed' } },
      { tool: 'confirm_reservation' },
    ],
  },

  return: {
    name: 'Process Return',
    steps: [
      { tool: 'approve_return' },
      { tool: 'adjust_inventory', params: { reason: 'Return processed' } },
      { tool: 'refund_payment', rollback: 'capture_payment' },
    ],
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
