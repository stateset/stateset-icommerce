/**
 * Shared channel base module for StateSet iCommerce
 *
 * Extracts reusable gateway logic (sessions, chunking, commands,
 * backoff, message pipeline) so each channel adapter is a thin wrapper.
 *
 * Inspired by moltbot's ChannelPlugin pattern.
 */

import { runAgentLoop } from '../claude-harness.js';
import { runMiddleware } from './middleware.js';
import { getMetrics } from './metrics.js';
import { getHandoffQueue } from './handoff.js';
import { getCommandRegistry } from './command-registry.js';
import { getPluginRegistry } from './plugin-api.js';
import {
  createOrderSummary,
  createOrderList,
  createInventoryCard,
  createCartSummary,
  createAnalyticsSummary,
  richMessageToPlainText,
} from './rich-messages.js';

// ============================================================================
// Constants
// ============================================================================

/** Prefix used to identify bot-generated messages and prevent self-reply loops. */
export const BOT_PREFIX = '[agent] ';

// ============================================================================
// Session management
// ============================================================================

export const SESSION_TTL_MS = 30 * 60 * 1000; // 30 minutes

/**
 * @typedef {Object} SenderSession
 * @property {string|null} sessionId   - Claude agent session ID for multi-turn
 * @property {string|null} agent       - Last-used agent name
 * @property {number}      lastActive  - Timestamp of last message
 * @property {boolean}     processing  - Whether a request is in-flight
 * @property {string[]}    queue       - Queued messages while processing
 */

/**
 * Create an isolated session store.
 * Each channel gateway gets its own store so sessions don't collide.
 *
 * @param {Object} [opts]
 * @param {import('./session-store.js').ChannelSessionStore} [opts.store] - Persistent session store
 * @param {string} [opts.channel] - Channel name for persistent store keying
 * @returns {{ getSession(id: string): SenderSession, startCleanup(): NodeJS.Timeout, stopCleanup(handle: NodeJS.Timeout): void }}
 */
export function createSessionManager({ store, channel } = {}) {
  /** @type {Map<string, SenderSession>} */
  const sessions = new Map();

  function getSession(id) {
    let session = sessions.get(id);
    if (!session || Date.now() - session.lastActive > SESSION_TTL_MS) {
      // Try loading from persistent store
      let persisted = null;
      if (store && channel) {
        persisted = store.get(channel, id);
      }

      if (persisted && Date.now() - persisted.lastActive <= SESSION_TTL_MS) {
        session = {
          sessionId: persisted.sessionId,
          agent: persisted.agent,
          lastActive: Date.now(),
          processing: false,
          queue: [],
        };
      } else {
        session = {
          sessionId: null,
          agent: null,
          lastActive: Date.now(),
          processing: false,
          queue: [],
        };
      }
      sessions.set(id, session);
    }
    session.lastActive = Date.now();
    return session;
  }

  /**
   * Persist a session to the store (if configured).
   * @param {string} id
   * @param {SenderSession} session
   */
  function persistSession(id, session) {
    if (store && channel) {
      store.upsert(channel, id, {
        sessionId: session.sessionId,
        agent: session.agent,
        lastActive: session.lastActive,
      });
    }
  }

  function startCleanup() {
    return setInterval(() => {
      const now = Date.now();
      for (const [id, session] of sessions) {
        if (now - session.lastActive > SESSION_TTL_MS) {
          sessions.delete(id);
        }
      }
      // Also clean persistent store
      if (store) {
        store.deleteExpired(SESSION_TTL_MS);
      }
    }, 5 * 60 * 1000);
  }

  function stopCleanup(handle) {
    clearInterval(handle);
  }

  return { getSession, persistSession, startCleanup, stopCleanup, _sessions: sessions };
}

// ============================================================================
// Message chunking
// ============================================================================

/**
 * Split a long response into platform-safe chunks.
 *
 * @param {string} text
 * @param {number} maxLength - Platform-specific max (WhatsApp 4000, Discord 2000, etc.)
 * @returns {string[]}
 */
