/**
 * Fine-Grained Permissions & Guardrails for StateSet CLI
 *
 * Provides configurable permission levels, spending limits,
 * rate limiting, and audit logging for agent operations.
 */

// ============================================================================
// Permission Levels
// ============================================================================

/**
 * Permission levels from lowest to highest privilege
 */
export const PERMISSION_LEVELS = {
  none: 0,      // No operations allowed
  read: 1,      // List, get, query operations
  preview: 2,   // Read + show what would happen
  write: 3,     // Create, update operations
  delete: 4,    // Cancel, void, delete operations
  admin: 5      // Bulk operations, settings, full access
};

/**
 * Map tool names to required permission level
 */
export const TOOL_PERMISSIONS = {
  // Customer tools
  list_customers: 'read',
  get_customer: 'read',
  create_customer: 'write',

  // Order tools
  list_orders: 'read',
  get_order: 'read',
  create_order: 'write',
  update_order_status: 'write',
  ship_order: 'write',
  cancel_order: 'delete',

  // Product tools
  list_products: 'read',
  get_product: 'read',
  get_product_variant: 'read',
  create_product: 'write',

  // Inventory tools
  get_stock: 'read',
  create_inventory_item: 'write',
  adjust_inventory: 'write',
  reserve_inventory: 'write',
  confirm_reservation: 'write',
  release_reservation: 'write',

  // Return tools
  list_returns: 'read',
  get_return: 'read',
  create_return: 'write',
  approve_return: 'write',
  reject_return: 'write',

  // Cart/Checkout tools
  list_carts: 'read',
  get_cart: 'read',
  create_cart: 'write',
  add_cart_item: 'write',
  update_cart_item: 'write',
  remove_cart_item: 'write',
  set_cart_shipping_address: 'write',
  set_cart_payment: 'write',
  apply_cart_discount: 'write',
  get_shipping_rates: 'read',
  complete_checkout: 'write',
  cancel_cart: 'delete',
  abandon_cart: 'write',
  get_abandoned_carts: 'read',

  // Analytics tools (all read-only)
  get_sales_summary: 'read',
  get_top_products: 'read',
  get_customer_metrics: 'read',
  get_top_customers: 'read',
  get_inventory_health: 'read',
  get_low_stock_items: 'read',
  get_demand_forecast: 'read',
  get_revenue_forecast: 'read',
  get_order_status_breakdown: 'read',
  get_return_metrics: 'read',

  // Currency tools
  get_exchange_rate: 'read',
  list_exchange_rates: 'read',
  convert_currency: 'read',
  set_exchange_rate: 'admin',
  get_currency_settings: 'read',
  set_base_currency: 'admin',
  enable_currencies: 'admin',
  format_currency: 'read',

  // Tax tools
  calculate_tax: 'read',
  get_tax_rate: 'read',
  list_tax_jurisdictions: 'read',
  list_tax_rates: 'read',
  get_tax_settings: 'read',
  get_us_state_tax_info: 'read',
  get_customer_tax_exemptions: 'read',
  create_tax_exemption: 'write',
  calculate_cart_tax: 'read'
};

// ============================================================================
// Default Guardrails
// ============================================================================

/**
 * Default guardrail configuration
 */
export const DEFAULT_GUARDRAILS = {
  // Spending limits
  maxOrderValue: 10000,           // Maximum single order value
  maxDailyOrderTotal: 100000,     // Maximum daily order total
  maxDailyOrderCount: 500,        // Maximum orders per day

  // Inventory limits
  maxInventoryAdjustment: 1000,   // Maximum single adjustment quantity

  // Rate limits (per minute)
  maxToolCallsPerMinute: 120,
  maxWriteOpsPerMinute: 30,

  // Confirmation thresholds
  confirmOrdersAbove: 1000,       // Ask for confirmation on orders > $1000
  confirmBulkOperations: true,    // Confirm bulk operations

  // Blocked operations
  blockedTools: [],               // Tools to completely disable

  // Require explicit approval for these tools
  requireApprovalFor: [
    'cancel_order',
    'complete_checkout'
  ]
};

