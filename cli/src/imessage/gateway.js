/**
 * iMessage Gateway for StateSet iCommerce
 *
 * Bridges iMessage conversations to the StateSet commerce agent via the
 * BlueBubbles HTTP API. Each iMessage sender gets their own agent session
 * for multi-turn conversations.
 *
 * Requires BlueBubbles server running locally or on the network.
 * @see https://bluebubbles.app/
 */

import { createSessionManager, createMessageHandler, BOT_PREFIX } from '../channels/base.js';
import { getNotifier } from '../channels/notifier.js';

// ============================================================================
// BlueBubbles REST helpers
// ============================================================================

const BLUEBUBBLES_AUTH_MODE_ENV = 'BLUEBUBBLES_AUTH_MODE';
const BLUEBUBBLES_AUTH_MODES = new Set(['auto', 'header', 'query']);

function normalizeBlueBubblesAuthMode(value) {
  const normalized = String(value || 'auto')
    .trim()
    .toLowerCase();
  return BLUEBUBBLES_AUTH_MODES.has(normalized) ? normalized : 'auto';
}

function applyBlueBubblesAuth(url, fetchOpts, password, authMode) {
  if (authMode === 'query') {
    url.searchParams.set('password', password);
    return;
  }
  fetchOpts.headers.Authorization = `Bearer ${password}`;
  fetchOpts.headers['X-BlueBubbles-Password'] = password;
}

/**
 * Make a request to the BlueBubbles API.
 * @param {string} baseUrl
 * @param {string} password
 * @param {string} path
 * @param {Object} [opts]
 * @returns {Promise<any>}
 */
async function bbFetch(baseUrl, password, path, opts = {}) {
  const authMode = normalizeBlueBubblesAuthMode(opts.authMode);
  const url = new URL(path, baseUrl);

  if (opts.params) {
    for (const [k, v] of Object.entries(opts.params)) {
      url.searchParams.set(k, String(v));
    }
  }

  async function attempt(mode) {
    const attemptUrl = new URL(url.toString());
    const fetchOpts = {
      method: opts.method || 'GET',
      headers: { 'Content-Type': 'application/json' },
    };
    applyBlueBubblesAuth(attemptUrl, fetchOpts, password, mode);
    if (opts.body) {
      fetchOpts.body = JSON.stringify(opts.body);
    }
    if (opts.signal) {
      fetchOpts.signal = opts.signal;
    }
    return fetch(attemptUrl.toString(), fetchOpts);
  }

  let res = await attempt(authMode === 'query' ? 'query' : 'header');
  if (authMode === 'auto' && (res.status === 401 || res.status === 403)) {
    res = await attempt('query');
  }
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(`BlueBubbles ${opts.method || 'GET'} ${path} failed: ${res.status} ${text}`);
  }
  return res.json();
}

// ============================================================================
// Channel adapter
// ============================================================================

/**
 * Create the iMessage channel adapter for the shared message handler pipeline.
 *
 * @param {string} baseUrl - BlueBubbles server URL
 * @param {string} password - BlueBubbles API password
 * @param {string} authMode - Authentication mode: auto, header, or query
 * @returns {import('../channels/base.js').ChannelAdapter}
 */