export function chunkMessage(text, maxLength) {
  if (text.length <= maxLength) return [text];

  const chunks = [];
  let remaining = text;

  while (remaining.length > maxLength) {
    let splitIndex = remaining.lastIndexOf('\n\n', maxLength);
    if (splitIndex < maxLength * 0.3) {
      splitIndex = remaining.lastIndexOf('\n', maxLength);
    }
    if (splitIndex < maxLength * 0.3) {
      splitIndex = remaining.lastIndexOf(' ', maxLength);
    }
    if (splitIndex < maxLength * 0.3) {
      splitIndex = maxLength;
    }

    chunks.push(remaining.slice(0, splitIndex).trimEnd());
    remaining = remaining.slice(splitIndex).trimStart();
  }

  if (remaining.trim()) {
    chunks.push(remaining.trim());
  }

  return chunks;
}

// ============================================================================
// Allowlist access control
// ============================================================================

/**
 * Check if a sender is allowed to interact with the agent.
 *
 * @param {string} senderId - Sender identifier (phone, user-id, etc.)
 * @param {string[]|null} allowlist - Allowed IDs (null = allow all)
 * @returns {boolean}
 */
export function isAllowed(senderId, allowlist) {
  if (!allowlist || allowlist.length === 0) return true;
  if (allowlist.includes('*')) return true;
  const normalize = (s) => s.replace(/[^\w]/g, '').toLowerCase();
  return allowlist.some((entry) => normalize(entry) === normalize(senderId));
}

// ============================================================================
// Bot commands
// ============================================================================

/**
 * Handle built-in bot commands.
 *
 * @param {string} text         - Raw message text
 * @param {SenderSession} session
 * @param {boolean} allowApply  - Whether write ops are enabled
 * @param {Object}  [opts]
 * @param {Object}  [opts.commerce] - Commerce instance for data commands
 * @param {import('./identity.js').CustomerIdentityStore} [opts.identityStore] - Identity store
 * @param {string}  [opts.channel] - Channel name for identity resolution
 * @param {string}  [opts.senderId] - Sender ID for identity resolution
 * @param {Object}  [opts.autonomousEngine] - Autonomous engine for dynamic commands
 * @returns {Promise<{ handled: boolean, response?: string, richMessage?: import('./rich-messages.js').RichMessage }>}
 */
