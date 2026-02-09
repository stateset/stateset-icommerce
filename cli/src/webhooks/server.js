/**
 * Webhook Event Handler for StateSet Commerce
 *
 * Enables AI agents to react to external events:
 * - Payment processor webhooks (Stripe, Square, PayPal)
 * - Shipping carrier updates (FedEx, UPS, USPS)
 * - Marketplace notifications (Shopify, Amazon, eBay)
 * - Custom integrations
 */

import { EventEmitter } from 'events';
import { randomUUID } from 'crypto';
import { createServer } from 'http';
import { createHmac, timingSafeEqual } from 'crypto';
import fs from 'fs';
import path from 'path';

/**
 * Webhook source configuration
 */
export class WebhookSource {
  constructor({
    id = randomUUID(),
    name,
    description = '',
    enabled = true,
    path, // URL path to listen on (e.g., '/webhooks/stripe')
    secret = null, // Signing secret for verification
    signatureHeader = 'x-signature', // Header containing signature
    signatureAlgorithm = 'sha256', // HMAC algorithm
    signaturePrefix = '', // Prefix in signature (e.g., 'sha256=')
    eventTypeField = 'type', // Field containing event type
    payloadField = null, // Field containing payload (null = entire body)
    retryOnFailure = true,
    maxRetries = 3,
    metadata = {},
  }) {
    this.id = id;
    this.name = name;
    this.description = description;
    this.enabled = enabled;
    this.path = path;
    this.secret = secret;
    this.signatureHeader = signatureHeader;
    this.signatureAlgorithm = signatureAlgorithm;
    this.signaturePrefix = signaturePrefix;
    this.eventTypeField = eventTypeField;
    this.payloadField = payloadField;
    this.retryOnFailure = retryOnFailure;
    this.maxRetries = maxRetries;
    this.metadata = metadata;
  }

  /**
   * Verify webhook signature
   */
  verifySignature(payload, signature) {
    if (!this.secret) return true; // No secret = no verification

    const expectedSignature =
      this.signaturePrefix +
      createHmac(this.signatureAlgorithm, this.secret).update(payload).digest('hex');

    try {
      return timingSafeEqual(Buffer.from(signature), Buffer.from(expectedSignature));
    } catch (err) {
      console.warn('[webhook] signature verification error:', err.message);
      return false;
    }
  }

  toJSON() {
    return {
      id: this.id,
      name: this.name,
      description: this.description,
      enabled: this.enabled,
      path: this.path,
      signatureHeader: this.signatureHeader,
      eventTypeField: this.eventTypeField,
      retryOnFailure: this.retryOnFailure,
      maxRetries: this.maxRetries,
      metadata: this.metadata,
      // Don't expose secret
    };
  }
}

/**
 * Webhook event handler mapping
 */
export class WebhookHandler {
  constructor({
    id = randomUUID(),
    name,
    description = '',
    enabled = true,
    sourceId, // WebhookSource ID
    eventTypes = ['*'], // Event types to handle ('*' = all)
    conditions = null, // Optional conditions for filtering
    action, // Action to execute: { agent, request } or { workflow }
    priority = 0,
    metadata = {},
  }) {
    this.id = id;
    this.name = name;
    this.description = description;
    this.enabled = enabled;
    this.sourceId = sourceId;
    this.eventTypes = eventTypes;
    this.conditions = conditions;
    this.action = action;
    this.priority = priority;
    this.metadata = metadata;
  }

  /**
   * Check if handler matches event
   */
  matches(eventType, payload) {
    if (!this.enabled) return false;

    // Check event type
    if (!this.eventTypes.includes('*') && !this.eventTypes.includes(eventType)) {
      return false;
    }

    // Check conditions if present
    if (this.conditions) {
      // Simple field matching
      for (const [field, expected] of Object.entries(this.conditions)) {
        const actual = getNestedValue(payload, field);
        if (actual !== expected) {
          return false;
        }
      }
    }

    return true;
  }

  toJSON() {
    return {
      id: this.id,
      name: this.name,
      description: this.description,
      enabled: this.enabled,
      sourceId: this.sourceId,
      eventTypes: this.eventTypes,
      conditions: this.conditions,
      action: this.action,
      priority: this.priority,
      metadata: this.metadata,
    };
  }
}

/**
 * Get nested value from object
 */
function getNestedValue(obj, path) {
  if (!path) return obj;
  return path.split('.').reduce((o, k) => o?.[k], obj);
}

/**
 * Webhook event record
 */
