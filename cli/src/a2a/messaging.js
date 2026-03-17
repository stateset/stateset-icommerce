/**
 * A2A Agent-to-Agent Messaging Service
 *
 * Provides direct messaging, task delegation, and status queries between
 * agents on the A2A commerce platform. Messages are stored in-memory using
 * an inbox/outbox pattern for pull-based consumption.
 *
 * Message Types:
 *   - text: Free-form text messages between agents
 *   - task_delegation: Delegate a task to another agent with deadline/reward/priority
 *   - status_query: Ask another agent about their current state
 *   - status_response: Reply to a status query
 *   - data_request: Request structured data from another agent
 *   - data_response: Reply to a data request
 *
 * @example
 * ```javascript
 * const messaging = createMessagingService();
 *
 * // Send a direct message
 * const msg = messaging.sendMessage({
 *   from: '0xAlice',
 *   to: '0xBob',
 *   type: 'text',
 *   payload: { body: 'Hello from Alice' },
 * });
 *
 * // Delegate a task
 * const task = messaging.delegateTask({
 *   from: '0xAlice',
 *   to: '0xBob',
 *   description: 'Fulfill order #42',
 *   deadline: new Date(Date.now() + 3600_000).toISOString(),
 *   reward: 25.00,
 *   priority: 'high',
 * });
 *
 * // Query agent status
 * const query = messaging.queryStatus({
 *   from: '0xAlice',
 *   to: '0xBob',
 *   queryType: 'order_status',
 *   context: { orderId: 'ORD-42' },
 * });
 *
 * // Read inbox
 * const inbox = messaging.getInbox('0xBob', { unreadOnly: true });
 *
 * // Respond to a task
 * messaging.respondToTask(task.id, { status: 'accepted' });
 *
 * // Retrieve a thread
 * const thread = messaging.getThread(msg.id);
 * ```
 */

import { randomUUID } from 'node:crypto';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Default time-to-live for messages (24 hours). */
const DEFAULT_TTL_MS = 24 * 60 * 60 * 1_000;

/** Valid message types. */
const VALID_MESSAGE_TYPES = new Set([
  'text',
  'task_delegation',
  'status_query',
  'status_response',
  'data_request',
  'data_response',
]);

/** Valid task priorities in ascending order. */
const PRIORITY_ORDER = /** @type {const} */ (['low', 'medium', 'high', 'critical']);

/** Map priority strings to numeric weight for sorting (higher = more urgent). */
const PRIORITY_WEIGHT = Object.freeze(Object.fromEntries(PRIORITY_ORDER.map((p, i) => [p, i])));

/** Valid task response statuses. */
const VALID_TASK_RESPONSE_STATUSES = new Set(['accepted', 'rejected', 'completed']);

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/**
 * Assert a value is a non-empty string.
 *
 * @param {unknown} value
 * @param {string} fieldName
 * @returns {string}
 */