export async function handleBotCommand(text, session, allowApply, { commerce, identityStore, channel, senderId, autonomousEngine } = {}) {
  const lower = text.toLowerCase().trim();
  const parts = lower.split(/\s+/);
  const cmd = parts[0];

  if (cmd === '/reset' || cmd === '/new') {
    session.sessionId = null;
    session.agent = null;
    return { handled: true, response: 'Session cleared. Starting fresh conversation.' };
  }

  if (cmd === '/help') {
    const baseHelp = [
      'StateSet Commerce Agent',
      '',
      'You can ask me about:',
      '- Orders: "show my recent orders"',
      '- Products: "what products do you have?"',
      '- Cart: "create a cart and add 2 widgets"',
      '- Inventory: "how much stock of WIDGET-001?"',
      '- Returns: "I want to return order #123"',
      '- Analytics: "what are my top sellers?"',
      '',
      'Commands:',
      '/help - Show this message',
      '/reset - Start a new conversation',
      '/status - Show current session',
      '/orders [n] - List last N orders (default 5)',
      '/order <id> - Order detail',
      '/inventory <sku> - Stock levels',
      '/cart [id] - Cart summary or list',
      '/track <order-id> - Shipment tracking',
      '/customers - Customer count',
      '/analytics - Today\'s sales summary',
      '/whoami - Show linked customer identity',
      '/link <email> - Link your identity to a customer',
      '/unlink - Remove identity link',
      '/stats - Bot statistics',
      '/escalate [reason] - Talk to a human agent',
      '/release - Return to AI mode (after escalation)',
      '/think <level> - Set thinking (off|low|medium|high)',
      '/provider <name> - Switch AI provider',
      '/memory - Toggle conversation memory',
      '/skills [category] - List loaded skills',
      '/skill-info <name> - Show skill details',
    ].join('\n');

    // Append dynamically registered commands
    const dynamicHelp = getCommandRegistry().generateHelp();

    return {
      handled: true,
      response: baseHelp + dynamicHelp,
    };
  }

  if (cmd === '/status') {
    return {
      handled: true,
      response: [
        'Session Status',
        `Agent: ${session.agent || 'auto-route'}`,
        `Session: ${session.sessionId ? 'active' : 'none'}`,
        `Mode: ${allowApply ? 'write enabled' : 'preview only'}`,
        `Provider: ${session.provider || 'claude'}`,
        `Thinking: ${session.thinkLevel || 'off'}`,
        `Memory: ${session.memoryEnabled ? 'on' : 'off'}`,
      ].join('\n'),
    };
  }

  // --- v0.2.8: Extended thinking, provider, memory commands ---

  if (cmd === '/think') {
    const level = (parts[1] || '').toLowerCase();
    if (['off', 'low', 'medium', 'med', 'high'].includes(level)) {
      session.thinkLevel = level === 'med' ? 'medium' : level;
      return { handled: true, response: `Extended thinking: ${session.thinkLevel}` };
    }
    return { handled: true, response: `Thinking: ${session.thinkLevel || 'off'}\nUsage: /think off|low|medium|high` };
  }

  if (cmd === '/provider') {
    const p = (parts[1] || '').toLowerCase();
    if (['claude', 'openai', 'gemini', 'ollama'].includes(p)) {
      session.provider = p;
      const note = p !== 'claude' ? '\nNote: Non-Claude providers run in chat-only mode (no tools)' : '';
      return { handled: true, response: `Provider: ${p}${note}` };
    }
    return { handled: true, response: `Provider: ${session.provider || 'claude'}\nUsage: /provider claude|openai|gemini|ollama` };
  }

  if (cmd === '/memory') {
    session.memoryEnabled = !session.memoryEnabled;
    return { handled: true, response: `Memory: ${session.memoryEnabled ? 'on' : 'off'}` };
  }

  // --- Commerce data commands (require commerce instance) ---

  if (cmd === '/orders' && commerce) {
    try {
      const limit = parseInt(parts[1], 10) || 5;
      const orders = await commerce.orders.list();
      const slice = orders.slice(0, limit);
      if (slice.length === 0) {
        return { handled: true, response: 'No orders found.' };
      }
      const richMessage = createOrderList(slice);
      const response = richMessageToPlainText(richMessage);
      return { handled: true, response, richMessage };
    } catch (err) {
      return { handled: true, response: `Error fetching orders: ${err.message}` };
    }
  }

  if (cmd === '/order' && commerce) {
    const orderId = parts[1];
    if (!orderId) return { handled: true, response: 'Usage: /order <id>' };
    try {
      const order = await commerce.orders.get(orderId);
      if (!order) return { handled: true, response: `Order ${orderId} not found.` };
      const richMessage = createOrderSummary(order);
      const response = richMessageToPlainText(richMessage);
      return { handled: true, response, richMessage };
    } catch (err) {
      return { handled: true, response: `Error fetching order: ${err.message}` };
    }
  }

  if (cmd === '/inventory' && commerce) {
    const sku = parts[1];
    if (!sku) return { handled: true, response: 'Usage: /inventory <sku>' };
    try {
      const stock = await commerce.inventory.getStock(sku.toUpperCase());
      if (!stock) return { handled: true, response: `SKU ${sku.toUpperCase()} not found.` };
      const richMessage = createInventoryCard(sku.toUpperCase(), stock);
      const response = richMessageToPlainText(richMessage);
      return { handled: true, response, richMessage };
    } catch (err) {
      return { handled: true, response: `Error fetching inventory: ${err.message}` };
    }
  }

  if (cmd === '/cart' && commerce) {
    const cartId = parts[1];
    try {
      if (cartId) {
        const cart = await commerce.carts.get(cartId);
        if (!cart) return { handled: true, response: `Cart ${cartId} not found.` };
        const richMessage = createCartSummary(cart);
        const response = richMessageToPlainText(richMessage);
        return { handled: true, response, richMessage };
      }
      const carts = await commerce.carts.list();
      const active = carts.filter((c) => c.status === 'active');
      if (active.length === 0) return { handled: true, response: 'No active carts.' };
      const lines = active.slice(0, 10).map((c) =>
        `${c.cartNumber || c.cart_number || c.id}: $${(c.subtotal || 0).toFixed(2)} (${c.items?.length || 0} items)`
      );
      return { handled: true, response: `Active Carts (${active.length}):\n${lines.join('\n')}` };
    } catch (err) {
      return { handled: true, response: `Error fetching cart: ${err.message}` };
    }
  }

  if (cmd === '/track' && commerce) {
    const orderId = parts[1];
    if (!orderId) return { handled: true, response: 'Usage: /track <order-id>' };
    try {
      const order = await commerce.orders.get(orderId);
      if (!order) return { handled: true, response: `Order ${orderId} not found.` };
      const tracking = order.trackingNumber || order.tracking_number;
      if (!tracking) return { handled: true, response: `No tracking number for order ${orderId}. Status: ${(order.status || 'unknown').toUpperCase()}` };
      return { handled: true, response: `Order ${orderId}\nStatus: ${(order.status || 'unknown').toUpperCase()}\nTracking: ${tracking}` };
    } catch (err) {
      return { handled: true, response: `Error fetching tracking: ${err.message}` };
    }
  }

  if (cmd === '/customers' && commerce) {
    try {
      const count = await commerce.customers.count();
      return { handled: true, response: `Total customers: ${count}` };
    } catch (err) {
      return { handled: true, response: `Error fetching customer count: ${err.message}` };
    }
  }

  if (cmd === '/analytics' && commerce) {
    try {
      const summary = await commerce.analytics.salesSummary({ period: 'today' });
      const richMessage = createAnalyticsSummary(summary);
      const response = richMessageToPlainText(richMessage);
      return { handled: true, response, richMessage };
    } catch (err) {
      return { handled: true, response: `Error fetching analytics: ${err.message}` };
    }
  }

  // --- Identity commands ---

  if (cmd === '/whoami' && identityStore && channel && senderId) {
    const link = identityStore.getCustomerId(channel, senderId);
    if (!link) {
      return { handled: true, response: 'No customer identity linked. Use /link <email> to connect your account.' };
    }
    let extra = '';
    if (commerce) {
      try {
        const customer = await commerce.customers.get(link.customerId);
        if (customer) {
          const name = [customer.firstName || customer.first_name, customer.lastName || customer.last_name].filter(Boolean).join(' ');
          extra = `\nName: ${name || 'N/A'}\nEmail: ${customer.email || 'N/A'}`;
        }
      } catch { /* ignore */ }
    }
    return { handled: true, response: `Linked customer: ${link.customerId} (via ${link.linkedBy})${extra}` };
  }

  if (cmd === '/link' && identityStore && channel && senderId && commerce) {
    const email = parts[1];
    if (!email) return { handled: true, response: 'Usage: /link <email>' };
    try {
      const customer = await commerce.customers.getByEmail(email);
      if (!customer) return { handled: true, response: `No customer found with email: ${email}` };
      identityStore.link(channel, senderId, customer.id, 'manual');
      const name = [customer.firstName || customer.first_name, customer.lastName || customer.last_name].filter(Boolean).join(' ');
      return { handled: true, response: `Identity linked to ${name || email} (${customer.id}).` };
    } catch (err) {
      return { handled: true, response: `Error linking identity: ${err.message}` };
    }
  }

  if (cmd === '/unlink' && identityStore && channel && senderId) {
    identityStore.unlink(channel, senderId);
    return { handled: true, response: 'Identity link removed.' };
  }

  // --- Stats command ---

  if (cmd === '/stats') {
    const metrics = getMetrics();
    return { handled: true, response: metrics.formatForDisplay() };
  }

  // --- Handoff commands ---

  if (cmd === '/escalate' && channel && senderId) {
    const handoff = getHandoffQueue();
    if (handoff.isHandedOff(channel, senderId)) {
      return { handled: true, response: 'You are already connected to a human agent. Send messages normally, or type /release to return to AI.' };
    }
    const reason = parts.slice(1).join(' ') || undefined;
    handoff.escalate(channel, senderId, '', reason);
    return { handled: true, response: 'You\'ve been connected to our support team. A human agent will respond shortly.\nType /release to return to the AI assistant.' };
  }

  if (cmd === '/release' && channel && senderId) {
    const handoff = getHandoffQueue();
    const { released } = handoff.release(channel, senderId);
    if (released) {
      return { handled: true, response: 'You\'re back with the AI assistant. How can I help?' };
    }
    return { handled: true, response: 'No active escalation found. You\'re already talking to the AI assistant.' };
  }

  // --- Skills commands ---
  if (cmd === '/skills') {
    try {
      const { getSkillRegistry } = await import('../skills/registry.js');
      const skillRegistry = getSkillRegistry();
      const category = parts[1] || null;
      const skills = category ? skillRegistry.listByCategory(category) : skillRegistry.list();
      if (skills.length === 0) {
        return { handled: true, response: category ? `No skills in category "${category}".` : 'No skills loaded.' };
      }
      const lines = skills.map((s) => `- ${s.name}: ${s.description.slice(0, 80)}`);
      const header = category ? `Skills (${category}):` : `Skills (${skills.length}):`;
      return { handled: true, response: [header, ...lines].join('\n') };
    } catch {
      return { handled: true, response: 'Skill system not available.' };
    }
  }

  if (cmd === '/skill-info') {
    const name = parts[1];
    if (!name) return { handled: true, response: 'Usage: /skill-info <skill-name>' };
    try {
      const { getSkillRegistry } = await import('../skills/registry.js');
      const skill = getSkillRegistry().get(name);
      if (!skill) return { handled: true, response: `Skill "${name}" not found.` };
      const info = [
        `Skill: ${skill.name}`,
        `Description: ${skill.description}`,
        `Category: ${skill.category}`,
        `Origin: ${skill.origin}`,
        `Tags: ${skill.tags.join(', ')}`,
        `References: ${skill.hasReferences ? 'yes' : 'no'}`,
        `Scripts: ${skill.hasScripts ? 'yes' : 'no'}`,
      ];
      return { handled: true, response: info.join('\n') };
    } catch {
      return { handled: true, response: 'Skill system not available.' };
    }
  }

  // --- Dynamic command registry lookup ---
  const registry = getCommandRegistry();
  const cmdName = cmd.startsWith('/') ? cmd.slice(1) : cmd;
  if (registry.has(cmdName)) {
    const def = registry.get(cmdName);
    const argText = parts.slice(1).join(' ');
    try {
      const result = await def.handler(argText, {
        senderId, channel, session, allowApply, commerce, identityStore, autonomousEngine,
      });
      return { handled: true, response: result.response, richMessage: result.richMessage };
    } catch (err) {
      return { handled: true, response: `Error: ${err.message}` };
    }
  }

  return { handled: false };
}

