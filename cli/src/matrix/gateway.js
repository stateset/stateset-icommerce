/**
 * Matrix Gateway for StateSet iCommerce
 *
 * Bridges Matrix messages to the StateSet commerce agent via the
 * Matrix Client-Server API v3. Uses long-polling /sync — no SDK dependency,
 * only the built-in `fetch` API.
 *
 * Environment variables:
 *   MATRIX_HOMESERVER_URL  - e.g. https://matrix.example.com
 *   MATRIX_ACCESS_TOKEN    - Bot account access token
 */

import { createSessionManager, createMessageHandler } from '../channels/base.js';
import { getNotifier } from '../channels/notifier.js';

// ============================================================================
// Helpers
// ============================================================================

/**
 * Make an authenticated request to the Matrix homeserver.
 *
 * @param {string} homeserver - Base URL (no trailing slash)
 * @param {string} accessToken
 * @param {string} method - HTTP method
 * @param {string} path - API path (e.g. /_matrix/client/v3/sync)
 * @param {Object} [body] - JSON body (for PUT/POST)
 * @param {URLSearchParams} [query] - Query parameters
 * @param {AbortSignal} [signal] - AbortSignal for cancellation
 * @returns {Promise<Object>}
 */
async function matrixFetch(homeserver, accessToken, method, path, { body, query, signal } = {}) {
  let url = `${homeserver}${path}`;
  if (query) {
    url += `?${query.toString()}`;
  }

  const headers = {
    Authorization: `Bearer ${accessToken}`,
  };

  const opts = { method, headers, signal };

  if (body !== undefined) {
    headers['Content-Type'] = 'application/json';
    opts.body = JSON.stringify(body);
  }

  const res = await fetch(url, opts);

  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(`Matrix API ${method} ${path} failed (${res.status}): ${text}`);
  }

  return res.json();
}

// ============================================================================
// startMatrixGateway
// ============================================================================

/**
 * Start the Matrix gateway.
 *
 * @param {Object} config
 * @param {string}   [config.dbPath='./store.db']
 * @param {boolean}  [config.allowApply=true]
 * @param {string}   [config.model]
 * @param {number}   [config.maxTurns=10]
 * @param {boolean}  [config.verbose=false]
 * @param {string[]|null} [config.allowlist=null]
 * @param {string}   [config.agent]
 * @param {boolean}  [config.autoJoin=true]        - Auto-join rooms on invite
 * @param {import('../channels/session-store.js').ChannelSessionStore} [config.sessionStore]
 * @param {import('../channels/identity.js').CustomerIdentityStore}    [config.identityStore]
 * @param {Function[]} [config.middleware]
 * @param {Object}   [config.autonomousEngine]
 * @returns {Promise<{ shutdown: () => void }>}
 */
