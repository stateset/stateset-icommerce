/**
 * Conversation Handoff System for StateSet Channel Gateways
 *
 * Enables escalation from AI bot to human agent. When a customer
 * types /escalate, the bot pauses automated responses and routes
 * messages to a human operator channel. The human can then respond
 * through the bot, and /release returns to AI mode.
 *
 * Flow:
 *   1. Customer sends /escalate
 *   2. Bot marks session as "handed off", notifies ops channel
 *   3. Customer messages are forwarded to the ops channel (not to AI)
 *   4. Human replies in ops channel are forwarded back to customer
 *   5. Human sends /release to return customer to AI mode
 */

import { getNotifier } from './notifier.js';

// ============================================================================
// HandoffQueue
// ============================================================================

export class HandoffQueue {
  constructor() {
    /** @type {Map<string, HandoffEntry>} */
    this._active = new Map();

    /** @type {{ channel: string, target: string }|null} */
    this._opsRoute = null;
  }

  /**
   * Set the ops channel where handoff notifications are sent.
   *
   * @param {string} channel - Channel name (e.g. 'slack')
   * @param {string} target - Target ID (e.g. '#support')
   */
  setOpsRoute(channel, target) {
    this._opsRoute = { channel, target };
  }

  /**
   * Escalate a conversation to a human agent.
   *
   * @param {string} channel - Source channel
   * @param {string} senderId - Customer's sender ID
   * @param {string} targetId - Chat/channel to reply to
   * @param {string} [reason] - Escalation reason
   * @returns {HandoffEntry}
   */
  escalate(channel, senderId, targetId, reason) {
    const key = `${channel}:${senderId}`;
    const entry = {
      channel,
      senderId,
      targetId,
      reason: reason || 'Customer requested human agent',
      escalatedAt: Date.now(),
      messages: [],
      assignedTo: null,
    };
    this._active.set(key, entry);

    // Notify ops channel
    this._notifyOps(entry);

    return entry;
  }

  /**
   * Check if a conversation is in handoff mode.
   *
   * @param {string} channel
   * @param {string} senderId
   * @returns {boolean}
   */
  isHandedOff(channel, senderId) {
    return this._active.has(`${channel}:${senderId}`);
  }

  /**
   * Get the handoff entry for a conversation.
   *
   * @param {string} channel
   * @param {string} senderId
   * @returns {HandoffEntry|null}
   */
  getEntry(channel, senderId) {
    return this._active.get(`${channel}:${senderId}`) || null;
  }

  /**
   * Record a customer message during handoff (forwarded to ops).
   *
   * @param {string} channel
   * @param {string} senderId
   * @param {string} text
   */
  recordMessage(channel, senderId, text) {
    const entry = this.getEntry(channel, senderId);
    if (entry) {
      entry.messages.push({
        from: 'customer',
        text,
        timestamp: Date.now(),
      });
    }
  }

  /**
   * Record a human agent reply (forwarded to customer).
   *
   * @param {string} channel
   * @param {string} senderId
   * @param {string} text
   * @param {string} agentName
   */
  recordReply(channel, senderId, text, agentName) {
    const entry = this.getEntry(channel, senderId);
    if (entry) {
      entry.messages.push({
        from: agentName || 'agent',
        text,
        timestamp: Date.now(),
      });
      if (!entry.assignedTo) entry.assignedTo = agentName;
    }
  }

  /**
   * Release a conversation back to AI mode.
   *
   * @param {string} channel
   * @param {string} senderId
   * @returns {{ released: boolean, entry: HandoffEntry|null }}
   */
  release(channel, senderId) {
    const key = `${channel}:${senderId}`;
    const entry = this._active.get(key);
    if (!entry) return { released: false, entry: null };

    this._active.delete(key);
    return { released: true, entry };
  }

  /**
   * Get all active handoffs.
   *
   * @returns {HandoffEntry[]}
   */
  listActive() {
    return [...this._active.values()];
  }

  /**
   * Get conversation history for a handoff.
   *
   * @param {string} channel
   * @param {string} senderId
   * @returns {string}
   */
  getHistoryText(channel, senderId) {
    const entry = this.getEntry(channel, senderId);
    if (!entry || entry.messages.length === 0) return 'No messages recorded.';

    return entry.messages.map((m) => {
      const ts = new Date(m.timestamp).toLocaleTimeString();
      return `[${ts}] ${m.from}: ${m.text}`;
    }).join('\n');
  }

  /**
   * Export conversation history as a structured object.
   *
   * @param {string} channel
   * @param {string} senderId
   * @returns {Object|null}
   */
  exportHistory(channel, senderId) {
    const entry = this.getEntry(channel, senderId);
    if (!entry) return null;

    return {
      channel: entry.channel,
      senderId: entry.senderId,
      reason: entry.reason,
      escalatedAt: new Date(entry.escalatedAt).toISOString(),
      assignedTo: entry.assignedTo,
      messageCount: entry.messages.length,
      messages: entry.messages.map((m) => ({
        from: m.from,
        text: m.text,
        timestamp: new Date(m.timestamp).toISOString(),
      })),
    };
  }

  /**
   * @private Send notification to ops channel about new escalation.
   */
  async _notifyOps(entry) {
    if (!this._opsRoute) return;

    const notifier = getNotifier();
    const adapter = notifier._channels.get(this._opsRoute.channel);
    if (!adapter) return;

    const message = [
      `New escalation from ${entry.channel}`,
      `Sender: ${entry.senderId}`,
      `Reason: ${entry.reason}`,
      `Time: ${new Date(entry.escalatedAt).toISOString()}`,
      '',
      `Reply with: /reply ${entry.channel}:${entry.senderId} <message>`,
      `Release with: /release ${entry.channel}:${entry.senderId}`,
    ].join('\n');

    try {
      if (adapter.formatForPlatform) {
        await adapter.send(this._opsRoute.target, adapter.formatForPlatform(message));
      } else {
        await adapter.send(this._opsRoute.target, message);
      }
    } catch (err) {
      console.error('[Handoff] Failed to notify ops:', err.message);
    }
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the global HandoffQueue instance.
 * @returns {HandoffQueue}
 */
export function getHandoffQueue() {
  if (!_instance) {
    _instance = new HandoffQueue();
  }
  return _instance;
}

/**
 * @typedef {Object} HandoffEntry
 * @property {string} channel
 * @property {string} senderId
 * @property {string} targetId
 * @property {string} reason
 * @property {number} escalatedAt
 * @property {{ from: string, text: string, timestamp: number }[]} messages
 * @property {string|null} assignedTo
 */
