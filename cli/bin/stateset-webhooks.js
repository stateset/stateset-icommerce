#!/usr/bin/env node

/**
 * StateSet Webhooks — Standalone Webhook Receiver
 *
 * Receives webhooks from Shopify, Stripe, and WooCommerce,
 * verifies signatures, and syncs data into the local iCommerce database.
 *
 * Usage:
 *   stateset-webhooks --port 3000
 *   stateset-webhooks --stripe-secret whsec_... --port 3000
 *   stateset-webhooks --shopify-secret shpss_... --port 3000
 *   stateset-webhooks --woocommerce-secret wc_... --port 3000
 */

import { parseArgs } from 'node:util';
import http from 'node:http';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const { values: args } = parseArgs({
  options: {
    port: { type: 'string', short: 'p', default: '3000' },
    'stripe-secret': { type: 'string', default: '' },
    'shopify-secret': { type: 'string', default: '' },
    'woocommerce-secret': { type: 'string', default: '' },
    db: { type: 'string', default: './store.db' },
    help: { type: 'boolean', short: 'h', default: false },
    version: { type: 'boolean', short: 'v', default: false },
  },
  strict: false,
  allowPositionals: true,
});

if (args.help) {
  console.log(`
StateSet Webhooks — Standalone Webhook Receiver

Usage:
  stateset-webhooks [options]

Options:
  -p, --port <port>               HTTP port (default: 3000)
  --stripe-secret <secret>        Stripe webhook signing secret (whsec_...)
  --shopify-secret <secret>       Shopify webhook signing secret (shpss_...)
  --woocommerce-secret <secret>   WooCommerce webhook signing secret
  --db <path>                     Database path (default: ./store.db)
  -h, --help                      Show this help
  -v, --version                   Show version

Endpoints:
  POST /webhooks/stripe           Stripe webhook events
  POST /webhooks/shopify          Shopify webhook events
  POST /webhooks/woocommerce      WooCommerce webhook events
  GET  /health                    Health check
`);
  process.exit(0);
}

if (args.version) {
  const { createRequire } = await import('node:module');
  const require = createRequire(import.meta.url);
  const pkg = require(join(__dirname, '..', 'package.json'));
  console.log(`stateset-webhooks v${pkg.version}`);
  process.exit(0);
}

const PORT = parseInt(args.port, 10) || 3000;

// Lazy-load adapters and commerce only when needed
let stripeHandlers = null;
let shopifyHandlers = null;
let woocommerceHandlers = null;

async function initAdapters() {
  // Import commerce engine
  const Commerce = (await import('../src/commerce.js')).default;
  const { IdMapStore } = await import('../src/adapters/id-map-store.js');

  const commerce = new Commerce({ dbPath: args.db });
  await commerce.init();

  const idMapStore = new IdMapStore(commerce.db);

  // Initialize Stripe handlers
  if (args['stripe-secret']) {
    const { createStripeWebhookHandlers } = await import('../src/adapters/stripe/webhooks.js');
    const { verifyStripeSignature } = await import('../src/adapters/stripe/signature.js');
    stripeHandlers = {
      handlers: createStripeWebhookHandlers(commerce, idMapStore),
      verify: (body, sig) => verifyStripeSignature(body, sig, args['stripe-secret']),
    };
    console.log('[webhooks] Stripe adapter enabled');
  }

  // Initialize Shopify handlers
  if (args['shopify-secret']) {
    const { createShopifyWebhookHandlers } = await import('../src/adapters/shopify/webhooks.js');
    shopifyHandlers = {
      handlers: createShopifyWebhookHandlers(commerce, idMapStore),
      secret: args['shopify-secret'],
    };
    console.log('[webhooks] Shopify adapter enabled');
  }

  // Initialize WooCommerce handlers
  if (args['woocommerce-secret']) {
    const { createWooCommerceWebhookHandlers, verifyWooCommerceSignature } =
      await import('../src/adapters/woocommerce/webhooks.js');
    woocommerceHandlers = {
      handlers: createWooCommerceWebhookHandlers(commerce, idMapStore),
      verify: (body, sig) => verifyWooCommerceSignature(body, sig, args['woocommerce-secret']),
    };
    console.log('[webhooks] WooCommerce adapter enabled');
  }

  return commerce;
}

/**
 * Read full request body as string.
 */
function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on('data', (chunk) => chunks.push(chunk));
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf-8')));
    req.on('error', reject);
  });
}

