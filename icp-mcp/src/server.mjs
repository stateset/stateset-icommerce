#!/usr/bin/env node
// icp-mcp — Model Context Protocol server for ICP-1.0
//
// Speaks JSON-RPC 2.0 over stdio per the MCP spec. Tools expose ICP
// operations so an LLM agent (Claude Desktop, Cursor, Windsurf) can
// transact through the protocol without learning HTTP.
//
// Zero external dependencies. Tested with `node --test`.
//
// Run standalone:
//   node src/server.mjs
//
// Plug into Claude Desktop (~/Library/Application Support/Claude/claude_desktop_config.json):
//   {
//     "mcpServers": {
//       "icp": {
//         "command": "node",
//         "args": ["/abs/path/to/icp-mcp/src/server.mjs"]
//       }
//     }
//   }

import { createInterface } from 'node:readline';
import { generateKeyPairSync, createHash, createPublicKey } from 'node:crypto';

import {
  canonicalJson,
  signEd25519,
  publicKeyToRaw,
  privateKeyFromSeed,
  newId,
  newNonceHex,
  base58btcEncode,
} from '../../icp-handler/src/codec.mjs';
import {
  submitIntent,
  acceptQuote,
  fulfillEscrow,
  getEscrowState,
  getSettlement,
  counts,
  ALLOWED_SETTLERS,
} from './backend.mjs';

// ---------------------------------------------------------------------------
// Merchant identity for THIS MCP server instance
// ---------------------------------------------------------------------------

const merchantKp = generateKeyPairSync('ed25519');
const merchantPubRaw = publicKeyToRaw(merchantKp.publicKey);
const merchantAid = `aid:v1:zMcpMerchant${process.pid}${Date.now()}`;

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

const tools = [
  {
    name: 'icp_capabilities',
    description: "Get this server's ICP-1.0 capabilities: spec version, merchant identity, allowed Settlers, supported verbs. Call this first to discover what this handler supports.",
    inputSchema: { type: 'object', properties: {}, additionalProperties: false },
  },
  {
    name: 'icp_keypair_generate',
    description: 'Generate a fresh Agent identity (Ed25519 + X25519 keypairs + derived AID per ICP-1.0 §4.2). Returns seed material the caller can use to sign Intents. For testing only — production agents persist their own keys.',
    inputSchema: { type: 'object', properties: {}, additionalProperties: false },
  },
  {
    name: 'icp_intent_build_and_sign',
    description: 'Build a complete signed ICP-1.0 purchase.create Intent from line items and an Ed25519 signing seed. Returns the full envelope ready to submit via icp_intent_submit. Handles canonical JSON serialization and signature.',
    inputSchema: {
      type: 'object',
      required: ['ed25519_seed_hex', 'x25519_pubkey_hex', 'merchant_aid', 'settler', 'items', 'max_total'],
      properties: {
        ed25519_seed_hex: { type: 'string', description: '32-byte Ed25519 seed in hex (from icp_keypair_generate)' },
        x25519_pubkey_hex: { type: 'string', description: '32-byte X25519 public key in hex (from icp_keypair_generate)' },
        merchant_aid: { type: 'string', description: 'AID of the counterparty (merchant)' },
        settler: { type: 'string', description: 'Settler identifier, e.g. settler:stateset.usdc.base-sepolia' },
        items: {
          type: 'array',
          description: 'Line items',
          items: {
            type: 'object',
            required: ['sku', 'quantity', 'unit_price'],
            properties: {
              sku: { type: 'string' },
              quantity: { type: 'integer', minimum: 1 },
              unit_price: { type: 'object', required: ['amount', 'currency'], properties: { amount: { type: 'string' }, currency: { type: 'string' } } },
            },
          },
        },
        max_total: { type: 'object', required: ['amount', 'currency'], properties: { amount: { type: 'string' }, currency: { type: 'string' } } },
      },
      additionalProperties: false,
    },
  },
  {
    name: 'icp_intent_submit',
    description: 'Submit a signed Intent to this handler. Returns a signed Quote (or an ICP error). Signature is verified against the supplied buyer pubkey.',
    inputSchema: {
      type: 'object',
      required: ['intent', 'signature', '_pubkey_hex'],
      properties: {
        intent: { type: 'object', description: 'The Intent payload (output of icp_intent_build_and_sign.intent)' },
        signature: { type: 'object', description: 'The signature envelope (output of icp_intent_build_and_sign.signature)' },
        _pubkey_hex: { type: 'string', description: '32-byte Ed25519 public key in hex for AID resolution (until a real resolver is wired)' },
      },
      additionalProperties: false,
    },
  },
  {
    name: 'icp_quote_accept',
    description: 'Accept a Quote by quote_id. Returns funding instructions including the on-chain ICPEscrow contract call to make.',
    inputSchema: { type: 'object', required: ['quote_id'], properties: { quote_id: { type: 'string' } }, additionalProperties: false },
  },
  {
    name: 'icp_escrow_state',
    description: 'Get the current state of an escrow plus all EscrowEvents observed so far. State machine: pending → funded → fulfilled → released. Or → disputed → released/refunded.',
    inputSchema: { type: 'object', required: ['escrow_id'], properties: { escrow_id: { type: 'string' } }, additionalProperties: false },
  },
  {
    name: 'icp_fulfill',
    description: 'Submit fulfillment evidence for an escrow. In the demo backend this auto-funds (mocks chain confirmation) and auto-releases (skips the dispute window) so a co-signed SettlementReceipt is produced immediately. Production behavior requires real chain events.',
    inputSchema: {
      type: 'object',
      required: ['escrow_id'],
      properties: {
        escrow_id: { type: 'string' },
        evidence_id: { type: 'string', description: 'Optional fulfillment evidence identifier' },
      },
      additionalProperties: false,
    },
  },
  {
    name: 'icp_settlement_get',
    description: 'Fetch a SettlementReceipt by settlement_id. The receipt is co-signed by the merchant and the Settler — it is the canonical proof of settlement for tax/audit/accounting.',
    inputSchema: { type: 'object', required: ['settlement_id'], properties: { settlement_id: { type: 'string' } }, additionalProperties: false },
  },
];

