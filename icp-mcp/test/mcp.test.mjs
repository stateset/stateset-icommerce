// End-to-end MCP test — drives the icp-mcp server via JSON-RPC 2.0 over stdio
// as Claude Desktop / Cursor / Windsurf would. No deps.
//
// Run: node --test test/mcp.test.mjs

import { test, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SERVER = resolve(__dirname, '..', 'src', 'server.mjs');

class McpClient {
  constructor() {
    this.proc = spawn('node', [SERVER], { stdio: ['pipe', 'pipe', 'pipe'] });
    this.rl = createInterface({ input: this.proc.stdout });
    this.pending = new Map();
    this.nextId = 1;
    this.rl.on('line', (line) => {
      if (!line.trim()) return;
      let msg;
      try { msg = JSON.parse(line); } catch (_) { return; }
      const resolver = this.pending.get(msg.id);
      if (resolver) {
        this.pending.delete(msg.id);
        resolver(msg);
      }
    });
  }

  async call(method, params) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, resolve);
      this.proc.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
      setTimeout(() => {
        if (this.pending.has(id)) {
          this.pending.delete(id);
          reject(new Error(`timeout: ${method}`));
        }
      }, 5000);
    });
  }

  notify(method, params) {
    this.proc.stdin.write(JSON.stringify({ jsonrpc: '2.0', method, params }) + '\n');
  }

  async tool(name, args) {
    const r = await this.call('tools/call', { name, arguments: args });
    if (r.error) throw new Error(`MCP error: ${r.error.message}`);
    return JSON.parse(r.result.content[0].text);
  }

  close() {
    this.proc.kill();
  }
}

const client = new McpClient();
after(() => client.close());

test('initialize handshake', async () => {
  const r = await client.call('initialize', {
    protocolVersion: '2024-11-05',
    capabilities: {},
    clientInfo: { name: 'test', version: '1.0' },
  });
  assert.equal(r.result.serverInfo.name, 'icp-mcp');
  assert.ok(r.result.capabilities.tools !== undefined);
  client.notify('notifications/initialized', {});
});

test('tools/list returns 8 ICP tools', async () => {
  const r = await client.call('tools/list', {});
  const names = r.result.tools.map((t) => t.name).sort();
  assert.deepEqual(names, [
    'icp_capabilities',
    'icp_escrow_state',
    'icp_fulfill',
    'icp_intent_build_and_sign',
    'icp_intent_submit',
    'icp_keypair_generate',
    'icp_quote_accept',
    'icp_settlement_get',
  ]);
});

test('icp_capabilities reports merchant info + allowlist', async () => {
  const caps = await client.tool('icp_capabilities', {});
  assert.equal(caps.spec, 'icp-1.0');
  assert.equal(caps.server, 'icp-mcp');
  assert.ok(caps.merchant_aid.startsWith('aid:v1:'));
  assert.ok(caps.settler_allowlist.includes('settler:stateset.usdc.base-sepolia'));
});

test('full ICP lifecycle via MCP tools: keypair → sign → submit → accept → fulfill → settlement', async () => {
  // 1. Generate buyer identity
  const kp = await client.tool('icp_keypair_generate', {});
  assert.ok(kp.aid.startsWith('aid:v1:'));
  assert.equal(Buffer.from(kp.ed25519_seed_hex, 'hex').length, 32);

  // 2. Build and sign Intent
  const signed = await client.tool('icp_intent_build_and_sign', {
    ed25519_seed_hex: kp.ed25519_seed_hex,
    x25519_pubkey_hex: kp.x25519_pubkey_hex,
    merchant_aid: 'aid:v1:zSomeMerchantPlaceholder',
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'SKU-MCP-001', quantity: 1, unit_price: { amount: '100.00', currency: 'USDC' } }],
    max_total: { amount: '110.00', currency: 'USDC' },
  });
  assert.equal(signed.intent.v, 'icp-1.0');
  assert.equal(signed.intent.verb, 'purchase.create');
  assert.equal(signed.intent.buyer, kp.aid);
  assert.equal(signed.signature.alg, 'ed25519');
  assert.equal(Buffer.from(signed.signature.sig, 'hex').length, 64);

  // 3. Submit Intent → get Quote
  const submitted = await client.tool('icp_intent_submit', {
    intent: signed.intent,
    signature: signed.signature,
    _pubkey_hex: signed._pubkey_hex,
  });
  assert.ok(submitted.quote, JSON.stringify(submitted));
  assert.equal(submitted.quote.intent_id, signed.intent.intent_id);
  assert.equal(submitted.quote.total.amount, '105.00'); // 100 * 1.05

  // 4. Accept Quote → funding instructions
  const accepted = await client.tool('icp_quote_accept', { quote_id: submitted.quote.quote_id });
  assert.ok(accepted.funding.escrow_id.startsWith('0x'));

  // 5. Fulfill → SettlementReceipt
  const fulfilled = await client.tool('icp_fulfill', {
    escrow_id: accepted.funding.escrow_id,
    evidence_id: 'icp_ful_MCP_TEST',
  });
  assert.equal(fulfilled.receipt.final_state, 'released');
  assert.equal(fulfilled.receipt.amount.amount, '105.00');

  // 6. Inspect escrow state — should be released with 4 events
  const state = await client.tool('icp_escrow_state', { escrow_id: accepted.funding.escrow_id });
  assert.equal(state.state, 'released');
  assert.equal(state.events.length, 4); // pending, funded, fulfilled, released
  assert.equal(state.events[0].to_state, 'pending');
  assert.equal(state.events[3].to_state, 'released');

  // 7. Fetch SettlementReceipt by ID
  const fetched = await client.tool('icp_settlement_get', { settlement_id: fulfilled.receipt.settlement_id });
  assert.equal(fetched.receipt.settlement_id, fulfilled.receipt.settlement_id);
});

test('unsigned/wrong-sig Intent is rejected via MCP tool error result', async () => {
  const kp = await client.tool('icp_keypair_generate', {});
  const signed = await client.tool('icp_intent_build_and_sign', {
    ed25519_seed_hex: kp.ed25519_seed_hex,
    x25519_pubkey_hex: kp.x25519_pubkey_hex,
    merchant_aid: 'aid:v1:zM',
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'X', quantity: 1, unit_price: { amount: '1', currency: 'USDC' } }],
    max_total: { amount: '2', currency: 'USDC' },
  });
  signed.signature.sig = '00'.repeat(64);
  const r = await client.tool('icp_intent_submit', {
    intent: signed.intent,
    signature: signed.signature,
    _pubkey_hex: signed._pubkey_hex,
  });
  assert.equal(r.error.code, 'signature.invalid');
});

test('disallowed settler is rejected via MCP tool error result', async () => {
  const kp = await client.tool('icp_keypair_generate', {});
  const signed = await client.tool('icp_intent_build_and_sign', {
    ed25519_seed_hex: kp.ed25519_seed_hex,
    x25519_pubkey_hex: kp.x25519_pubkey_hex,
    merchant_aid: 'aid:v1:zM',
    settler: 'settler:evil.fake.network',
    items: [{ sku: 'X', quantity: 1, unit_price: { amount: '1', currency: 'USDC' } }],
    max_total: { amount: '2', currency: 'USDC' },
  });
  const r = await client.tool('icp_intent_submit', {
    intent: signed.intent,
    signature: signed.signature,
    _pubkey_hex: signed._pubkey_hex,
  });
  assert.equal(r.error.code, 'policy.settler.not_allowed');
});
