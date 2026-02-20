/**
 * Microsoft Teams Gateway for StateSet iCommerce
 *
 * Bridges Microsoft Teams messages to the StateSet commerce agent via
 * the Bot Framework REST API. Uses a lightweight HTTP webhook approach
 * (no Bot Framework SDK dependency) with Node's built-in http module
 * and fetch for outbound API calls.
 *
 * Each Teams user gets their own agent session for multi-turn conversations.
 *
 * Requires:
 * - Azure Bot registration with Messaging Endpoint set to this server's URL
 * - TEAMS_APP_ID and TEAMS_APP_PASSWORD environment variables
 */

import http from 'node:http';
import { createSessionManager, createMessageHandler, BOT_PREFIX } from '../channels/base.js';
import { getNotifier } from '../channels/notifier.js';
import { richMessageToPlainText } from '../channels/rich-messages.js';
import { isSafeDisplayUrl } from '../utils/url-validator.js';

// ============================================================================
// Bot Framework REST helpers
// ============================================================================

const TOKEN_ENDPOINT = 'https://login.microsoftonline.com/botframework.com/oauth2/v2.0/token';
const TOKEN_SCOPE = 'https://api.botframework.com/.default';

/** Cached OAuth token and expiry. */
let _cachedToken = null;
let _tokenExpiresAt = 0;

/**
 * Obtain a Bot Framework OAuth token (with caching).
 *
 * @param {string} appId
 * @param {string} appPassword
 * @returns {Promise<string>}
 */
async function getBotToken(appId, appPassword) {
  // Return cached token if still valid (with 5-minute buffer)
  if (_cachedToken && Date.now() < _tokenExpiresAt - 5 * 60 * 1000) {
    return _cachedToken;
  }

  const body = new URLSearchParams({
    grant_type: 'client_credentials',
    client_id: appId,
    client_secret: appPassword,
    scope: TOKEN_SCOPE,
  });

  const res = await fetch(TOKEN_ENDPOINT, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: body.toString(),
  });

  if (!res.ok) {
    const text = await res.text();
    throw new Error(`Bot Framework token request failed (${res.status}): ${text}`);
  }

  const data = await res.json();
  _cachedToken = data.access_token;
  _tokenExpiresAt = Date.now() + (data.expires_in || 3600) * 1000;

  return _cachedToken;
}

/**
 * Send an activity to a conversation via the Bot Framework REST API.
 *
 * @param {string} serviceUrl - The serviceUrl from the incoming activity
 * @param {string} conversationId - Conversation ID
 * @param {string} activityId - The inbound activity ID (for reply-to-activity), or null
 * @param {Object} activity - The outbound activity payload
 * @param {string} appId
 * @param {string} appPassword
 * @returns {Promise<Object>}
 */
async function sendActivity(serviceUrl, conversationId, activityId, activity, appId, appPassword) {
  const token = await getBotToken(appId, appPassword);

  // Normalize serviceUrl (strip trailing slash)
  const base = serviceUrl.replace(/\/+$/, '');

  // Use replyToActivity if we have an activityId, otherwise sendToConversation
  const url = activityId
    ? `${base}/v3/conversations/${encodeURIComponent(conversationId)}/activities/${encodeURIComponent(activityId)}`
    : `${base}/v3/conversations/${encodeURIComponent(conversationId)}/activities`;

  const res = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify(activity),
  });

  if (!res.ok) {
    const text = await res.text();
    throw new Error(`Bot Framework send failed (${res.status}): ${text}`);
  }

  // 201 Created or 200 OK — may have empty body
  const contentType = res.headers.get('content-type') || '';
  if (contentType.includes('application/json')) {
    return res.json();
  }
  return {};
}

// ============================================================================
// HTTP request helpers
// ============================================================================

/**
 * Parse JSON body from an incoming HTTP request.
 *
 * @param {http.IncomingMessage} req
 * @returns {Promise<Object>}
 */
function parseBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on('data', (chunk) => chunks.push(chunk));
    req.on('end', () => {
      try {
        const raw = Buffer.concat(chunks).toString('utf-8');
        resolve(raw ? JSON.parse(raw) : {});
      } catch (err) {
        reject(new Error(`Invalid JSON body: ${err.message}`));
      }
    });
    req.on('error', reject);
  });
}

/**
 * Send a JSON HTTP response.
 *
 * @param {http.ServerResponse} res
 * @param {number} status
 * @param {Object} [data]
 */
function sendJson(res, status, data) {
  const body = data ? JSON.stringify(data) : '';
  res.writeHead(status, {
    'Content-Type': 'application/json',
    'Content-Length': Buffer.byteLength(body),
  });
  res.end(body);
}

