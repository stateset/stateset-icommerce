/**
 * Agent Troubleshooting & Debugging Tools for StateSet MCP Server
 * Helps agents self-diagnose and recover from errors
 */

import { EventEmitter } from 'events';

export class AgentDebugger extends EventEmitter {
  constructor(commerce) {
    super();
    this.commerce = commerce;
    this.debugSessions = new Map();
    this.errorPatterns = new Map();
    this.solutions = new Map();
    this.initializeErrorPatterns();
    this.initializeSolutions();
  }

  /**
   * Initialize common error patterns for commerce operations
   */
  initializeErrorPatterns() {
    this.errorPatterns.set('insufficient_stock', {
      pattern: /insufficient stock/i,
      category: 'inventory',
      severity: 'high',
      context: 'User tried to reserve more items than available',
    });

    this.errorPatterns.set('order_not_found', {
      pattern: /order not found/i,
      category: 'orders',
      severity: 'medium',
      context: 'User tried to access an order that does not exist',
    });

    this.errorPatterns.set('customer_not_found', {
      pattern: /customer not found/i,
      category: 'customers',
      severity: 'high',
      context: 'User tried to access a customer that does not exist',
    });

    this.errorPatterns.set('invalid_status_transition', {
      pattern: /invalid.*transition|cannot.*transition/i,
      category: 'state_machine',
      severity: 'high',
      context: 'User tried to transition order to an invalid state',
    });

    this.errorPatterns.set('payment_failed', {
      pattern: /payment.*failed|authorization.*failed/i,
      category: 'payments',
      severity: 'high',
      context: 'Payment processing failed for order',
    });

    this.errorPatterns.set('duplicate_email', {
      pattern: /email.*already exists|duplicate.*email/i,
      category: 'customers',
      severity: 'medium',
      context: 'User tried to create customer with existing email',
    });

    this.errorPatterns.set('validation_error', {
      pattern: /validation error|validation failed/i,
      category: 'validation',
      severity: 'medium',
      context: 'Input validation failed',
    });

    this.errorPatterns.set('reservation_expired', {
      pattern: /reservation.*expired|expired reservation/i,
      category: 'inventory',
      severity: 'high',
      context: 'Inventory reservation has expired',
    });
  }

  /**
   * Initialize solution templates for common errors
   */
  initializeSolutions() {
    this.solutions.set('insufficient_stock', [
      {
        solution: 'check_stock_levels',
        description: 'Check current stock levels for the SKU',
        action: async (params) => {
          try {
            const stock = await this.commerce.inventory.getStock(params.sku);
            return {
              success: true,
              stock: {
                sku: stock.sku,
                totalOnHand: stock.totalOnHand,
                totalAvailable: stock.totalAvailable,
                totalAllocated: stock.totalAllocated,
              },
            };
          } catch (error) {
            return { success: false, error: error.message };
          }
        },
      },
      {
        solution: 'adjust_inventory',
        description: 'Add more stock to inventory',
        action: async (params) => ({
          tool: 'adjust_inventory',
          params: {
            sku: params.sku,
            quantity: params.quantityToAdd,
            reason: 'Restocking for order',
          },
        }),
      },
      {
        solution: 'create_backorder',
        description: 'Create backorder for unavailable items',
        action: async (params) => ({
          tool: 'create_backorder',
          params: {
            orderId: params.orderId,
            sku: params.sku,
            quantity: params.quantity,
            expectedDate: new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString(),
          },
        }),
      },
    ]);

    this.solutions.set('invalid_status_transition', [
      {
        solution: 'check_valid_transitions',
        description: 'Show valid transitions from current status',
        action: async (params) => {
          const validTransitions = {
            pending: ['confirmed', 'cancelled'],
            confirmed: ['processing', 'cancelled'],
            processing: ['shipped', 'cancelled'],
            shipped: ['delivered'],
            delivered: ['refunded'],
            cancelled: [],
            refunded: [],
          };

          return {
            success: true,
            currentStatus: params.currentStatus,
            validTransitions: validTransitions[params.currentStatus] || [],
          };
        },
      },
      {
        solution: 'query_order_status',
        description: 'Query current order status',
        action: async (params) => ({
          tool: 'get_order',
          params: { orderId: params.orderId },
        }),
      },
    ]);

    this.solutions.set('payment_failed', [
      {
        solution: 'retry_payment',
        description: 'Retry payment processing',
        action: async (params) => ({
          tool: 'create_payment',
          params: {
            orderId: params.orderId,
            amount: params.amount,
            method: params.method,
          },
        }),
      },
      {
        solution: 'check_payment_status',
        description: 'Check payment status',
        action: async (params) => ({
          tool: 'get_order',
          params: { orderId: params.orderId },
        }),
      },
    ]);
  }

