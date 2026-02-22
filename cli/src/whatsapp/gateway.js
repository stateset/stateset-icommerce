/**
 * WhatsApp Gateway for StateSet iCommerce
 *
 * Bridges WhatsApp messages to the StateSet commerce agent.
 * Each WhatsApp sender gets their own agent session so multi-turn
 * conversations work naturally (e.g. building a cart across messages).
 *
 * Includes automatic reconnection with exponential backoff,
 * modeled after moltbot's reconnect strategy.
 *
 * Shared logic (sessions, chunking, commands, agent processing) lives in
 * channels/base.js — this file only contains WhatsApp-specific wiring
 * (Baileys reconnect loop, self-chat detection, group filtering).
 */

import {
  createSessionManager,
  createMessageHandler,
  RECONNECT_POLICY,
  computeBackoff,
  sleep,
} from '../channels/base.js';

import {
  createWhatsAppSocket,
  waitForConnection,
  extractText as waExtractText,
  jidToPhone,
  isGroup,
  getStatusCode,
  DisconnectReason,
  clearAuth,
  DEFAULT_AUTH_DIR,
} from './session.js';

import { getNotifier } from '../channels/notifier.js';
import { richMessageToPlainText } from '../channels/rich-messages.js';

// ============================================================================
// WhatsApp-specific markdown cleanup
// ============================================================================

/**
 * Convert markdown formatting to WhatsApp-friendly text.
 * WhatsApp supports *bold*, _italic_, ~strikethrough~, and ```monospace```.
 *
 * @param {string} text
 * @returns {string}
 */
function cleanForWhatsApp(text) {
  return (
    text
      // Convert markdown headers to bold
      .replace(/^#{1,6}\s+(.+)$/gm, '*$1*')
      // Convert **bold** to *bold* (WhatsApp bold)
      .replace(/\*\*(.+?)\*\*/g, '*$1*')
      // Convert markdown tables to plain text
      .replace(/\|([^|]+)\|/g, (_, content) => content.trim())
      .replace(/^[-|:\s]+$/gm, '')
      // Convert markdown links [text](url) to text (url)
      .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '$1 ($2)')
      // Clean up excessive blank lines
      .replace(/\n{3,}/g, '\n\n')
      .trim()
  );
}

// ============================================================================
// Gateway core with reconnect loop
// ============================================================================

/**
 * Start the WhatsApp gateway with automatic reconnection.
 *
 * @param {Object} options
 * @param {string}   [options.dbPath='./store.db']  - Path to commerce SQLite DB
 * @param {boolean}  [options.allowApply=true]       - Enable write operations
 * @param {string}   [options.model]                 - Claude model override
 * @param {number}   [options.maxTurns=10]           - Max agent turns per message
 * @param {string}   [options.authDir]               - WhatsApp credentials directory
 * @param {boolean}  [options.verbose=false]          - Verbose logging
 * @param {string[]|null} [options.allowlist=null]   - Allowed phone numbers (null = allow all)
 * @param {boolean}  [options.allowGroups=false]      - Respond to group messages
 * @param {string}   [options.agent]                 - Force specific agent (e.g. 'customer-service')
 * @param {import('../channels/session-store.js').ChannelSessionStore} [options.sessionStore]
 * @param {Function[]} [options.middleware]
 * @returns {Promise<{ sock: WASocket, shutdown: () => void }>}
 */