export class WebhookEvent {
  constructor({
    id = randomUUID(),
    sourceId,
    sourceName,
    eventType,
    payload,
    headers = {},
    status = 'pending', // pending, processing, completed, failed
    receivedAt = new Date().toISOString(),
    processedAt = null,
    handlers = [],
    results = [],
    error = null,
    retryCount = 0,
  }) {
    this.id = id;
    this.sourceId = sourceId;
    this.sourceName = sourceName;
    this.eventType = eventType;
    this.payload = payload;
    this.headers = headers;
    this.status = status;
    this.receivedAt = receivedAt;
    this.processedAt = processedAt;
    this.handlers = handlers;
    this.results = results;
    this.error = error;
    this.retryCount = retryCount;
  }

  toJSON() {
    return {
      id: this.id,
      sourceId: this.sourceId,
      sourceName: this.sourceName,
      eventType: this.eventType,
      payload: this.payload,
      status: this.status,
      receivedAt: this.receivedAt,
      processedAt: this.processedAt,
      handlers: this.handlers,
      results: this.results,
      error: this.error,
      retryCount: this.retryCount,
    };
  }
}

/**
 * Webhook Server
 */
export class WebhookServer extends EventEmitter {
  constructor({
    port = 3000,
    host = '0.0.0.0',
    storePath = null,
    executor = null, // Function to execute actions
    autoStart = false,
  }) {
    super();

    this.port = port;
    this.host = host;
    this.storePath = storePath;
    this.executor = executor;

    this.sources = new Map();
    this.handlers = new Map();
    this.eventHistory = [];
    this.pendingEvents = [];

    this.server = null;
    this.isRunning = false;

    if (autoStart) {
      this.start();
    }
  }

  /**
   * Load configuration from storage
   */
  async load() {
    if (!this.storePath) return;

    try {
      const sourcesFile = path.join(this.storePath, 'webhook-sources.json');
      const handlersFile = path.join(this.storePath, 'webhook-handlers.json');

      if (fs.existsSync(sourcesFile)) {
        const data = JSON.parse(fs.readFileSync(sourcesFile, 'utf-8'));
        for (const sourceData of data) {
          const source = new WebhookSource(sourceData);
          this.sources.set(source.id, source);
        }
      }

      if (fs.existsSync(handlersFile)) {
        const data = JSON.parse(fs.readFileSync(handlersFile, 'utf-8'));
        for (const handlerData of data) {
          const handler = new WebhookHandler(handlerData);
          this.handlers.set(handler.id, handler);
        }
      }

      this.emit('loaded', {
        sourceCount: this.sources.size,
        handlerCount: this.handlers.size,
      });
    } catch (error) {
      this.emit('error', { type: 'load', error });
    }
  }

  /**
   * Save configuration to storage
   */
  async save() {
    if (!this.storePath) return;

    try {
      fs.mkdirSync(this.storePath, { recursive: true });

      const sourcesFile = path.join(this.storePath, 'webhook-sources.json');
      const handlersFile = path.join(this.storePath, 'webhook-handlers.json');

      // Don't save secrets to disk - they should come from environment
      const sourcesData = Array.from(this.sources.values()).map((s) => ({
        ...s.toJSON(),
        secret: null, // Redact
      }));
      fs.writeFileSync(sourcesFile, JSON.stringify(sourcesData, null, 2));

      const handlersData = Array.from(this.handlers.values()).map((h) => h.toJSON());
      fs.writeFileSync(handlersFile, JSON.stringify(handlersData, null, 2));

      this.emit('saved');
    } catch (error) {
      this.emit('error', { type: 'save', error });
    }
  }

  /**
   * Register a webhook source
   */
  registerSource(config) {
    const source = config instanceof WebhookSource ? config : new WebhookSource(config);
    this.sources.set(source.id, source);
    this.emit('source:registered', { source: source.toJSON() });
    this.save();
    return source;
  }

  /**
   * Register a webhook handler
   */
  registerHandler(config) {
    const handler = config instanceof WebhookHandler ? config : new WebhookHandler(config);
    this.handlers.set(handler.id, handler);
    this.emit('handler:registered', { handler: handler.toJSON() });
    this.save();
    return handler;
  }

  /**
   * Find source by path
   */
  findSourceByPath(urlPath) {
    for (const source of this.sources.values()) {
      if (source.enabled && source.path === urlPath) {
        return source;
      }
    }
    return null;
  }

  /**
   * Find handlers for source and event type
   */
  findHandlers(sourceId, _eventType) {
    return Array.from(this.handlers.values())
      .filter((h) => h.sourceId === sourceId || h.sourceId === '*')
      .filter((h) => h.enabled)
      .sort((a, b) => b.priority - a.priority);
  }

