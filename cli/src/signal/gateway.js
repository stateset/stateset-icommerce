/**
 * Signal Gateway for StateSet iCommerce
 *
 * Bridges Signal messages to the StateSet commerce agent via signal-cli
 * JSON-RPC daemon over a Unix socket. No external SDK needed — uses Node.js `net`.
 *
 * Prerequisite: signal-cli must be installed and running as a daemon:
 *   signal-cli -u +14155551234 daemon --json --socket /tmp/signal-cli.sock
 */

import { createConnection } from 'node:net';
import {
  createSessionManager,
  createMessageHandler,
  RECONNECT_POLICY,
  computeBackoff,
  sleep,
  BOT_PREFIX,
} from '../channels/base.js';
import { getNotifier } from '../channels/notifier.js';
import { richMessageToPlainText } from '../channels/rich-messages.js';

/**
 * Start the Signal gateway.
 *
 * @param {Object} options
 * @param {string}   [options.dbPath='./store.db']
 * @param {boolean}  [options.allowApply=true]
 * @param {string}   [options.model]
 * @param {number}   [options.maxTurns=10]
 * @param {boolean}  [options.verbose=false]
 * @param {string[]|null} [options.allowlist=null]
 * @param {string}   [options.agent]
 * @param {string}   options.phone              - Registered Signal phone number
 * @param {string}   [options.socket='/tmp/signal-cli.sock'] - Path to signal-cli socket
 * @param {import('../channels/session-store.js').ChannelSessionStore} [options.sessionStore]
 * @param {Function[]} [options.middleware]
 * @returns {Promise<{ shutdown: () => void }>}
 */
