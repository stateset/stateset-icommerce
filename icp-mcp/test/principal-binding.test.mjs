// The binding this MCP server builds must satisfy an ENFORCING handler.
//
// The server used to stamp every Intent with `kid: 'self'`, `sig: 'deadbeef'`
// under a made-up principal. It looked like a delegation, proved nothing, and
// meant an LLM driving these tools could only ever transact against a handler
// with delegation checking switched off. Now the server signs a real binding
// with an operator-configured principal key — and carries none at all when
// none is configured, which is the honest answer.
//
// Run: node --test test/principal-binding.test.mjs

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

import {
  generatePrincipalIdentity,
  canonicalJson,
  verifyEd25519,
} from '../../packages/icp-client/src/index.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SERVER = resolve(__dirname, '..', 'src', 'server.mjs');

const PRINCIPAL = 'did:web:mcp-binding.example';
const MERCHANT = 'aid:v1:zMcpBindingTestMerchant';
const SETTLER = 'settler:stateset.usdc.base-sepolia';
const principalIdentity = generatePrincipalIdentity();

/** Minimal JSON-RPC-over-stdio client, same shape as mcp.test.mjs. */
class McpClient {
  constructor(env = {}) {
    this.proc = spawn('node', [SERVER], { stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, ...env } });
    this.stderr = '';
    this.proc.stderr.on('data', (chunk) => { this.stderr += chunk; });
    // Bounded: a server that neither starts nor exits is itself a failure,
    // and an unbounded await would hang the suite instead of reporting it.
    this.exit = new Promise((r) => {
      const timer = setTimeout(() => r('did-not-exit'), 8000);
      this.proc.once('exit', (code) => { clearTimeout(timer); r(code); });
    });
    this.rl = createInterface({ input: this.proc.stdout });
    this.pending = new Map();
    this.nextId = 1;
    this.rl.on('line', (line) => {
      if (!line.trim()) return;
      let msg;
      try { msg = JSON.parse(line); } catch { return; }
      const resolver = this.pending.get(msg.id);
      if (resolver) { this.pending.delete(msg.id); resolver(msg); }
    });
  }

  call(method, params) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, resolve);
      this.proc.stdin.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`);
      setTimeout(() => {
        if (this.pending.has(id)) { this.pending.delete(id); reject(new Error(`timeout: ${method}`)); }
      }, 5000);
    });
  }

  async tool(name, args) {
    const r = await this.call('tools/call', { name, arguments: args });
    if (r.error) throw new Error(`MCP error: ${r.error.message}`);
    return JSON.parse(r.result.content[0].text);
  }

  close() { this.proc.kill(); }
}

const buildArgs = (kp) => ({
  ed25519_seed_hex: kp.ed25519_seed_hex,
  x25519_pubkey_hex: kp.x25519_pubkey_hex,
  merchant_aid: MERCHANT,
  settler: SETTLER,
  items: [{ sku: 'WIDGET-001', quantity: 1, unit_price: { amount: '10.00', currency: 'USDC' } }],
  max_total: { amount: '20.00', currency: 'USDC' },
});

// A delegated server signs the Intent first; only then can the handler be
// started with the agent key it must admit (an operator-keyed handler accepts
// only signer AIDs it was configured with, and the module reads env once).
const delegated = new McpClient({
  ICP_MCP_PRINCIPAL: PRINCIPAL,
  ICP_MCP_PRINCIPAL_SEED_HEX: principalIdentity.ed25519_seed.toString('hex'),
});
const kp = await delegated.tool('icp_keypair_generate', {});
const signed = await delegated.tool('icp_intent_build_and_sign', buildArgs(kp));

process.env.PORT ??= '0';
process.env.ICP_TRUST_MODE = 'enforce';
process.env.ICP_MERCHANT_AID = MERCHANT;
process.env.ICP_PRINCIPAL_KEYS_JSON = JSON.stringify({
  [PRINCIPAL]: principalIdentity.ed25519_pubkey.toString('hex'),
});
process.env.ICP_AGENT_KEYS_JSON = JSON.stringify({ [kp.aid]: signed._pubkey_hex });
const { server } = await import('../../icp-handler/src/server.mjs');

let handlerUrl;
before(async () => {
  if (!server.listening) await new Promise((r) => server.once('listening', r));
  handlerUrl = `http://127.0.0.1:${server.address().port}`;
});
after(() => { delegated.close(); server.close(); });