const handlers = {
  icp_capabilities: () => ({
    spec: 'icp-1.0',
    server: 'icp-mcp',
    server_version: '0.1.0',
    merchant_aid: merchantAid,
    merchant_pubkey_hex: merchantPubRaw.toString('hex'),
    settler_allowlist: [...ALLOWED_SETTLERS],
    supported_verbs: ['purchase.create'],
    backend: 'stub (in-memory, auto-fulfilling — for demos)',
    counts: counts(),
  }),

  icp_keypair_generate: () => {
    const ed = generateKeyPairSync('ed25519');
    const x = generateKeyPairSync('x25519');
    // Extract raw seed via PKCS#8 (Ed25519 PKCS#8 = prefix + 32-byte seed).
    const edPkcs8 = ed.privateKey.export({ format: 'der', type: 'pkcs8' });
    const xPkcs8 = x.privateKey.export({ format: 'der', type: 'pkcs8' });
    const edSeed = edPkcs8.subarray(16, 48);
    const xSeed = xPkcs8.subarray(16, 48);
    const edPubRaw = publicKeyToRaw(ed.publicKey);
    const xPubRaw = publicKeyToRaw(x.publicKey);

    const buf = Buffer.concat([edPubRaw, Buffer.from([0x00]), xPubRaw]);
    const aid = `aid:v1:z${base58btcEncode(createHash('sha256').update(buf).digest())}`;

    return {
      aid,
      ed25519_seed_hex: edSeed.toString('hex'),
      ed25519_pubkey_hex: edPubRaw.toString('hex'),
      x25519_seed_hex: xSeed.toString('hex'),
      x25519_pubkey_hex: xPubRaw.toString('hex'),
      _note: 'For TESTING only. Production keys live in HSM/KMS and never round-trip through stdout.',
    };
  },

  icp_intent_build_and_sign: (args) => {
    const seed = Buffer.from(args.ed25519_seed_hex, 'hex');
    if (seed.length !== 32) throw new Error('ed25519_seed_hex must decode to 32 bytes');
    const edPriv = privateKeyFromSeed(seed);
    const edPub = publicKeyToRaw(createPublicKey(edPriv));
    const xPubRaw = Buffer.from(args.x25519_pubkey_hex, 'hex');
    const buf = Buffer.concat([edPub, Buffer.from([0x00]), xPubRaw]);
    const aid = `aid:v1:z${base58btcEncode(createHash('sha256').update(buf).digest())}`;

    const now = new Date();
    const exp = new Date(now.getTime() + 300 * 1000);
    const intent = {
      v: 'icp-1.0',
      verb: 'purchase.create',
      intent_id: newId('icp_int'),
      buyer: aid,
      merchant: args.merchant_aid,
      settler: args.settler,
      items: args.items,
      max_total: args.max_total,
      expiry: exp.toISOString(),
      principal_binding: {
        principal: 'did:web:icp-mcp-demo.example',
        agent: aid,
        authority: { max_per_intent: { amount: '10000', currency: args.max_total.currency }, verbs: ['purchase.create'] },
        expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
        revocation: 'https://icp-mcp-demo.example/revoke',
        signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
      },
      nonce: newNonceHex(),
      iat: now.toISOString(),
      exp: exp.toISOString(),
    };
    const canonical = canonicalJson(intent);
    const sig = signEd25519(canonical, edPriv);
    return {
      intent,
      signature: { alg: 'ed25519', kid: aid, sig },
      _pubkey_hex: edPub.toString('hex'),
      canonical_string: canonical,
    };
  },

  icp_intent_submit: (args) => {
    const r = submitIntent(args, merchantKp.privateKey, merchantAid);
    if (!r.ok) return { error: r.error };
    return { quote: r.quote, signature: r.signature };
  },

  icp_quote_accept: (args) => {
    const r = acceptQuote(args.quote_id, merchantKp.privateKey, merchantAid);
    if (!r.ok) return { error: r.error };
    return { funding: r.funding };
  },

  icp_escrow_state: (args) => {
    const r = getEscrowState(args.escrow_id);
    if (!r.ok) return { error: r.error };
    return r;
  },

  icp_fulfill: (args) => {
    const r = fulfillEscrow(args.escrow_id, args.evidence_id, merchantKp.privateKey, merchantAid);
    if (!r.ok) return { error: r.error };
    return { receipt: r.receipt };
  },

  icp_settlement_get: (args) => {
    const r = getSettlement(args.settlement_id);
    if (!r.ok) return { error: r.error };
    return r;
  },
};

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 loop
// ---------------------------------------------------------------------------