// ============================================================================
// Permission Gate
// ============================================================================

/**
 * PermissionGate - Enforces permission levels and guardrails
 *
 * Usage:
 *   const gate = new PermissionGate({ level: 'write', guardrails: { maxOrderValue: 5000 } });
 *   const check = await gate.checkPermission('create_order', { totalAmount: 100 });
 *   if (check.allowed) { ... }
 */
export class PermissionGate {
  constructor(options = {}) {
    // Permission level
    const levelName = options.level || 'preview';
    this.level = PERMISSION_LEVELS[levelName] ?? PERMISSION_LEVELS.preview;
    this.levelName = levelName;

    // Guardrails
    this.guardrails = { ...DEFAULT_GUARDRAILS, ...options.guardrails };

    // Callbacks
    this.onConfirmRequired = options.onConfirmRequired || null;
    this.onPermissionDenied = options.onPermissionDenied || null;

    // Audit log
    this.auditLog = [];

    // Rate limiting state
    this.rateLimitState = {
      toolCalls: [],
      writeOps: [],
      dailyOrders: { count: 0, total: 0, date: this._getDateKey() }
    };
  }

  // --------------------------------------------------------------------------
  // Permission Checking
  // --------------------------------------------------------------------------

  /**
   * Check if an operation is allowed
   *
   * @param {string} toolName - Tool name (without mcp__ prefix)
   * @param {object} params - Tool parameters
   * @returns {object} { allowed, reason?, preview? }
   */
  async checkPermission(toolName, params = {}) {
    // Normalize tool name
    const normalizedName = toolName.replace('mcp__stateset-commerce__', '');

    // Check blocked tools
    if (this.guardrails.blockedTools.includes(normalizedName)) {
      return this._deny(`Tool '${normalizedName}' is blocked by policy`);
    }

    // Check permission level
    const requiredLevel = TOOL_PERMISSIONS[normalizedName] || 'read';
    const requiredValue = PERMISSION_LEVELS[requiredLevel] || 0;

    if (requiredValue > this.level) {
      // Special case: preview mode shows what would happen
      if (this.level === PERMISSION_LEVELS.preview && requiredValue <= PERMISSION_LEVELS.write) {
        return {
          allowed: false,
          preview: true,
          reason: `Preview mode: would execute '${normalizedName}' if --apply flag is set`,
          wouldDo: { tool: normalizedName, params }
        };
      }

      return this._deny(
        `Operation requires '${requiredLevel}' permission (current: '${this.levelName}')`
      );
    }

    // Check rate limits
    const rateLimitCheck = this._checkRateLimits(normalizedName);
    if (!rateLimitCheck.allowed) {
      return rateLimitCheck;
    }

    // Check domain-specific guardrails
    const guardrailCheck = await this._checkGuardrails(normalizedName, params);
    if (!guardrailCheck.allowed) {
      return guardrailCheck;
    }

    // Check if confirmation is required
    if (this.guardrails.requireApprovalFor.includes(normalizedName)) {
      if (this.onConfirmRequired) {
        const confirmed = await this.onConfirmRequired({
          tool: normalizedName,
          params,
          message: `Confirm execution of '${normalizedName}'?`
        });
        if (!confirmed) {
          return this._deny('User declined confirmation');
        }
      }
    }

    // Log the allowed operation
    this._logAudit(normalizedName, params, 'allowed');

    return { allowed: true };
  }

  // --------------------------------------------------------------------------
  // Rate Limiting
  // --------------------------------------------------------------------------

