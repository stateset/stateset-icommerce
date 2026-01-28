/**
 * Proactive Notification System for StateSet Channel Gateways
 *
 * Routes event-driven notifications to registered channels based on
 * configurable routing rules. Used by the autonomous engine to push
 * alerts (order shipped, low inventory, approval requests) to the
 * appropriate messaging channels.
 *
 * Singleton: use getNotifier() to access the global instance.
 */

import { richMessageToPlainText } from './rich-messages.js';

// ============================================================================
// ChannelNotifier
// ============================================================================

export class ChannelNotifier {
  constructor() {
    /** @type {Map<string, { send: Function, sendRichMessage?: Function, formatForPlatform?: Function }>} */
    this._channels = new Map();

    /** @type {Map<string, { channel: string, target: string }[]>} */
    this._routes = new Map();
  }

  /**
   * Register a channel for outbound notifications.
   *
   * @param {string} name - Channel name (e.g. 'telegram', 'slack')
   * @param {Object} adapter
   * @param {Function} adapter.send - (targetId, text) => Promise<void>
   * @param {Function} [adapter.sendRichMessage] - (targetId, richMsg) => Promise<void>
   * @param {Function} [adapter.formatForPlatform] - (text) => string
   */
  registerChannel(name, adapter) {
    this._channels.set(name, adapter);
  }

  /**
   * Unregister a channel (on shutdown).
   *
   * @param {string} name
   */
  unregisterChannel(name) {
    this._channels.delete(name);
  }

  /**
   * Add a notification route.
   *
   * @param {string} eventType - Event type (e.g. 'order.shipped') or '*' for wildcard
   * @param {string} channelName - Registered channel name
   * @param {string} targetId - Target identifier (channel ID, phone number, etc.)
   */
  addRoute(eventType, channelName, targetId) {
    const routes = this._routes.get(eventType) || [];
    // Avoid duplicates
    if (!routes.some((r) => r.channel === channelName && r.target === targetId)) {
      routes.push({ channel: channelName, target: targetId });
      this._routes.set(eventType, routes);
    }
  }

  /**
   * Bulk load routes from a config object.
   *
   * @param {Object} configObj - { [eventType]: [{ channel, target }] }
   */
  loadRoutes(configObj) {
    if (!configObj || typeof configObj !== 'object') return;

    for (const [eventType, destinations] of Object.entries(configObj)) {
      if (!Array.isArray(destinations)) continue;
      for (const dest of destinations) {
        if (dest.channel && dest.target) {
          this.addRoute(eventType, dest.channel, dest.target);
        }
      }
    }
  }

  /**
   * Send a notification to all matching routes.
   *
   * @param {Object} notification
   * @param {string} notification.type - Event type (e.g. 'order.shipped')
   * @param {string} notification.message - Plain text message
   * @param {import('./rich-messages.js').RichMessage} [notification.richMessage] - Optional rich message
   * @returns {Promise<{ sent: number, errors: number }>}
   */
  async sendNotification(notification) {
    const { type, message, richMessage } = notification;

    // Collect routes: exact match + wildcard
    const exactRoutes = this._routes.get(type) || [];
    const wildcardRoutes = this._routes.get('*') || [];

    // Deduplicate
    const seen = new Set();
    const allRoutes = [];
    for (const route of [...exactRoutes, ...wildcardRoutes]) {
      const key = `${route.channel}:${route.target}`;
      if (!seen.has(key)) {
        seen.add(key);
        allRoutes.push(route);
      }
    }

    let sent = 0;
    let errors = 0;

    for (const route of allRoutes) {
      const adapter = this._channels.get(route.channel);
      if (!adapter) {
        console.warn(`[Notifier] Channel '${route.channel}' not registered, skipping route to ${route.target}`);
        errors++;
        continue;
      }

      try {
        // Prefer rich message if adapter supports it
        if (richMessage && adapter.sendRichMessage) {
          await adapter.sendRichMessage(route.target, richMessage);
        } else {
          // Fall back to plain text
          let text = message;
          if (!text && richMessage) {
            text = richMessageToPlainText(richMessage);
          }
          if (adapter.formatForPlatform) {
            text = adapter.formatForPlatform(text);
          }
          await adapter.send(route.target, text);
        }
        sent++;
      } catch (err) {
        console.error(`[Notifier] Failed to send to ${route.channel}:${route.target}:`, err.message);
        errors++;
      }
    }

    return { sent, errors };
  }

  /**
   * Broadcast a message to all default ('*') routes.
   *
   * @param {string} message
   * @param {import('./rich-messages.js').RichMessage} [richMessage]
   * @returns {Promise<{ sent: number, errors: number }>}
   */
  async broadcast(message, richMessage) {
    return this.sendNotification({ type: '*', message, richMessage });
  }

  /**
   * Send a notification directly to a specific customer via their linked channel.
   * Uses the identity store to find the customer's preferred channel.
   *
   * @param {string} customerId - Commerce customer ID
   * @param {Object} notification - { message, richMessage? }
   * @param {import('./identity.js').CustomerIdentityStore} identityStore
   * @returns {Promise<{ sent: number, errors: number }>}
   */
  async sendToCustomer(customerId, notification, identityStore) {
    if (!identityStore) {
      console.warn('[Notifier] No identity store provided for customer notification');
      return { sent: 0, errors: 0 };
    }

    const links = identityStore.getChannelsForCustomer(customerId);
    if (links.length === 0) {
      return { sent: 0, errors: 0 };
    }

    let sent = 0;
    let errors = 0;

    for (const { channel, senderId } of links) {
      const adapter = this._channels.get(channel);
      if (!adapter) continue;

      try {
        if (notification.richMessage && adapter.sendRichMessage) {
          await adapter.sendRichMessage(senderId, notification.richMessage);
        } else {
          let text = notification.message;
          if (!text && notification.richMessage) {
            text = richMessageToPlainText(notification.richMessage);
          }
          if (adapter.formatForPlatform) {
            text = adapter.formatForPlatform(text);
          }
          await adapter.send(senderId, text);
        }
        sent++;
      } catch (err) {
        console.error(`[Notifier] Failed to send to customer ${customerId} via ${channel}:`, err.message);
        errors++;
      }
    }

    return { sent, errors };
  }

  /**
   * Get a list of registered channels.
   *
   * @returns {string[]}
   */
  getRegisteredChannels() {
    return [...this._channels.keys()];
  }

  /**
   * Get all configured routes.
   *
   * @returns {Object}
   */
  getRoutes() {
    const routes = {};
    for (const [type, dests] of this._routes) {
      routes[type] = dests.map((d) => ({ ...d }));
    }
    return routes;
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the global ChannelNotifier instance.
 *
 * @returns {ChannelNotifier}
 */
export function getNotifier() {
  if (!_instance) {
    _instance = new ChannelNotifier();
  }
  return _instance;
}
