/**
 * MCP Event Stream Service
 *
 * Provides real-time MCP tool execution events with:
 * - In-memory subscriptions with filter support
 * - Optional SSE push to connected HTTP clients
 * - Event history and replay
 */

import { randomUUID } from 'node:crypto';

/**
 * Default heartbeat interval for SSE connections (ms)
 */
const SSE_HEARTBEAT_INTERVAL_MS = 30_000;

/**
 * Default history size for recent events.
 */
const DEFAULT_HISTORY_LIMIT = 500;

const GLOBAL_SESSION = '__global__';

const normalizeLimit = (value, fallback = DEFAULT_HISTORY_LIMIT) => {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return fallback;
  const floor = Math.floor(parsed);
  if (floor <= 0) return 1;
  return floor;
};

const normalizeEventTypes = (value) => {
  if (!Array.isArray(value) || value.length === 0) return ['*'];
  return value
    .map((raw) => (typeof raw === 'string' ? raw.trim() : ''))
    .filter(Boolean)
    .map((entry) => (entry === '*' ? '*' : String(entry)));
};

const matchEventType = (eventType, filters) => {
  if (!Array.isArray(filters) || filters.length === 0) return false;
  for (const filter of filters) {
    if (filter === '*') return true;
    if (filter === eventType) return true;
    if (filter.endsWith('.*') && eventType.startsWith(filter.slice(0, -1))) return true;
  }
  return false;
};

const isIsoDate = (value) => {
  if (typeof value !== 'string') return false;
  const d = new Date(value);
  return !Number.isNaN(d.getTime());
};

const cloneValue = (value) => {
  if (value === null || value === undefined) return value;
  if (typeof value !== 'object') return value;
  try {
    return JSON.parse(JSON.stringify(value));
  } catch {
    return value;
  }
};

/**
 * Create an MCP event stream service.
 *
 * Events schema (minimum):
 * {
 *   id, type, status, tool, requestId, sessionId, timestamp, payload
 * }
 *
 * @param {Object} options
 * @param {number} [options.historyLimit]
 * @returns {Object}
 */