/**
 * Send JSON response.
 */
function sendJson(res, status, data) {
  res.writeHead(status, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(data));
}

/**
 * Handle incoming webhook requests.
 */
async function handleRequest(req, res) {
  const url = new URL(req.url, `http://localhost:${PORT}`);

  // Health check
  if (url.pathname === '/health' && req.method === 'GET') {
    return sendJson(res, 200, {
      status: 'ok',
      adapters: {
        stripe: !!stripeHandlers,
        shopify: !!shopifyHandlers,
        woocommerce: !!woocommerceHandlers,
      },
    });
  }

  // Only accept POST to webhook endpoints
  if (req.method !== 'POST') {
    return sendJson(res, 405, { error: 'Method not allowed' });
  }

  const rawBody = await readBody(req);

  try {
    // Stripe webhooks
    if (url.pathname === '/webhooks/stripe') {
      if (!stripeHandlers) {
        return sendJson(res, 503, { error: 'Stripe adapter not configured' });
      }

      const sig = req.headers['stripe-signature'];
      const verification = stripeHandlers.verify(rawBody, sig);
      if (!verification.valid) {
        return sendJson(res, 401, { error: 'Invalid signature', detail: verification.error });
      }

      const payload = JSON.parse(rawBody);
      const eventType = payload.type;
      const handler = stripeHandlers.handlers[eventType];

      if (!handler) {
        return sendJson(res, 200, { received: true, action: 'ignored', eventType });
      }

      const result = await handler(payload);
      return sendJson(res, 200, { received: true, ...result });
    }

    // Shopify webhooks
    if (url.pathname === '/webhooks/shopify') {
      if (!shopifyHandlers) {
        return sendJson(res, 503, { error: 'Shopify adapter not configured' });
      }

      const topic = req.headers['x-shopify-topic'];
      const handler = shopifyHandlers.handlers[topic];

      if (!handler) {
        return sendJson(res, 200, { received: true, action: 'ignored', topic });
      }

      const payload = JSON.parse(rawBody);
      const result = await handler(payload);
      return sendJson(res, 200, { received: true, ...result });
    }

    // WooCommerce webhooks
    if (url.pathname === '/webhooks/woocommerce') {
      if (!woocommerceHandlers) {
        return sendJson(res, 503, { error: 'WooCommerce adapter not configured' });
      }

      const sig = req.headers['x-wc-webhook-signature'];
      const verification = woocommerceHandlers.verify(rawBody, sig);
      if (!verification.valid) {
        return sendJson(res, 401, { error: 'Invalid signature', detail: verification.error });
      }

      const topic = req.headers['x-wc-webhook-topic'];
      const handler = woocommerceHandlers.handlers[topic];

      if (!handler) {
        return sendJson(res, 200, { received: true, action: 'ignored', topic });
      }

      const payload = JSON.parse(rawBody);
      const result = await handler(payload);
      return sendJson(res, 200, { received: true, ...result });
    }

    return sendJson(res, 404, { error: 'Not found' });
  } catch (error) {
    console.error(`[webhooks] Error: ${error.message}`);
    return sendJson(res, 500, { error: 'Internal server error' });
  }
}

// Start server
const commerce = await initAdapters();

const server = http.createServer(handleRequest);

server.listen(PORT, () => {
  console.log(`[webhooks] Listening on port ${PORT}`);
  console.log(`[webhooks] Database: ${args.db}`);
  console.log(`[webhooks] Endpoints:`);
  if (stripeHandlers) console.log(`  POST http://localhost:${PORT}/webhooks/stripe`);
  if (shopifyHandlers) console.log(`  POST http://localhost:${PORT}/webhooks/shopify`);
  if (woocommerceHandlers) console.log(`  POST http://localhost:${PORT}/webhooks/woocommerce`);
  console.log(`  GET  http://localhost:${PORT}/health`);
});

// Graceful shutdown
function shutdown(signal) {
  console.log(`\n[webhooks] ${signal} received, shutting down...`);
  server.close(() => {
    if (commerce && typeof commerce.close === 'function') {
      commerce.close();
    }
    process.exit(0);
  });

  // Force exit after 5 seconds
  setTimeout(() => {
    console.error('[webhooks] Forced shutdown after timeout');
    process.exit(1);
  }, 5000).unref();
}

process.on('SIGINT', () => shutdown('SIGINT'));
process.on('SIGTERM', () => shutdown('SIGTERM'));
