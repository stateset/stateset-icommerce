#!/usr/bin/env node
/**
 * Real x402 Server Example (Sequencer-backed)
 *
 * Runs a paywalled endpoint that accepts X-Payment or X-Payment-Receipt headers.
 */

import http from 'http';
import { loadSyncConfig, SyncConfig } from '../src/sync/config.js';
import { X402SequencerClient } from '../src/x402/client.js';
import {
  encodeBase64Json,
  decodeBase64Json,
  normalizeAsset,
  normalizeNetwork,
} from '../src/x402/crypto.js';
import { verifyPaymentHeader } from '../src/x402/agent.js';

const PORT = Number(process.env.X402_PORT || 8402);
const PAYEE_ADDRESS = process.env.X402_PAYEE_ADDRESS;
const PAYMENT_AMOUNT = Number(process.env.X402_AMOUNT || 100000);
const PAYMENT_ASSET = normalizeAsset(process.env.X402_ASSET || 'usdc');
const PAYMENT_NETWORKS = (process.env.X402_NETWORKS || 'set_chain')
  .split(',')
  .map(s => s.trim())
  .filter(Boolean)
  .map(normalizeNetwork);
const REQUIRE_RECEIPT = process.env.X402_REQUIRE_RECEIPT === 'true';

if (!PAYEE_ADDRESS) {
  console.error('X402_PAYEE_ADDRESS is required');
  process.exit(1);
}

function buildSequencerConfig() {
  const loaded = loadSyncConfig(process.cwd());
  if (loaded) return new SyncConfig(loaded);
  const url = process.env.SEQUENCER_URL;
  if (!url) {
    throw new Error('SEQUENCER_URL or .stateset/sync.json is required');
  }
  return {
    sequencerUrl: url,
    auth: {
      apiKey: process.env.SEQUENCER_API_KEY || null,
      jwt: process.env.SEQUENCER_JWT || null,
    },
  };
}

const sequencer = new X402SequencerClient(buildSequencerConfig());

function buildPaymentRequired(req) {
  const resourceUri = `http://${req.headers.host}${req.url}`;
  return {
    version: '1.0',
    payee_address: PAYEE_ADDRESS,
    amount: PAYMENT_AMOUNT,
    amount_display: `${(PAYMENT_AMOUNT / 1_000_000).toFixed(6)} ${PAYMENT_ASSET.toUpperCase()}`,
    asset: PAYMENT_ASSET,
    networks: PAYMENT_NETWORKS,
    resource_uri: resourceUri,
    resource_method: req.method,
    description: 'Premium API access',
    validity_seconds: 3600,
    merchant_id: process.env.X402_MERCHANT_ID || null,
    merchant_name: process.env.X402_MERCHANT_NAME || null,
    generated_at: new Date().toISOString(),
  };
}

function sendJson(res, status, body, headers = {}) {
  res.statusCode = status;
  res.setHeader('Content-Type', 'application/json');
  for (const [key, value] of Object.entries(headers)) {
    res.setHeader(key, value);
  }
  res.end(JSON.stringify(body));
}

async function verifyReceipt(receipt) {
  try {
    const remote = await sequencer.getPaymentReceipt(receipt.intent_id);
    if (!remote?.receipt) return { ok: false, reason: 'Receipt not found' };
    if (remote.receipt.intent_id !== receipt.intent_id) {
      return { ok: false, reason: 'Receipt intent mismatch' };
    }
    if (remote.receipt.payee_address !== PAYEE_ADDRESS) {
      return { ok: false, reason: 'Payee mismatch' };
    }
    if (remote.receipt.amount !== PAYMENT_AMOUNT) {
      return { ok: false, reason: 'Amount mismatch' };
    }
    return { ok: true, receipt: remote.receipt };
  } catch (err) {
    return { ok: false, reason: err.message };
  }
}

const server = http.createServer(async (req, res) => {
  if (req.url !== '/premium') {
    return sendJson(res, 404, { error: 'Not Found' });
  }

  const paymentRequired = buildPaymentRequired(req);

  const receiptHeader = req.headers['x-payment-receipt'];
  if (receiptHeader) {
    try {
      const receipt = decodeBase64Json(receiptHeader);
      const verification = await verifyReceipt(receipt);
      if (!verification.ok) {
        return sendJson(res, 402, { error: verification.reason }, {
          'X-Payment-Required': encodeBase64Json(paymentRequired),
        });
      }
      return sendJson(res, 200, { data: 'Premium data unlocked', status: 'paid' }, {
        'X-Payment-Receipt': encodeBase64Json(verification.receipt),
      });
    } catch (err) {
      return sendJson(res, 400, { error: `Invalid receipt: ${err.message}` });
    }
  }

  const paymentHeader = req.headers['x-payment'];
  if (paymentHeader) {
    try {
      const payload = decodeBase64Json(paymentHeader);
      const verification = verifyPaymentHeader(payload);
      if (!verification.ok) {
        return sendJson(res, 403, { error: verification.reason });
      }
      if (payload.payee_address !== PAYEE_ADDRESS) {
        return sendJson(res, 403, { error: 'Payee address mismatch' });
      }
      if (payload.amount !== PAYMENT_AMOUNT) {
        return sendJson(res, 403, { error: 'Amount mismatch' });
      }
      if (payload.asset !== PAYMENT_ASSET) {
        return sendJson(res, 403, { error: 'Asset mismatch' });
      }
      if (!PAYMENT_NETWORKS.includes(payload.network)) {
        return sendJson(res, 403, { error: 'Network mismatch' });
      }
      if (payload.valid_until && payload.valid_until < Math.floor(Date.now() / 1000)) {
        return sendJson(res, 403, { error: 'Payment intent expired' });
      }

      const response = await sequencer.submitPaymentIntent(payload);

      let receipt = null;
      if (REQUIRE_RECEIPT) {
        try {
          await sequencer.createBatch({
            tenant_id: payload.tenant_id,
            store_id: payload.store_id,
            network: payload.network,
          });
        } catch (_) {
          // best effort
        }
        receipt = await sequencer.waitForReceipt(response.intent_id, { timeoutMs: 300_000 });
      }

      return sendJson(res, 200, { data: 'Premium data unlocked', status: 'paid' }, {
        ...(receipt ? { 'X-Payment-Receipt': encodeBase64Json(receipt) } : {}),
      });
    } catch (err) {
      return sendJson(res, 400, { error: `Invalid payment header: ${err.message}` });
    }
  }

  return sendJson(res, 402, { error: 'Payment required' }, {
    'X-Payment-Required': encodeBase64Json(paymentRequired),
  });
});

server.listen(PORT, () => {
  console.log(`x402 server running on http://localhost:${PORT}/premium`);
  console.log(`Payee: ${PAYEE_ADDRESS}`);
  console.log(`Amount: ${(PAYMENT_AMOUNT / 1_000_000).toFixed(6)} ${PAYMENT_ASSET.toUpperCase()}`);
  console.log(`Networks: ${PAYMENT_NETWORKS.join(', ')}`);
  if (REQUIRE_RECEIPT) {
    console.log('Receipt enforcement: ON');
  }
});