  /**
   * Process a webhook event
   */
  async processEvent(event) {
    event.status = 'processing';
    this.emit('event:processing', { event: event.toJSON() });

    const handlers = this.findHandlers(event.sourceId, event.eventType).filter((h) =>
      h.matches(event.eventType, event.payload),
    );

    event.handlers = handlers.map((h) => h.id);

    if (handlers.length === 0) {
      event.status = 'completed';
      event.processedAt = new Date().toISOString();
      event.results.push({ message: 'No matching handlers' });
      this.emit('event:completed', { event: event.toJSON() });
      return event;
    }

    for (const handler of handlers) {
      try {
        let result = null;

        if (this.executor && handler.action) {
          // Interpolate payload values into action
          const interpolatedAction = this.interpolateAction(handler.action, event);
          result = await this.executor(interpolatedAction, {
            eventId: event.id,
            eventType: event.eventType,
            sourceId: event.sourceId,
            payload: event.payload,
          });
        }

        event.results.push({
          handlerId: handler.id,
          handlerName: handler.name,
          success: true,
          result,
        });

        this.emit('handler:executed', { handler: handler.toJSON(), event: event.toJSON(), result });
      } catch (error) {
        event.results.push({
          handlerId: handler.id,
          handlerName: handler.name,
          success: false,
          error: error.message,
        });

        this.emit('handler:failed', { handler: handler.toJSON(), event: event.toJSON(), error });
      }
    }

    event.status = 'completed';
    event.processedAt = new Date().toISOString();

    this.eventHistory.push(event);
    if (this.eventHistory.length > 1000) {
      this.eventHistory = this.eventHistory.slice(-1000);
    }

    this.emit('event:completed', { event: event.toJSON() });
    return event;
  }

  /**
   * Interpolate payload values into action
   */
  interpolateAction(action, event) {
    const interpolate = (str) => {
      if (typeof str !== 'string') return str;

      return str.replace(/\{([^}]+)\}/g, (match, path) => {
        // Support both payload.field and just field
        let value = getNestedValue(event.payload, path);
        if (value === undefined) {
          value = getNestedValue(event, path);
        }
        return value !== undefined ? value : match;
      });
    };

    const result = { ...action };

    if (result.request) {
      result.request = interpolate(result.request);
    }

    if (result.workflow) {
      result.workflow = interpolate(result.workflow);
    }

    return result;
  }

  /**
   * Handle incoming HTTP request
   */
  async handleRequest(req, res) {
    // Parse URL
    const url = new URL(req.url, `http://${req.headers.host}`);

    // Health check
    if (url.pathname === '/health') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ status: 'ok', timestamp: new Date().toISOString() }));
      return;
    }

    // Find source
    const source = this.findSourceByPath(url.pathname);
    if (!source) {
      res.writeHead(404, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'Unknown webhook endpoint' }));
      return;
    }

    // Only accept POST
    if (req.method !== 'POST') {
      res.writeHead(405, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'Method not allowed' }));
      return;
    }

    // Read body
    let body = '';
    for await (const chunk of req) {
      body += chunk;
    }

    // Verify signature
    const signature = req.headers[source.signatureHeader.toLowerCase()];
    if (source.secret && !source.verifySignature(body, signature || '')) {
      this.emit('event:rejected', { reason: 'Invalid signature', source: source.name });
      res.writeHead(401, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'Invalid signature' }));
      return;
    }

    // Parse payload
    let payload;
    try {
      payload = JSON.parse(body);
    } catch {
      res.writeHead(400, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'Invalid JSON' }));
      return;
    }

    // Extract event type
    const eventType = getNestedValue(payload, source.eventTypeField) || 'unknown';

    // Extract actual payload if nested
    const eventPayload = source.payloadField
      ? getNestedValue(payload, source.payloadField)
      : payload;

    // Create event
    const event = new WebhookEvent({
      sourceId: source.id,
      sourceName: source.name,
      eventType,
      payload: eventPayload,
      headers: { ...req.headers },
    });

    this.emit('event:received', { event: event.toJSON() });

    // Respond immediately (async processing)
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ received: true, eventId: event.id }));

    // Process asynchronously
    this.processEvent(event).catch((error) => {
      event.status = 'failed';
      event.error = error.message;
      this.emit('event:failed', { event: event.toJSON(), error });
    });
  }

  /**
   * Start the webhook server
   */
  start() {
    if (this.isRunning) return;

    this.server = createServer((req, res) => {
      this.handleRequest(req, res).catch((error) => {
        this.emit('error', { type: 'request', error });
        if (!res.headersSent) {
          res.writeHead(500, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ error: 'Internal server error' }));
        }
      });
    });

    this.server.listen(this.port, this.host, () => {
      this.isRunning = true;
      this.emit('started', { port: this.port, host: this.host });
    });

    this.server.on('error', (error) => {
      this.emit('error', { type: 'server', error });
    });
  }

  /**
   * Stop the webhook server
   */
  stop() {
    if (!this.isRunning || !this.server) return;

    return new Promise((resolve) => {
      this.server.close(() => {
        this.isRunning = false;
        this.emit('stopped');
        resolve();
      });
    });
  }

  /**
   * Get server status
   */
  getStatus() {
    return {
      isRunning: this.isRunning,
      port: this.port,
      host: this.host,
      sourceCount: this.sources.size,
      handlerCount: this.handlers.size,
      recentEvents: this.eventHistory.slice(-10).map((e) => e.toJSON()),
    };
  }

  /**
   * Get event history
   */
  getHistory({ sourceId = null, eventType = null, status = null, limit = 100 } = {}) {
    let history = this.eventHistory;

    if (sourceId) {
      history = history.filter((e) => e.sourceId === sourceId);
    }

    if (eventType) {
      history = history.filter((e) => e.eventType === eventType);
    }

    if (status) {
      history = history.filter((e) => e.status === status);
    }

    return history.slice(-limit).map((e) => e.toJSON());
  }

  /**
   * List sources
   */
  listSources() {
    return Array.from(this.sources.values()).map((s) => s.toJSON());
  }

  /**
   * List handlers
   */
  listHandlers() {
    return Array.from(this.handlers.values()).map((h) => h.toJSON());
  }
}