function requireString(value, fieldName) {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${fieldName} must be a non-empty string`);
  }
  return value;
}

/**
 * Validate and normalise a message type.
 *
 * @param {string} type
 * @returns {string}
 */
function validateMessageType(type) {
  const t = requireString(type, 'type');
  if (!VALID_MESSAGE_TYPES.has(t)) {
    throw new Error(
      `Invalid message type "${t}". Valid types: ${[...VALID_MESSAGE_TYPES].join(', ')}`,
    );
  }
  return t;
}

/**
 * Validate a priority value.
 *
 * @param {string} priority
 * @returns {string}
 */
function validatePriority(priority) {
  const p = requireString(priority, 'priority');
  if (!(p in PRIORITY_WEIGHT)) {
    throw new Error(`Invalid priority "${p}". Valid priorities: ${PRIORITY_ORDER.join(', ')}`);
  }
  return p;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create a new in-memory A2A messaging service.
 *
 * @returns {{
 *   sendMessage:    (params: {from: string, to: string, type: string, payload: object, ttlMs?: number, parentMessageId?: string, priority?: string}) => object,
 *   getInbox:       (agentAddress: string, opts?: {unreadOnly?: boolean, type?: string, limit?: number, offset?: number}) => object[],
 *   getOutbox:      (agentAddress: string, opts?: {type?: string, limit?: number, offset?: number}) => object[],
 *   getMessage:     (messageId: string) => object|null,
 *   markRead:       (messageId: string) => object,
 *   delegateTask:   (params: {from: string, to: string, description: string, deadline: string, reward: number, priority: string}) => object,
 *   respondToTask:  (messageId: string, response: {status: string, result?: *}) => object,
 *   queryStatus:    (params: {from: string, to: string, queryType: string, context?: object}) => object,
 *   getThread:      (parentMessageId: string) => object[],
 *   getMetrics:     () => {totalMessages: number, unreadCount: number, avgResponseTimeMs: number},
 *   purgeExpired:   () => {purged: number},
 * }}
 */
export function createMessagingService() {
  // ---- Storage -----------------------------------------------------------
  /** @type {Map<string, object>} messageId -> message */
  const messages = new Map();

  /** @type {Map<string, string[]>} agentAddress -> [messageId] (received) */
  const inboxes = new Map();

  /** @type {Map<string, string[]>} agentAddress -> [messageId] (sent) */
  const outboxes = new Map();

  /** Monotonic insertion counter — ensures stable ordering when timestamps tie. */
  let sequenceCounter = 0;

  // ---- Internal helpers --------------------------------------------------

  /**
   * Ensure an array exists for `key` in `map`, return it.
   *
   * @param {Map<string, string[]>} map
   * @param {string} key
   * @returns {string[]}
   */
  function ensureList(map, key) {
    let list = map.get(key);
    if (!list) {
      list = [];
      map.set(key, list);
    }
    return list;
  }

  /**
   * Return the numeric priority weight for a message.
   * Messages without an explicit priority default to 'low'.
   *
   * @param {object} msg
   * @returns {number}
   */
  function priorityOf(msg) {
    return PRIORITY_WEIGHT[msg.priority] ?? PRIORITY_WEIGHT.low;
  }

  /**
   * Compare two messages: higher priority first, then newer timestamp first.
   * When timestamps are identical (same millisecond), use the monotonic
   * sequence counter as a stable tie-breaker (higher _seq = newer).
   *
   * @param {object} a
   * @param {object} b
   * @returns {number}
   */
  function compareMessages(a, b) {
    const pw = priorityOf(b) - priorityOf(a);
    if (pw !== 0) return pw;
    // Newer first within the same priority
    const timeDiff = new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
    if (timeDiff !== 0) return timeDiff;
    // Tie-breaker: higher sequence = newer
    return (b._seq ?? 0) - (a._seq ?? 0);
  }

  /**
   * Check whether a message has expired.
   *
   * @param {object} msg
   * @returns {boolean}
   */
  function isExpired(msg) {
    return Date.now() >= new Date(msg.expiresAt).getTime();
  }

  /**
   * Resolve message objects from a list of IDs, filtering out expired ones.
   *
   * @param {string[]} ids
   * @param {{unreadOnly?: boolean, type?: string, limit?: number, offset?: number}} opts
   * @returns {object[]}
   */
  function resolveMessages(ids, opts = {}) {
    const { unreadOnly = false, type, limit, offset = 0 } = opts;

    let result = [];
    for (const id of ids) {
      const msg = messages.get(id);
      if (!msg) continue;
      if (isExpired(msg)) continue;
      if (unreadOnly && msg.read) continue;
      if (type && msg.type !== type) continue;
      result.push(msg);
    }

    // Sort: priority descending, then timestamp descending
    result.sort(compareMessages);

    // Pagination
    if (offset > 0) {
      result = result.slice(offset);
    }
    if (typeof limit === 'number' && limit > 0) {
      result = result.slice(0, limit);
    }

    return result;
  }

  // ---- Public API --------------------------------------------------------

  /**
   * Send a message from one agent to another.
   *
   * The message is placed in the sender's outbox and the receiver's inbox.
   *
   * @param {object} params
   * @param {string} params.from        - Sender agent address
   * @param {string} params.to          - Receiver agent address
   * @param {string} params.type        - One of VALID_MESSAGE_TYPES
   * @param {object} params.payload     - Arbitrary message payload
   * @param {number} [params.ttlMs]     - Time-to-live in ms (default 24h)
   * @param {string} [params.parentMessageId] - Parent message ID for threading
   * @param {string} [params.priority]  - low | medium | high | critical (default low)
   * @returns {object} The created message
   */
  function sendMessage(params) {
    const {
      from,
      to,
      type,
      payload,
      ttlMs = DEFAULT_TTL_MS,
      parentMessageId = null,
      priority = 'low',
    } = params;

    requireString(from, 'from');
    requireString(to, 'to');
    validateMessageType(type);
    validatePriority(priority);

    if (payload === null || payload === undefined || typeof payload !== 'object') {
      throw new Error('payload must be a non-null object');
    }
    if (typeof ttlMs !== 'number' || ttlMs <= 0) {
      throw new Error('ttlMs must be a positive number');
    }
    if (parentMessageId !== null && parentMessageId !== undefined) {
      requireString(parentMessageId, 'parentMessageId');
    }

    const now = new Date();
    const message = {
      id: randomUUID(),
      from,
      to,
      type,
      payload,
      priority,
      parentMessageId: parentMessageId || null,
      read: false,
      taskStatus: null,
      taskResponse: null,
      createdAt: now.toISOString(),
      expiresAt: new Date(now.getTime() + ttlMs).toISOString(),
      _seq: sequenceCounter++,
    };

    messages.set(message.id, message);
    ensureList(outboxes, from).push(message.id);
    ensureList(inboxes, to).push(message.id);

    return message;
  }

  /**
   * Retrieve messages in an agent's inbox.
   *
   * Results are sorted by priority (critical > high > medium > low),
   * then by timestamp (newest first).
   *
   * @param {string} agentAddress
   * @param {object} [opts]
   * @param {boolean} [opts.unreadOnly]
   * @param {string}  [opts.type]
   * @param {number}  [opts.limit]
   * @param {number}  [opts.offset]
   * @returns {object[]}
   */
  function getInbox(agentAddress, opts = {}) {
    requireString(agentAddress, 'agentAddress');
    const ids = inboxes.get(agentAddress) || [];
    return resolveMessages(ids, opts);
  }

  /**
   * Retrieve messages in an agent's outbox (sent messages).
   *
   * @param {string} agentAddress
   * @param {object} [opts]
   * @param {string}  [opts.type]
   * @param {number}  [opts.limit]
   * @param {number}  [opts.offset]
   * @returns {object[]}
   */
  function getOutbox(agentAddress, opts = {}) {
    requireString(agentAddress, 'agentAddress');
    const ids = outboxes.get(agentAddress) || [];
    return resolveMessages(ids, opts);
  }

  /**
   * Retrieve a single message by ID.
   *
   * @param {string} messageId
   * @returns {object|null}
   */
  function getMessage(messageId) {
    requireString(messageId, 'messageId');
    return messages.get(messageId) || null;
  }

  /**
   * Mark a message as read.
   *
   * @param {string} messageId
   * @returns {object} The updated message
   */
  function markRead(messageId) {
    requireString(messageId, 'messageId');
    const msg = messages.get(messageId);
    if (!msg) {
      throw new Error(`Message not found: ${messageId}`);
    }
    msg.read = true;
    return msg;
  }

  /**
   * Delegate a task to another agent.
   *
   * Creates a `task_delegation` message with structured payload containing
   * the task description, deadline, reward, and priority.
   *
   * @param {object} params
   * @param {string} params.from        - Delegating agent address
   * @param {string} params.to          - Target agent address
   * @param {string} params.description - Human-readable task description
   * @param {string} params.deadline    - ISO 8601 deadline string
   * @param {number} params.reward      - Payment offered for completion
   * @param {string} params.priority    - low | medium | high | critical
   * @returns {object} The created task_delegation message
   */
  function delegateTask(params) {
    const { from, to, description, deadline, reward, priority } = params;

    requireString(from, 'from');
    requireString(to, 'to');
    requireString(description, 'description');
    requireString(deadline, 'deadline');
    validatePriority(priority);

    if (typeof reward !== 'number' || reward < 0) {
      throw new Error('reward must be a non-negative number');
    }

    return sendMessage({
      from,
      to,
      type: 'task_delegation',
      payload: {
        description,
        deadline,
        reward,
        priority,
      },
      priority,
    });
  }

  /**
   * Respond to a task delegation message.
   *
   * Updates the original task message's `taskStatus` and `taskResponse`
   * fields, and sends a response message back to the delegator.
   *
   * @param {string} messageId  - The task_delegation message ID
   * @param {object} response
   * @param {string} response.status - accepted | rejected | completed
   * @param {*}      [response.result] - Optional result payload for completed tasks
   * @returns {object} The response message sent back to the delegator
   */
  function respondToTask(messageId, response) {
    requireString(messageId, 'messageId');

    const taskMsg = messages.get(messageId);
    if (!taskMsg) {
      throw new Error(`Task message not found: ${messageId}`);
    }
    if (taskMsg.type !== 'task_delegation') {
      throw new Error(`Message ${messageId} is not a task_delegation`);
    }

    const { status, result = null } = response;
    requireString(status, 'response.status');
    if (!VALID_TASK_RESPONSE_STATUSES.has(status)) {
      throw new Error(
        `Invalid task response status "${status}". Valid: ${[...VALID_TASK_RESPONSE_STATUSES].join(', ')}`,
      );
    }

    // Update the original task message
    taskMsg.taskStatus = status;
    taskMsg.taskResponse = { status, result, respondedAt: new Date().toISOString() };

    // Send a response message back to the delegator
    return sendMessage({
      from: taskMsg.to,
      to: taskMsg.from,
      type: 'status_response',
      payload: {
        taskMessageId: messageId,
        status,
        result,
      },
      parentMessageId: messageId,
      priority: taskMsg.priority,
    });
  }

  /**
   * Query another agent's status.
   *
   * Creates a `status_query` message addressed to the target agent.
   *
   * @param {object} params
   * @param {string} params.from       - Querying agent address
   * @param {string} params.to         - Target agent address
   * @param {string} params.queryType  - Type of query (e.g. 'order_status')
   * @param {object} [params.context]  - Additional query context
   * @returns {object} The created status_query message
   */
  function queryStatus(params) {
    const { from, to, queryType, context = {} } = params;

    requireString(from, 'from');
    requireString(to, 'to');
    requireString(queryType, 'queryType');

    return sendMessage({
      from,
      to,
      type: 'status_query',
      payload: {
        queryType,
        context,
      },
    });
  }

  /**
   * Retrieve all messages in a thread.
   *
   * Returns the parent message plus all messages whose `parentMessageId`
   * matches, sorted by creation time (oldest first).
   *
   * @param {string} parentMessageId
   * @returns {object[]}
   */
  function getThread(parentMessageId) {
    requireString(parentMessageId, 'parentMessageId');

    const thread = [];
    for (const msg of messages.values()) {
      if (msg.id === parentMessageId || msg.parentMessageId === parentMessageId) {
        if (!isExpired(msg)) {
          thread.push(msg);
        }
      }
    }

    // Sort oldest first within a thread
    thread.sort((a, b) => new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime());

    return thread;
  }

  /**
   * Compute aggregate metrics across all messages.
   *
   * @returns {{ totalMessages: number, unreadCount: number, avgResponseTimeMs: number }}
   */
  function getMetrics() {
    let totalMessages = 0;
    let unreadCount = 0;
    let totalResponseTimeMs = 0;
    let responseCount = 0;

    for (const msg of messages.values()) {
      if (isExpired(msg)) continue;
      totalMessages += 1;
      if (!msg.read) {
        unreadCount += 1;
      }

      // Calculate response time for threaded responses
      if (msg.parentMessageId) {
        const parent = messages.get(msg.parentMessageId);
        if (parent) {
          const parentTime = new Date(parent.createdAt).getTime();
          const replyTime = new Date(msg.createdAt).getTime();
          const delta = replyTime - parentTime;
          if (delta >= 0) {
            totalResponseTimeMs += delta;
            responseCount += 1;
          }
        }
      }
    }

    return {
      totalMessages,
      unreadCount,
      avgResponseTimeMs: responseCount > 0 ? Math.round(totalResponseTimeMs / responseCount) : 0,
    };
  }

  /**
   * Purge all expired messages from storage.
   *
   * Removes expired messages from the main map and from all inbox/outbox
   * arrays to prevent unbounded memory growth.
   *
   * @returns {{ purged: number }}
   */
  function purgeExpired() {
    const expiredIds = new Set();

    for (const [id, msg] of messages) {
      if (isExpired(msg)) {
        expiredIds.add(id);
        messages.delete(id);
      }
    }

    if (expiredIds.size === 0) {
      return { purged: 0 };
    }

    // Clean inbox/outbox arrays
    for (const [, ids] of inboxes) {
      for (let i = ids.length - 1; i >= 0; i--) {
        if (expiredIds.has(ids[i])) {
          ids.splice(i, 1);
        }
      }
    }
    for (const [, ids] of outboxes) {
      for (let i = ids.length - 1; i >= 0; i--) {
        if (expiredIds.has(ids[i])) {
          ids.splice(i, 1);
        }
      }
    }

    return { purged: expiredIds.size };
  }

  // ---- Return public interface -------------------------------------------

  return {
    sendMessage,
    getInbox,
    getOutbox,
    getMessage,
    markRead,
    delegateTask,
    respondToTask,
    queryStatus,
    getThread,
    getMetrics,
    purgeExpired,
  };
}