const submit = (envelope) =>
  fetch(`${handlerUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(envelope),
  }).then(async (r) => ({ status: r.status, body: await r.json() }));

test('the binding it signs verifies against the registered principal key', { timeout: 20000 }, async () => {
  const binding = signed.intent.principal_binding;
  assert.ok(binding, 'a configured server must carry a binding');
  assert.equal(binding.principal, PRINCIPAL);
  assert.equal(binding.agent, kp.aid);
  assert.equal(binding.signature.alg, 'ed25519');
  assert.equal(binding.signature.kid, PRINCIPAL, 'kid names the principal, not "self"');
  assert.notEqual(binding.signature.sig, 'deadbeef');
  assert.ok(binding.authority.verbs.includes('purchase.create'));

  // The bytes the handler will re-derive: the binding minus its signature.
  const { signature, ...unsigned } = binding;
  assert.ok(
    verifyEd25519(canonicalJson(unsigned), signature.sig, principalIdentity.ed25519_pubkey),
    'binding must verify under the principal key an operator registered',
  );
});

test('an enforcing handler quotes the Intent it built', { timeout: 20000 }, async () => {
  const { status, body } = await submit({
    intent: signed.intent,
    signature: signed.signature,
    _pubkey_hex: signed._pubkey_hex,
    _x_pubkey_hex: kp.x25519_pubkey_hex,
  });
  assert.equal(status, 200, JSON.stringify(body));
  assert.ok(body.quote?.quote_id, JSON.stringify(body));
});

test('the Intent signature covers the binding, so it cannot be widened in flight', { timeout: 20000 }, async () => {
  // An MCP server hands the signed envelope back to an LLM, which relays it.
  // Anything that edits the binding on the way through must break the Intent
  // signature — otherwise a relay could quietly raise its own ceiling.
  const tampered = structuredClone(signed);
  tampered.intent.principal_binding.authority.max_per_intent = {
    amount: '1000000',
    currency: 'USDC',
  };
  const { body } = await submit({
    intent: tampered.intent,
    signature: tampered.signature,
    _pubkey_hex: signed._pubkey_hex,
    _x_pubkey_hex: kp.x25519_pubkey_hex,
  });
  assert.equal(body.code, 'signature.invalid', JSON.stringify(body));
});

test('with no principal configured the Intent carries NO binding at all', { timeout: 20000 }, async () => {
  const plain = new McpClient({ ICP_MCP_PRINCIPAL: '', ICP_MCP_PRINCIPAL_SEED_HEX: '' });
  try {
    // Same agent identity as above: an operator-keyed handler admits only the
    // signer AIDs it was configured with, and this test is about the
    // delegation gate, not the admission one.
    const out = await plain.tool('icp_intent_build_and_sign', buildArgs(kp));
    assert.equal(
      'principal_binding' in out.intent,
      false,
      'the key must be ABSENT, not null: canonical bytes cover what is there',
    );
    // …and the enforcing handler says so plainly.
    const { status, body } = await submit({
      intent: out.intent,
      signature: out.signature,
      _pubkey_hex: out._pubkey_hex,
      _x_pubkey_hex: kp.x25519_pubkey_hex,
    });
    assert.equal(status, 403, JSON.stringify(body));
    assert.equal(body.code, 'delegation.required');
  } finally {
    plain.close();
  }
});

test('capabilities report whether this server holds a delegation', { timeout: 20000 }, async () => {
  const caps = await delegated.tool('icp_capabilities', {});
  assert.equal(caps.principal, PRINCIPAL);
});

test('a half-configured principal fails fast instead of transacting undelegated', { timeout: 20000 }, async () => {
  const broken = new McpClient({ ICP_MCP_PRINCIPAL: PRINCIPAL, ICP_MCP_PRINCIPAL_SEED_HEX: '' });
  const code = await broken.exit;
  broken.close();
  assert.notEqual(code, 0, 'server must refuse to start');
  assert.match(broken.stderr, /ICP_MCP_PRINCIPAL_SEED_HEX/);

  const badSeed = new McpClient({ ICP_MCP_PRINCIPAL: PRINCIPAL, ICP_MCP_PRINCIPAL_SEED_HEX: 'ab' });
  const badCode = await badSeed.exit;
  badSeed.close();
  assert.notEqual(badCode, 0, 'a short seed must not be padded into a key');
});
