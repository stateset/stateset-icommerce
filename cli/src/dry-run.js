/**
 * Dry Run Module for StateSet CLI
 *
 * Provides --dry-run functionality to preview operations
 * without executing them.
 */

import { createLogger } from './logger.js';
import { ICONS } from './output.js';

const logger = createLogger({ context: { module: 'dry-run' } });

/**
 * Operations that can be previewed in dry-run mode
 */
export const PREVIEWABLE_OPERATIONS = {
  // Customer operations
  create_customer: {
    description: 'Create new customer',
    format: (params) => `Create customer: ${params.email} (${params.firstName} ${params.lastName})`
  },

  // Order operations
  create_order: {
    description: 'Create new order',
    format: (params) => {
      const itemCount = params.items?.length || 0;
      const total = params.items?.reduce((sum, i) => sum + (i.quantity || 1) * (i.unitPrice || 0), 0) || 0;
      return `Create order for customer ${params.customerId} with ${itemCount} items (${params.currency || 'USD'} ${total.toFixed(2)})`
    }
  },
  update_order_status: {
    description: 'Update order status',
    format: (params) => `Update order ${params.orderId} status to: ${params.status}`
  },
  ship_order: {
    description: 'Ship order',
    format: (params) => `Ship order ${params.orderId}${params.trackingNumber ? ` with tracking: ${params.trackingNumber}` : ''}`
  },
  cancel_order: {
    description: 'Cancel order',
    format: (params) => `Cancel order ${params.orderId}`
  },

  // Inventory operations
  create_inventory_item: {
    description: 'Create inventory item',
    format: (params) => `Create inventory item: ${params.sku} (${params.name}) with ${params.initialQuantity || 0} units`
  },
  adjust_inventory: {
    description: 'Adjust inventory',
    format: (params) => {
      const sign = params.quantity > 0 ? '+' : '';
      return `Adjust ${params.sku} by ${sign}${params.quantity} (${params.reason})`
    }
  },
  reserve_inventory: {
    description: 'Reserve inventory',
    format: (params) => `Reserve ${params.quantity} units of ${params.sku}${params.orderId ? ` for order ${params.orderId}` : ''}`
  },
  confirm_reservation: {
    description: 'Confirm reservation',
    format: (params) => `Confirm reservation for ${params.sku}`
  },
  release_reservation: {
    description: 'Release reservation',
    format: (params) => `Release ${params.quantity} reserved units of ${params.sku}`
  },

  // Return operations
  create_return: {
    description: 'Create return',
    format: (params) => `Create return for order ${params.orderId} (${params.reason})`
  },
  approve_return: {
    description: 'Approve return',
    format: (params) => `Approve return ${params.returnId}`
  },
  reject_return: {
    description: 'Reject return',
    format: (params) => `Reject return ${params.returnId} (${params.reason})`
  },

  // Cart operations
  create_cart: {
    description: 'Create cart',
    format: (params) => `Create cart${params.customerEmail ? ` for ${params.customerEmail}` : ''}`
  },
  add_cart_item: {
    description: 'Add item to cart',
    format: (params) => `Add ${params.quantity || 1}x ${params.sku} to cart ${params.cartId}`
  },
  update_cart_item: {
    description: 'Update cart item',
    format: (params) => `Update cart item quantity to ${params.quantity}`
  },
  remove_cart_item: {
    description: 'Remove cart item',
    format: (params) => `Remove item ${params.itemId} from cart ${params.cartId}`
  },
  complete_checkout: {
    description: 'Complete checkout',
    format: (params) => `Complete checkout for cart ${params.cartId}`
  },
  cancel_cart: {
    description: 'Cancel cart',
    format: (params) => `Cancel cart ${params.cartId}`
  },

  // Admin operations
  set_exchange_rate: {
    description: 'Set exchange rate',
    format: (params) => `Set exchange rate ${params.fromCurrency} → ${params.toCurrency}: ${params.rate}`
  },
  set_base_currency: {
    description: 'Set base currency',
    format: (params) => `Set store base currency to ${params.currency}`
  }
};

/**
 * DryRunManager - Manages dry-run mode operations
 */
