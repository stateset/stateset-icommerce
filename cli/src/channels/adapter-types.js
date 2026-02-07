/**
 * Specialized Channel Adapter Interfaces for StateSet iCommerce
 *
 * Breaks the monolithic ChannelAdapter into composable adapters.
 * Each adapter represents a capability that channels may or may not support.
 * Adapters are discovered by interface check (duck typing) and used
 * throughout the pipeline.
 *
 * Inspired by moltbot's 15+ adapter types:
 *   config, setup, pairing, security, status, gateway, outbound,
 *   messaging, streaming, threading, directory, resolver, mentions,
 *   actions, commands, heartbeat
 */

// ============================================================================
// Core Adapter (required for all channels)
// ============================================================================

/**
 * @typedef {Object} CoreAdapter
 * @property {(raw: any) => string|null} extractText - Extract text from raw message
 * @property {(raw: any) => string} getSenderId - Get sender identifier
 * @property {(raw: any) => string} getTargetId - Get target/chat identifier
 * @property {(raw: any) => boolean} isOwnMessage - Check if message is from this bot
 * @property {(text: string) => string} formatForPlatform - Format text for platform
 * @property {number} maxMessageLength - Platform max message length
 */

// ============================================================================
// Outbound Adapter
// ============================================================================

/**
 * @typedef {Object} OutboundAdapter
 * @property {(targetId: string, text: string) => Promise<void>} send - Send text message
 * @property {(targetId: string, text: string, opts?: SendOptions) => Promise<string>} [sendWithId] - Send and return message ID
 * @property {(targetId: string, messages: string[]) => Promise<void>} [sendBatch] - Send multiple messages
 * @property {(targetId: string) => Promise<void>} [sendTyping] - Send typing indicator
 * @property {number} [rateLimitMs] - Minimum ms between messages to same target
 */

/**
 * @typedef {Object} SendOptions
 * @property {boolean} [silent=false] - No notification sound
 * @property {string} [replyTo] - Message ID to reply to
 * @property {boolean} [disablePreview=false] - Disable URL previews
 */

// ============================================================================
// Rich Message Adapter
// ============================================================================

/**
 * @typedef {Object} RichMessageAdapter
 * @property {(targetId: string, richMsg: import('./rich-messages.js').RichMessage) => Promise<void>} sendRichMessage
 * @property {(targetId: string, buttons: ButtonDef[]) => Promise<void>} [sendButtons] - Send button row
 * @property {(targetId: string, embed: EmbedDef) => Promise<void>} [sendEmbed] - Send embed/card
 */

/**
 * @typedef {Object} ButtonDef
 * @property {string} label - Button text
 * @property {string} action - Callback action/data
 * @property {'primary'|'secondary'|'danger'|'link'} [style='primary']
 * @property {string} [url] - For link-type buttons
 */

/**
 * @typedef {Object} EmbedDef
 * @property {string} title
 * @property {string} [description]
 * @property {string} [color] - Hex color
 * @property {Array<{ name: string, value: string, inline?: boolean }>} [fields]
 * @property {string} [thumbnail] - Image URL
 * @property {string} [footer]
 */

// ============================================================================
// Streaming Adapter
// ============================================================================

/**
 * @typedef {Object} StreamingAdapter
 * @property {(targetId: string) => StreamingSession} startStream - Begin a streaming response
 */

/**
 * @typedef {Object} StreamingSession
 * @property {(chunk: string) => Promise<void>} write - Send a chunk of text
 * @property {() => Promise<string>} end - Finalize and return message ID
 * @property {() => void} abort - Cancel stream
 * @property {string} [messageId] - Platform message ID (available after first write)
 */

// ============================================================================
// Threading Adapter
// ============================================================================

/**
 * @typedef {Object} ThreadingAdapter
 * @property {(raw: any) => string|null} getThreadId - Get thread ID from raw message
 * @property {(targetId: string, threadId: string, text: string) => Promise<void>} sendToThread - Reply in thread
 * @property {(targetId: string, text: string) => Promise<string>} [createThread] - Create a new thread
 */

// ============================================================================
// Actions Adapter (message reactions, edits, deletes)
// ============================================================================

/**
 * @typedef {Object} ActionsAdapter
 * @property {(targetId: string, messageId: string, emoji: string) => Promise<void>} [addReaction]
 * @property {(targetId: string, messageId: string, emoji: string) => Promise<void>} [removeReaction]
 * @property {(targetId: string, messageId: string, newText: string) => Promise<void>} [editMessage]
 * @property {(targetId: string, messageId: string) => Promise<void>} [deleteMessage]
 * @property {(targetId: string, messageId: string) => Promise<void>} [pinMessage]
 * @property {() => string[]} getSupportedActions - List supported action types
 */

// ============================================================================
// Directory Adapter (user/group lookups)
// ============================================================================

/**
 * @typedef {Object} DirectoryAdapter
 * @property {(userId: string) => Promise<UserInfo|null>} getUser - Get user info
 * @property {(groupId: string) => Promise<GroupInfo|null>} [getGroup] - Get group info
 * @property {(query: string) => Promise<UserInfo[]>} [searchUsers] - Search for users
 * @property {(groupId: string) => Promise<UserInfo[]>} [getGroupMembers] - List members of a group
 */

