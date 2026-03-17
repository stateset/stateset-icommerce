/**
 * A2A Event Stream Service
 *
 * Real-time event delivery for agent-to-agent communication.
 * Supports persistent event subscriptions, event logging, SSE push,
 * and wildcard/prefix-based event type filtering.
 *
 * @example
 * ```javascript
 * const stream = createEventStreamService(store);
 *
 * // Subscribe an agent to payment events
 * const sub = await stream.subscribe({
 *   agentAddress: '0xAgent',
 *   eventTypes: ['a2a_payment.*', 'a2a_escrow.released'],
 * });
 *
 * // Push an event (notifies matching SSE clients)
 * await stream.pushEvent({
 *   eventType: 'a2a_payment.created',
 *   agentAddress: '0xAgent',
 *   payload: { paymentId: 'pay-123', amount: 50 },
 * });
 *
 * // Query event history
 * const history = await stream.getEventHistory({
 *   agentAddress: '0xAgent',
 *   since: '2026-01-01T00:00:00Z',
 *   limit: 50,
 * });
 *
 * // Handle SSE connection from HTTP gateway
 * stream.handleSSEConnection(req, res, '0xAgent');
 * ```
 */

import { randomUUID } from 'node:crypto';

/** Default heartbeat interval for SSE connections (30 seconds) */
const SSE_HEARTBEAT_INTERVAL_MS = 30_000;

/**
 * Check whether an event type matches a subscription filter entry.
 *
 * Matching rules:
 *   - `'*'` matches everything
 *   - Exact string equality
 *   - Wildcard prefix: `'a2a_payment.*'` matches any event type starting
 *     with `'a2a_payment.'`
 *
 * @param {string} eventType - The concrete event type to test
 * @param {string[]} filters - Array of filter patterns from the subscription
 * @returns {boolean} Whether the event type matches at least one filter
 */
function matchesEventFilter(eventType, filters) {
  if (!Array.isArray(filters) || filters.length === 0) {
    return false;
  }

  for (const filter of filters) {
    // Wildcard — matches everything
    if (filter === '*') {
      return true;
    }

    // Exact match
    if (filter === eventType) {
      return true;
    }

    // Prefix wildcard: 'a2a_payment.*' matches 'a2a_payment.created'
    if (filter.endsWith('.*')) {
      const prefix = filter.slice(0, -1); // 'a2a_payment.'
      if (eventType.startsWith(prefix)) {
        return true;
      }
    }
  }

  return false;
}

/**
 * Format a raw subscription row (snake_case) into a camelCase object.
 *
 * @param {Object} row - Raw subscription record from the store
 * @returns {Object|null} Formatted subscription or null
 */
