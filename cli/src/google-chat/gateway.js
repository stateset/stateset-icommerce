/**
 * Google Chat Gateway for StateSet iCommerce
 *
 * Bridges Google Chat messages to the StateSet commerce agent via
 * the Google Chat API and Cloud Pub/Sub for message delivery.
 *
 * Requires:
 * - GCP project with Chat API enabled
 * - Service account with Chat API permissions
 * - Pub/Sub subscription configured for the Chat app
 * - GOOGLE_APPLICATION_CREDENTIALS env var pointing to service account JSON
 */

import { createSessionManager, createMessageHandler } from '../channels/base.js';
import { getNotifier } from '../channels/notifier.js';
import { richMessageToPlainText } from '../channels/rich-messages.js';

/**
 * Start the Google Chat gateway.
 *
 * @param {Object} options
 * @param {string}   [options.dbPath='./store.db']
 * @param {boolean}  [options.allowApply=true]
 * @param {string}   [options.model]
 * @param {number}   [options.maxTurns=10]
 * @param {boolean}  [options.verbose=false]
 * @param {string[]|null} [options.allowlist=null]
 * @param {string}   [options.agent]
 * @param {string}   options.subscription  - Pub/Sub subscription name
 * @param {import('../channels/session-store.js').ChannelSessionStore} [options.sessionStore]
 * @param {Function[]} [options.middleware]
 * @returns {Promise<{ shutdown: () => void }>}
 */
export async function startGoogleChatGateway({
  dbPath = './store.db',
  allowApply = true,
  model,
  maxTurns = 10,
  verbose = false,
  allowlist = null,
  agent,
  subscription,
  sessionStore,
  middleware = [],
} = {}) {
  // Dynamic imports — clear error if not installed
  let google, PubSub;
  try {
    ({ google } = await import('googleapis'));
  } catch (err) {
    throw new Error(
      `googleapis is not installed. Install it with: npm install googleapis (${err.message || err})`,
    );
  }
  try {
    ({ PubSub } = await import('@google-cloud/pubsub'));
  } catch (err) {
    throw new Error(
      `@google-cloud/pubsub is not installed. Install it with: npm install @google-cloud/pubsub (${err.message || err})`,
    );
  }

  if (!process.env.GOOGLE_APPLICATION_CREDENTIALS) {
    throw new Error(
      'GOOGLE_APPLICATION_CREDENTIALS environment variable is required.\n' +
        'Set it to the path of your service account JSON key file.\n\n' +
        'Setup steps:\n' +
        '1. Create a GCP project at https://console.cloud.google.com\n' +
        '2. Enable the Google Chat API and Cloud Pub/Sub API\n' +
        '3. Create a service account with Chat API permissions\n' +
        '4. Download the JSON key and set GOOGLE_APPLICATION_CREDENTIALS\n' +
        '5. Configure a Chat app in Google Chat API settings\n' +
        '6. Create a Pub/Sub topic and subscription for the Chat app',
    );
  }

  if (!subscription) {
    throw new Error(
      '--subscription is required. Provide the Pub/Sub subscription name\n' +
        '(e.g. projects/my-project/subscriptions/chat-sub).',
    );
  }

  console.log('Starting StateSet Google Chat Gateway...');

  // Initialize Google Chat API client
  const auth = new google.auth.GoogleAuth({
    scopes: ['https://www.googleapis.com/auth/chat.bot'],
  });
  const chat = google.chat({ version: 'v1', auth });

  // Initialize Pub/Sub client
  const pubsub = new PubSub();
  const sub = pubsub.subscription(subscription);

  const sessionManager = createSessionManager({ store: sessionStore, channel: 'google-chat' });
  const cleanupHandle = sessionManager.startCleanup();

  /**
   * Format text for Google Chat (subset of markdown).
   */
  function formatForGChat(text) {
    return text.replace(/^#{1,6}\s+(.+)$/gm, '*$1*').replace(/\*\*(.+?)\*\*/g, '*$1*');
  }

  /** @type {import('../channels/base.js').ChannelAdapter} */
  const adapter = {
    extractText: (event) => event.message?.text || null,
    getSenderId: (event) => event.user?.name || '', // "users/123456"
    getTargetId: (event) => event.space?.name || '', // "spaces/AAAA"
    isOwnMessage: () => false, // Google Chat doesn't echo bot messages
    send: async (space, text) => {
      await chat.spaces.messages.create({
        parent: space,
        requestBody: { text },
      });
    },
    sendTyping: null, // Not supported by Google Chat API
    formatForPlatform: formatForGChat,
    maxMessageLength: 4096,

    /**
     * Rich message fallback — Google Chat cards are complex; use plain text.
     */
    sendRichMessage: async (space, richMsg) => {
      const text = formatForGChat(richMessageToPlainText(richMsg));
      await chat.spaces.messages.create({
        parent: space,
        requestBody: { text },
      });
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
    channel: 'google-chat',
  });

  // Listen for Pub/Sub messages
  sub.on('message', async (message) => {
    try {
      const data = JSON.parse(message.data.toString());

      // Only process MESSAGE events (not ADDED_TO_SPACE, REMOVED, etc.)
      if (data.type !== 'MESSAGE') {
        message.ack();
        return;
      }

      if (verbose) {
        console.log(`[Google Chat] Received event type=${data.type} from ${data.user?.name}`);
      }

      await handleMessage(data);
      message.ack();
    } catch (err) {
      console.error('Error handling Google Chat message:', err.message);
      if (verbose) console.error(err);
      // Nack so it can be retried
      message.nack();
    }
  });

  sub.on('error', (err) => {
    console.error('Pub/Sub subscription error:', err.message);
  });

  console.log(`Google Chat gateway connected. Listening on subscription: ${subscription}`);

  // Register with notifier
  getNotifier().registerChannel('google-chat', {
    send: adapter.send,
    sendRichMessage: adapter.sendRichMessage,
    formatForPlatform: adapter.formatForPlatform,
  });

  const shutdown = async () => {
    getNotifier().unregisterChannel('google-chat');
    sessionManager.stopCleanup(cleanupHandle);
    sub.removeAllListeners();
    await sub.close();
    console.log('Google Chat gateway shut down.');
  };

  return { shutdown };
}