/**
 * @typedef {Object} UserInfo
 * @property {string} id
 * @property {string} [name]
 * @property {string} [displayName]
 * @property {string} [email]
 * @property {string} [avatar] - Avatar URL
 * @property {boolean} [isBot]
 */

/**
 * @typedef {Object} GroupInfo
 * @property {string} id
 * @property {string} [name]
 * @property {number} [memberCount]
 * @property {string} [type] - 'group', 'channel', 'dm'
 */

// ============================================================================
// Mentions Adapter
// ============================================================================

/**
 * @typedef {Object} MentionsAdapter
 * @property {(userId: string) => string} formatMention - Format user mention for platform
 * @property {(text: string) => string[]} [extractMentions] - Extract mentioned user IDs from text
 * @property {() => string} [getBotMention] - Get the bot's mention string
 */

// ============================================================================
// Media Adapter
// ============================================================================

/**
 * @typedef {Object} MediaAdapter
 * @property {(targetId: string, media: MediaPayload) => Promise<void>} sendMedia
 * @property {(raw: any) => MediaPayload[]|null} [extractMedia] - Extract media from incoming message
 * @property {string[]} supportedTypes - e.g., ['image', 'video', 'audio', 'document']
 * @property {number} [maxFileSize] - Max file size in bytes
 */

/**
 * @typedef {Object} MediaPayload
 * @property {'image'|'video'|'audio'|'document'} type
 * @property {string|Buffer} source - URL, file path, or Buffer
 * @property {string} [filename]
 * @property {string} [mimeType]
 * @property {string} [caption]
 */

// ============================================================================
// Heartbeat Adapter (connection health)
// ============================================================================

/**
 * @typedef {Object} HeartbeatAdapter
 * @property {() => Promise<HealthStatus>} checkHealth
 * @property {() => number} getLastActivityMs - ms since last activity
 * @property {(handler: (status: HealthStatus) => void) => void} [onHealthChange]
 */

/**
 * @typedef {Object} HealthStatus
 * @property {'connected'|'disconnected'|'degraded'|'reconnecting'} status
 * @property {number} latencyMs
 * @property {string} [message]
 * @property {number} timestamp
 */

// ============================================================================
// Commands Adapter (channel-native commands)
// ============================================================================

/**
 * @typedef {Object} CommandsAdapter
 * @property {(commands: NativeCommand[]) => Promise<void>} registerNativeCommands - Register slash commands with platform
 * @property {(commands: string[]) => Promise<void>} [unregisterNativeCommands]
 * @property {() => NativeCommand[]} [getRegisteredCommands]
 */

/**
 * @typedef {Object} NativeCommand
 * @property {string} name
 * @property {string} description
 * @property {NativeCommandOption[]} [options]
 */

/**
 * @typedef {Object} NativeCommandOption
 * @property {string} name
 * @property {string} description
 * @property {'string'|'integer'|'boolean'|'user'|'channel'} type
 * @property {boolean} [required=false]
 * @property {Array<{ name: string, value: string }>} [choices]
 */

// ============================================================================
// Adapter Detection Utilities
// ============================================================================

/**
 * Check if an adapter supports a specific adapter interface.
 *
 * @param {Object} adapter
 * @param {string} adapterType
 * @returns {boolean}
 */
export function hasAdapterCapability(adapter, adapterType) {
  switch (adapterType) {
    case 'outbound':
      return typeof adapter.send === 'function';
    case 'richMessage':
      return typeof adapter.sendRichMessage === 'function';
    case 'streaming':
      return typeof adapter.startStream === 'function';
    case 'threading':
      return (
        typeof adapter.getThreadId === 'function' && typeof adapter.sendToThread === 'function'
      );
    case 'actions':
      return typeof adapter.getSupportedActions === 'function';
    case 'directory':
      return typeof adapter.getUser === 'function';
    case 'mentions':
      return typeof adapter.formatMention === 'function';
    case 'media':
      return typeof adapter.sendMedia === 'function';
    case 'heartbeat':
      return typeof adapter.checkHealth === 'function';
    case 'commands':
      return typeof adapter.registerNativeCommands === 'function';
    default:
      return false;
  }
}

/**
 * Get all supported adapter capabilities for a channel adapter.
 *
 * @param {Object} adapter
 * @returns {string[]}
 */
export function getAdapterCapabilities(adapter) {
  const types = [
    'outbound',
    'richMessage',
    'streaming',
    'threading',
    'actions',
    'directory',
    'mentions',
    'media',
    'heartbeat',
    'commands',
  ];

  return types.filter((type) => hasAdapterCapability(adapter, type));
}

/**
 * Create a composite adapter by merging capabilities from multiple sources.
 * Later sources override earlier ones.
 *
 * @param {...Object} adapters
 * @returns {Object}
 */
export function composeAdapters(...adapters) {
  return Object.assign({}, ...adapters);
}

/**
 * Wrap an adapter to add logging for all method calls.
 *
 * @param {Object} adapter
 * @param {string} channelName
 * @param {boolean} [verbose=false]
 * @returns {Object}
 */
export function withAdapterLogging(adapter, channelName, verbose = false) {
  if (!verbose) return adapter;

  return new Proxy(adapter, {
    get(target, prop) {
      const value = target[prop];
      if (typeof value === 'function') {
        return (...args) => {
          console.log(`[${channelName}] adapter.${String(prop)}(${args.length} args)`);
          return value.apply(target, args);
        };
      }
      return value;
    },
  });
}
