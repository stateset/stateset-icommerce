/**
 * WhatsApp Session Manager for StateSet iCommerce
 *
 * Manages Baileys WhatsApp Web socket connections, QR code authentication,
 * and credential persistence. Modeled after moltbot's session handling.
 */

import fs from 'node:fs';
import path from 'node:path';
import {
  DisconnectReason,
  fetchLatestBaileysVersion,
  makeCacheableSignalKeyStore,
  makeWASocket,
  normalizeMessageContent,
  useMultiFileAuthState,
} from '@whiskeysockets/baileys';

// Default auth directory under user home
const DEFAULT_AUTH_DIR = path.join(
  process.env.HOME || process.env.USERPROFILE || '.',
  '.stateset',
  'whatsapp-auth',
);

/**
 * Ensure a directory exists, creating it recursively if needed.
 */
function ensureDir(dirPath) {
  if (!fs.existsSync(dirPath)) {
    fs.mkdirSync(dirPath, { recursive: true });
  }
}

/**
 * Create a Baileys WhatsApp Web socket with persistent multi-file auth.
 *
 * @param {Object} options
 * @param {boolean} [options.printQr=true] - Print QR code to terminal for login
 * @param {string}  [options.authDir]      - Directory for credential storage
 * @param {boolean} [options.verbose=false] - Enable verbose Baileys logging
 * @param {(qr: string) => void} [options.onQr] - Custom QR code callback
 * @param {(status: string, error?: Error) => void} [options.onConnectionUpdate] - Connection status callback
 * @returns {Promise<{ sock: WASocket, saveCreds: () => Promise<void> }>}
 */
export async function createWhatsAppSocket({
  printQr = true,
  authDir = DEFAULT_AUTH_DIR,
  verbose = false,
  onQr,
  onConnectionUpdate,
} = {}) {
  ensureDir(authDir);

  const { state, saveCreds } = await useMultiFileAuthState(authDir);
  const { version } = await fetchLatestBaileysVersion();

  // Baileys expects a pino-compatible logger; suppress unless verbose
  const logger = createSilentLogger(verbose);

  const sock = makeWASocket({
    auth: {
      creds: state.creds,
      keys: makeCacheableSignalKeyStore(state.keys, logger),
    },
    version,
    logger,
    printQRInTerminal: false,
    browser: ['StateSet Commerce', 'CLI', '0.7.4'],
    syncFullHistory: false,
    markOnlineOnConnect: false,
  });

  // Persist credentials on update
  sock.ev.on('creds.update', saveCreds);

  // Handle connection lifecycle
  sock.ev.on('connection.update', (update) => {
    const { connection, lastDisconnect, qr } = update;

    if (qr) {
      if (onQr) onQr(qr);
      if (printQr) {
        // Dynamic import so qrcode-terminal is optional
        import('qrcode-terminal')
          .then((mod) => {
            const qrcode = mod.default || mod;
            console.debug('\nScan this QR code in WhatsApp > Linked Devices:\n');
            qrcode.generate(qr, { small: true });
          })
          .catch(() => {
            console.debug('\nQR code (install qrcode-terminal for visual display):');
            console.debug(qr);
          });
      }
    }

    if (connection === 'close') {
      const statusCode = getStatusCode(lastDisconnect?.error);
      if (onConnectionUpdate) {
        onConnectionUpdate('close', lastDisconnect?.error);
      }
      if (statusCode === DisconnectReason.loggedOut) {
        console.error(
          'WhatsApp session logged out. Delete auth directory and re-scan QR:',
          authDir,
        );
      }
    }

    if (connection === 'open') {
      if (onConnectionUpdate) onConnectionUpdate('open');
    }
  });

  // Handle WebSocket-level errors to prevent crashes
  if (sock.ws && typeof sock.ws.on === 'function') {
    sock.ws.on('error', (err) => {
      if (verbose) console.error('WebSocket error:', err.message);
    });
  }

  return { sock, saveCreds };
}

