/**
 * A2A Agent Notification Webhook Service
 *
 * Manages outbound webhook notifications for agent-to-agent commerce events.
 * Supports webhook configuration, HMAC-SHA256 signed delivery, automatic retries
 * with exponential backoff, and delivery logging.
 *
 * @example
 * ```javascript
 * const notifications = createNotificationService(store);
 *
 * // Configure webhooks for an agent
 * await notifications.configureWebhooks({
 *   agentAddress: '0xSeller',
 *   endpointUrl: 'https://seller-bot.example.com/webhooks',
 *   secret: 'whsec_abc123',
 *   enabledEvents: ['payment.completed', 'escrow.released'],
 * });
 *
 * // Send a notification
 * const log = await notifications.sendNotification({
 *   recipientAddress: '0xSeller',
 *   eventType: 'payment.completed',
 *   payload: { paymentId: 'pay-123', amount: 50.00 },
 * });
 *
 * // Retry failed deliveries
 * const result = await notifications.retryPendingNotifications();
 * // => { retried: 3, succeeded: 2, failed: 1 }
 * ```
 */

import { randomUUID, createHmac } from 'node:crypto';

// Try to import SSRF-safe URL validator; fall back to basic protocol check
let validateFetchUrl;
try {
  const urlValidator = await import('../utils/url-validator.js');
  if (typeof urlValidator.validateFetchUrl === 'function') {
    validateFetchUrl = urlValidator.validateFetchUrl;
  }
} catch {
  // url-validator.js not available or does not export validateFetchUrl
  validateFetchUrl = null;
}

/**
 * Validate that a URL uses http:// or https:// protocol.
 * If the full SSRF validator is available, delegates to it.
 *
 * @param {string} url - The URL to validate
 * @throws {Error} If the URL is invalid or blocked
 */
function safeValidateUrl(url) {
  if (validateFetchUrl) {
    validateFetchUrl(url);
    return;
  }
  // Fallback: basic protocol check
  if (!url || (!url.startsWith('http://') && !url.startsWith('https://'))) {
    throw new Error(`Invalid webhook URL: must start with http:// or https://`);
  }
}

/**
 * Format a notification log record from snake_case to camelCase
 *
 * @param {Object} row - Raw notification log record from the store
 * @returns {Object} Formatted notification log with camelCase keys
 */
