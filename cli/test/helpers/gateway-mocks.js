/**
 * Gateway test mocks for messaging channel tests.
 *
 * Provides reusable mocks for session managers, message handlers, and SDK stubs.
 * All messaging gateways (Discord, Slack, Telegram, etc.) follow the same pattern:
 *   1. Dynamic import of an SDK (discord.js, grammy, @slack/bolt, etc.)
 *   2. Check for an env var (DISCORD_BOT_TOKEN, TELEGRAM_BOT_TOKEN, etc.)
 *   3. Call createSessionManager() and createMessageHandler() from channels/base.js
 *   4. Return { shutdown }
 *
 * These mocks replicate the API surface used by gateway code so tests can run
 * without installing platform SDKs or setting real credentials.
 */

/**
 * Create a mock session manager matching the channels/base.js API.
 *
 * Mirrors: getSession, persistSession, startCleanup, stopCleanup, _sessions
 *
 * @returns {object} Mock session manager
 */
export function createMockSessionManager() {
  const sessions = new Map();

  function getSession(id) {
    let session = sessions.get(id);
    if (!session) {
      session = {
        sessionId: null,
        agent: null,
        lastActive: Date.now(),
        processing: false,
        queue: [],
      };
      sessions.set(id, session);
    }
    session.lastActive = Date.now();
    return session;
  }

  function persistSession(id, session) {
    sessions.set(id, { ...session, lastActive: Date.now() });
  }

  function startCleanup() {
    // Return a handle that can be passed to stopCleanup
    return { _mock: true };
  }

  function stopCleanup(_handle) {
    // No-op in tests
  }

  return { getSession, persistSession, startCleanup, stopCleanup, _sessions: sessions };
}

/**
 * Create a mock message handler matching the channels/base.js createMessageHandler signature.
 *
 * Returns an async function that records every call. Optionally customize the
 * response or throw to simulate errors.
 *
 * @param {object} [overrides]
 * @param {object} [overrides.response] - Custom response text/toolCalls to return
 * @param {Error}  [overrides.error]    - If set, the handler rejects with this error
 * @returns {Function & { calls: object[] }} Mock handler with call recording
 */
export function createMockMessageHandler(overrides = {}) {
  const calls = [];
  const handler = async (message) => {
    calls.push(message);
    if (overrides.error) {
      throw overrides.error;
    }
    return overrides.response || { text: 'Mock response', toolCalls: [] };
  };
  handler.calls = calls;
  return handler;
}

/**
 * Create a mock notifier matching the channels/notifier.js API.
 *
 * Records all notifications for assertions.
 *
 * @returns {{ notify: Function, registerChannel: Function, unregisterChannel: Function, notifications: object[], channels: Map }}
 */
export function createMockNotifier() {
  const notifications = [];
  const channels = new Map();
  return {
    notify: async (channel, message) => {
      notifications.push({ channel, message });
    },
    registerChannel: (name, adapter) => {
      channels.set(name, adapter);
    },
    unregisterChannel: (name) => {
      channels.delete(name);
    },
    notifications,
    channels,
  };
}

/**
 * Create a complete set of mock gateway dependencies for a given channel type.
 *
 * Bundles sessionManager, messageHandler, and notifier into one object — the
 * three dependencies every gateway module needs.
 *
 * @param {string} channelType - Channel identifier (e.g. 'discord', 'telegram', 'slack')
 * @returns {{ sessionManager: object, messageHandler: Function, notifier: object, channelType: string }}
 */
export function createMockGatewayDeps(channelType) {
  return {
    sessionManager: createMockSessionManager(),
    messageHandler: createMockMessageHandler(),
    notifier: createMockNotifier(),
    channelType,
  };
}

/**
 * Create a mock channel adapter matching the ChannelAdapter interface from base.js.
 *
 * @param {object} [overrides] - Override specific adapter methods
 * @returns {object} Mock channel adapter
 */
export function createMockChannelAdapter(overrides = {}) {
  const sentMessages = [];
  const richMessages = [];
  const typingTargets = [];

  return {
    extractText: (msg) => msg.text || msg.content || null,
    getSenderId: (msg) => msg.senderId || msg.author?.id || 'mock_sender',
    getTargetId: (msg) => msg.targetId || msg.channelId || 'mock_channel',
    isOwnMessage: (msg) => msg.isBot === true,
    send: async (targetId, text) => {
      sentMessages.push({ targetId, text });
    },
    sendTyping: async (targetId) => {
      typingTargets.push(targetId);
    },
    formatForPlatform: (text) => text,
    maxMessageLength: 2000,
    sendRichMessage: async (targetId, richMsg) => {
      richMessages.push({ targetId, ...richMsg });
    },
    // Test inspection helpers
    sentMessages,
    richMessages,
    typingTargets,
    ...overrides,
  };
}

/**
 * Create a mock SDK client for Discord (discord.js Client-like).
 *
 * @returns {object} Mock Discord client
 */
export function createMockDiscordClient() {
  const channels = new Map();
  const listeners = new Map();
  return {
    user: { id: 'bot_123', tag: 'TestBot#0001' },
    channels: {
      fetch: async (id) =>
        channels.get(id) || {
          id,
          send: async () => {},
          sendTyping: async () => {},
        },
    },
    on: (event, handler) => {
      if (!listeners.has(event)) listeners.set(event, []);
      listeners.get(event).push(handler);
    },
    once: (event, handler) => {
      if (!listeners.has(event)) listeners.set(event, []);
      listeners.get(event).push(handler);
      // Auto-fire 'ready' so tests don't hang
      if (event === 'ready') {
        queueMicrotask(() => handler());
      }
    },
    login: async () => {},
    destroy: () => {},
    _listeners: listeners,
    _channels: channels,
  };
}

/**
 * Create a mock SDK client for Telegram (grammY Bot-like).
 *
 * @returns {object} Mock Telegram bot
 */
export function createMockTelegramBot() {
  const listeners = new Map();
  return {
    api: {
      sendMessage: async () => {},
      sendChatAction: async () => {},
    },
    on: (event, handler) => {
      if (!listeners.has(event)) listeners.set(event, []);
      listeners.get(event).push(handler);
    },
    start: async () => {},
    stop: () => {},
    _listeners: listeners,
  };
}

/**
 * Create a mock SDK for Slack (@slack/bolt App-like).
 *
 * @returns {object} Mock Slack app
 */
export function createMockSlackApp() {
  const listeners = new Map();
  return {
    message: (handler) => {
      if (!listeners.has('message')) listeners.set('message', []);
      listeners.get('message').push(handler);
    },
    event: (name, handler) => {
      if (!listeners.has(name)) listeners.set(name, []);
      listeners.get(name).push(handler);
    },
    start: async () => {},
    stop: async () => {},
    client: {
      chat: {
        postMessage: async () => ({ ok: true }),
      },
    },
    _listeners: listeners,
  };
}