  /**
   * Analyze an error and provide AI-friendly explanation
   * @param {Error} error - The error to analyze
   * @param {Object} context - Context about what operation failed
   * @returns {Promise<Object>} Detailed error analysis and suggestions
   */
  async analyzeError(error, context = {}) {
    const errorAnalysis = {
      error: error.message,
      errorType: error.name,
      timestamp: new Date().toISOString(),
      context,
      pattern: null,
      suggestions: [],
      recoveryActions: [],
    };

    // Identify error pattern
    for (const [name, pattern] of this.errorPatterns) {
      if (pattern.pattern.test(error.message)) {
        errorAnalysis.pattern = {
          name,
          ...pattern,
        };
        break;
      }
    }

    // Add technical explanation
    errorAnalysis.technicalExplanation = this.generateTechnicalExplanation(
      error,
      errorAnalysis.pattern,
    );

    // Add context-specific suggestions
    if (context.tool) {
      errorAnalysis.toolContext = this.analyzeToolContext(context.tool, error);
    }

    // Add suggested solutions
    if (errorAnalysis.pattern && this.solutions.has(errorAnalysis.pattern.name)) {
      const solutions = this.solutions.get(errorAnalysis.pattern.name);
      errorAnalysis.suggestedSolutions = solutions.map((s) => ({
        solution: s.solution,
        description: s.description,
        canAutoApply: typeof s.action === 'function',
      }));
    }

    // Add recovery examples
    errorAnalysis.recoveryExamples = this.generateRecoveryExamples(
      error,
      errorAnalysis.pattern,
      context,
    );

    this.emit('error:analyzed', { error, analysis: errorAnalysis });
    return errorAnalysis;
  }

  /**
   * Generate technical explanation for error
   */
  generateTechnicalExplanation(error, pattern) {
    if (!pattern) {
      return `Unknown error: ${error.message}. This error doesn't match any known patterns. Check the error details and context for more information.`;
    }

    const explanations = {
      insufficient_stock: `The system cannot fulfill this request because there is insufficient inventory available. Specifically, the requested quantity exceeds the available stock (on-hand - allocated). You either need to: (1) Add more stock to inventory, (2) Reduce the order quantity, or (3) Create a backorder for the unavailable items.`,

      invalid_status_transition: `Orders can only transition between certain states according to the state machine. The attempted transition is not allowed because one of the pre-conditions is not met. Check the current order status and verify the target status is in the valid transitions list.`,

      order_not_found: `The order with the provided ID could not be found in the database. This could mean: (1) The ID is incorrect, (2) The order was deleted, or (3) The order does not exist yet. Verify the order ID and try again.`,

      customer_not_found: `The customer with the provided ID or email could not be found. This could mean: (1) The customer ID/email is incorrect, (2) The customer was deleted, or (3) The customer needs to be created first.`,

      payment_failed: `The payment could not be processed due to validation errors, insufficient funds, declined card, payment gateway issues, or fraud detection. Check the payment details, ensure the payment method is valid, and try again.`,

      duplicate_email: `A customer with this email already exists. Customer emails must be unique. You can either: (1) Use the existing customer's ID, or (2) Use a different email address.`,

      reservation_expired: `The inventory reservation has expired before it could be confirmed. This typically happens when a reservation is held too long without being confirmed. Create a new reservation and confirm it promptly.`,

      validation_error: `The provided input did not pass validation. Check that all required fields are present, values are in the correct format (UUIDs, email addresses, numbers), and values meet validation constraints (positive quantities, valid status enums, etc.).`,
    };

    return explanations[pattern.name] || `Error in ${pattern.category}: ${pattern.context}`;
  }

