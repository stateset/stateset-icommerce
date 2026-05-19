// Guard test: openapi.yaml MUST stay in sync with the actual handler routes.
//
// Cheap drift detector — no YAML lib dependency. Treats the YAML as text,
// pattern-matches paths + method-under-path nesting, and asserts each
// (method, normalized-path) tuple from the handler appears in the spec
// and vice versa. Catches the "added a route, forgot the OpenAPI doc"
// regression that would otherwise quietly break partner codegen.
//
// Run: node --test test/openapi-sync.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

// Hand-rolled tuple list from src/server.mjs `routes` array. Keep in sync.
// Path patterns use OpenAPI-style `{param}` notation.
const HANDLER_ROUTES = [
  ['GET',  '/healthz'],
  ['GET',  '/icp/v1/.well-known/icp'],
  ['GET',  '/icp/v1/settlers'],
  ['POST', '/icp/v1/intents'],
  ['POST', '/icp/v1/quotes/{quote_id}/accept'],
  ['POST', '/icp/v1/escrows/{escrow_id}/fulfill'],
  ['POST', '/icp/v1/escrows/{escrow_id}/dispute'],
  ['GET',  '/icp/v1/escrows/{escrow_id}/events'],
  ['GET',  '/icp/v1/settlements/{settlement_id}'],
  ['GET',  '/icp/v1/channels/{channel_id}'],
  ['GET',  '/icp/v1/channels/{channel_id}/events'],
];

// Map handler routes back to their regex pattern in server.mjs so this
// test fails loudly if someone edits the source without updating both.
// We grep server.mjs for path-literal occurrences (string OR regex form).
const SOURCE_PATH_PATTERNS = [
  { method: 'GET',  literal: "'/healthz'" },
  { method: 'GET',  literal: "'/icp/v1/.well-known/icp'" },
  { method: 'GET',  literal: "'/icp/v1/settlers'" },
  { method: 'POST', literal: "'/icp/v1/intents'" },
  { method: 'POST', literal: '/^\\/icp\\/v1\\/quotes\\/([^/]+)\\/accept$/' },
  { method: 'POST', literal: '/^\\/icp\\/v1\\/escrows\\/([^/]+)\\/fulfill$/' },
  { method: 'POST', literal: '/^\\/icp\\/v1\\/escrows\\/([^/]+)\\/dispute$/' },
  { method: 'GET',  literal: '/^\\/icp\\/v1\\/escrows\\/([^/]+)\\/events$/' },
  { method: 'GET',  literal: '/^\\/icp\\/v1\\/settlements\\/([^/]+)$/' },
  { method: 'GET',  literal: '/^\\/icp\\/v1\\/channels\\/([^/]+)$/' },
  { method: 'GET',  literal: '/^\\/icp\\/v1\\/channels\\/([^/]+)\\/events$/' },
];

test('openapi.yaml exists and is non-empty', () => {
  const yaml = readFileSync(join(ROOT, 'openapi.yaml'), 'utf8');
  assert.ok(yaml.length > 1000, 'openapi.yaml must be substantive');
  assert.match(yaml, /^openapi: 3\.1\.0$/m, 'must declare OpenAPI 3.1.0');
  assert.match(yaml, /paths:/, 'must have paths section');
  assert.match(yaml, /components:/, 'must have components section');
});

test('every handler route appears in openapi.yaml', () => {
  const yaml = readFileSync(join(ROOT, 'openapi.yaml'), 'utf8');
  for (const [method, path] of HANDLER_ROUTES) {
    // Build a regex matching `  /path...:` at indent level 2 followed
    // somewhere downstream by the lowercased method as a key.
    const pathLine = new RegExp(`^  ${path.replace(/[.]/g, '\\.').replace(/[{}]/g, '\\$&')}:`, 'm');
    assert.match(yaml, pathLine, `openapi.yaml missing path: ${path}`);

    // Find the path's block and search until the next top-level path entry.
    const startIdx = yaml.search(pathLine);
    assert.ok(startIdx > -1, `path block for ${path} not located`);
    const rest = yaml.slice(startIdx);
    // Next sibling path or end of paths section.
    const nextPath = rest.slice(1).search(/^  \/[^ ]/m);
    const block = nextPath === -1 ? rest : rest.slice(0, nextPath + 1);
    assert.match(
      block,
      new RegExp(`^    ${method.toLowerCase()}:`, 'm'),
      `openapi.yaml path ${path} missing ${method} operation`,
    );
  }
});

test('every openapi.yaml route is implemented in server.mjs', () => {
  const source = readFileSync(join(ROOT, 'src/server.mjs'), 'utf8');
  for (const { method, literal } of SOURCE_PATH_PATTERNS) {
    // Find the literal somewhere in source AND a sibling literal of the same method.
    assert.ok(
      source.includes(literal),
      `server.mjs missing route literal: ${literal}`,
    );
    assert.ok(
      source.includes(`'${method}'`) || source.includes(`"${method}"`),
      `server.mjs missing method literal: ${method}`,
    );
  }
});