/**
 * Pre-configured webhook source templates
 */
export const WebhookSourceTemplates = {
  stripe: {
    name: 'Stripe',
    description: 'Stripe payment webhooks',
    path: '/webhooks/stripe',
    signatureHeader: 'stripe-signature',
    signatureAlgorithm: 'sha256',
    signaturePrefix: '',
    eventTypeField: 'type',
    payloadField: 'data.object',
  },

  shopify: {
    name: 'Shopify',
    description: 'Shopify store webhooks',
    path: '/webhooks/shopify',
    signatureHeader: 'x-shopify-hmac-sha256',
    signatureAlgorithm: 'sha256',
    signaturePrefix: '',
    eventTypeField: 'topic',
  },

  square: {
    name: 'Square',
    description: 'Square payment webhooks',
    path: '/webhooks/square',
    signatureHeader: 'x-square-signature',
    signatureAlgorithm: 'sha256',
    signaturePrefix: '',
    eventTypeField: 'type',
    payloadField: 'data',
  },

  shippo: {
    name: 'Shippo',
    description: 'Shippo shipping webhooks',
    path: '/webhooks/shippo',
    signatureHeader: 'x-shippo-signature',
    signatureAlgorithm: 'sha256',
    signaturePrefix: '',
    eventTypeField: 'event',
  },

  custom: {
    name: 'Custom',
    description: 'Custom webhook endpoint',
    path: '/webhooks/custom',
    signatureHeader: 'x-signature',
    signatureAlgorithm: 'sha256',
    signaturePrefix: '',
    eventTypeField: 'event_type',
  },
};

/**
 * Pre-configured webhook handler templates
 */
export const WebhookHandlerTemplates = {
  stripePaymentSucceeded: {
    name: 'Stripe Payment Succeeded',
    eventTypes: ['payment_intent.succeeded', 'charge.succeeded'],
    action: {
      agent: 'payments',
      request: 'Record successful payment of {amount} {currency} for customer {customer}',
    },
  },

  stripePaymentFailed: {
    name: 'Stripe Payment Failed',
    eventTypes: ['payment_intent.payment_failed', 'charge.failed'],
    action: {
      agent: 'payments',
      request: 'Handle failed payment for customer {customer}: {failure_message}',
    },
  },

  stripeSubscriptionUpdated: {
    name: 'Stripe Subscription Updated',
    eventTypes: ['customer.subscription.updated', 'customer.subscription.deleted'],
    action: {
      agent: 'subscriptions',
      request: 'Sync subscription status for {id}: status is now {status}',
    },
  },

  shopifyOrderCreated: {
    name: 'Shopify Order Created',
    eventTypes: ['orders/create'],
    action: {
      agent: 'orders',
      request: 'Import order {order_number} from Shopify with total {total_price}',
    },
  },

  shippoTrackingUpdate: {
    name: 'Shippo Tracking Update',
    eventTypes: ['track_updated'],
    action: {
      agent: 'shipments',
      request: 'Update tracking for {tracking_number}: status is {tracking_status.status}',
    },
  },
};

export default WebhookServer;
