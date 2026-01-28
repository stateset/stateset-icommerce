/**
 * Channel Capabilities for StateSet iCommerce
 *
 * Defines feature flags for each messaging channel. Plugins and
 * components can query capabilities to adapt behavior (e.g., send
 * rich messages only on channels that support them).
 *
 * Usage:
 *   const caps = getCapabilities('telegram');
 *   if (caps.buttons) sendInlineKeyboard();
 */

// ============================================================================
// Default Capability Map
// ============================================================================

/**
 * @typedef {Object} ChannelCapabilities
 * @property {boolean} richMessages - Supports formatted text (bold, italic, links)
 * @property {boolean} buttons - Supports inline buttons/keyboards
 * @property {boolean} reactions - Supports message reactions
 * @property {boolean} media - Supports images, videos, documents
 * @property {boolean} threading - Supports threaded replies
 * @property {boolean} typing - Supports typing indicator
 * @property {boolean} polls - Supports native polls
 * @property {boolean} streaming - Supports streamed/chunked responses
 */

/** @type {Object<string, ChannelCapabilities>} */
const DEFAULT_CAPABILITIES = {
  telegram: {
    richMessages: true,
    buttons: true,
    reactions: false,
    media: true,
    threading: false,
    typing: true,
    polls: false,
    streaming: false,
  },

  discord: {
    richMessages: true,
    buttons: true,
    reactions: true,
    media: true,
    threading: true,
    typing: true,
    polls: false,
    streaming: false,
  },

  slack: {
    richMessages: true,
    buttons: true,
    reactions: true,
    media: true,
    threading: true,
    typing: false,
    polls: false,
    streaming: false,
  },

  whatsapp: {
    richMessages: false,
    buttons: false,
    reactions: false,
    media: true,
    threading: false,
    typing: true,
    polls: false,
    streaming: false,
  },

  signal: {
    richMessages: false,
    buttons: false,
    reactions: false,
    media: false,
    threading: false,
    typing: false,
    polls: false,
    streaming: false,
  },

  'google-chat': {
    richMessages: false,
    buttons: false,
    reactions: false,
    media: false,
    threading: false,
    typing: false,
    polls: false,
    streaming: false,
  },
};

// ============================================================================
// Runtime overrides
// ============================================================================

/** @type {Object<string, ChannelCapabilities>} */
const _overrides = {};

// ============================================================================
// Public API
// ============================================================================

/**
 * Get capabilities for a channel.
 *
 * Returns the merged result of default capabilities and any registered overrides.
 *
 * @param {string} channelName
 * @returns {ChannelCapabilities}
 */
export function getCapabilities(channelName) {
  const defaults = DEFAULT_CAPABILITIES[channelName] || createEmptyCapabilities();
  const overrides = _overrides[channelName] || {};
  return { ...defaults, ...overrides };
}

/**
 * Register or update capabilities for a channel.
 *
 * Merges with existing capabilities — only provided fields are overwritten.
 *
 * @param {string} channelName
 * @param {Partial<ChannelCapabilities>} caps
 */
export function registerCapabilities(channelName, caps) {
  if (!_overrides[channelName]) {
    _overrides[channelName] = {};
  }
  Object.assign(_overrides[channelName], caps);
}

/**
 * Get all capabilities (defaults + overrides) for all channels.
 *
 * @returns {Object<string, ChannelCapabilities>}
 */
export function getAllCapabilities() {
  const allChannels = new Set([
    ...Object.keys(DEFAULT_CAPABILITIES),
    ...Object.keys(_overrides),
  ]);

  const result = {};
  for (const channel of allChannels) {
    result[channel] = getCapabilities(channel);
  }
  return result;
}

/**
 * Check if a specific capability is supported by a channel.
 *
 * @param {string} channelName
 * @param {keyof ChannelCapabilities} capability
 * @returns {boolean}
 */
export function hasCapability(channelName, capability) {
  const caps = getCapabilities(channelName);
  return caps[capability] === true;
}

/**
 * Get list of channels that support a specific capability.
 *
 * @param {keyof ChannelCapabilities} capability
 * @returns {string[]}
 */
export function getChannelsWithCapability(capability) {
  const all = getAllCapabilities();
  return Object.entries(all)
    .filter(([, caps]) => caps[capability] === true)
    .map(([channel]) => channel);
}

/**
 * Clear all registered overrides.
 * Useful for testing.
 */
export function resetCapabilities() {
  for (const key of Object.keys(_overrides)) {
    delete _overrides[key];
  }
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Create an empty capabilities object (all false).
 *
 * @returns {ChannelCapabilities}
 */
function createEmptyCapabilities() {
  return {
    richMessages: false,
    buttons: false,
    reactions: false,
    media: false,
    threading: false,
    typing: false,
    polls: false,
    streaming: false,
  };
}