function formatNotificationLog(row) {
  if (!row) return null;

  let payload = row.payload;
  if (typeof payload === 'string') {
    try {
      payload = JSON.parse(payload);
    } catch {
      console.warn('Failed to parse notification payload for log', row.id);
    }
  }

  return {
    id: row.id,
    recipientAddress: row.recipient_address,
    endpointUrl: row.endpoint_url,
    eventType: row.event_type,
    payload,
    signature: row.signature,
    status: row.status,
    attempts: row.attempts,
    lastAttemptAt: row.last_attempt_at,
    lastError: row.last_error,
    deliveredAt: row.delivered_at,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

/**
 * Create an A2A Notification Webhook Service instance
 *
 * @param {Object} store - A2A store with notification/webhook CRUD methods
 * @param {Function} store.createNotificationLog - Persist a new notification log record
 * @param {Function} store.getNotificationLog - Retrieve notification log by ID
 * @param {Function} store.updateNotificationLog - Update notification log fields by ID
 * @param {Function} store.listNotificationLog - List notification logs with filter
 * @param {Function} store.getPendingNotifications - Get pending notifications under max attempts
 * @param {Function} store.upsertWebhookConfig - Create or update webhook configuration
 * @param {Function} store.getWebhookConfig - Get webhook config for an agent address
 * @param {Function} store.listWebhookConfigs - List webhook configs with optional filter
 * @returns {Object} Notification service API
 */
export function createNotificationService(store) {
  /**
   * Send a webhook notification to a recipient agent
   *
   * Looks up the recipient's webhook configuration, validates the endpoint,
   * computes an HMAC-SHA256 signature, delivers the payload via HTTP POST,
   * and logs the result.
   *
   * @param {Object} params - Notification parameters
   * @param {string} params.recipientAddress - Recipient agent wallet address
   * @param {string} params.eventType - Event type (e.g. 'payment.completed')
   * @param {Object} params.payload - Event payload to deliver
   * @param {string} [params.endpointUrl] - Override endpoint URL (bypasses config lookup)
   * @returns {Promise<Object>} Formatted notification log record
   */
  async function sendNotification(params) {
    const { recipientAddress, eventType, payload, endpointUrl: overrideUrl } = params;

    if (!recipientAddress) {
      throw new Error('recipientAddress is required');
    }
    if (!eventType) {
      throw new Error('eventType is required');
    }

    // Look up webhook configuration for the recipient
    const config = await store.getWebhookConfig(recipientAddress);

    // Determine the endpoint URL
    let endpointUrl = overrideUrl;

    if (!config && !overrideUrl) {
      throw new Error(`No webhook endpoint configured for ${recipientAddress}`);
    }

    if (config) {
      // Check if the webhook is active
      if (!config.active) {
        throw new Error(`Webhook configuration for ${recipientAddress} is not active`);
      }

      // Check if the event type is enabled
      const enabledEvents = config.enabled_events || ['*'];
      if (!enabledEvents.includes('*') && !enabledEvents.includes(eventType)) {
        throw new Error(
          `Event type '${eventType}' is not enabled for ${recipientAddress}. ` +
            `Enabled events: ${enabledEvents.join(', ')}`,
        );
      }
    }

    // Use override URL if provided, otherwise use config URL
    if (!endpointUrl) {
      endpointUrl = config.endpoint_url;
    }

    // Validate the endpoint URL
    safeValidateUrl(endpointUrl);

    // Build the signed payload
    const timestamp = new Date().toISOString();
    const signedPayload = {
      event_type: eventType,
      payload,
      timestamp,
    };

    // Compute HMAC-SHA256 signature
    const secret = config && config.secret ? config.secret : '';
    const signatureBody = JSON.stringify(signedPayload);
    const signature = createHmac('sha256', secret).update(signatureBody).digest('hex');

    // Prepare the notification log ID
    const logId = randomUUID();
    const now = new Date().toISOString();

    // Attempt delivery
    let status = 'pending';
    let deliveredAt = null;
    let lastError = null;
    const attempts = 1;

    try {
      const response = await fetch(endpointUrl, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-StateSet-Signature': `sha256=${signature}`,
          'X-StateSet-Timestamp': timestamp,
          'X-StateSet-Event': eventType,
        },
        body: signatureBody,
      });

      if (response.ok) {
        status = 'delivered';
        deliveredAt = now;
      } else {
        status = 'pending';
        lastError = `HTTP ${response.status}: ${response.statusText}`;
      }
    } catch (error) {
      status = 'pending';
      lastError = error.message || 'Unknown delivery error';
      console.warn(`Webhook delivery failed for ${recipientAddress}:`, lastError);
    }

    // Log the notification
    const logRecord = {
      id: logId,
      recipient_address: recipientAddress,
      endpoint_url: endpointUrl,
      event_type: eventType,
      payload: signedPayload,
      signature,
      status,
      attempts,
      last_attempt_at: now,
      last_error: lastError,
      delivered_at: deliveredAt,
      created_at: now,
      updated_at: now,
    };

    await store.createNotificationLog(logRecord);

    // Re-fetch to get the stored version
    const stored = await store.getNotificationLog(logId);

    return formatNotificationLog(stored || logRecord);
  }

  /**
   * Retry pending notification deliveries with exponential backoff
   *
   * Fetches pending notifications that have not exceeded the maximum attempt count (3),
   * skips any where the backoff period has not yet elapsed, and reattempts delivery.
   *
   * @returns {Promise<Object>} Retry summary: { retried, succeeded, failed }
   */
  async function retryPendingNotifications() {
    const maxAttempts = 3;
    const pending = await store.getPendingNotifications(maxAttempts, 50);

    let retried = 0;
    let succeeded = 0;
    let failed = 0;

    const now = Date.now();

    for (const notification of pending) {
      const currentAttempts = notification.attempts || 0;

      // Exponential backoff: skip if not enough time has passed
      if (notification.last_attempt_at) {
        const lastAttempt = new Date(notification.last_attempt_at).getTime();
        const backoffMs = Math.min(1000 * Math.pow(2, currentAttempts), 30000);
        if (lastAttempt + backoffMs > now) {
          continue;
        }
      }

      retried++;

      // Parse the stored payload
      let body;
      if (typeof notification.payload === 'string') {
        body = notification.payload;
      } else {
        body = JSON.stringify(notification.payload);
      }

      const attemptTime = new Date().toISOString();
      const newAttempts = currentAttempts + 1;

      try {
        const response = await fetch(notification.endpoint_url, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'X-StateSet-Signature': `sha256=${notification.signature || ''}`,
            'X-StateSet-Timestamp': attemptTime,
            'X-StateSet-Event': notification.event_type,
          },
          body,
        });

        if (response.ok) {
          succeeded++;
          await store.updateNotificationLog(notification.id, {
            status: 'delivered',
            attempts: newAttempts,
            last_attempt_at: attemptTime,
            delivered_at: attemptTime,
            last_error: null,
          });
        } else {
          const errorMsg = `HTTP ${response.status}: ${response.statusText}`;
          const newStatus = newAttempts >= maxAttempts ? 'failed' : 'pending';
          if (newStatus === 'failed') {
            failed++;
          }
          await store.updateNotificationLog(notification.id, {
            status: newStatus,
            attempts: newAttempts,
            last_attempt_at: attemptTime,
            last_error: errorMsg,
          });
        }
      } catch (error) {
        const errorMsg = error.message || 'Unknown delivery error';
        console.warn(`Retry delivery failed for notification ${notification.id}:`, errorMsg);

        const newStatus = newAttempts >= maxAttempts ? 'failed' : 'pending';
        if (newStatus === 'failed') {
          failed++;
        }
        await store.updateNotificationLog(notification.id, {
          status: newStatus,
          attempts: newAttempts,
          last_attempt_at: attemptTime,
          last_error: errorMsg,
        });
      }
    }

    return { retried, succeeded, failed };
  }

  /**
   * Configure webhook settings for an agent
   *
   * Validates the endpoint URL and upserts the webhook configuration in the store.
   *
   * @param {Object} params - Webhook configuration parameters
   * @param {string} params.agentAddress - Agent wallet address
   * @param {string} params.endpointUrl - Webhook endpoint URL (must be http:// or https://)
   * @param {string} [params.secret] - HMAC signing secret
   * @param {string[]} [params.enabledEvents] - Event types to receive (default: ['*'])
   * @returns {Promise<Object>} Stored webhook configuration
   */
  async function configureWebhooks(params) {
    const { agentAddress, endpointUrl, secret, enabledEvents = ['*'] } = params;

    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }
    if (!endpointUrl) {
      throw new Error('endpointUrl is required');
    }
    if (!endpointUrl.startsWith('http://') && !endpointUrl.startsWith('https://')) {
      throw new Error('endpointUrl must start with http:// or https://');
    }

    const config = {
      agent_address: agentAddress,
      endpoint_url: endpointUrl,
      secret: secret || null,
      enabled_events: enabledEvents,
      active: true,
    };

    await store.upsertWebhookConfig(config);

    const stored = await store.getWebhookConfig(agentAddress);

    return stored || config;
  }

  /**
   * Retrieve notification logs with optional filtering
   *
   * @param {Object} [filter] - Filter options
   * @param {string} [filter.recipient_address] - Filter by recipient
   * @param {string} [filter.event_type] - Filter by event type
   * @param {string} [filter.status] - Filter by status
   * @param {number} [filter.limit] - Max results
   * @param {number} [filter.offset] - Pagination offset
   * @returns {Promise<Array>} Formatted notification log records
   */
  async function getNotificationLog(filter = {}) {
    const logs = await store.listNotificationLog(filter);
    return logs.map(formatNotificationLog);
  }

  return {
    // Core notification operations
    sendNotification,
    retryPendingNotifications,

    // Webhook configuration
    configureWebhooks,

    // Query operations
    getNotificationLog,

    // Format helper (exposed for testing/reuse)
    formatNotificationLog,
  };
}

export default { createNotificationService };