// ============================================================================
// Agent processing
// ============================================================================

/**
 * Run the commerce agent on a message and return the result.
 *
 * @param {string} text
 * @param {SenderSession} session
 * @param {Object} opts
 * @param {string}  opts.dbPath
 * @param {boolean} opts.allowApply
 * @param {string}  [opts.model]
 * @param {number}  [opts.maxTurns]
 * @param {string}  [opts.agent]
 * @param {boolean} [opts.verbose]
 * @returns {Promise<{ response: string, agent?: string }>}
 */
export async function processWithAgent(text, session, opts) {
  const { dbPath, allowApply, model, maxTurns = 10, agent, verbose } = opts;

  // Resolve extended thinking level from session override or shared config
  const thinkLevel = session.thinkLevel || opts.thinkLevel || 'off';

  // Resolve provider from session override or shared config
  const provider = session.provider || opts.provider || 'claude';

  // If using a non-Claude provider, use the fallback chain (chat-only mode)
  if (provider !== 'claude') {
    try {
      const { getFallbackChain } = await import('../providers/base.js');
      const chain = getFallbackChain({ verbose });
      const messages = [
        { role: 'system', content: 'You are StateSet iCommerce, an AI-powered commerce assistant. Help the user with their commerce operations.' },
        { role: 'user', content: text },
      ];
      const result = await chain.chat(messages, { preferredProvider: provider });
      return { response: result.text, agent: 'chat-only', provider: result.provider };
    } catch (err) {
      // If fallback fails too, return the error
      return { response: `Provider ${provider} failed: ${err.message}`, agent: 'error' };
    }
  }

  // Primary path: Claude Agent SDK with full MCP tools + extended thinking
  try {
    const result = await runAgentLoop({
      request: text,
      dbPath,
      model,
      allowApply,
      maxTurns,
      resumeSessionId: session.sessionId,
      agent: agent || session.agent,
      verbose,
      thinkLevel,
    });

    if (result.sessionId) session.sessionId = result.sessionId;
    if (result.agent) session.agent = result.agent;

    const response = result.response?.trim() || 'I processed your request but have no text response.';
    return { response, agent: result.agent };
  } catch (err) {
    // Claude failed — try automatic fallback if enabled
    if (opts.enableFallback !== false) {
      try {
        const { getFallbackChain } = await import('../providers/base.js');
        const chain = getFallbackChain({ verbose });
        const messages = [
          { role: 'system', content: 'You are StateSet iCommerce, an AI-powered commerce assistant. Help the user with their commerce operations. Note: advanced commerce tools are temporarily unavailable.' },
          { role: 'user', content: text },
        ];
        const result = await chain.chat(messages);
        if (verbose) {
          console.log(`[Gateway] Claude failed, fell back to ${result.provider}`);
        }
        return { response: result.text, agent: 'chat-only', provider: result.provider, failedOver: true };
      } catch {
        // All providers failed
      }
    }
    const response = `Sorry, I encountered an error: ${err.message}`;
    return { response, agent: 'error' };
  }
}

