/**
 * Slack Gateway for StateSet iCommerce
 *
 * Bridges Slack messages to the StateSet commerce agent via Bolt (Socket Mode).
 * Each Slack user gets their own agent session for multi-turn conversations.
 */

import {
  createSessionManager,
  createMessageHandler,
  BOT_PREFIX,
} from '../channels/base.js';
import { getNotifier } from '../channels/notifier.js';
import { richMessageToPlainText } from '../channels/rich-messages.js';

/**
 * Start the Slack gateway.
 *
 * @param {Object} options
 * @param {string}   [options.dbPath='./store.db']
 * @param {boolean}  [options.allowApply=true]
 * @param {string}   [options.model]
 * @param {number}   [options.maxTurns=10]
 * @param {boolean}  [options.verbose=false]
 * @param {string[]|null} [options.allowlist=null]
 * @param {string}   [options.agent]
 * @param {import('../channels/session-store.js').ChannelSessionStore} [options.sessionStore]
 * @param {Function[]} [options.middleware]
 * @returns {Promise<{ shutdown: () => void }>}
 */
export async function startSlackGateway({
  dbPath = './store.db',
  allowApply = true,
  model,
  maxTurns = 10,
  verbose = false,
  allowlist = null,
  agent,
  sessionStore,
  middleware = [],
} = {}) {
  // Dynamic import — clear error if not installed
  let App;
  try {
    ({ default: { App } } = await import('@slack/bolt'));
  } catch {
    try {
      ({ App } = await import('@slack/bolt'));
    } catch {
      throw new Error(
        '@slack/bolt is not installed. Install it with: npm install @slack/bolt'
      );
    }
  }

  const botToken = process.env.SLACK_BOT_TOKEN;
  const appToken = process.env.SLACK_APP_TOKEN;

  if (!botToken) {
    throw new Error(
      'SLACK_BOT_TOKEN environment variable is required.\n' +
      'Get one from your Slack app settings: https://api.slack.com/apps'
    );
  }
  if (!appToken) {
    throw new Error(
      'SLACK_APP_TOKEN environment variable is required (starts with xapp-).\n' +
      'Enable Socket Mode and generate an app-level token in your Slack app settings.'
    );
  }

  console.log('Starting StateSet Slack Gateway...');

  const app = new App({
    token: botToken,
    appToken: appToken,
    socketMode: true,
  });

  const sessionManager = createSessionManager({ store: sessionStore, channel: 'slack' });
  const cleanupHandle = sessionManager.startCleanup();

  // Get bot user ID so we can detect mentions
  let botUserId = null;
  try {
    const auth = await app.client.auth.test({ token: botToken });
    botUserId = auth.user_id;
  } catch {
    // Will be set after first message if needed
  }

  /** @type {import('../channels/base.js').ChannelAdapter} */
  const adapter = {
    extractText: (event) => {
      let text = event.text || '';

      // In channels (not DMs), only respond when mentioned or in thread with bot
      if (event.channel_type !== 'im') {
        if (botUserId && !text.includes(`<@${botUserId}>`)) {
          // Not mentioned — check if this is a thread reply where bot participated
          if (!event.thread_ts) return null;
          // Allow thread replies (the thread may have started with a mention)
        }
        // Strip the mention
        if (botUserId) {
          text = text.replace(new RegExp(`<@${botUserId}>`, 'g'), '').trim();
        }
      }

      return text || null;
    },
    getSenderId: (event) => event.user,
    getTargetId: (event) => event.channel,
    isOwnMessage: (event) => event.subtype === 'bot_message' || !!event.bot_id,
    send: async (channel, text) => {
      await app.client.chat.postMessage({
        token: botToken,
        channel,
        text,
      });
    },
    sendTyping: null, // Slack bots can't show typing in Socket Mode
    formatForPlatform: (text) => {
      // Convert markdown bold to Slack bold (*text* stays the same)
      // Convert markdown headers to bold
      return text
        .replace(/^#{1,6}\s+(.+)$/gm, '*$1*')
        .replace(/\*\*(.+?)\*\*/g, '*$1*');
    },
    maxMessageLength: 3000,

    /**
     * Send a rich message using Slack Block Kit.
     */
    sendRichMessage: async (channel, richMsg) => {
      const blocks = [];

      // Header block
      blocks.push({
        type: 'header',
        text: { type: 'plain_text', text: richMsg.title.slice(0, 150), emoji: true },
      });

      // Description
      if (richMsg.description) {
        blocks.push({
          type: 'section',
          text: { type: 'mrkdwn', text: richMsg.description },
        });
      }

      // Fields (grouped in pairs for side-by-side)
      if (richMsg.fields && richMsg.fields.length > 0) {
        const fieldBlocks = [];
        for (let i = 0; i < richMsg.fields.length; i += 2) {
          const fields = [];
          fields.push({
            type: 'mrkdwn',
            text: `*${richMsg.fields[i].name}*\n${richMsg.fields[i].value}`,
          });
          if (richMsg.fields[i + 1]) {
            fields.push({
              type: 'mrkdwn',
              text: `*${richMsg.fields[i + 1].name}*\n${richMsg.fields[i + 1].value}`,
            });
          }
          fieldBlocks.push({ type: 'section', fields });
        }
        blocks.push(...fieldBlocks);
      }

      // Buttons
      if (richMsg.buttons && richMsg.buttons.length > 0) {
        const elements = richMsg.buttons.slice(0, 5).map((btn) => {
          if (btn.url) {
            return {
              type: 'button',
              text: { type: 'plain_text', text: btn.label },
              url: btn.url,
            };
          }
          return {
            type: 'button',
            text: { type: 'plain_text', text: btn.label },
            action_id: btn.action || btn.label.toLowerCase().replace(/\s+/g, '_'),
          };
        });
        blocks.push({ type: 'actions', elements });
      }

      // Footer as context
      if (richMsg.footer) {
        blocks.push({
          type: 'context',
          elements: [{ type: 'mrkdwn', text: richMsg.footer }],
        });
      }

      await app.client.chat.postMessage({
        token: botToken,
        channel,
        text: richMessageToPlainText(richMsg), // Fallback for notifications
        blocks,
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
    channel: 'slack',
  });

  app.message(async ({ event }) => {
    try {
      await handleMessage(event);
    } catch (err) {
      console.error('Error handling Slack message:', err.message);
      if (verbose) console.error(err);
    }
  });

  // Handle button actions from rich messages (Block Kit)
  app.action(/.*/, async ({ action, body, ack, respond }) => {
    await ack();

    try {
      const actionId = action.action_id || action.value || '';
      const userId = body.user?.id;
      const channel = body.channel?.id;

      if (!userId || !channel) return;

      // Route the action as a bot command
      const syntheticText = slackActionToCommand(actionId);
      if (syntheticText) {
        const syntheticEvent = {
          text: syntheticText,
          user: userId,
          channel,
          channel_type: 'im', // Treat as DM to bypass mention checks
        };
        await handleMessage(syntheticEvent);
      }
    } catch (err) {
      console.error('Error handling Slack action:', err.message);
      if (verbose) console.error(err);
      try {
        await respond({ text: 'Sorry, I encountered an error processing that action.', replace_original: false });
      } catch { /* ignore */ }
    }
  });

  // Bolt handles reconnection internally via Socket Mode
  await app.start();
  console.log('Slack bot connected. Gateway is ready for messages.');

  // Register with notifier
  getNotifier().registerChannel('slack', {
    send: adapter.send,
    sendRichMessage: adapter.sendRichMessage,
    formatForPlatform: adapter.formatForPlatform,
  });

  const shutdown = async () => {
    getNotifier().unregisterChannel('slack');
    sessionManager.stopCleanup(cleanupHandle);
    await app.stop();
    console.log('Slack gateway shut down.');
  };

  return { shutdown };
}

/**
 * Convert a Slack action_id to a bot command.
 */
function slackActionToCommand(actionId) {
  if (!actionId) return null;
  if (actionId.startsWith('/')) return actionId;

  const patterns = [
    { match: /^view_order[_:\s](.+)$/i, cmd: '/order' },
    { match: /^view_cart[_:\s](.+)$/i, cmd: '/cart' },
    { match: /^track[_:\s](.+)$/i, cmd: '/track' },
    { match: /^inventory[_:\s](.+)$/i, cmd: '/inventory' },
  ];

  for (const { match, cmd } of patterns) {
    const m = actionId.match(match);
    if (m) return `${cmd} ${m[1]}`;
  }

  return actionId;
}
