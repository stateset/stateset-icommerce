/**
 * Discord Gateway for StateSet iCommerce
 *
 * Bridges Discord messages to the StateSet commerce agent via discord.js.
 * Each Discord user gets their own agent session for multi-turn conversations.
 */

import {
  createSessionManager,
  createMessageHandler,
  BOT_PREFIX,
} from '../channels/base.js';
import { getNotifier } from '../channels/notifier.js';
import { richMessageToPlainText } from '../channels/rich-messages.js';

/**
 * Start the Discord gateway.
 *
 * @param {Object} options
 * @param {string}   [options.dbPath='./store.db']
 * @param {boolean}  [options.allowApply=true]
 * @param {string}   [options.model]
 * @param {number}   [options.maxTurns=10]
 * @param {boolean}  [options.verbose=false]
 * @param {string[]|null} [options.allowlist=null]
 * @param {string}   [options.agent]
 * @param {boolean}  [options.mentionOnly=false]
 * @param {import('../channels/session-store.js').ChannelSessionStore} [options.sessionStore]
 * @param {Function[]} [options.middleware]
 * @returns {Promise<{ shutdown: () => void }>}
 */
export async function startDiscordGateway({
  dbPath = './store.db',
  allowApply = true,
  model,
  maxTurns = 10,
  verbose = false,
  allowlist = null,
  agent,
  mentionOnly = false,
  sessionStore,
  middleware = [],
} = {}) {
  // Dynamic import — clear error if not installed
  let Client, GatewayIntentBits, EmbedBuilder, ActionRowBuilder, ButtonBuilder, ButtonStyle;
  try {
    ({ Client, GatewayIntentBits, EmbedBuilder, ActionRowBuilder, ButtonBuilder, ButtonStyle } = await import('discord.js'));
  } catch {
    throw new Error(
      'discord.js is not installed. Install it with: npm install discord.js'
    );
  }

  const token = process.env.DISCORD_BOT_TOKEN;
  if (!token) {
    throw new Error(
      'DISCORD_BOT_TOKEN environment variable is required.\n' +
      'Get one from the Discord Developer Portal: https://discord.com/developers/applications'
    );
  }

  console.log('Starting StateSet Discord Gateway...');

  const client = new Client({
    intents: [
      GatewayIntentBits.Guilds,
      GatewayIntentBits.GuildMessages,
      GatewayIntentBits.DirectMessages,
      GatewayIntentBits.MessageContent,
    ],
  });

  const sessionManager = createSessionManager({ store: sessionStore, channel: 'discord' });
  const cleanupHandle = sessionManager.startCleanup();

  /** @type {import('../channels/base.js').ChannelAdapter} */
  const adapter = {
    extractText: (msg) => {
      let content = msg.content || '';

      // In mention-only mode in servers, require @mention
      if (mentionOnly && msg.guild) {
        const mentionPrefix = `<@${client.user.id}>`;
        if (!content.includes(mentionPrefix)) return null;
        // Strip the mention from the text
        content = content.replace(new RegExp(`<@!?${client.user.id}>`, 'g'), '').trim();
      }

      return content || null;
    },
    getSenderId: (msg) => msg.author.id,
    getTargetId: (msg) => msg.channelId,
    isOwnMessage: (msg) => msg.author.id === client.user?.id || msg.author.bot,
    send: async (channelId, text) => {
      const channel = await client.channels.fetch(channelId);
      if (channel) await channel.send(text);
    },
    sendTyping: async (channelId) => {
      const channel = await client.channels.fetch(channelId);
      if (channel) await channel.sendTyping();
    },
    formatForPlatform: (text) => text, // Discord supports markdown
    maxMessageLength: 2000,

    /**
     * Send a rich message using Discord embeds and buttons.
     */
    sendRichMessage: async (channelId, richMsg) => {
      const channel = await client.channels.fetch(channelId);
      if (!channel) return;

      const embed = new EmbedBuilder()
        .setTitle(richMsg.title);

      if (richMsg.description) embed.setDescription(richMsg.description);
      if (richMsg.color) embed.setColor(parseInt(richMsg.color.replace('#', ''), 16));
      if (richMsg.footer) embed.setFooter({ text: richMsg.footer });
      if (richMsg.imageUrl) embed.setImage(richMsg.imageUrl);

      if (richMsg.fields) {
        for (const f of richMsg.fields) {
          embed.addFields({ name: f.name, value: f.value, inline: f.inline ?? false });
        }
      }

      const payload = { embeds: [embed] };

      if (richMsg.buttons && richMsg.buttons.length > 0) {
        const row = new ActionRowBuilder();
        for (const btn of richMsg.buttons.slice(0, 5)) {
          const b = new ButtonBuilder().setLabel(btn.label);
          if (btn.url) {
            b.setStyle(ButtonStyle.Link).setURL(btn.url);
          } else {
            b.setStyle(ButtonStyle.Primary).setCustomId(btn.action || btn.label);
          }
          row.addComponents(b);
        }
        payload.components = [row];
      }

      await channel.send(payload);
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
    channel: 'discord',
  });

  client.on('messageCreate', async (msg) => {
    try {
      await handleMessage(msg);
    } catch (err) {
      console.error('Error handling Discord message:', err.message);
      if (verbose) console.error(err);
    }
  });

  // Handle button interactions from rich messages
  client.on('interactionCreate', async (interaction) => {
    if (!interaction.isButton()) return;

    try {
      const action = interaction.customId;
      const senderId = interaction.user.id;
      const channelId = interaction.channelId;

      // Acknowledge the interaction
      await interaction.deferReply();

      // Route the action as a bot command
      const syntheticText = discordActionToCommand(action);
      if (syntheticText) {
        // Build a synthetic message for the handler
        const syntheticMsg = {
          content: syntheticText,
          author: { id: senderId, bot: false },
          channelId,
          guild: interaction.guild,
        };
        await handleMessage(syntheticMsg);
      } else {
        await interaction.editReply('Action processed.');
      }
    } catch (err) {
      console.error('Error handling Discord interaction:', err.message);
      if (verbose) console.error(err);
      try {
        if (interaction.deferred) {
          await interaction.editReply('Sorry, I encountered an error processing that action.');
        }
      } catch { /* ignore */ }
    }
  });

  // Wait for ready
  await new Promise((resolve, reject) => {
    client.once('ready', () => {
      console.log(`Discord bot connected as ${client.user.tag}. Gateway is ready for messages.`);
      resolve();
    });
    client.once('error', reject);
    client.login(token);
  });

  // Register with notifier
  getNotifier().registerChannel('discord', {
    send: adapter.send,
    sendRichMessage: adapter.sendRichMessage,
    formatForPlatform: adapter.formatForPlatform,
  });

  const shutdown = () => {
    getNotifier().unregisterChannel('discord');
    sessionManager.stopCleanup(cleanupHandle);
    client.destroy();
    console.log('Discord gateway shut down.');
  };

  return { shutdown };
}

/**
 * Convert a button customId to a bot command.
 */
function discordActionToCommand(action) {
  if (!action) return null;
  if (action.startsWith('/')) return action;

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

  return action;
}