const PROTOCOL_VERSION = '2024-11-05';

function dispatch(message) {
  const { id, method, params } = message;
  try {
    switch (method) {
      case 'initialize':
        return {
          jsonrpc: '2.0',
          id,
          result: {
            protocolVersion: PROTOCOL_VERSION,
            serverInfo: { name: 'icp-mcp', version: '0.1.0' },
            capabilities: { tools: {} },
            instructions: 'This server exposes the Intelligent Commerce Protocol (ICP-1.0) as MCP tools. Call icp_capabilities first to discover supported verbs and allowed Settlers, then icp_keypair_generate → icp_intent_build_and_sign → icp_intent_submit → icp_quote_accept → icp_fulfill → icp_settlement_get for a full transaction.',
          },
        };
      case 'notifications/initialized':
        return null; // no response for notifications
      case 'tools/list':
        return { jsonrpc: '2.0', id, result: { tools } };
      case 'tools/call': {
        const { name, arguments: args } = params ?? {};
        const handler = handlers[name];
        if (!handler) {
          return { jsonrpc: '2.0', id, error: { code: -32601, message: `unknown tool: ${name}` } };
        }
        const out = handler(args ?? {});
        return {
          jsonrpc: '2.0',
          id,
          result: {
            content: [{ type: 'text', text: JSON.stringify(out, null, 2) }],
            isError: Boolean(out?.error),
          },
        };
      }
      case 'ping':
        return { jsonrpc: '2.0', id, result: {} };
      default:
        return { jsonrpc: '2.0', id, error: { code: -32601, message: `unknown method: ${method}` } };
    }
  } catch (err) {
    return { jsonrpc: '2.0', id, error: { code: -32603, message: `internal error: ${err.message}` } };
  }
}

function write(message) {
  process.stdout.write(JSON.stringify(message) + '\n');
}

const rl = createInterface({ input: process.stdin, terminal: false });
rl.on('line', (line) => {
  if (!line.trim()) return;
  let msg;
  try {
    msg = JSON.parse(line);
  } catch (_) {
    write({ jsonrpc: '2.0', id: null, error: { code: -32700, message: 'parse error' } });
    return;
  }
  const response = dispatch(msg);
  if (response !== null) write(response);
});

process.stderr.write(`icp-mcp ready · merchant_aid=${merchantAid}\n`);

export { dispatch, tools, handlers };
