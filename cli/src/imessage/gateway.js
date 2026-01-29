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

import {
  createSessionManager,
  createMessageHandler,
  BOT_PREFIX,
} from '../channels/base.js';
import { getNotifier } from '../channels/notifier.js';
import { richMessageToPlainText } from '../channels/rich-messages.js';

// ============================================================================
// BlueBubbles REST helpers
// ============================================================================

/**
 * Make a request to the BlueBubbles API.
 * @param {string} baseUrl
 * @param {string} password
 * @param {string} path
 * @param {Object} [opts]
 * @returns {Promise<any>}
 */
async function bbFetch(baseUrl, password, path, opts = {}) {
  const url = new URL(path, baseUrl);
  url.searchParams.set('password', password);

  if (opts.params) {
    for (const [k, v] of Object.entries(opts.params)) {
      url.searchParams.set(k, String(v));
    }
  }

  const fetchOpts = {
    method: opts.method || 'GET',
    headers: { 'Content-Type': 'application/json' },
  };

  if (opts.body) {
    fetchOpts.body = JSON.stringify(opts.body);
  }

  if (opts.signal) {
    fetchOpts.signal = opts.signal;
  }

  const res = await fetch(url.toString(), fetchOpts);
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
 * @returns {import('../channels/base.js').ChannelAdapter}
 */
function createAdapter(baseUrl, password) {
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
 * @param {number} opts.pollIntervalMs
 * @param {Function} opts.onMessage
 * @param {AbortSignal} opts.signal
 * @param {boolean} [opts.verbose]
 * @returns {{ lastMessageDate: number }}
 */
async function startPolling({ baseUrl, password, pollIntervalMs, onMessage, signal, verbose }) {
  // Get the most recent message timestamp so we only process new messages
  let lastDate = Date.now();

  try {
    const recent = await bbFetch(baseUrl, password, '/api/v1/message', {
      params: { limit: 1, sort: 'DESC', with: 'chat,handle' },
    });
    if (recent.data?.[0]?.dateCreated) {
      lastDate = recent.data[0].dateCreated;
    }
  } catch (err) {
    console.error('[iMessage] Failed to get initial message timestamp:', err.message);
  }

  if (verbose) {
    console.log(`[iMessage] Polling from timestamp ${lastDate}`);
  }

  async function poll() {
    if (signal.aborted) return;

    try {
      const result = await bbFetch(baseUrl, password, '/api/v1/message', {
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
      setTimeout(poll, pollIntervalMs);
    }
  }

  // Start first poll
  poll();

  return { get lastMessageDate() { return lastDate; } };
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

  if (!password) {
    throw new Error(
      'iMessage gateway requires BLUEBUBBLES_PASSWORD env var or blueBubblesPassword in config'
    );
  }

  const pollIntervalMs = config.pollIntervalMs || 3000;
  const verbose = shared.verbose || false;

  console.log(`[iMessage] Connecting to BlueBubbles at ${baseUrl}`);

  // Verify connection
  try {
    const serverInfo = await bbFetch(baseUrl, password, '/api/v1/server/info');
    console.log(`[iMessage] Connected to BlueBubbles ${serverInfo.data?.os_version || 'unknown'}`);
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
  const adapter = createAdapter(baseUrl, password);
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
  const poller = await startPolling({
    baseUrl,
    password,
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

  console.log(`[iMessage] Gateway running (poll every ${pollIntervalMs}ms)`);

  return {
    shutdown() {
      console.log('[iMessage] Shutting down...');
      abortController.abort();
      notifier.unregister('imessage');
      sessionMgr.stopCleanup(cleanupHandle);
    },
  };
}