// ============================================================================
// Teams channel adapter
// ============================================================================

/**
 * Format text for Microsoft Teams.
 *
 * Teams supports a subset of HTML and markdown. We convert standard
 * markdown to Teams-friendly formatting.
 *
 * @param {string} text
 * @returns {string}
 */
function formatForTeams(text) {
  return (
    text
      // Convert markdown headers to bold
      .replace(/^#{1,6}\s+(.+)$/gm, '**$1**')
      // Convert markdown code blocks to Teams code format (triple backticks work)
      // Convert markdown horizontal rules
      .replace(/^---+$/gm, '---')
    // Convert markdown links [text](url) — Teams supports this natively
  );
}

// ============================================================================
// Gateway
// ============================================================================

/**
 * Start the Microsoft Teams gateway.
 *
 * @param {Object} options
 * @param {import('../channels/session-store.js').ChannelSessionStore} [options.sessionStore]
 * @param {import('../channels/identity.js').CustomerIdentityStore} [options.identityStore]
 * @param {Function[]} [options.middleware]
 * @param {string}   [options.dbPath='./store.db']
 * @param {boolean}  [options.allowApply=true]
 * @param {string}   [options.model]
 * @param {number}   [options.maxTurns=10]
 * @param {string}   [options.agent]
 * @param {boolean}  [options.verbose=false]
 * @param {string[]|null} [options.allowlist=null]
 * @param {number}   [options.webhookPort=3978]
 * @param {Object}   [options.autonomousEngine]
 * @returns {Promise<{ shutdown: () => Promise<void> }>}
 */
export async function startTeamsGateway({
  sessionStore,
  identityStore,
  middleware = [],
  dbPath = './store.db',
  allowApply = true,
  model,
  maxTurns = 10,
  agent,
  verbose = false,
  allowlist = null,
  webhookPort = 3978,
  autonomousEngine,
} = {}) {
  const appId = process.env.TEAMS_APP_ID;
  const appPassword = process.env.TEAMS_APP_PASSWORD;

  if (!appId) {
    throw new Error(
      'TEAMS_APP_ID environment variable is required.\n' +
        'Register a bot at https://dev.botframework.com or via Azure Bot Service.\n' +
        'Set TEAMS_APP_ID to the Microsoft App ID from your bot registration.',
    );
  }
  if (!appPassword) {
    throw new Error(
      'TEAMS_APP_PASSWORD environment variable is required.\n' +
        'Set TEAMS_APP_PASSWORD to the client secret from your Azure AD app registration.',
    );
  }

  console.log('Starting StateSet Microsoft Teams Gateway...');

  // ---- Session management ----
  const sessionManager = createSessionManager({ store: sessionStore, channel: 'teams' });
  const cleanupHandle = sessionManager.startCleanup();

  // ---- Conversation reference cache ----
  // Maps conversationId to { serviceUrl, conversationId, activityId } for proactive messaging.
  /** @type {Map<string, Object>} */
  const conversationRefs = new Map();

  /**
   * Build or retrieve a conversation reference from an activity.
   *
   * @param {Object} activity - Bot Framework activity
   * @returns {Object} Conversation reference object
   */
  function getConversationRef(activity) {
    const ref = {
      serviceUrl: activity.serviceUrl,
      conversationId: activity.conversation?.id,
      activityId: activity.id,
      channelId: activity.channelId,
      recipientId: activity.from?.id,
      recipientName: activity.from?.name,
      botId: activity.recipient?.id,
      botName: activity.recipient?.name,
    };

    // Cache for proactive/notification use
    if (ref.conversationId) {
      conversationRefs.set(ref.conversationId, ref);
    }

    return ref;
  }

  // ---- Channel adapter ----

  /** @type {import('../channels/base.js').ChannelAdapter} */
  const adapter = {
    /**
     * Extract the text content from a Bot Framework activity.
     */
    extractText(activity) {
      if (activity.type !== 'message') return null;

      let text = activity.text || '';

      // Strip bot mentions (Teams includes <at>BotName</at> in group chats)
      text = text.replace(/<at>.*?<\/at>/gi, '').trim();

      // Strip leading whitespace after mention removal
      text = text.replace(/^\s+/, '');

      return text || null;
    },

    /**
     * Get the sender's unique identifier.
     * Prefer AAD Object ID for consistent identity; fall back to name.
     */
    getSenderId(activity) {
      return activity.from?.aadObjectId || activity.from?.id || activity.from?.name || 'unknown';
    },

    /**
     * Get the conversation reference (serialized as JSON) for routing replies.
     * We use the conversation ID as the target identifier, but store the full
     * reference in the cache for actual API calls.
     */
    getTargetId(activity) {
      // Ensure the reference is cached
      getConversationRef(activity);
      return activity.conversation?.id || '';
    },

    /**
     * Check if an activity was sent by the bot itself.
     */
    isOwnMessage(activity) {
      if (activity.from?.id === appId) return true;
      if (activity.from?.name === 'Bot' && activity.from?.role === 'bot') return true;
      return false;
    },

    /**
     * Send a text reply to a conversation.
     *
     * @param {string} conversationId - The conversation ID
     * @param {string} text - Message text
     */
    async send(conversationId, text) {
      const ref = conversationRefs.get(conversationId);
      if (!ref) {
        console.error(`[Teams] No conversation reference found for ${conversationId}`);
        return;
      }

      const replyActivity = {
        type: 'message',
        text,
        textFormat: 'markdown',
      };

      await sendActivity(
        ref.serviceUrl,
        ref.conversationId,
        ref.activityId,
        replyActivity,
        appId,
        appPassword,
      );
    },

    /**
     * Send a typing indicator.
     *
     * @param {string} conversationId
     */
    async sendTyping(conversationId) {
      const ref = conversationRefs.get(conversationId);
      if (!ref) return;

      const typingActivity = {
        type: 'typing',
      };

      try {
        await sendActivity(
          ref.serviceUrl,
          ref.conversationId,
          ref.activityId,
          typingActivity,
          appId,
          appPassword,
        );
      } catch (err) {
        console.debug('[teams] Typing indicator failed (non-critical):', err.message || err);
      }
    },

    /**
     * Format text for the Teams platform.
     */
    formatForPlatform: formatForTeams,

    /**
     * Teams supports messages up to ~28 KB.
     */
    maxMessageLength: 28000,

    /**
     * Send a rich message using Teams Adaptive Card format.
     *
     * @param {string} conversationId
     * @param {import('../channels/rich-messages.js').RichMessage} richMsg
     */
    async sendRichMessage(conversationId, richMsg) {
      const ref = conversationRefs.get(conversationId);
      if (!ref) {
        console.error(`[Teams] No conversation reference for rich message to ${conversationId}`);
        return;
      }

      // Build Adaptive Card
      const cardBody = [];

      // Title
      cardBody.push({
        type: 'TextBlock',
        text: richMsg.title,
        weight: 'Bolder',
        size: 'Medium',
        wrap: true,
      });

      // Description
      if (richMsg.description) {
        cardBody.push({
          type: 'TextBlock',
          text: richMsg.description,
          wrap: true,
        });
      }

      // Fields as fact set
      if (richMsg.fields && richMsg.fields.length > 0) {
        cardBody.push({
          type: 'FactSet',
          facts: richMsg.fields.map((f) => ({
            title: f.name,
            value: String(f.value),
          })),
        });
      }

      // Footer
      if (richMsg.footer) {
        cardBody.push({
          type: 'TextBlock',
          text: richMsg.footer,
          isSubtle: true,
          size: 'Small',
          wrap: true,
        });
      }

      // Actions from buttons
      const actions = [];
      if (richMsg.buttons && richMsg.buttons.length > 0) {
        for (const btn of richMsg.buttons.slice(0, 6)) {
          if (btn.url && isSafeDisplayUrl(btn.url)) {
            actions.push({
              type: 'Action.OpenUrl',
              title: btn.label,
              url: btn.url,
            });
          } else {
            actions.push({
              type: 'Action.Submit',
              title: btn.label,
              data: { action: btn.action || btn.label },
            });
          }
        }
      }

      const card = {
        type: 'AdaptiveCard',
        $schema: 'http://adaptivecards.io/schemas/adaptive-card.json',
        version: '1.4',
        body: cardBody,
      };

      if (actions.length > 0) {
        card.actions = actions;
      }

      const replyActivity = {
        type: 'message',
        attachments: [
          {
            contentType: 'application/vnd.microsoft.card.adaptive',
            content: card,
          },
        ],
        // Plain text fallback for notifications/search
        text: richMessageToPlainText(richMsg),
      };

      await sendActivity(
        ref.serviceUrl,
        ref.conversationId,
        ref.activityId,
        replyActivity,
        appId,
        appPassword,
      );
    },
  };

  // ---- Message handler ----

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
    channel: 'teams',
    identityStore: identityStore || null,
    autonomousEngine: autonomousEngine || null,
  });

  // ---- HTTP webhook server ----

  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, `http://${req.headers.host || 'localhost'}`);
    const pathname = url.pathname;
    const method = req.method.toUpperCase();

    // Health check
    if (method === 'GET' && pathname === '/api/health') {
      sendJson(res, 200, { status: 'ok', channel: 'teams', timestamp: new Date().toISOString() });
      return;
    }

    // Bot Framework messages endpoint
    if (method === 'POST' && pathname === '/api/messages') {
      let activity;
      try {
        activity = await parseBody(req);
      } catch (err) {
        console.error('[Teams] Failed to parse request body:', err.message);
        sendJson(res, 400, { error: 'Invalid request body' });
        return;
      }

      if (verbose) {
        console.log(
          `[Teams] Received activity type=${activity.type} from=${activity.from?.name || activity.from?.id || 'unknown'}`,
        );
      }

      // Acknowledge receipt immediately (Bot Framework expects 200 within seconds)
      sendJson(res, 200, {});

      // Process asynchronously
      try {
        switch (activity.type) {
          case 'message':
            await handleMessage(activity);
            break;

          case 'invoke': {
            // Handle Adaptive Card Action.Submit invokes
            const invokeValue = activity.value;
            if (invokeValue && invokeValue.action) {
              const syntheticActivity = {
                ...activity,
                type: 'message',
                text: actionToCommand(invokeValue.action),
              };
              await handleMessage(syntheticActivity);
            }
            break;
          }

          case 'conversationUpdate': {
            // Greet new members added to the conversation
            const membersAdded = activity.membersAdded || [];
            for (const member of membersAdded) {
              // Don't greet the bot itself
              if (member.id === activity.recipient?.id) continue;

              const ref = getConversationRef(activity);
              const greeting =
                BOT_PREFIX +
                "Hello! I'm the StateSet Commerce Assistant. " +
                'Ask me about orders, products, inventory, returns, and more. ' +
                'Type /help to see available commands.';

              try {
                await sendActivity(
                  ref.serviceUrl,
                  ref.conversationId,
                  null,
                  { type: 'message', text: greeting, textFormat: 'markdown' },
                  appId,
                  appPassword,
                );
              } catch (err) {
                console.error('[Teams] Failed to send welcome message:', err.message);
              }
            }
            break;
          }

          default:
            if (verbose) {
              console.log(`[Teams] Ignoring activity type: ${activity.type}`);
            }
            break;
        }
      } catch (err) {
        console.error('[Teams] Error processing activity:', err.message);
        if (verbose) console.error(err);
      }
      return;
    }

    // 404 for unknown routes
    sendJson(res, 404, { error: 'Not found', path: pathname });
  });

  // Start listening
  await new Promise((resolve, reject) => {
    server.listen(webhookPort, () => {
      const addr = server.address();
      console.log(`Teams webhook server listening on port ${addr.port}`);
      console.log(`Messaging endpoint: http://localhost:${addr.port}/api/messages`);
      console.log('Teams gateway is ready for messages.');
      resolve();
    });
    server.on('error', (err) => {
      console.error('[Teams] Server error:', err.message);
      reject(err);
    });
  });

  // ---- Register with notifier ----

  getNotifier().registerChannel('teams', {
    send: adapter.send,
    sendRichMessage: adapter.sendRichMessage,
    formatForPlatform: adapter.formatForPlatform,
  });

  // ---- Shutdown ----

  const shutdown = async () => {
    getNotifier().unregisterChannel('teams');
    sessionManager.stopCleanup(cleanupHandle);
    conversationRefs.clear();

    await new Promise((resolve) => {
      server.close(() => {
        console.log('Teams gateway shut down.');
        resolve();
      });
    });
  };

  return { shutdown };
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Convert an Adaptive Card Action.Submit value to a bot command.
 *
 * @param {string} action - The action string from card submit
 * @returns {string}
 */
function actionToCommand(action) {
  if (!action) return '';

  // Direct command passthrough (starts with /)
  if (action.startsWith('/')) return action;

  // Map common action patterns
  const patterns = [
    { match: /^view_order[_:\s](.+)$/i, cmd: '/order' },
    { match: /^view_cart[_:\s](.+)$/i, cmd: '/cart' },
    { match: /^track[_:\s](.+)$/i, cmd: '/track' },
    { match: /^inventory[_:\s](.+)$/i, cmd: '/inventory' },
  ];

  for (const { match, cmd } of patterns) {
    const m = action.match(match);
    if (m) return `${cmd} ${m[1]}`;
  }

  // Treat as plain text query
  return action;
}