function createAdapter(baseUrl, password, authMode) {
  return {
    extractText(raw) {
      if (!raw || !raw.text) return null;
      // BlueBubbles marks outgoing messages with isFromMe
      if (raw.isFromMe) return null;
      return raw.text.trim() || null;
    },

    getSenderId(raw) {
      return raw.handle?.address || raw.handleId || 'unknown';
    },

    getTargetId(raw) {
      // For iMessage, the target is the chat GUID
      return raw.chats?.[0]?.guid || raw.chatGuid || raw.handle?.address || 'unknown';
    },

    isOwnMessage(raw) {
      return !!raw.isFromMe;
    },

    async send(chatGuid, text) {
      await bbFetch(baseUrl, password, '/api/v1/message/text', {
        authMode,
        method: 'POST',
        body: {
          chatGuid,
          message: text,
          method: 'apple-script', // Reliable for macOS
        },
      });
    },

    sendTyping: null, // iMessage doesn't support typing indicators via API

    formatForPlatform(text) {
      // iMessage supports basic text; strip any markdown code fences
      return text
        .replace(/```[\s\S]*?```/g, (match) => {
          const inner = match.replace(/```\w*\n?/, '').replace(/\n?```$/, '');
          return inner;
        })
        .replace(/`([^`]+)`/g, '$1');
    },

    maxMessageLength: 20000, // iMessage has generous limits
  };
}

// ============================================================================
// Polling loop
// ============================================================================

/**
 * Start polling BlueBubbles for new messages.
 *
 * @param {Object} opts
 * @param {string} opts.baseUrl
 * @param {string} opts.password
 * @param {string} opts.authMode
 * @param {number} opts.pollIntervalMs
 * @param {Function} opts.onMessage
 * @param {AbortSignal} opts.signal
 * @param {boolean} [opts.verbose]
 * @returns {{ lastMessageDate: number }}
 */
async function startPolling({
  baseUrl,
  password,
  authMode,
  pollIntervalMs,
  onMessage,
  signal,
  verbose,
}) {
  // Get the most recent message timestamp so we only process new messages
  let lastDate = Date.now();

  try {
    const recent = await bbFetch(baseUrl, password, '/api/v1/message', {
      authMode,
      params: { limit: 1, sort: 'DESC', with: 'chat,handle' },
    });
    if (recent.data?.[0]?.dateCreated) {
      lastDate = recent.data[0].dateCreated;
    }
  } catch (err) {
    console.error('[iMessage] Failed to get initial message timestamp:', err.message);
  }

  if (verbose) {
    console.debug(`[iMessage] Polling from timestamp ${lastDate}`);
  }

  async function poll() {
    if (signal.aborted) return;

    try {
      const result = await bbFetch(baseUrl, password, '/api/v1/message', {
        authMode,
        params: {
          after: lastDate,
          sort: 'ASC',
          limit: 50,
          with: 'chat,handle',
        },
        signal,
      });

      const messages = result.data || [];
      for (const msg of messages) {
        if (msg.isFromMe) continue;
        if (!msg.text) continue;

        // Update the high-water mark
        if (msg.dateCreated > lastDate) {
          lastDate = msg.dateCreated;
        }

        try {
          await onMessage(msg);
        } catch (err) {
          console.error('[iMessage] Handler error:', err.message);
        }
      }
    } catch (err) {
      if (signal.aborted) return;
      console.error('[iMessage] Poll error:', err.message);
    }

    if (!signal.aborted) {
      const pollTimer = setTimeout(poll, pollIntervalMs);
      if (pollTimer.unref) pollTimer.unref();
    }
  }

  // Start first poll
  poll();

  return {
    get lastMessageDate() {
      return lastDate;
    },
  };
}

// ============================================================================
// Gateway entry point
// ============================================================================

/**
 * Start the iMessage gateway.
 *
 * @param {Object} config - Channel configuration from gateway.config.json
 * @param {Object} shared - Shared gateway config (dbPath, model, etc.)
 * @returns {Promise<{ shutdown(): void }>}
 */
export async function startIMessageGateway(config, shared) {
  const baseUrl = config.blueBubblesUrl || process.env.BLUEBUBBLES_URL || 'http://localhost:1234';
  const password = config.blueBubblesPassword || process.env.BLUEBUBBLES_PASSWORD;
  const authMode = normalizeBlueBubblesAuthMode(
    config.blueBubblesAuthMode || process.env[BLUEBUBBLES_AUTH_MODE_ENV],
  );

  if (!password) {
    throw new Error(
      'iMessage gateway requires BLUEBUBBLES_PASSWORD env var or blueBubblesPassword in config',
    );
  }

  const pollIntervalMs = config.pollIntervalMs || 3000;
  const verbose = shared.verbose || false;

  console.debug(`[iMessage] Connecting to BlueBubbles at ${baseUrl}`);

  // Verify connection
  try {
    const serverInfo = await bbFetch(baseUrl, password, '/api/v1/server/info', { authMode });
    console.debug(
      `[iMessage] Connected to BlueBubbles ${serverInfo.data?.os_version || 'unknown'}`,
    );
  } catch (err) {
    throw new Error(`Failed to connect to BlueBubbles at ${baseUrl}: ${err.message}`);
  }

  // Session management
  const sessionMgr = createSessionManager({
    store: shared.sessionStore,
    channel: 'imessage',
  });
  const cleanupHandle = sessionMgr.startCleanup();

  // Create adapter and message handler
  const adapter = createAdapter(baseUrl, password, authMode);
  const handleMessage = createMessageHandler(adapter, {
    getSession: sessionMgr.getSession,
    persistSession: sessionMgr.persistSession,
    dbPath: shared.dbPath,
    allowApply: shared.allowApply ?? false,
    model: shared.model,
    maxTurns: shared.maxTurns || 10,
    agent: shared.agent || null,
    verbose,
    allowlist: config.allowlist || null,
    middleware: shared.middleware || [],
    channel: 'imessage',
    identityStore: shared.identityStore || null,
    autonomousEngine: shared.autonomousEngine || null,
    thinkLevel: shared.thinkLevel || 'off',
    provider: shared.provider || 'claude',
    enableFallback: shared.enableFallback ?? true,
  });

  // Abort controller for clean shutdown
  const abortController = new AbortController();

  // Start polling
  await startPolling({
    baseUrl,
    password,
    authMode,
    pollIntervalMs,
    onMessage: handleMessage,
    signal: abortController.signal,
    verbose,
  });

  // Register with notifier for outbound messages
  const notifier = getNotifier();
  notifier.register('imessage', async (targetId, text) => {
    await adapter.send(targetId, BOT_PREFIX + text);
  });

  console.debug(`[iMessage] Gateway running (poll every ${pollIntervalMs}ms)`);

  return {
    shutdown() {
      console.debug('[iMessage] Shutting down...');
      abortController.abort();
      notifier.unregister('imessage');
      sessionMgr.stopCleanup(cleanupHandle);
    },
  };
}