test('all 7 commerce verbs (plus channel.register) appear in the IntentBase verb enum', () => {
  const yaml = readFileSync(join(ROOT, 'openapi.yaml'), 'utf8');
  const verbs = [
    'purchase.create',
    'purchase.return',
    'subscription.create',
    'subscription.cancel',
    'inventory.query',
    'quote.request',
    'payout.request',
    'channel.register',
  ];
  for (const verb of verbs) {
    // Verbs now appear as YAML enum values (`- purchase.create`) on the
    // IntentBase.verb enum, not as discriminator keys.
    assert.match(
      yaml,
      new RegExp(`- ${verb.replace('.', '\\.')}\\b`),
      `IntentBase.verb enum missing: ${verb}`,
    );
  }
});

test('IntentEnvelope and IntentBase match handler wire reality', () => {
  const yaml = readFileSync(join(ROOT, 'openapi.yaml'), 'utf8');

  // IntentEnvelope: handler requires {intent, signature}, not {intent, auth}.
  assert.match(yaml, /IntentEnvelope:\s*\n\s*type: object\s*\n\s*required: \[intent, signature\]/);

  // IntentBase: handler keys are v / intent_id / merchant / settler / expiry /
  // iat / exp (RFC 3339), not version / unix-seconds exp.
  assert.match(yaml, /IntentBase:[\s\S]*?required: \[v, verb, intent_id, buyer, merchant, settler/);

  // PrincipalBinding: handler uses `authority` not `authority_caps`.
  assert.match(
    yaml,
    /PrincipalBinding:[\s\S]*?required: \[principal, agent, authority, expiry, revocation, signature\]/,
  );

  // Signature schema is its own definition (reused for envelope + responses).
  assert.match(yaml, /Signature:\s*\n\s*type: object\s*\n\s*required: \[alg, kid, sig\]/);

  // Authority schema is referenced from PrincipalBinding.
  assert.match(yaml, /Authority:\s*\n\s*type: object\s*\n\s*required: \[max_per_intent, verbs\]/);
});

test('verb response shapes are wrapped { <payload_key>, signature } per handler reality', () => {
  const yaml = readFileSync(join(ROOT, 'openapi.yaml'), 'utf8');
  // Each verb has a wrapper schema with the correct payload key + a
  // `signature` field that references the shared Signature schema.
  const wrappers = [
    ['PurchaseCreateResponse', 'quote'],
    ['PurchaseReturnResponse', 'authorization'],
    ['SubscriptionCreateResponse', 'authorization'],
    ['SubscriptionCancelResponse', 'authorization'],
    ['InventoryQueryResponse', 'snapshot'],
    ['QuoteRequestResponse', 'proposal'],
    ['PayoutRequestResponse', 'authorization'],
    ['ChannelRegisterResponse', 'channel'],
  ];
  for (const [schema, payloadKey] of wrappers) {
    const reqPattern = new RegExp(
      `${schema}:\\s*\\n\\s*type: object\\s*\\n\\s*required: \\[${payloadKey}, signature\\]`,
    );
    assert.match(yaml, reqPattern, `${schema} missing required: [${payloadKey}, signature]`);
  }
  // No stale `signature_hex` top-level field in response schemas.
  // (It still appears in SSE example data: lines, which we tolerate.)
  const flatLines = yaml
    .split('\n')
    .filter((l) => l.match(/required: \[.*signature_hex\]/));
  assert.equal(
    flatLines.length,
    0,
    `flat signature_hex response shape lingers: ${flatLines.join(' | ')}`,
  );
});

test('WellKnown matches handler wire reality', () => {
  const yaml = readFileSync(join(ROOT, 'openapi.yaml'), 'utf8');
  // Handler returns {spec, handler, handler_version, merchant_aid,
  // merchant_pubkey, capabilities, settler_allowlist, docs?}. Validate.
  assert.match(
    yaml,
    /WellKnown:[\s\S]*?required: \[spec, handler, handler_version, merchant_aid, merchant_pubkey, capabilities, settler_allowlist\]/,
  );
  // merchant_pubkey is a {alg, raw_hex} object, NOT a flat ed25519_pubkey_hex string.
  assert.match(yaml, /merchant_pubkey:\s*\n\s*type: object\s*\n\s*required: \[alg, raw_hex\]/);
  // capabilities is a nested object with verbs + transports + push_channels.
  assert.match(yaml, /capabilities:\s*\n\s*type: object\s*\n\s*required: \[verbs, transports\]/);
  // No stale flat ed25519_pubkey_hex / x25519_pubkey_hex top-level fields.
  assert.doesNotMatch(yaml, /^\s*ed25519_pubkey_hex: \{ type: string, pattern:/m);
  assert.doesNotMatch(yaml, /^\s*x25519_pubkey_hex: \{ type: string, pattern:/m);
});

test('error responses cover the primary error-code namespaces', () => {
  const yaml = readFileSync(join(ROOT, 'openapi.yaml'), 'utf8');
  const namespaces = [
    'signature.invalid',
    'policy.settler.not_allowed',
    'replay.duplicate_nonce',
    'replay.expired_window',
    'rate.too_many_requests',
    'settler.unavailable',
    'escrow.invalid_state',
    'format.missing_field',
    'format.not_found',
  ];
  for (const code of namespaces) {
    assert.match(yaml, new RegExp(code.replace(/\./g, '\\.')), `error code missing: ${code}`);
  }
});