export async function startMatrixGateway({
  dbPath = './store.db',
  allowApply = true,
  model,
  maxTurns = 10,
  verbose = false,
  allowlist = null,
  agent,
  autoJoin = true,
  sessionStore,
  identityStore,
  middleware = [],
  autonomousEngine,
} = {}) {
  const homeserver = process.env.MATRIX_HOMESERVER_URL;
  if (!homeserver) {
    throw new Error(
      'MATRIX_HOMESERVER_URL environment variable is required.\n' +
        'Set it to the base URL of your Matrix homeserver, e.g. https://matrix.example.com',
    );
  }

  const accessToken = process.env.MATRIX_ACCESS_TOKEN;
  if (!accessToken) {
    throw new Error(
      'MATRIX_ACCESS_TOKEN environment variable is required.\n' +
        'Create a bot account and generate an access token via Element or the API.',
    );
  }

  // Normalise homeserver URL (strip trailing slash)
  const hs = homeserver.replace(/\/+$/, '');

  console.log('Starting StateSet Matrix Gateway...');
  console.log(`Homeserver: ${hs}`);

  // --------------------------------------------------------------------
  // Identify the bot's own user ID via /account/whoami
  // --------------------------------------------------------------------
  let botUserId;
  try {
    const whoami = await matrixFetch(hs, accessToken, 'GET', '/_matrix/client/v3/account/whoami');
    botUserId = whoami.user_id;
    console.log(`Authenticated as ${botUserId}`);
  } catch (err) {
    throw new Error(`Failed to authenticate with Matrix homeserver: ${err.message}`);
  }

  // --------------------------------------------------------------------
  // Session manager
  // --------------------------------------------------------------------
  const sessionManager = createSessionManager({ store: sessionStore, channel: 'matrix' });
  const cleanupHandle = sessionManager.startCleanup();

  // Monotonic transaction counter for send deduplication
  let txnCounter = Date.now();

  // --------------------------------------------------------------------
  // Channel adapter
  // --------------------------------------------------------------------

  /** @type {import('../channels/base.js').ChannelAdapter} */
  const adapter = {
    /**
     * Extract the text body from an m.room.message event.
     */
    extractText(raw) {
      if (raw?.type !== 'm.room.message') return null;
      const content = raw.content;
      if (!content || content.msgtype !== 'm.text') return null;
      return content.body || null;
    },

    /**
     * Get the Matrix user ID of the sender (@user:server).
     */
    getSenderId(raw) {
      return raw.sender || '';
    },

    /**
     * Get the room ID where the event occurred.
     */
    getTargetId(raw) {
      return raw.room_id || '';
    },

    /**
     * Detect messages sent by this bot.
     */
    isOwnMessage(raw) {
      return raw.sender === botUserId;
    },

    /**
     * Send an m.room.message to a room via PUT (idempotent with txnId).
     */
    async send(roomId, text) {
      const txnId = `ssi_${txnCounter++}`;
      const encodedRoomId = encodeURIComponent(roomId);
      const encodedTxnId = encodeURIComponent(txnId);

      await matrixFetch(
        hs,
        accessToken,
        'PUT',
        `/_matrix/client/v3/rooms/${encodedRoomId}/send/m.room.message/${encodedTxnId}`,
        {
          body: {
            msgtype: 'm.text',
            body: text,
          },
        },
      );
    },

    /**
     * Send a typing indicator to a room.
     */
    async sendTyping(roomId) {
      const encodedRoomId = encodeURIComponent(roomId);
      const encodedUserId = encodeURIComponent(botUserId);

      await matrixFetch(
        hs,
        accessToken,
        'PUT',
        `/_matrix/client/v3/rooms/${encodedRoomId}/typing/${encodedUserId}`,
        {
          body: {
            typing: true,
            timeout: 15000,
          },
        },
      );
    },

    /**
     * Matrix supports full Unicode and markdown-ish formatting natively;
     * no transformation needed.
     */
    formatForPlatform(text) {
      return text;
    },

    /**
     * Matrix allows very large messages (spec says 65535 bytes body).
     */
    maxMessageLength: 65535,
  };

  // --------------------------------------------------------------------
  // Message handler (shared pipeline from base.js)
  // --------------------------------------------------------------------
  const handleMessage = createMessageHandler(adapter, {
    getSession: sessionManager.getSession,
    persistSession: sessionManager.persistSession,
    dbPath,
    allowApply,
    model,
    maxTurns,
    agent,
    verbose,
    allowlist,
    middleware,
    channel: 'matrix',
    identityStore,
    autonomousEngine,
  });

  // --------------------------------------------------------------------
  // Register with the notifier for outbound notifications
  // --------------------------------------------------------------------
  getNotifier().registerChannel('matrix', {
    send: adapter.send,
    formatForPlatform: adapter.formatForPlatform,
  });

  // --------------------------------------------------------------------
  // Auto-join on invite
  // --------------------------------------------------------------------

  /**
   * Process invite events from the /sync response.
   * Automatically joins rooms the bot is invited to.
   *
   * @param {Object} invite - The invite section from /sync
   */
  async function processInvites(invite) {
    if (!autoJoin || !invite) return;

    for (const roomId of Object.keys(invite)) {
      try {
        const encodedRoomId = encodeURIComponent(roomId);
        await matrixFetch(hs, accessToken, 'POST', `/_matrix/client/v3/join/${encodedRoomId}`, {
          body: {},
        });
        console.log(`[Matrix] Auto-joined room ${roomId}`);
      } catch (err) {
        console.error(`[Matrix] Failed to auto-join room ${roomId}: ${err.message}`);
      }
    }
  }

  // --------------------------------------------------------------------
  // Long-polling sync loop
  // --------------------------------------------------------------------
  let stopped = false;
  let syncAbort = null;
  let nextBatch = null;

  /**
   * Run the /sync long-poll loop. On first call we do an initial sync
   * (without timeout) to establish the `next_batch` token, then switch
   * to long-polling with timeout=30000.
   */
  async function syncLoop() {
    // Initial sync — catch up without processing old messages
    try {
      const query = new URLSearchParams({
        timeout: '0',
        // Only fetch room events we care about
        filter: JSON.stringify({
          room: {
            timeline: { limit: 0 },
            state: { lazy_load_members: true },
          },
          presence: { types: [] },
          account_data: { types: [] },
        }),
      });

      const initial = await matrixFetch(hs, accessToken, 'GET', '/_matrix/client/v3/sync', {
        query,
      });
      nextBatch = initial.next_batch;

      // Process any pending invites from the initial sync
      if (initial.rooms?.invite) {
        await processInvites(initial.rooms.invite);
      }

      if (verbose) {
        console.log(`[Matrix] Initial sync complete, next_batch: ${nextBatch}`);
      }
    } catch (err) {
      throw new Error(`Matrix initial sync failed: ${err.message}`);
    }

    console.log('Matrix gateway connected. Listening for messages...');

    // Long-poll loop
    while (!stopped) {
      try {
        syncAbort = new AbortController();

        const query = new URLSearchParams({
          since: nextBatch,
          timeout: '30000',
          filter: JSON.stringify({
            room: {
              timeline: { limit: 50 },
              state: { lazy_load_members: true },
            },
            presence: { types: [] },
            account_data: { types: [] },
          }),
        });

        const syncResponse = await matrixFetch(hs, accessToken, 'GET', '/_matrix/client/v3/sync', {
          query,
          signal: syncAbort.signal,
        });

        nextBatch = syncResponse.next_batch;

        // --- Process invites ---
        if (syncResponse.rooms?.invite) {
          await processInvites(syncResponse.rooms.invite);
        }

        // --- Process joined room timelines ---
        const joinedRooms = syncResponse.rooms?.join;
        if (joinedRooms) {
          for (const [roomId, roomData] of Object.entries(joinedRooms)) {
            const events = roomData.timeline?.events;
            if (!events || events.length === 0) continue;

            for (const event of events) {
              // Attach room_id to the event for the adapter
              event.room_id = roomId;

              // Only process m.room.message events
              if (event.type !== 'm.room.message') continue;

              try {
                await handleMessage(event);
              } catch (err) {
                console.error(`[Matrix] Error handling message in ${roomId}: ${err.message}`);
                if (verbose) console.error(err);
              }
            }
          }
        }
      } catch (err) {
        if (stopped) break;

        // AbortError is expected on shutdown
        if (err.name === 'AbortError') continue;

        console.error(`[Matrix] Sync error: ${err.message}`);
        if (verbose) console.error(err);

        // Backoff before retrying
        await new Promise((resolve) => setTimeout(resolve, 5000));
      }
    }
  }

  // Start the sync loop (non-blocking — runs in background)
  const syncPromise = syncLoop();

  // If the initial sync fails the promise rejects; we need to wait for it
  // to ensure the gateway is connected before returning.
  // We give it a brief window for the initial sync to complete.
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      reject(
        new Error(
          `Matrix gateway timed out connecting to ${hs}.\n` +
            'Check MATRIX_HOMESERVER_URL and MATRIX_ACCESS_TOKEN.',
        ),
      );
    }, 30_000);

    // Poll for the initial sync to complete (nextBatch is set)
    const check = setInterval(() => {
      if (nextBatch) {
        clearTimeout(timeout);
        clearInterval(check);
        resolve();
      }
      if (stopped) {
        clearTimeout(timeout);
        clearInterval(check);
        reject(new Error('Gateway stopped before initial sync completed'));
      }
    }, 100);

    // Also catch immediate failures
    syncPromise.catch((err) => {
      clearTimeout(timeout);
      clearInterval(check);
      reject(err);
    });
  });

  // --------------------------------------------------------------------
  // Shutdown handle
  // --------------------------------------------------------------------
  function shutdown() {
    if (stopped) return;
    stopped = true;

    getNotifier().unregisterChannel('matrix');
    sessionManager.stopCleanup(cleanupHandle);

    if (syncAbort) {
      try {
        syncAbort.abort();
      } catch (err) {
        console.warn('[matrix] Sync abort error:', err.message);
      }
    }

    console.log('Matrix gateway shut down.');
  }

  return { shutdown, _syncPromise: syncPromise };
}
