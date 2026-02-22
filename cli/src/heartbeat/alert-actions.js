/**
 * Heartbeat Alert Actions
 *
 * Maps heartbeat check IDs to recommended automated response actions.
 * Used by reactive alert handlers to decide what to do when a check triggers.
 *
 * Each mapping describes:
 *   action      – the category of response (notify, celebrate, etc.)
 *   description – human-readable explanation of the action
 *   channels    – preferred delivery channels for the action
 */

// ============================================================================
// Alert action mapping
// ============================================================================

/**
 * Maps check IDs to their recommended alert actions.
 *
 * @type {Object<string, { action: string, description: string, channels: string[] }>}
 */
export const ALERT_ACTION_MAP = {
  'low-stock': {
    action: 'notify',
    description: 'Send low stock notification',
    channels: ['slack', 'email'],
  },
  'abandoned-carts': {
    action: 'notify',
    description: 'Send cart recovery reminder',
    channels: ['email'],
  },
  'overdue-invoices': {
    action: 'notify',
    description: 'Send payment reminder',
    channels: ['email'],
  },
  'pending-returns': {
    action: 'notify',
    description: 'Alert returns team',
    channels: ['slack'],
  },
  'subscription-churn': {
    action: 'notify',
    description: 'Send retention offer',
    channels: ['email'],
  },
  'revenue-milestone': {
    action: 'celebrate',
    description: 'Share milestone achievement',
    channels: ['slack'],
  },
};

// ============================================================================
// Action resolver
// ============================================================================

/**
 * Map a heartbeat alert to its recommended action.
 *
 * Returns `null` if no mapping exists for the given `alert.checkId`.
 *
 * @param {{ checkId: string, checkName: string, status: string, details: Object, timestamp: number }} alert
 * @returns {{ action: string, description: string, channels: string[], alert: Object, suggestedAt: number } | null}
 */
export function mapAlertToAction(alert) {
  const mapping = ALERT_ACTION_MAP[alert.checkId];
  if (!mapping) return null;
  return {
    ...mapping,
    alert,
    suggestedAt: Date.now(),
  };
}