/**
 * Wait for the socket to reach "open" state.
 *
 * @param {WASocket} sock
 * @returns {Promise<void>}
 */
export function waitForConnection(sock) {
  return new Promise((resolve, reject) => {
    const handler = (update) => {
      if (update.connection === 'open') {
        sock.ev.off('connection.update', handler);
        resolve();
      }
      if (update.connection === 'close') {
        sock.ev.off('connection.update', handler);
        reject(update.lastDisconnect?.error || new Error('Connection closed'));
      }
    };
    sock.ev.on('connection.update', handler);
  });
}

/**
 * Extract text content from a raw WhatsApp message.
 * Uses Baileys normalizeMessageContent to unwrap the nested message
 * structure (viewOnce, ephemeral, etc.) before extracting text.
 *
 * @param {Object} rawMessage - Baileys proto.IMessage
 * @returns {string|undefined}
 */
export function extractText(rawMessage) {
  if (!rawMessage) return undefined;

  // Normalize first — unwraps viewOnce, ephemeral, documentWithCaption, etc.
  const message = normalizeMessageContent(rawMessage) || rawMessage;

  // Simple conversation text
  if (typeof message.conversation === 'string' && message.conversation.trim()) {
    return message.conversation.trim();
  }

  // Extended text (messages with links, formatting, etc.)
  const extended = message.extendedTextMessage?.text;
  if (extended?.trim()) return extended.trim();

  // Image/video/document captions
  const caption =
    message.imageMessage?.caption ??
    message.videoMessage?.caption ??
    message.documentMessage?.caption;
  if (caption?.trim()) return caption.trim();

  // Buttons / list response messages
  const buttonsResponse = message.buttonsResponseMessage?.selectedDisplayText;
  if (buttonsResponse?.trim()) return buttonsResponse.trim();

  const listResponse = message.listResponseMessage?.title;
  if (listResponse?.trim()) return listResponse.trim();

  return undefined;
}

/**
 * Normalize a phone number or JID to WhatsApp JID format.
 *
 * @param {string} input - Phone number or JID
 * @returns {string} JID like "1234567890@s.whatsapp.net"
 */
export function toJid(input) {
  if (!input) return input;
  // Already a JID
  if (input.includes('@')) return input;
  // Strip + and any non-digit chars, append @s.whatsapp.net
  const digits = input.replace(/[^\d]/g, '');
  return `${digits}@s.whatsapp.net`;
}

/**
 * Extract the phone number (E.164 without +) from a JID.
 *
 * @param {string} jid
 * @returns {string}
 */
export function jidToPhone(jid) {
  if (!jid) return jid;
  return jid.split('@')[0].split(':')[0];
}

/**
 * Check if a JID is a group chat.
 *
 * @param {string} jid
 * @returns {boolean}
 */
export function isGroup(jid) {
  return jid?.endsWith('@g.us') ?? false;
}

/**
 * Extract the Baileys status code from a disconnect error.
 */
export function getStatusCode(err) {
  return err?.output?.statusCode ?? err?.status;
}

export { DisconnectReason };

/**
 * Create a minimal pino-compatible logger that suppresses output unless verbose.
 */
function createSilentLogger(verbose = false) {
  const noop = () => {};
  const level = verbose ? 'info' : 'silent';
  const logger = {
    level,
    trace: noop,
    debug: noop,
    info: verbose ? console.log.bind(console) : noop,
    warn: verbose ? console.warn.bind(console) : noop,
    error: console.error.bind(console),
    fatal: console.error.bind(console),
    child() {
      return logger;
    },
  };
  return logger;
}

/**
 * Delete stored auth credentials (for logout/reset).
 *
 * @param {string} [authDir] - Auth directory to clear
 */
export function clearAuth(authDir = DEFAULT_AUTH_DIR) {
  if (fs.existsSync(authDir)) {
    fs.rmSync(authDir, { recursive: true, force: true });
  }
}

export { DEFAULT_AUTH_DIR };