export async function startSignalGateway({
  dbPath = './store.db',
  allowApply = true,
  model,
  maxTurns = 10,
  verbose = false,
  allowlist = null,
  agent,
  phone,
  socket: socketPath = '/tmp/signal-cli.sock',
  sessionStore,
  middleware = [],
} = {}) {
  if (!phone) {
    throw new Error(
      '--phone is required. Provide the registered Signal phone number (e.g. +14155551234).'
    );
  }

  console.log('Starting StateSet Signal Gateway...');
  console.log(`Connecting to signal-cli daemon at ${socketPath}...`);

  const sessionManager = createSessionManager({ store: sessionStore, channel: 'signal' });
  const cleanupHandle = sessionManager.startCleanup();

  let stopped = false;
  let conn = null;
  let rpcId = 0;
  const pendingRpc = new Map();

  /**
   * Send a JSON-RPC call to signal-cli.
   */
  function jsonRpc(method, params) {
    return new Promise((resolve, reject) => {
      const id = ++rpcId;
      const msg = JSON.stringify({ jsonrpc: '2.0', method, id, params }) + '\n';
      pendingRpc.set(id, { resolve, reject });
      conn.write(msg);
    });
  }

  /**
   * Format text for Signal (strip markdown headers, basic formatting).
   */
  function formatForSignal(text) {
    return text
      .replace(/^#{1,6}\s+(.+)$/gm, '$1')
      .replace(/\*\*(.+?)\*\*/g, '*$1*');
  }

  /** @type {import('../channels/base.js').ChannelAdapter} */
  const adapter = {
    extractText: (envelope) => envelope.dataMessage?.message || null,
    getSenderId: (envelope) => envelope.source || '',
    getTargetId: (envelope) => {
      // Group messages: use groupId; DMs: use source
      return envelope.dataMessage?.groupInfo?.groupId || envelope.source;
    },
    isOwnMessage: (envelope) => envelope.source === phone,
    send: async (target, text) => {
      // Determine if target is a group or individual
      if (target.startsWith('+') || target.match(/^\d/)) {
        await jsonRpc('send', { recipient: [target], message: text });
      } else {
        await jsonRpc('send', { groupId: target, message: text });
      }
    },
    sendTyping: null, // signal-cli typing indicators are unreliable
    formatForPlatform: formatForSignal,
    maxMessageLength: 6000,

    /**
     * Rich message fallback — Signal doesn't support structured cards.
     */
    sendRichMessage: async (target, richMsg) => {
      const text = formatForSignal(richMessageToPlainText(richMsg));
      if (target.startsWith('+') || target.match(/^\d/)) {
        await jsonRpc('send', { recipient: [target], message: text });
      } else {
        await jsonRpc('send', { groupId: target, message: text });
      }
    },
  };

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
    channel: 'signal',
  });

  // Register with notifier
  getNotifier().registerChannel('signal', {
    send: adapter.send,
    sendRichMessage: adapter.sendRichMessage,
    formatForPlatform: adapter.formatForPlatform,
  });

  /**
   * Connect to signal-cli daemon and process messages.
   */
  async function connectAndListen() {
    return new Promise((resolve, reject) => {
      conn = createConnection(socketPath);
      let buffer = '';

      conn.on('connect', () => {
        console.log('Connected to signal-cli daemon. Gateway is ready for messages.');
        resolve();
      });

      conn.on('data', (data) => {
        buffer += data.toString();
        const lines = buffer.split('\n');
        buffer = lines.pop(); // Keep incomplete line in buffer

        for (const line of lines) {
          if (!line.trim()) continue;
          try {
            const msg = JSON.parse(line);
            // Handle JSON-RPC response
            if (msg.id && pendingRpc.has(msg.id)) {
              const { resolve: res, reject: rej } = pendingRpc.get(msg.id);
              pendingRpc.delete(msg.id);
              if (msg.error) rej(new Error(msg.error.message));
              else res(msg.result);
              continue;
            }
            // Handle incoming message envelope
            const envelope = msg.params?.envelope || msg.envelope;
            if (envelope && envelope.dataMessage) {
              handleMessage(envelope).catch((err) => {
                console.error('Error handling Signal message:', err.message);
                if (verbose) console.error(err);
              });
            }
          } catch (err) {
            if (verbose) console.error('Failed to parse signal-cli message:', err.message);
          }
        }
      });

      conn.on('error', (err) => {
        if (!conn._connected) reject(err);
        else console.error('Signal socket error:', err.message);
      });

      conn.once('connect', () => { conn._connected = true; });

      conn.on('close', () => {
        console.log('Signal socket closed.');
      });
    });
  }

  // Main loop with reconnection
  async function runLoop() {
    let attempts = 0;

    while (!stopped) {
      try {
        await connectAndListen();
        attempts = 0;

        // Wait for disconnect
        await new Promise((resolve) => {
          conn.on('close', resolve);
        });

        if (stopped) break;
      } catch (err) {
        if (stopped) break;
        attempts++;

        if (RECONNECT_POLICY.maxAttempts > 0 && attempts >= RECONNECT_POLICY.maxAttempts) {
          console.error(`Max reconnect attempts reached (${attempts}). Exiting.`);
          break;
        }

        const delay = computeBackoff(RECONNECT_POLICY, attempts);
        console.error(
          `Signal connection failed: ${err.message}. Reconnecting in ${(delay / 1000).toFixed(1)}s (attempt ${attempts}/${RECONNECT_POLICY.maxAttempts})...`
        );
        await sleep(delay);
      }
    }
  }

  const loopPromise = runLoop();

  const shutdown = () => {
    stopped = true;
    getNotifier().unregisterChannel('signal');
    sessionManager.stopCleanup(cleanupHandle);
    if (conn) {
      try { conn.destroy(); } catch { /* ignore */ }
    }
    console.log('Signal gateway shut down.');
  };

  // Wait for first successful connection
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      reject(new Error(
        `Could not connect to signal-cli daemon at ${socketPath}.\n` +
        'Ensure signal-cli is running: signal-cli -u ' + phone + ' daemon --json --socket ' + socketPath
      ));
    }, 15_000);

    const check = setInterval(() => {
      if (conn?._connected) {
        clearTimeout(timeout);
        clearInterval(check);
        resolve();
      }
      if (stopped) {
        clearTimeout(timeout);
        clearInterval(check);
        reject(new Error('Gateway stopped before connecting'));
      }
    }, 200);
  });

  return { shutdown, _loopPromise: loopPromise };
}