// ============================================================================
// Reconnect / backoff utilities
// ============================================================================

export const RECONNECT_POLICY = {
  initialMs: 2_000,
  maxMs: 30_000,
  factor: 1.8,
  jitter: 0.25,
  maxAttempts: 12,
};

/**
 * Compute backoff delay for a given attempt number.
 */
export function computeBackoff(policy, attempt) {
  const base = policy.initialMs * Math.pow(policy.factor, attempt - 1);
  const clamped = Math.min(base, policy.maxMs);
  const jitter = 1 + (Math.random() * 2 - 1) * policy.jitter;
  return Math.round(clamped * jitter);
}

export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ============================================================================
// Message handler factory
// ============================================================================

/**
 * @typedef {Object} ChannelAdapter
 * @property {(raw: any) => string|null} extractText
 * @property {(raw: any) => string}      getSenderId
 * @property {(raw: any) => string}      getTargetId
 * @property {(raw: any) => boolean}     isOwnMessage
 * @property {(targetId: string, text: string) => Promise<void>} send
 * @property {((targetId: string) => Promise<void>)|null} sendTyping
 * @property {(text: string) => string}  formatForPlatform
 * @property {number}                    maxMessageLength
 * @property {((targetId: string, richMsg: import('./rich-messages.js').RichMessage) => Promise<void>)} [sendRichMessage]
 */