  /**
   * Analyze context based on the tool that failed
   */
  analyzeToolContext(tool, _error) {
    const contexts = {
      create_order: {
        preamble: 'Failed while creating a new order.',
        commonCauses: [
          'Invalid customer ID',
          'Invalid product SKU',
          'Negative quantity or price',
          'Missing required fields',
        ],
        nextSteps: [
          'Verify customer ID exists with get_customer',
          'Verify product SKU exists with get_product_variant',
          'Ensure all quantities and prices are positive numbers',
        ],
      },

      reserve_inventory: {
        preamble: 'Failed while reserving inventory for an order.',
        commonCauses: [
          'Insufficient stock available',
          'Invalid SKU',
          'Existing reservation conflicts',
        ],
        nextSteps: [
          'Check stock levels with get_stock',
          'Verify SKU is valid',
          'Wait for existing reservations to expire',
        ],
      },

      update_order_status: {
        preamble: 'Failed while updating order status.',
        commonCauses: [
          'Invalid status transition',
          'Order not in correct state for transition',
          'Order has already been shipped/delivered',
        ],
        nextSteps: [
          'Check current order status with get_order',
          'Review valid status transitions',
          'Verify order is in appropriate state for transition',
        ],
      },

      create_payment: {
        preamble: 'Failed while processing payment.',
        commonCauses: [
          'Invalid payment method details',
          'Payment gateway connection issues',
          'Invalid order or amount',
          'Payment method already used',
        ],
        nextSteps: [
          'Verify payment method is valid',
          'Check payment gateway status',
          'Ensure order is in payable state',
        ],
      },
    };

    return (
      contexts[tool] || {
        preamble: `Failed while executing tool: ${tool}`,
        commonCauses: ['Invalid input', 'Resource not found', 'Permission denied'],
        nextSteps: ['Verify input parameters', 'Check resource exists', 'Review error details'],
      }
    );
  }

  /**
   * Generate recovery examples with actual code
   */
  generateRecoveryExamples(error, pattern, context) {
    const examples = [];

    if (pattern?.name === 'insufficient_stock' && context?.tool === 'create_order') {
      examples.push({
        title: 'Check Stock Before Creating Order',
        description: 'First check stock levels, then adjust inventory or create backorder',
        steps: [
          'Step 1: Check stock levels with get_stock',
          'Step 2: Verify available quantity meets order requirement',
          'Step 3a: If sufficient, proceed with create_order',
          'Step 3b: If insufficient, either adjust_inventory or create_backorder',
        ],
        example: {
          step1: { tool: 'get_stock', params: { sku: 'PRODUCT-001' } },
          step2a: {
            if: 'stock.totalAvailable >= orderQuantity',
            then: {
              tool: 'create_order',
              params: {
                customerId: 'CUSTOMER_ID',
                items: [{ sku: 'PRODUCT-001', quantity: 1 }],
                currency: 'USD',
              },
            },
          },
          step2b: {
            if: 'stock.totalAvailable < orderQuantity',
            options: [
              {
                tool: 'adjust_inventory',
                params: { sku: 'PRODUCT-001', quantity: 100, reason: 'Restock' },
              },
              {
                tool: 'create_backorder',
                params: {
                  orderId: 'ORDER_ID',
                  sku: 'PRODUCT-001',
                  quantity: 'orderQuantity - stock.totalAvailable',
                },
              },
            ],
          },
        },
      });
    }

    if (pattern?.name === 'invalid_status_transition' && context?.tool === 'update_order_status') {
      examples.push({
        title: 'Check Valid Status Transitions',
        description: 'Verify the current order status and target status is valid',
        steps: [
          'Step 1: Get current order status with get_order',
          'Step 2: Check valid transitions for current status',
          'Step 3: If transition is invalid, use a valid status',
        ],
        example: {
          step1: { tool: 'get_order', params: { orderId: 'xxx-xxx-xxx' } },
          step2: {
            validTransitions: {
              pending: ['confirmed', 'cancelled'],
              confirmed: ['processing', 'cancelled'],
              processing: ['shipped', 'cancelled'],
              shipped: ['delivered'],
            },
          },
          step3: {
            note: 'If trying to go from pending to shipped, this is invalid. Use confirmed → processing → shipped instead',
          },
        },
      });
    }

    return examples;
  }