function formatSubscription(row) {
  if (!row) return null;

  return {
    id: row.id,
    agentAddress: row.agent_address,
    eventTypes: row.event_types,
    active: row.active,
    lastEventId: row.last_event_id || null,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

/**
 * Format a raw event log row (snake_case) into a camelCase object.
 *
 * @param {Object} row - Raw event log record from the store
 * @returns {Object|null} Formatted event or null
 */
function formatEvent(row) {
  if (!row) return null;

  let payload = row.payload;
  if (typeof payload === 'string') {
    try {
      payload = JSON.parse(payload);
    } catch (err) {
      console.debug(
        '[a2a/event-stream] Payload is not valid JSON, leaving as string:',
        err.message || err,
      );
    }
  }

  return {
    id: row.id,
    eventType: row.event_type,
    agentAddress: row.agent_address,
    payload,
    createdAt: row.created_at,
  };
}

/**
 * Create an A2A Event Stream Service instance
 *
 * @param {Object} store - A2A store with event subscription and event log methods
 * @param {Function} store.createEventSubscription - Persist a new event subscription
 * @param {Function} store.getEventSubscription - Retrieve subscription by ID
 * @param {Function} store.updateEventSubscription - Update subscription fields
 * @param {Function} store.listEventSubscriptions - List subscriptions with filter
 * @param {Function} store.createEventLog - Persist an event log entry
 * @param {Function} store.getEventLog - Retrieve event log entry by ID
 * @param {Function} store.listEventLog - List event log entries with filter
 * @returns {Object} Event stream service API
 */
export function createEventStreamService(store) {
  /**
   * In-memory SSE client registry.
   * Key: agentAddress (string)
   * Value: Set of response objects (writable HTTP responses)
   * @type {Map<string, Set<import('node:http').ServerResponse>>}
   */
  const _sseClients = new Map();

  /**
   * Subscribe an agent to receive events matching the given types.
   *
   * @param {Object} params - Subscription parameters
   * @param {string} params.agentAddress - Agent wallet address (required)
   * @param {string[]} [params.eventTypes=['*']] - Event type filters
   * @returns {Promise<Object>} Created subscription result
   */
  async function subscribe({ agentAddress, eventTypes }) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    const resolvedEventTypes = eventTypes ?? ['*'];

    if (!Array.isArray(resolvedEventTypes)) {
      throw new Error('eventTypes must be an array');
    }

    const subscriptionId = randomUUID();
    const stored = await store.createEventSubscription({
      id: subscriptionId,
      agent_address: agentAddress,
      event_types: resolvedEventTypes,
      active: true,
    });

    return {
      success: true,
      subscription: formatSubscription(stored),
    };
  }

  /**
   * Unsubscribe (deactivate) an existing event subscription.
   *
   * @param {string} subscriptionId - Subscription ID
   * @returns {Promise<Object>} Deactivated subscription result
   */
  async function unsubscribe(subscriptionId) {
    const existing = await store.getEventSubscription(subscriptionId);
    if (!existing) {
      throw new Error('Subscription not found');
    }

    const updated = await store.updateEventSubscription(subscriptionId, {
      active: false,
    });

    return {
      success: true,
      subscription: formatSubscription(updated),
    };
  }

  /**
   * Push an event into the log, notify matching subscriptions,
   * and broadcast to connected SSE clients.
   *
   * @param {Object} params - Event parameters
   * @param {string} params.eventType - Event type identifier (required)
   * @param {string} params.agentAddress - Target agent address (required)
   * @param {*} [params.payload] - Event payload (any JSON-serializable value)
   * @returns {Promise<Object>} Created event log entry
   */
  async function pushEvent({ eventType, agentAddress, payload }) {
    if (!eventType) {
      throw new Error('eventType is required');
    }
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    const eventId = randomUUID();

    // Persist the event
    const eventRecord = await store.createEventLog({
      id: eventId,
      event_type: eventType,
      agent_address: agentAddress,
      payload: payload !== undefined ? payload : {},
    });

    // Find all active subscriptions for this agent
    const subscriptions = await store.listEventSubscriptions({
      agent_address: agentAddress,
      active: true,
    });

    // Update last_event_id on matching subscriptions
    for (const sub of subscriptions) {
      if (matchesEventFilter(eventType, sub.event_types)) {
        try {
          await store.updateEventSubscription(sub.id, {
            last_event_id: eventId,
          });
        } catch (err) {
          console.warn(`Failed to update last_event_id on subscription ${sub.id}:`, err.message);
        }
      }
    }

    // Notify connected SSE clients
    const clients = _sseClients.get(agentAddress);
    if (clients && clients.size > 0) {
      const ssePayload = JSON.stringify(payload !== undefined ? payload : {});
      const sseMessage = `id: ${eventId}\nevent: ${eventType}\ndata: ${ssePayload}\n\n`;

      for (const res of clients) {
        try {
          res.write(sseMessage);
        } catch (err) {
          console.warn('Failed to write to SSE client:', err.message);
        }
      }
    }

    return formatEvent(eventRecord);
  }

  /**
   * Retrieve historical events for an agent.
   *
   * @param {Object} params - Query parameters
   * @param {string} params.agentAddress - Agent wallet address (required)
   * @param {string[]} [params.eventTypes] - Filter by event types (uses first entry)
   * @param {string} [params.since] - ISO timestamp — only events created after this
   * @param {number} [params.limit] - Maximum number of events to return
   * @returns {Promise<Object[]>} Array of formatted event records
   */
  async function getEventHistory({ agentAddress, eventTypes, since, limit }) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    const rows = await store.listEventLog({
      agent_address: agentAddress,
      event_type: eventTypes?.[0],
      since,
      limit,
    });

    return rows.map(formatEvent);
  }

  /**
   * List active event subscriptions for an agent.
   *
   * @param {Object} params - Query parameters
   * @param {string} params.agentAddress - Agent wallet address
   * @returns {Promise<Object[]>} Array of formatted subscription records
   */
  async function listSubscriptions({ agentAddress }) {
    const rows = await store.listEventSubscriptions({
      agent_address: agentAddress,
      active: true,
    });

    return rows.map(formatSubscription);
  }

  /**
   * Handle an incoming SSE (Server-Sent Events) connection.
   *
   * Sets appropriate headers, registers the client for push delivery,
   * establishes a heartbeat interval, and cleans up on disconnect.
   *
   * Supports `Last-Event-ID` header for reconnection replay: if the client
   * reconnects with a `Last-Event-ID`, all events logged after that ID are
   * replayed before live streaming begins.
   *
   * @param {import('node:http').IncomingMessage} req - HTTP request
   * @param {import('node:http').ServerResponse} res - HTTP response (kept open)
   * @param {string} agentAddress - Agent address to stream events for
   * @returns {Function} Cleanup function that removes the client and clears the heartbeat
   */
  function handleSSEConnection(req, res, agentAddress) {
    // Set SSE headers
    res.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      Connection: 'keep-alive',
      'X-Accel-Buffering': 'no',
    });

    // Register client in the in-memory registry
    if (!_sseClients.has(agentAddress)) {
      _sseClients.set(agentAddress, new Set());
    }
    _sseClients.get(agentAddress).add(res);

    // Send initial connected event
    const connectedPayload = JSON.stringify({ agentAddress });
    res.write(`event: connected\ndata: ${connectedPayload}\n\n`);

    // Replay missed events if Last-Event-ID header is present
    const lastEventId = req?.headers?.['last-event-id'];
    if (lastEventId) {
      _replayEvents(res, agentAddress, lastEventId).catch((err) => {
        console.warn('[a2a/event-stream] Failed to replay events:', err.message);
      });
    }

    // Heartbeat to keep the connection alive
    const heartbeatInterval = setInterval(() => {
      try {
        const heartbeatPayload = JSON.stringify({
          timestamp: new Date().toISOString(),
        });
        res.write(`event: heartbeat\ndata: ${heartbeatPayload}\n\n`);
      } catch (err) {
        console.warn('Failed to send SSE heartbeat:', err.message);
      }
    }, SSE_HEARTBEAT_INTERVAL_MS);
    if (heartbeatInterval.unref) heartbeatInterval.unref();

    /**
     * Clean up this SSE connection: remove from registry, clear heartbeat.
     */
    function cleanup() {
      const clients = _sseClients.get(agentAddress);
      if (clients) {
        clients.delete(res);
        if (clients.size === 0) {
          _sseClients.delete(agentAddress);
        }
      }
      clearInterval(heartbeatInterval);
    }

    // Clean up when the client disconnects
    req.on('close', cleanup);

    return cleanup;
  }

  /**
   * Replay events logged after a given event ID.
   * Called on SSE reconnection when client sends Last-Event-ID header.
   *
   * @param {import('node:http').ServerResponse} res
   * @param {string} agentAddress
   * @param {string} lastEventId
   */
  async function _replayEvents(res, agentAddress, lastEventId) {
    // Get the timestamp of the last known event
    const lastEvent = await store.getEventLog(lastEventId);
    if (!lastEvent) return;

    // Fetch all events after the last known one
    const missedEvents = await store.listEventLog({
      agent_address: agentAddress,
      since: lastEvent.created_at,
      limit: 1000,
    });

    for (const evt of missedEvents) {
      // Skip the event the client already has
      if (evt.id === lastEventId) continue;

      let payload = evt.payload;
      if (typeof payload === 'string') {
        try {
          payload = JSON.parse(payload);
        } catch (_) {
          // leave as string
        }
      }

      const ssePayload = JSON.stringify(payload || {});
      try {
        res.write(`id: ${evt.id}\nevent: ${evt.event_type}\ndata: ${ssePayload}\n\n`);
      } catch (err) {
        console.warn('[a2a/event-stream] Failed to replay event:', err.message);
        break;
      }
    }
  }

  return {
    // Subscription management
    subscribe,
    unsubscribe,
    listSubscriptions,

    // Event operations
    pushEvent,
    getEventHistory,

    // SSE delivery
    handleSSEConnection,
  };
}

export default { createEventStreamService };