/**
 * Create a full message-handling pipeline from a channel adapter.
 *
 * The returned function should be called for every incoming raw message.
 * It handles: extractText -> skipOwn -> allowlist -> BOT_PREFIX check ->
 *   middleware -> bot commands -> queue -> agent -> chunk -> format -> send
 *
 * @param {ChannelAdapter} adapter
 * @param {Object} opts
 * @param {Function} opts.getSession   - (senderId) => SenderSession
 * @param {Function} [opts.persistSession] - (senderId, session) => void
 * @param {string}   opts.dbPath
 * @param {boolean}  opts.allowApply
 * @param {string}   [opts.model]
 * @param {number}   [opts.maxTurns]
 * @param {string}   [opts.agent]
 * @param {boolean}  [opts.verbose]
 * @param {string[]|null} [opts.allowlist]
 * @param {Function[]} [opts.middleware] - Middleware stack
 * @param {string}   [opts.channel]     - Channel name for middleware context
 * @param {import('./identity.js').CustomerIdentityStore} [opts.identityStore] - Identity store
 * @param {Object}   [opts.autonomousEngine] - Autonomous engine for dynamic commands
 * @returns {(raw: any) => Promise<void>}
 */
export function createMessageHandler(adapter, opts) {
  const {
    getSession,
    persistSession,
    dbPath,
    allowApply,
    model,
    maxTurns = 10,
    agent,
    verbose = false,
    allowlist = null,
    middleware = [],
    channel = 'unknown',
    identityStore = null,
    autonomousEngine = null,
    thinkLevel = 'off',
    provider = 'claude',
    enableFallback = true,
  } = opts;

  // Lazy-init commerce for bot commands
  let _commerce = null;
  async function ensureCommerce() {
    if (_commerce) return _commerce;
    try {
      const pkg = await import('@stateset/embedded');
      const Commerce = pkg.Commerce || pkg.default?.Commerce;
      if (Commerce) {
        _commerce = new Commerce(dbPath);
      }
    } catch {
      // @stateset/embedded not available or dbPath issue — commerce commands will be unavailable
    }
    return _commerce;
  }

  return async function onMessage(raw) {
    // 1. Extract text
    const text = adapter.extractText(raw);
    if (!text) return;

    // 2. Skip own messages
    if (adapter.isOwnMessage(raw)) return;

    // 3. Allowlist
    const senderId = adapter.getSenderId(raw);
    if (!isAllowed(senderId, allowlist)) {
      if (verbose) console.log(`Blocked message from unauthorized sender: ${senderId}`);
      return;
    }

    // 4. Skip bot prefix (self-reply loop prevention)
    if (text.startsWith(BOT_PREFIX)) return;

    const targetId = adapter.getTargetId(raw);

    console.log(`[${new Date().toISOString()}] ${senderId}: ${text.slice(0, 100)}${text.length > 100 ? '...' : ''}`);

    // 5. Run middleware pipeline
    if (middleware.length > 0) {
      const session = getSession(senderId);
      const ctx = {
        text,
        senderId,
        targetId,
        session,
        raw,
        channel,
        metadata: {},
        blocked: false,
        blockReason: null,
      };

      await runMiddleware(middleware, ctx);

      if (ctx.blocked) {
        if (verbose) console.log(`Message blocked by middleware for ${senderId}: ${ctx.blockReason}`);
        return;
      }
    }

    // 6. Session + queue
    const session = getSession(senderId);

    if (session.processing) {
      session.queue.push(text);
      if (verbose) console.log(`Queued message from ${senderId} (${session.queue.length} in queue)`);
      return;
    }

    session.processing = true;

    // Lazy-load commerce for data commands
    const commerce = await ensureCommerce();

    try {
      await processSingle(adapter, targetId, senderId, text, session, {
        dbPath, allowApply, model, maxTurns, agent, verbose, commerce, persistSession, channel, identityStore, autonomousEngine,
      });

      // Drain the queue
      while (session.queue.length > 0) {
        const queued = session.queue.shift();
        await processSingle(adapter, targetId, senderId, queued, session, {
          dbPath, allowApply, model, maxTurns, agent, verbose, commerce, persistSession, channel, identityStore, autonomousEngine,
        });
      }
    } finally {
      session.processing = false;
    }
  };
}