export function createMcpEventStreamer(options = {}) {
  const historyLimit = normalizeLimit(options.historyLimit, DEFAULT_HISTORY_LIMIT);
  const streamName = options.streamName || 'stateset-mcp';

  /**
   * In-memory event history
   * @type {Array<Object>}
   */
  const _eventHistory = [];

  /**
   * In-memory SSE clients map.
   * sessionId => Set<ServerResponse>
   * @type {Map<string, Set<import('node:http').ServerResponse>>}
   */
  const _sseClients = new Map();

  /**
   * Subscriptions map.
   * subscriptionId => { id, sessionId, eventTypes, active, createdAt, lastEventId }
   * @type {Map<string, Object>}
   */
  const _subscriptions = new Map();

  /**
   * Event callbacks.
   * @type {Set<Function>}
   */
  const _listeners = new Set();

  const safeEventType = (status) => {
    if (!status) return 'event';
    if (typeof status === 'string') return status.trim() || 'event';
    return String(status);
  };

  const normalizeSession = (sessionId) => {
    if (typeof sessionId !== 'string' || !sessionId.trim()) return GLOBAL_SESSION;
    return sessionId.trim();
  };

  const normalizeEvent = (status, payload = {}) => {
    const event = {
      id: randomUUID(),
      stream: streamName,
      type: safeEventType(status),
      status,
      timestamp: isIsoDate(payload.timestamp) ? payload.timestamp : new Date().toISOString(),
      requestId: payload.requestId || null,
      sessionId: normalizeSession(payload.sessionId),
      tool: payload.tool || null,
      source: payload.source || 'mcp_server',
      result: cloneValue(payload.result),
      error: payload.error || null,
      details: cloneValue(payload.details || null),
      params: cloneValue(payload.params || null),
      policy: cloneValue(payload.policy || null),
      permission: cloneValue(payload.permission || null),
      charge: cloneValue(payload.charge || null),
      elapsedMs: payload.elapsedMs ?? null,
      raw: cloneValue(payload.raw || null),
    };

    return event;
  };

  const getHistorySnapshot = (events, since) => {
    if (!since) return events;
    const sinceTs = new Date(since).getTime();
    if (!Number.isFinite(sinceTs)) return events;
    return events.filter((event) => new Date(event.timestamp).getTime() > sinceTs);
  };

  const notifyListeners = (event) => {
    for (const listener of _listeners) {
      try {
        listener(event);
      } catch (error) {
        // Listener failures should not affect producer path.
        console.warn('[MCP Event Streamer] listener callback failed:', error.message);
      }
    }
  };

  const sendToClients = (sessionId, event) => {
    const eventPayload = JSON.stringify(event);
    const sseEvent = `event: ${event.type}\ndata: ${eventPayload}\n\n`;

    const clientSet = _sseClients.get(sessionId) || new Set();
    for (const client of clientSet) {
      try {
        client.write(sseEvent);
      } catch (error) {
        console.warn('[MCP Event Streamer] failed to write SSE event:', error.message);
      }
    }
  };

  return {
    /**
     * Publish a new MCP event.
     * @param {Object} rawEvent
     * @returns {Object} published event
     */
    publish(rawEvent = {}) {
      const normalized = normalizeEvent(rawEvent.status || rawEvent.type || 'event', rawEvent);
      _eventHistory.push(normalized);
      while (_eventHistory.length > historyLimit) _eventHistory.shift();

      notifyListeners(normalized);
      const notifiedSessions = new Set();
      let hasActiveGlobalSubscription = false;

      for (const subscription of _subscriptions.values()) {
        if (!subscription.active) continue;
        if (subscription.sessionId === GLOBAL_SESSION) {
          hasActiveGlobalSubscription = true;
        }
        if (
          subscription.sessionId !== normalized.sessionId &&
          subscription.sessionId !== GLOBAL_SESSION
        ) {
          continue;
        }
        if (!matchEventType(normalized.type, subscription.eventTypes)) {
          continue;
        }

        subscription.lastEventId = normalized.id;
        if (notifiedSessions.has(subscription.sessionId)) {
          continue;
        }

        notifiedSessions.add(subscription.sessionId);
        sendToClients(subscription.sessionId, normalized);
      }

      // Send to global clients even without explicit subscription so dashboard users can monitor all.
      if (
        !hasActiveGlobalSubscription &&
        !notifiedSessions.has(GLOBAL_SESSION) &&
        (_sseClients.get(GLOBAL_SESSION) || new Set()).size > 0
      ) {
        sendToClients(GLOBAL_SESSION, normalized);
      }

      return normalized;
    },

    /**
     * Subscribe to events for a specific session or global stream.
     *
     * @param {Object} params
     * @param {string} [params.sessionId]
     * @param {string[]} [params.eventTypes=['*']]
     * @returns {Promise<{ success: boolean, subscription: Object }>}
     */
    async subscribe({ sessionId, eventTypes }) {
      const subscriptionId = randomUUID();
      const normalizedSession = normalizeSession(sessionId);
      const resolvedTypes = normalizeEventTypes(eventTypes);

      const subscription = {
        id: subscriptionId,
        sessionId: normalizedSession,
        eventTypes: resolvedTypes,
        active: true,
        createdAt: new Date().toISOString(),
        lastEventId: null,
      };

      _subscriptions.set(subscriptionId, subscription);
      return {
        success: true,
        subscription: { ...subscription },
      };
    },

    /**
     * Remove a subscription.
     *
     * @param {string} subscriptionId
     * @returns {Promise<{ success: boolean, subscription: Object | null }>}
     */
    async unsubscribe(subscriptionId) {
      const existing = _subscriptions.get(subscriptionId);
      if (!existing) {
        return {
          success: false,
          subscription: null,
          error: 'Subscription not found',
        };
      }

      existing.active = false;
      _subscriptions.delete(subscriptionId);
      return {
        success: true,
        subscription: { ...existing },
      };
    },

    /**
     * List subscriptions.
     * @param {Object} [params]
     * @param {string} [params.sessionId]
     * @returns {Promise<Object[]>}
     */
    async listSubscriptions({ sessionId } = {}) {
      const hasSessionFilter = typeof sessionId === 'string' && sessionId.trim() !== '';
      const target = hasSessionFilter ? normalizeSession(sessionId) : GLOBAL_SESSION;
      const list = [];
      for (const sub of _subscriptions.values()) {
        if (hasSessionFilter) {
          if (sub.sessionId !== target) {
            continue;
          }
        } else if (sub.sessionId !== GLOBAL_SESSION) {
          continue;
        }

        list.push({ ...sub });
      }
      return list;
    },

    /**
     * Fetch event history.
     * @param {Object} [params]
     * @param {string} [params.sessionId]
     * @param {string[]} [params.eventTypes]
     * @param {string} [params.since]
     * @param {number} [params.limit]
     */
    async getEventHistory({ sessionId, eventTypes, since, limit } = {}) {
      const targetSession = normalizeSession(sessionId);
      const types = normalizeEventTypes(eventTypes);
      const filtered = _eventHistory.filter((event) => {
        if (targetSession !== GLOBAL_SESSION && event.sessionId !== targetSession) {
          return false;
        }
        return matchEventType(event.type, types);
      });

      const limited = limit
        ? Math.max(1, Math.min(historyLimit, Number(limit) || historyLimit))
        : historyLimit;
      const sorted = getHistorySnapshot(filtered, since).slice(-limited);
      return sorted.map((event) => ({ ...event }));
    },

    /**
     * Register a callback for all events.
     * @param {Function} callback
     */
    onEvent(callback) {
      if (typeof callback !== 'function') {
        throw new Error('callback must be a function');
      }
      _listeners.add(callback);
      return () => _listeners.delete(callback);
    },

    /**
     * Register a client SSE connection.
     *
     * @param {import('node:http').IncomingMessage} req
     * @param {import('node:http').ServerResponse} res
     * @param {string} [sessionId]
     */
    handleSSEConnection(req, res, sessionId) {
      const normalizedSession = normalizeSession(sessionId);
      res.writeHead(200, {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        Connection: 'keep-alive',
        'X-Accel-Buffering': 'no',
      });

      if (!_sseClients.has(normalizedSession)) {
        _sseClients.set(normalizedSession, new Set());
      }
      const clients = _sseClients.get(normalizedSession);
      clients.add(res);

      const connectedPayload = JSON.stringify({
        sessionId: normalizedSession,
        stream: streamName,
        timestamp: new Date().toISOString(),
      });
      res.write(`event: connected\ndata: ${connectedPayload}\n\n`);

      const heartbeatInterval = setInterval(() => {
        try {
          const heartbeat = JSON.stringify({ timestamp: new Date().toISOString() });
          res.write(`event: heartbeat\ndata: ${heartbeat}\n\n`);
        } catch (_error) {
          clearInterval(heartbeatInterval);
        }
      }, SSE_HEARTBEAT_INTERVAL_MS);

      const cleanup = () => {
        const active = _sseClients.get(normalizedSession);
        if (active) {
          active.delete(res);
          if (active.size === 0) {
            _sseClients.delete(normalizedSession);
          }
        }
        clearInterval(heartbeatInterval);
      };

      req.on('close', cleanup);
      req.on('error', cleanup);

      return cleanup;
    },

    /**
     * Clear in-memory state (for tests and diagnostics).
     */
    clear() {
      _eventHistory.length = 0;
      _subscriptions.clear();
      _listeners.clear();
      for (const clients of _sseClients.values()) {
        clients.clear();
      }
      _sseClients.clear();
    },
  };
}

export default { createMcpEventStreamer };