export class DryRunManager {
  constructor(options = {}) {
    this.enabled = options.enabled || false;
    this.operations = [];
    this.onPreview = options.onPreview || null;
  }

  /**
   * Check if dry-run mode is enabled
   */
  isEnabled() {
    return this.enabled;
  }

  /**
   * Enable/disable dry-run mode
   */
  setEnabled(enabled) {
    this.enabled = enabled;
  }

  /**
   * Preview an operation without executing it
   */
  preview(operationName, params) {
    const operation = PREVIEWABLE_OPERATIONS[operationName];

    const preview = {
      operation: operationName,
      description: operation?.description || `Execute ${operationName}`,
      formatted: operation?.format?.(params) || `${operationName}(${JSON.stringify(params)})`,
      params,
      timestamp: new Date().toISOString()
    };

    this.operations.push(preview);

    if (this.onPreview) {
      this.onPreview(preview);
    }

    logger.debug('Dry-run preview', preview);

    return preview;
  }

  /**
   * Get all previewed operations
   */
  getOperations() {
    return [...this.operations];
  }

  /**
   * Clear previewed operations
   */
  clear() {
    this.operations = [];
  }

  /**
   * Get summary of previewed operations
   */
  getSummary() {
    const byType = {};
    for (const op of this.operations) {
      byType[op.operation] = (byType[op.operation] || 0) + 1;
    }

    return {
      total: this.operations.length,
      byType
    };
  }

  /**
   * Format operations for display
   */
  formatOperations(options = {}) {
    if (this.operations.length === 0) {
      return 'No operations to preview.';
    }

    const lines = [
      `\n${ICONS.info} Dry-run mode: The following operations would be executed:\n`
    ];

    for (let i = 0; i < this.operations.length; i++) {
      const op = this.operations[i];
      lines.push(`  ${i + 1}. ${op.formatted}`);

      if (options.verbose) {
        lines.push(`     ${JSON.stringify(op.params)}`);
      }
    }

    lines.push(`\nTotal: ${this.operations.length} operation(s)`);
    lines.push('\nTo execute these operations, run without --dry-run flag.\n');

    return lines.join('\n');
  }

  /**
   * Create a tool wrapper for dry-run mode
   */
  wrapTool(toolName, handler) {
    return async (params) => {
      if (this.enabled && this.isWriteOperation(toolName)) {
        const preview = this.preview(toolName, params);

        return {
          content: [{
            type: 'text',
            text: JSON.stringify({
              dryRun: true,
              wouldExecute: toolName,
              preview: preview.formatted,
              params
            }, null, 2)
          }]
        };
      }

      return handler(params);
    };
  }

  /**
   * Check if operation is a write operation
   */
  isWriteOperation(operationName) {
    const writeOperations = Object.keys(PREVIEWABLE_OPERATIONS);
    return writeOperations.includes(operationName);
  }
}

/**
 * Create a dry-run manager
 */
export function createDryRunManager(options = {}) {
  return new DryRunManager(options);
}

/**
 * Format dry-run result for CLI output
 */
export function formatDryRunResult(result, options = {}) {
  const { color = true } = options;

  const yellow = color ? '\x1b[33m' : '';
  const gray = color ? '\x1b[90m' : '';
  const reset = color ? '\x1b[0m' : '';

  const lines = [
    `\n${yellow}[DRY-RUN]${reset} Would execute: ${result.operation}`,
    `${gray}${result.formatted}${reset}`
  ];

  if (options.showParams) {
    lines.push(`${gray}Parameters: ${JSON.stringify(result.params, null, 2)}${reset}`);
  }

  return lines.join('\n');
}

/**
 * Parse --dry-run flag from CLI arguments
 */
export function parseDryRunFlag(args) {
  const index = args.findIndex(a => a === '--dry-run' || a === '-n');
  if (index !== -1) {
    return {
      enabled: true,
      args: [...args.slice(0, index), ...args.slice(index + 1)]
    };
  }
  return { enabled: false, args };
}

export default {
  PREVIEWABLE_OPERATIONS,
  DryRunManager,
  createDryRunManager,
  formatDryRunResult,
  parseDryRunFlag
};