/**
 * Process a single message through the agent and send back the response.
 * @private
 */
async function processSingle(adapter, targetId, senderId, text, session, opts) {
  const { dbPath, allowApply, model, maxTurns, agent, verbose, commerce, persistSession, channel, identityStore, autonomousEngine } = opts;
  const startTime = Date.now();
  const metrics = getMetrics();
  const hookRunner = getPluginRegistry().getHookRunner();

  // Fire message_received hook (parallel, fire-and-forget)
  hookRunner.run('message_received', { text, senderId, channel });

  // Typing indicator
  if (adapter.sendTyping) {
    try {
      await adapter.sendTyping(targetId);
    } catch { /* ignore */ }
  }

  // Handoff check — if conversation is escalated to a human, forward to ops instead of AI
  if (channel) {
    const handoff = getHandoffQueue();
    if (handoff.isHandedOff(channel, senderId)) {
      handoff.recordMessage(channel, senderId, text);
      await adapter.send(targetId, BOT_PREFIX + 'Your message has been forwarded to a human agent. Please wait for a response.');
      return;
    }
  }

  // Bot commands (now async to support commerce data commands + identity)
  const cmd = await handleBotCommand(text, session, allowApply, { commerce, identityStore, channel, senderId, autonomousEngine });
  if (cmd.handled) {
    // Record command usage
    const cmdName = text.toLowerCase().trim().split(/\s+/)[0];
    metrics.recordCommand(cmdName);

    // Send rich message if adapter supports it and command returned one
    if (cmd.richMessage && adapter.sendRichMessage) {
      try {
        await adapter.sendRichMessage(targetId, cmd.richMessage);
        metrics.recordResponse(channel || 'unknown', Date.now() - startTime);
        hookRunner.run('message_sent', { text: cmd.response, senderId, channel });
        return;
      } catch {
        // Fall through to plain text
      }
    }

    const formatted = adapter.formatForPlatform(cmd.response);
    await adapter.send(targetId, BOT_PREFIX + formatted);
    metrics.recordResponse(channel || 'unknown', Date.now() - startTime);
    hookRunner.run('message_sent', { text: cmd.response, senderId, channel });

    // Persist session after bot command (e.g. /reset clears sessionId)
    if (persistSession) persistSession(senderId, session);
    return;
  }

  // Agent processing
  try {
    // Fire before_agent_start hook (sequential, can modify text)
    let processedText = text;
    if (hookRunner.hasHooks('before_agent_start')) {
      const hookResult = await hookRunner.run('before_agent_start', { text, session });
      if (hookResult.text) processedText = hookResult.text;
    }

    const result = await processWithAgent(processedText, session, {
      dbPath, allowApply, model, maxTurns, agent, verbose,
      thinkLevel, provider, enableFallback,
    });

    // Fire agent_end hook (parallel)
    hookRunner.run('agent_end', { response: result.response, agent: result.agent });

    // Fire message_sending hook (sequential, can modify response)
    let finalResponse = result.response;
    if (hookRunner.hasHooks('message_sending')) {
      const sendingResult = await hookRunner.run('message_sending', { text: result.response });
      if (sendingResult.text) finalResponse = sendingResult.text;
    }

    const formatted = adapter.formatForPlatform(finalResponse);
    const chunks = chunkMessage(formatted, adapter.maxMessageLength);

    for (const chunk of chunks) {
      await adapter.send(targetId, BOT_PREFIX + chunk);
    }

    // Fire message_sent hook (parallel)
    hookRunner.run('message_sent', { text: finalResponse, senderId, channel });

    metrics.recordResponse(channel || 'unknown', Date.now() - startTime);

    // Persist session after agent updates sessionId/agent
    if (persistSession) persistSession(senderId, session);

    console.log(`[${new Date().toISOString()}] Replied to ${senderId} (${finalResponse.length} chars, agent: ${result.agent})`);
  } catch (err) {
    metrics.recordError(channel || 'unknown');
    console.error(`Agent error for ${senderId}:`, err.message);
    await adapter.send(targetId, BOT_PREFIX + 'Sorry, I encountered an error processing your request. Please try again.');
  }
}