  _checkRateLimits(toolName) {
    const now = Date.now();
    const oneMinuteAgo = now - 60000;

    // Clean old entries
    this.rateLimitState.toolCalls = this.rateLimitState.toolCalls.filter(t => t > oneMinuteAgo);
    this.rateLimitState.writeOps = this.rateLimitState.writeOps.filter(t => t > oneMinuteAgo);

    // Check total tool calls
    if (this.rateLimitState.toolCalls.length >= this.guardrails.maxToolCallsPerMinute) {
      return this._deny(`Rate limit exceeded: ${this.guardrails.maxToolCallsPerMinute} tool calls per minute`);
    }

    // Check write operations
    const isWriteOp = ['write', 'delete', 'admin'].includes(TOOL_PERMISSIONS[toolName]);
    if (isWriteOp && this.rateLimitState.writeOps.length >= this.guardrails.maxWriteOpsPerMinute) {
      return this._deny(`Rate limit exceeded: ${this.guardrails.maxWriteOpsPerMinute} write operations per minute`);
    }

    // Record this call
    this.rateLimitState.toolCalls.push(now);
    if (isWriteOp) {
      this.rateLimitState.writeOps.push(now);
    }

    return { allowed: true };
  }

  // --------------------------------------------------------------------------
  // Domain-Specific Guardrails
  // --------------------------------------------------------------------------

  async _checkGuardrails(toolName, params) {
    // Order value limits
    if (toolName === 'create_order') {
      const total = this._calculateOrderTotal(params);
      if (total > this.guardrails.maxOrderValue) {
        return this._deny(`Order value $${total} exceeds maximum $${this.guardrails.maxOrderValue}`);
      }

      // Check daily limits
      this._resetDailyCountersIfNeeded();
      if (this.rateLimitState.dailyOrders.count >= this.guardrails.maxDailyOrderCount) {
        return this._deny(`Daily order limit (${this.guardrails.maxDailyOrderCount}) exceeded`);
      }
      if (this.rateLimitState.dailyOrders.total + total > this.guardrails.maxDailyOrderTotal) {
        return this._deny(`Daily order total would exceed $${this.guardrails.maxDailyOrderTotal}`);
      }

      // Confirmation for large orders
      if (total > this.guardrails.confirmOrdersAbove && this.onConfirmRequired) {
        const confirmed = await this.onConfirmRequired({
          tool: toolName,
          params,
          amount: total,
          message: `Create order for $${total.toFixed(2)}?`
        });
        if (!confirmed) {
          return this._deny('User declined confirmation for large order');
        }
      }
    }

    // Checkout value limits
    if (toolName === 'complete_checkout') {
      // Would need to fetch cart to check value
      // For now, always require confirmation (handled in requireApprovalFor)
    }

    // Inventory adjustment limits
    if (toolName === 'adjust_inventory') {
      const qty = Math.abs(params.quantity || 0);
      if (qty > this.guardrails.maxInventoryAdjustment) {
        return this._deny(`Adjustment quantity ${qty} exceeds maximum ${this.guardrails.maxInventoryAdjustment}`);
      }
    }

    return { allowed: true };
  }

  _calculateOrderTotal(params) {
    if (params.totalAmount) return params.totalAmount;
    if (params.items) {
      return params.items.reduce((sum, item) => {
        return sum + (item.quantity || 1) * (item.unitPrice || 0);
      }, 0);
    }
    return 0;
  }

  _getDateKey() {
    return new Date().toISOString().split('T')[0];
  }

  _resetDailyCountersIfNeeded() {
    const today = this._getDateKey();
    if (this.rateLimitState.dailyOrders.date !== today) {
      this.rateLimitState.dailyOrders = { count: 0, total: 0, date: today };
    }
  }

  // --------------------------------------------------------------------------
  // Audit Logging
  // --------------------------------------------------------------------------

  _logAudit(toolName, params, result, reason = null) {
    const entry = {
      timestamp: new Date().toISOString(),
      tool: toolName,
      params: this._sanitizeParams(params),
      result,
      reason,
      level: this.levelName
    };

    this.auditLog.push(entry);

    // Keep last 1000 entries
    if (this.auditLog.length > 1000) {
      this.auditLog = this.auditLog.slice(-1000);
    }
  }