  /**
   * Auto-recover from common errors (when safe to do so)
   */
  async attemptAutoRecovery(error, context) {
    const analysis = await this.analyzeError(error, context);

    if (!analysis.pattern) {
      return { canAutoRecover: false, reason: 'Unknown error pattern' };
    }

    const recoveryStrategies = {
      insufficient_stock: async () => {
        // Cannot auto-recover stock issues - requires user/conversation
        return {
          canAutoRecover: false,
          reason:
            'Insufficient stock requires manual intervention or inventory adjustment. Ask user how they want to proceed.',
        };
      },

      invalid_status_transition: async () => {
        // Could retry with next valid status
        const currentStatus = context?.currentStatus;
        if (currentStatus) {
          const validTransitions = {
            pending: ['confirmed', 'cancelled'],
            confirmed: ['processing', 'cancelled'],
            processing: ['shipped', 'cancelled'],
            shipped: ['delivered'],
          };

          return {
            canAutoRecover: true,
            strategy: 'retry_with_valid_status',
            suggestion: `Current status is '${currentStatus}'. Valid next statuses are: ${validTransitions[currentStatus]?.join(', ') || 'none'}`,
            attempt: {
              tool: 'update_order_status',
              params: {
                orderId: context.orderId,
                status: validTransitions[currentStatus]?.[0] || suggestedAction(),
              },
            },
          };
        }

        return { canAutoRecover: false, reason: 'Cannot auto-recover without current status' };
      },

      order_not_found: async () => {
        return {
          canAutoRecover: false,
          reason: 'Order not found. Verify the order ID or create a new order if needed.',
          suggestion:
            'Use list_orders to find the correct order ID, or create a new order with create_order',
        };
      },
    };

    const strategy = recoveryStrategies[analysis.pattern.name];
    return strategy
      ? strategy()
      : { canAutoRecover: false, reason: 'No auto-recovery strategy available' };
  }

  /**
   * Create a debug session for tracking multiple related errors
   */
  createDebugSession(context = {}) {
    const sessionId = `debug-${Date.now()}`;
    const session = {
      id: sessionId,
      createdAt: new Date().toISOString(),
      context,
      errors: [],
      analysis: null,
      recoveryAttempted: false,
      status: 'active',
    };

    this.debugSessions.set(sessionId, session);
    this.emit('debug:session:created', session);
    return session;
  }

  /**
   * Add error to debug session
   */
  addErrorToSession(sessionId, error, context) {
    const session = this.debugSessions.get(sessionId);
    if (!session) {
      throw new Error(`Debug session ${sessionId} not found`);
    }

    session.errors.push({
      error: error.message,
      timestamp: new Date().toISOString(),
      context,
    });

    this.emit('debug:session:error_added', { sessionId, error });
    return session;
  }

  /**
   * Debug session health check
   */
  async diagnoseSession(sessionId) {
    const session = this.debugSessions.get(sessionId);
    if (!session) {
      throw new Error(`Debug session ${sessionId} not found`);
    }

    const diagnoses = await Promise.all(
      session.errors.map((e) => this.analyzeError(new Error(e.error), e.context)),
    );

    return {
      sessionId,
      errorCount: session.errors.length,
      diagnoses,
      recommendations: this.generateSessionRecommendations(diagnoses),
    };
  }

  /**
   * Generate recommendations for debugging session
   */
  generateSessionRecommendations(diagnoses) {
    const recommendations = [];
    const patternCounts = {};

    for (const diagnosis of diagnoses) {
      if (diagnosis.pattern) {
        patternCounts[diagnosis.pattern.name] = (patternCounts[diagnosis.pattern.name] || 0) + 1;
      }
    }

    for (const [pattern, count] of Object.entries(patternCounts)) {
      if (count > 1) {
        recommendations.push({
          issue: pattern,
          count,
          severity: this.errorPatterns.get(pattern)?.severity || 'medium',
          suggestion: `The ${pattern} error occurred ${count} times. This suggests a systematic issue that should be addressed. Review the error analysis for ${pattern} for detailed solutions.`,
        });
      }
    }

    return recommendations;
  }
}

/**
 * Helper function to suggest valid action
 */
function suggestedAction() {
  return 'Check error analysis for suggested next steps';
}
