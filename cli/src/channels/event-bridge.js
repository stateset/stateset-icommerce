/**
 * Event-to-Notification Bridge for StateSet iCommerce
 *
 * Subscribes to AutonomousEngine events and routes them as rich
 * notifications through the ChannelNotifier. This closes the loop
 * between commerce events and customer/ops messaging channels.
 *
 * Usage:
 *   const bridge = new EventBridge({ engine, notifier });
 *   bridge.start();
 */

import { createOrderSummary, createInventoryCard } from './rich-messages.js';

// ============================================================================
// Event-to-notification mapping
// ============================================================================

/**
 * @typedef {Object} EventMapping
 * @property {string}   notificationType - Notification routing key
 * @property {(data: any) => string} message - Plain text message builder
 * @property {(data: any) => import('./rich-messages.js').RichMessage|null} [richMessage] - Rich message builder
 */

/** @type {Object<string, EventMapping>} */
const DEFAULT_EVENT_MAP = {
  'scheduler:job:completed': {
    notificationType: 'job.completed',
    message: (d) => `Job completed: ${d.job?.name || 'unknown'} (${d.result?.duration || 0}ms)`,
  },

  'scheduler:job:failed': {
    notificationType: 'job.failed',
    message: (d) =>
      `Job failed: ${d.job?.name || 'unknown'} — ${d.result?.error || 'unknown error'}`,
  },

  'workflows:instance:completed': {
    notificationType: 'workflow.completed',
    message: (d) => `Workflow completed: ${d.instance?.workflowName || 'unknown'}`,
  },

  'workflows:instance:failed': {
    notificationType: 'workflow.failed',
    message: (d) => `Workflow failed: ${d.instance?.workflowName || 'unknown'}`,
  },

  'approvals:request:created': {
    notificationType: 'approvals:request:created',
    message: (d) => {
      const req = d.request || d;
      return `Approval required: ${req.title || 'Untitled'} ($${req.amount || 'N/A'})`;
    },
  },

  'approvals:request:approved': {
    notificationType: 'approval.approved',
    message: (d) => `Approved: ${d.request?.title || 'request'}`,
  },

  'approvals:request:denied': {
    notificationType: 'approval.denied',
    message: (d) => `Denied: ${d.request?.title || 'request'} — ${d.reason || 'no reason given'}`,
  },

  'webhooks:event:received': {
    notificationType: 'webhook.received',
    message: (d) =>
      `Webhook received: ${d.event?.sourceName || 'unknown'} — ${d.event?.eventType || 'unknown'}`,
  },

  notification: {
    notificationType: 'general',
    message: (d) => d.message || JSON.stringify(d),
  },

  // Heartbeat events
  'heartbeat:alert': {
    notificationType: 'heartbeat.alert',
    message: (d) => `Heartbeat Alert [${d.checkName || d.checkId}]: ${d.summary || 'triggered'}`,
  },

  'heartbeat:check:error': {
    notificationType: 'heartbeat.error',
    message: (d) => `Heartbeat check error [${d.checkId}]: ${d.error || 'unknown'}`,
  },
};

// ============================================================================
// Commerce event handlers (for direct commerce integration)
// ============================================================================

const COMMERCE_EVENT_BUILDERS = {
  /**
   * Build notification for order status changes.
   */
  'order.shipped': {
    notificationType: 'order.shipped',
    message: (order) =>
      `Order ${order.orderNumber || order.order_number || order.id} has shipped! Tracking: ${order.trackingNumber || order.tracking_number || 'pending'}`,
    richMessage: (order) => createOrderSummary(order),
  },

  'order.delivered': {
    notificationType: 'order.delivered',
    message: (order) =>
      `Order ${order.orderNumber || order.order_number || order.id} has been delivered.`,
    richMessage: (order) => createOrderSummary(order),
  },

  'order.cancelled': {
    notificationType: 'order.cancelled',
    message: (order) =>
      `Order ${order.orderNumber || order.order_number || order.id} has been cancelled.`,
    richMessage: (order) => createOrderSummary(order),
  },

  'inventory.low': {
    notificationType: 'inventory.low',
    message: (data) =>
      `Low stock alert: ${data.sku} has ${data.available ?? 0} units remaining (reorder point: ${data.reorderPoint ?? 0})`,
    richMessage: (data) => createInventoryCard(data.sku, data),
  },

  'inventory.out': {
    notificationType: 'inventory.out',
    message: (data) => `OUT OF STOCK: ${data.sku} — ${data.name || data.sku} has 0 units available`,
    richMessage: (data) => createInventoryCard(data.sku, { ...data, available: 0 }),
  },
};

// ============================================================================
// EventBridge
// ============================================================================

export class EventBridge {
  /**
   * @param {Object} opts
   * @param {import('../autonomous/engine.js').AutonomousEngine} [opts.engine]
   * @param {import('./notifier.js').ChannelNotifier} opts.notifier
   * @param {Object} [opts.eventMap] - Custom event mappings (merged with defaults)
   * @param {boolean} [opts.verbose=false]
   */
  constructor({ engine, notifier, eventMap, verbose = false }) {
    this._engine = engine;
    this._notifier = notifier;
    this._verbose = verbose;
    this._listeners = [];

    // Merge custom mappings over defaults
    this._eventMap = { ...DEFAULT_EVENT_MAP, ...COMMERCE_EVENT_BUILDERS, ...(eventMap || {}) };
  }

  /**
   * Start listening for engine events and forwarding them as notifications.
   */
  start() {
    if (!this._engine) return;

    for (const [eventName, mapping] of Object.entries(this._eventMap)) {
      const handler = async (...args) => {
        const data = args[0] || {};
        try {
          const notification = {
            type: mapping.notificationType,
            message:
              typeof mapping.message === 'function'
                ? mapping.message(data)
                : String(mapping.message),
            richMessage:
              typeof mapping.richMessage === 'function' ? mapping.richMessage(data) : null,
          };

          if (this._verbose) {
            console.log(
              `[EventBridge] ${eventName} → ${notification.type}: ${notification.message.slice(0, 80)}`,
            );
          }

          await this._notifier.sendNotification(notification);
        } catch (err) {
          console.error(`[EventBridge] Failed to bridge event ${eventName}:`, err.message);
        }
      };

      this._engine.on(eventName, handler);
      this._listeners.push({ event: eventName, handler });
    }

    if (this._verbose) {
      console.log(`[EventBridge] Listening for ${this._listeners.length} event types`);
    }
  }

  /**
   * Send a commerce event notification directly (not from engine).
   * Use this when you detect commerce state changes outside the engine.
   *
   * @param {string} eventType - e.g. 'order.shipped', 'inventory.low'
   * @param {Object} data - Event data
   * @returns {Promise<{ sent: number, errors: number }>}
   */
  async sendCommerceEvent(eventType, data) {
    const mapping = this._eventMap[eventType];
    if (!mapping) {
      // No mapping — send as generic notification
      return this._notifier.sendNotification({
        type: eventType,
        message: JSON.stringify(data),
      });
    }

    return this._notifier.sendNotification({
      type: mapping.notificationType,
      message:
        typeof mapping.message === 'function' ? mapping.message(data) : String(mapping.message),
      richMessage: typeof mapping.richMessage === 'function' ? mapping.richMessage(data) : null,
    });
  }

  /**
   * Stop listening for events.
   */
  stop() {
    if (!this._engine) return;

    for (const { event, handler } of this._listeners) {
      this._engine.removeListener(event, handler);
    }
    this._listeners = [];
  }
}