  _sanitizeParams(params) {
    // Remove sensitive fields for audit log
    const sanitized = { ...params };
    const sensitiveFields = ['password', 'token', 'secret', 'key', 'paymentToken'];
    for (const field of sensitiveFields) {
      if (sanitized[field]) {
        sanitized[field] = '[REDACTED]';
      }
    }
    return sanitized;
  }

  /**
   * Get audit log
   */
  getAuditLog(options = {}) {
    let log = this.auditLog;

    if (options.tool) {
      log = log.filter(e => e.tool === options.tool);
    }
    if (options.result) {
      log = log.filter(e => e.result === options.result);
    }
    if (options.since) {
      log = log.filter(e => new Date(e.timestamp) >= new Date(options.since));
    }
    if (options.limit) {
      log = log.slice(-options.limit);
    }

    return log;
  }

  /**
   * Export audit log for compliance
   */
  exportAuditLog() {
    return {
      exportedAt: new Date().toISOString(),
      permissionLevel: this.levelName,
      guardrails: this.guardrails,
      entries: this.auditLog
    };
  }

  // --------------------------------------------------------------------------
  // Helpers
  // --------------------------------------------------------------------------

  _deny(reason) {
    if (this.onPermissionDenied) {
      this.onPermissionDenied({ reason });
    }
    return { allowed: false, reason };
  }

  /**
   * Record a successful operation (for daily limits tracking)
   */
  recordOperation(toolName, params = {}) {
    if (toolName === 'create_order') {
      this._resetDailyCountersIfNeeded();
      this.rateLimitState.dailyOrders.count++;
      this.rateLimitState.dailyOrders.total += this._calculateOrderTotal(params);
    }
    this._logAudit(toolName, params, 'executed');
  }

  /**
   * Get current permission level name
   */
  getLevelName() {
    return this.levelName;
  }

  /**
   * Check if a specific permission level is met
   */
  hasPermission(level) {
    const requiredValue = PERMISSION_LEVELS[level] || 0;
    return this.level >= requiredValue;
  }

  /**
   * Get summary of current state
   */
  getSummary() {
    this._resetDailyCountersIfNeeded();
    const now = Date.now();
    const oneMinuteAgo = now - 60000;

    return {
      level: this.levelName,
      rateLimits: {
        toolCallsLastMinute: this.rateLimitState.toolCalls.filter(t => t > oneMinuteAgo).length,
        maxToolCallsPerMinute: this.guardrails.maxToolCallsPerMinute,
        writeOpsLastMinute: this.rateLimitState.writeOps.filter(t => t > oneMinuteAgo).length,
        maxWriteOpsPerMinute: this.guardrails.maxWriteOpsPerMinute
      },
      dailyLimits: {
        ordersToday: this.rateLimitState.dailyOrders.count,
        maxDailyOrders: this.guardrails.maxDailyOrderCount,
        totalToday: this.rateLimitState.dailyOrders.total,
        maxDailyTotal: this.guardrails.maxDailyOrderTotal
      },
      auditLogSize: this.auditLog.length
    };
  }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/**
 * Create a permission gate from CLI flags
 */
export function createPermissionGate(options = {}) {
  let level = 'preview';

  if (options.apply) {
    level = 'write';
  }
  if (options.admin) {
    level = 'admin';
  }
  if (options.readonly) {
    level = 'read';
  }

  return new PermissionGate({
    level,
    guardrails: options.guardrails,
    onConfirmRequired: options.onConfirmRequired,
    onPermissionDenied: options.onPermissionDenied
  });
}

/**
 * Get level name from CLI flags
 */
export function getLevelFromFlags(flags) {
  if (flags.admin) return 'admin';
  if (flags.apply) return 'write';
  if (flags.readonly) return 'read';
  return 'preview';
}

export default PermissionGate;
