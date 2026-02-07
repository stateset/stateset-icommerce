/**
 * Example Plugin: Slack Webhook Integration
 * 
 * This plugin demonstrates:
 * - HTTP route registration for webhook endpoints
 * - Background service for periodic data sync
 * - Configuration with schema validation
 * - Message transformation hooks
 * 
 * Features:
 * - POST /webhooks/slack to receive Slack events
 * - Pushes Slack messages to agent conversations
 * - Background task syncs agent responses back to Slack
 */

export default function init(api, context) {
  const { config, manifest, runtime } = context;

  console.log(`[slack-webhook] Loading plugin v${manifest.version}`);

  // ============================================================================
  // Configuration
  // ============================================================================

  const webhookPath = config.webhookPath || '/webhooks/slack';
  const slackToken = config.slackToken;
  const slackChannel = config.slackChannel;
  const syncInterval = config.syncInterval || 60000; // 1 minute

  if (!slackToken) {
    console.warn('[slack-webhook] No slackToken configured; plugin will only log, not send');
  }

  // ========================================================================
  // HTTP Routes
  // ========================================================================

  api.registerHttpRoute({
    method: 'POST',
    path: webhookPath,
    level: 'none', // public webhook endpoint; validate Slack token/signature inside the handler
    handler: async ({ body }) => {
      try {
        const payload = body || {};

        // Verify Slack token if provided
        if (slackToken && payload.token !== slackToken) {
          return { status: 401, body: { error: 'Invalid token' } };
        }

        // Handle different event types
        if (payload.type === 'url_verification') {
          // Slack challenge
          return { status: 200, body: { challenge: payload.challenge } };
        }

        if (payload.type === 'event_callback') {
          const event = payload.event;

          // Only handle message events
          if (event.type === 'message' && !event.subtype && !event.bot_id) {
            // Store the message for delivery to agents
            const slackMessage = {
              channel: event.channel,
              user: event.user,
              text: event.text,
              ts: event.ts,
              thread_ts: event.thread_ts,
            };

            // Add to pending messages queue
            pendingMessages.push(slackMessage);
            console.log(`[slack-webhook] Received message from user ${event.user}: ${event.text.substring(0, 50)}...`);
          }
        }

        return { status: 200, body: { status: 'ok' } };
      } catch (err) {
        console.error('[slack-webhook] Webhook error:', err);
        return { status: 500, body: { error: 'Internal server error' } };
      }
    }
  });

  // ========================================================================
  // Message Hooks
  // ========================================================================

  // Track pending messages from Slack
  const pendingMessages = [];

  // Track messages sent to Slack (for thread replies)
  const sentMessages = new Map();

  // Hook: Transform incoming messages from Slack
  api.on('message_received', async (data) => {
    // Check if this is a Slack message
    if (data.source && data.source.startsWith('slack')) {
      // Add metadata for the agent
      data.metadata = data.metadata || {};
      data.metadata.slackChannel = data.channel;
      data.metadata.slackUser = data.user;
      data.metadata.slackThread = data.thread_ts;

      // Format the message for the agent
      data.content = `(From Slack user ${data.user}): ${data.content}`;
    }

    return data;
  });

  // Hook: Track outgoing messages for thread replies
  api.on('message_sent', async (data) => {
    if (data.metadata && data.metadata.slackChannel) {
      sentMessages.set(data.id, {
        channel: data.metadata.slackChannel,
        thread: data.metadata.slackThread,
        timestamp: Date.now(),
      });
      console.log(`[slack-webhook] Tracking message for thread reply: ${data.id}`);
    }
  });

  // ========================================================================
  // Background Services
  // ========================================================================

  // Service: Process pending messages
  const messageProcessor = {
    name: 'slack-webhook-processor',
    async start() {
      console.log('[slack-webhook-processor] Starting message processor');

      this.interval = setInterval(async () => {
        if (pendingMessages.length === 0) return;

        const message = pendingMessages.shift();

        // In a real implementation, this would deliver the message
        // to the appropriate agent conversation
        console.log(`[slack-webhook-processor] Delivering message to agent:`);
        console.log(`  Channel: ${message.channel}`);
        console.log(`  User: ${message.user}`);
        console.log(`  Text: ${message.text}`);

        // Trigger the message_received hook to send to agent
        try {
          await api._registry.getHookRunner().run('message_received', {
            content: message.text,
            source: `slack:${message.channel}`,
            metadata: {
              slackChannel: message.channel,
              slackUser: message.user,
              slackThread: message.thread_ts,
            },
          });
        } catch (err) {
          console.error('[slack-webhook-processor] Error delivering message:', err);
        }
      }, 1000); // Check every second
    },

    async stop() {
      console.log('[slack-webhook-processor] Stopping message processor');
      if (this.interval) {
        clearInterval(this.interval);
        this.interval = null;
      }
    }
  };

  // Service: Sync responses back to Slack (mock implementation)
  const slackSyncService = {
    name: 'slack-webhook-sync',
    async start() {
      console.log('[slack-webhook-sync] Starting Slack sync service');

      this.interval = setInterval(() => {
        // In a real implementation, this would:
        // 1. Query for recent agent responses
        // 2. Match them to sentMessages map
        // 3. Send replies to Slack via API
        
        if (sentMessages.size === 0) return;

        console.log(`[slack-webhook-sync] Would sync ${sentMessages.size} messages to Slack`);
        
        // Clean up old messages
        const now = Date.now();
        for (const [id, msg] of sentMessages.entries()) {
          if (now - msg.timestamp > 300000) { // 5 minutes
            sentMessages.delete(id);
          }
        }
      }, syncInterval);
    },

    async stop() {
      console.log('[slack-webhook-sync] Stopping Slack sync service');
      if (this.interval) {
        clearInterval(this.interval);
        this.interval = null;
      }
    }
  };

  api.registerService(messageProcessor);
  api.registerService(slackSyncService);

  // ========================================================================
  // Commands
  // ========================================================================

  api.registerCommand({
    name: 'slack-status',
    description: 'Show Slack webhook integration status',
    handler: async (context, args) => {
      const lines = [
        '📊 Slack Webhook Integration Status',
        '',
        `Webhook Path: ${webhookPath}`,
        `Slack Token: ${slackToken ? 'configured' : 'not configured'}`,
        `Default Channel: ${slackChannel || 'none'}`,
        '',
        `Pending Messages: ${pendingMessages.length}`,
        `Tracked Messages: ${sentMessages.size}`,
        '',
        'Services Running:',
        messageProcessor.name ? `  ✅ ${messageProcessor.name}` : '  ❌ processor',
        slackSyncService.name ? `  ✅ ${slackSyncService.name}` : '  ❌ sync',
      ];

      return lines.join('\n');
    }
  });

  api.registerCommand({
    name: 'slack-clear',
    description: 'Clear pending Slack messages',
    handler: async (context, args) => {
      const count = pendingMessages.length;
      pendingMessages.length = 0;
      
      return `Cleared ${count} pending message(s) from the queue.`;
    }
  });

  console.log(`[slack-webhook] Plugin registered with ${webhookPath} endpoint`);
}