export async function startWhatsAppGateway({
  dbPath = './store.db',
  allowApply = true,
  model,
  maxTurns = 10,
  authDir,
  verbose = false,
  allowlist = null,
  allowGroups = false,
  agent,
  sessionStore,
  middleware = [],
} = {}) {
  console.info('Starting StateSet WhatsApp Gateway...');

  const sessionManager = createSessionManager({ store: sessionStore, channel: 'whatsapp' });
  const cleanupHandle = sessionManager.startCleanup();
  let stopped = false;
  let currentSock = null;
  let reconnectAttempts = 0;
  let hasConnectedOnce = false;
  const resolvedAuthDir = authDir || DEFAULT_AUTH_DIR;

  /**
   * Build the message handler for the current socket.
   * Called each time we reconnect since the sock reference changes.
   */
  function buildHandler(sock) {
    /** @type {import('../channels/base.js').ChannelAdapter} */
    const adapter = {
      extractText: (msg) => {
        if (!msg.message) return null;
        const remoteJid = msg.key?.remoteJid;
        if (!remoteJid || remoteJid === 'status@broadcast') return null;

        // Skip group messages unless enabled
        if (isGroup(remoteJid) && !allowGroups) return null;

        return waExtractText(msg.message) || null;
      },
      getSenderId: (msg) => {
        const remoteJid = msg.key?.remoteJid;
        const senderJid = isGroup(remoteJid) ? msg.key.participant : remoteJid;
        return senderJid || '';
      },
      getTargetId: (msg) => msg.key?.remoteJid || '',
      isOwnMessage: (msg) => {
        if (!msg.key?.fromMe) return false;
        // Self-chat: remoteJid matches our own JID
        const remoteJid = msg.key.remoteJid;
        const myPhone = jidToPhone(sock.user?.id);
        const remotePhone = jidToPhone(remoteJid);
        const isLidSelfChat = remoteJid?.endsWith('@lid');
        const isSelfChat = remoteJid === sock.user?.id || remotePhone === myPhone || isLidSelfChat;
        // If it IS a self-chat (messaging yourself), allow it (return false = not "own")
        // If it's NOT a self-chat, it's an outgoing message to someone else → skip
        return !isSelfChat;
      },
      send: async (remoteJid, text) => {
        await sock.sendMessage(remoteJid, { text });
      },
      sendTyping: async (remoteJid) => {
        await sock.sendPresenceUpdate('composing', remoteJid);
      },
      formatForPlatform: cleanForWhatsApp,
      maxMessageLength: 4000,

      /**
       * Rich message fallback — WhatsApp doesn't support structured cards.
       * Use plain-text through cleanForWhatsApp.
       */
      sendRichMessage: async (remoteJid, richMsg) => {
        const text = cleanForWhatsApp(richMessageToPlainText(richMsg));
        await sock.sendMessage(remoteJid, { text });
      },
    };

    // Register with notifier (re-register on each reconnect since sock changes)
    getNotifier().registerChannel('whatsapp', {
      send: adapter.send,
      sendRichMessage: adapter.sendRichMessage,
      formatForPlatform: adapter.formatForPlatform,
    });

    return createMessageHandler(adapter, {
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
      channel: 'whatsapp',
    });
  }

  /**
   * Connect (or reconnect) to WhatsApp and wire up message handling.
   * Returns when the connection closes so the outer loop can retry.
   */
  async function connectAndListen() {
    const socketOpts = { printQr: true, verbose };
    if (authDir) socketOpts.authDir = authDir;

    const { sock } = await createWhatsAppSocket(socketOpts);
    currentSock = sock;

    await waitForConnection(sock);
    hasConnectedOnce = true;
    reconnectAttempts = 0;
    console.info('WhatsApp connected. Gateway is ready for messages.');

    const handleMessage = buildHandler(sock);

    sock.ev.on('messages.upsert', async ({ messages, type }) => {
      if (type !== 'notify' && type !== 'append') return;
      if (verbose) console.debug(`[messages.upsert] type=${type}, count=${messages?.length}`);

      for (const msg of messages) {
        try {
          await handleMessage(msg);
        } catch (err) {
          console.error('Error handling message:', err.message);
          if (verbose) console.error(err);
        }
      }
    });

    // Wait until the connection closes, then return the close reason
    return new Promise((resolve) => {
      sock.ev.on('connection.update', (update) => {
        if (update.connection === 'close') {
          const statusCode = getStatusCode(update.lastDisconnect?.error);
          const loggedOut = statusCode === DisconnectReason.loggedOut;
          resolve({ statusCode, loggedOut, error: update.lastDisconnect?.error });
        }
      });
    });
  }

  // Main reconnect loop (WhatsApp-specific — Baileys doesn't auto-reconnect)
  async function runLoop() {
    while (!stopped) {
      try {
        const closeReason = await connectAndListen();

        if (stopped) break;

        if (closeReason.loggedOut) {
          if (!hasConnectedOnce) {
            console.info('Stale credentials detected. Clearing auth and retrying with fresh QR...');
            clearAuth(resolvedAuthDir);
            reconnectAttempts = 0;
            await sleep(1_000);
            continue;
          }
          console.error('WhatsApp session was logged out. Run: stateset-whatsapp --reset');
          break;
        }

        if (hasConnectedOnce) {
          // Reset counter after a successful stretch (moltbot pattern)
        }
        hasConnectedOnce = true;
        reconnectAttempts += 1;

        if (RECONNECT_POLICY.maxAttempts > 0 && reconnectAttempts >= RECONNECT_POLICY.maxAttempts) {
          console.error(
            `Max reconnect attempts reached (${reconnectAttempts}/${RECONNECT_POLICY.maxAttempts}). Exiting.`,
          );
          break;
        }

        const delay = computeBackoff(RECONNECT_POLICY, reconnectAttempts);
        const statusCode = closeReason.statusCode || 'unknown';
        console.info(
          `WhatsApp disconnected (status ${statusCode}). Reconnecting in ${(delay / 1000).toFixed(1)}s (attempt ${reconnectAttempts}/${RECONNECT_POLICY.maxAttempts})...`,
        );

        await sleep(delay);
      } catch (err) {
        if (stopped) break;

        const statusCode = getStatusCode(err);
        const isLoggedOut = statusCode === DisconnectReason.loggedOut;

        if (isLoggedOut && !hasConnectedOnce) {
          console.info('Stale credentials detected. Clearing auth and retrying with fresh QR...');
          clearAuth(resolvedAuthDir);
          reconnectAttempts = 0;
          await sleep(1_000);
          continue;
        }

        reconnectAttempts += 1;

        if (RECONNECT_POLICY.maxAttempts > 0 && reconnectAttempts >= RECONNECT_POLICY.maxAttempts) {
          console.error(
            `Max reconnect attempts reached (${reconnectAttempts}/${RECONNECT_POLICY.maxAttempts}). Exiting.`,
          );
          break;
        }

        const delay = computeBackoff(RECONNECT_POLICY, reconnectAttempts);
        console.error(
          `Connection failed: ${err.message}. Reconnecting in ${(delay / 1000).toFixed(1)}s (attempt ${reconnectAttempts}/${RECONNECT_POLICY.maxAttempts})...`,
        );

        await sleep(delay);
      }
    }
  }

  // Start the loop (don't await — let it run in background)
  const loopPromise = runLoop();

  const shutdown = () => {
    stopped = true;
    getNotifier().unregisterChannel('whatsapp');
    sessionManager.stopCleanup(cleanupHandle);
    try {
      if (currentSock) currentSock.end(undefined);
    } catch (err) {
      console.warn('[whatsapp] Socket close error:', err.message);
    }
    console.info('WhatsApp gateway shut down.');
  };

  // Wait for the first connection to succeed before returning
  await new Promise((resolve, reject) => {
    const check = setInterval(() => {
      if (stopped) {
        clearInterval(check);
        reject(new Error('Gateway stopped before connecting'));
      }
      if (currentSock?.user) {
        clearInterval(check);
        resolve();
      }
    }, 200);

    setTimeout(() => {
      clearInterval(check);
      if (!currentSock?.user) {
        reject(
          new Error('Timed out waiting for WhatsApp connection (120s). Did you scan the QR code?'),
        );
      }
    }, 120_000);
  });

  return { sock: currentSock, shutdown, _loopPromise: loopPromise };
}

export { SESSION_TTL_MS } from '../channels/base.js';
