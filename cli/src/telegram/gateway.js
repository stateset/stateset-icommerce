/**
 * Telegram Gateway for StateSet iCommerce
 *
 * Bridges Telegram messages to the StateSet commerce agent via grammY.
 * Each Telegram user gets their own agent session for multi-turn conversations.
 */

import { createSessionManager, createMessageHandler } from '../channels/base.js';
import { getNotifier } from '../channels/notifier.js';
import { isSafeDisplayUrl } from '../utils/url-validator.js';

/**
 * Start the Telegram gateway.
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
export async function startTelegramGateway({
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
  let Bot;
  try {
    ({ Bot } = await import('grammy'));
  } catch {
    throw new Error('grammy is not installed. Install it with: npm install grammy');
  }

  const token = process.env.TELEGRAM_BOT_TOKEN;
  if (!token) {
    throw new Error(
      'TELEGRAM_BOT_TOKEN environment variable is required.\n' +
        'Get one from @BotFather on Telegram: https://t.me/BotFather',
    );
  }

  console.log('Starting StateSet Telegram Gateway...');

  const bot = new Bot(token);
  const sessionManager = createSessionManager({ store: sessionStore, channel: 'telegram' });
  const cleanupHandle = sessionManager.startCleanup();

  /** @type {import('../channels/base.js').ChannelAdapter} */
  const adapter = {
    extractText: (ctx) => ctx.message?.text || null,
    getSenderId: (ctx) => String(ctx.from.id),
    getTargetId: (ctx) => ctx.chat.id,
    isOwnMessage: () => false, // Bots don't receive their own messages
    send: async (chatId, text) => {
      await bot.api.sendMessage(chatId, text);
    },
    sendTyping: async (chatId) => {
      await bot.api.sendChatAction(chatId, 'typing');
    },
    formatForPlatform: (text) => text, // Telegram supports markdown natively
    maxMessageLength: 4096,

    /**
     * Send a rich message via Telegram HTML formatting + inline keyboard.
     */
    sendRichMessage: async (chatId, richMsg) => {
      const lines = [];
      lines.push(`<b>${escapeHtml(richMsg.title)}</b>`);
      if (richMsg.description) lines.push(escapeHtml(richMsg.description));
      lines.push('');

      if (richMsg.fields) {
        for (const f of richMsg.fields) {
          lines.push(`<b>${escapeHtml(f.name)}:</b> ${escapeHtml(f.value)}`);
        }
      }

      if (richMsg.footer) {
        lines.push('');
        lines.push(`<i>${escapeHtml(richMsg.footer)}</i>`);
      }

      const opts = { parse_mode: 'HTML' };

      // Inline keyboard buttons
      if (richMsg.buttons && richMsg.buttons.length > 0) {
        opts.reply_markup = {
          inline_keyboard: [
            richMsg.buttons.map((btn) => {
              if (btn.url && isSafeDisplayUrl(btn.url)) return { text: btn.label, url: btn.url };
              return { text: btn.label, callback_data: btn.action || btn.label };
            }),
          ],
        };
      }

      await bot.api.sendMessage(chatId, lines.join('\n'), opts);
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
    channel: 'telegram',
  });

  bot.on('message:text', async (ctx) => {
    try {
      await handleMessage(ctx);
    } catch (err) {
      console.error('Error handling Telegram message:', err.message);
      if (verbose) console.error(err);
    }
  });

  // Handle callback queries from inline keyboard buttons
  bot.on('callback_query:data', async (ctx) => {
    try {
      const action = ctx.callbackQuery.data;
      const chatId = ctx.chat?.id || ctx.callbackQuery.message?.chat?.id;

      if (!chatId) {
        await ctx.answerCallbackQuery({ text: 'Unable to process action.' });
        return;
      }

      // Acknowledge the button press
      await ctx.answerCallbackQuery();

      // Route the action as a bot command (e.g. "view_order:123" → "/order 123")
      const syntheticText = actionToCommand(action);
      if (syntheticText) {
        // Build a synthetic context for the message handler
        const syntheticCtx = {
          message: { text: syntheticText },
          from: { id: ctx.from.id },
          chat: { id: chatId },
        };
        await handleMessage(syntheticCtx);
      }
    } catch (err) {
      console.error('Error handling Telegram callback:', err.message);
      if (verbose) console.error(err);
    }
  });

  // grammY handles reconnection internally via long polling
  bot.start({
    onStart: () => {
      console.log('Telegram bot connected. Gateway is ready for messages.');
    },
  });

  // Register with notifier
  getNotifier().registerChannel('telegram', {
    send: adapter.send,
    sendRichMessage: adapter.sendRichMessage,
    formatForPlatform: adapter.formatForPlatform,
  });

  const shutdown = () => {
    getNotifier().unregisterChannel('telegram');
    sessionManager.stopCleanup(cleanupHandle);
    bot.stop();
    console.log('Telegram gateway shut down.');
  };

  return { shutdown };
}

/**
 * Escape HTML special characters for Telegram HTML parse mode.
 */
function escapeHtml(text) {
  return String(text).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

/**
 * Convert a callback action string to a bot command.
 * Supports patterns like "view_order:123" → "/order 123"
 */
function actionToCommand(action) {
  if (!action) return null;

  // Direct command passthrough (action starts with /)
  if (action.startsWith('/')) return action;

  // Map common action patterns
  const patterns = [
    { match: /^view_order[:\s](.+)$/i, cmd: '/order' },
    { match: /^view_cart[:\s](.+)$/i, cmd: '/cart' },
    { match: /^track[:\s](.+)$/i, cmd: '/track' },
    { match: /^inventory[:\s](.+)$/i, cmd: '/inventory' },
  ];

  for (const { match, cmd } of patterns) {
    const m = action.match(match);
    if (m) return `${cmd} ${m[1]}`;
  }

  // Treat as plain text query to the agent
  return action;
}
