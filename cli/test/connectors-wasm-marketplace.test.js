import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { mkdtemp, rm, writeFile, readFile, mkdir } from 'node:fs/promises';
import { createHmac, createHash } from 'node:crypto';

import {
  CONNECTOR_SCHEMA_VERSION,
  CONNECTOR_CATALOG_SCHEMA_VERSION,
  CONNECTOR_RUNTIME_KINDS,
  getConnectorHome,
  listConnectorMarketplace,
  publishConnector,
  installConnector,
  uninstallConnector,
  listInstalledConnectors,
  getInstalledConnector,
  signConnectorAttestation,
  verifyConnectorAttestation,
  assessConnectorSafety,
  certifyConnector,
  executeInstalledConnectorAction,
  __resetWasmConnectorState,
} from '../src/connectors/wasm-marketplace.js';

// ---------------------------------------------------------------------------
// Minimal valid WASM module: exports a single function `add(i32, i32) -> i32`
// ---------------------------------------------------------------------------
const ADD_WASM_BYTES = Buffer.from([
  0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
  0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
  0x03, 0x02, 0x01, 0x00,
  0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
  0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
]);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function sha256Hex(data) {
  return createHash('sha256').update(data).digest('hex');
}

// Save env vars, clear connector-related ones, restore in afterEach.
function makeEnvGuard() {
  let saved = {};
  const KEYS = [
    'STATESET_CONNECTOR_HOME',
    'STATESET_CONNECTOR_SIGNING_KEY',
    'STATESET_CONNECTOR_VERIFY_STRICT',
    'STATESET_CONNECTOR_REQUIRE_CERTIFIED',
    'STATESET_CONNECTOR_MIN_SAFETY_SCORE',
  ];
  return {
    save() {
      for (const k of KEYS) saved[k] = process.env[k];
      for (const k of KEYS) delete process.env[k];
    },
    restore() {
      for (const k of KEYS) {
        if (saved[k] === undefined) delete process.env[k];
        else process.env[k] = saved[k];
      }
      saved = {};
    },
  };
}

const envGuard = makeEnvGuard();

// ---------------------------------------------------------------------------
// Test fixture: publish a connector and return its entry
// ---------------------------------------------------------------------------
async function publishAddConnector(connectorHome, {
  connectorId = 'math.add',
  version = '1.0.0',
  actions = [{ name: 'add', exportName: 'add', args: ['a', 'b'] }],
  publisher = 'test-publisher',
  tags = ['math'],
  description = 'A minimal addition connector for testing purposes.',
} = {}) {
  const wasmPath = path.join(connectorHome, `${connectorId.replace(/\./g, '-')}-${version}.wasm`);
  await writeFile(wasmPath, ADD_WASM_BYTES);
  return publishConnector({
    connectorHome,
    connectorId,
    version,
    wasmPath,
    runtimeKind: 'native-export',
    actions,
    publisher,
    tags,
    description,
    name: connectorId,
  });
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

describe('exported constants', () => {
  it('CONNECTOR_SCHEMA_VERSION is the expected string', () => {
    assert.equal(CONNECTOR_SCHEMA_VERSION, 'wasm-connector/v1');
  });

  it('CONNECTOR_CATALOG_SCHEMA_VERSION is the expected string', () => {
    assert.equal(CONNECTOR_CATALOG_SCHEMA_VERSION, 'wasm-catalog/v1');
  });

  it('CONNECTOR_RUNTIME_KINDS contains native-export and wasi-command', () => {
    assert.ok(Array.isArray(CONNECTOR_RUNTIME_KINDS));
    assert.ok(CONNECTOR_RUNTIME_KINDS.includes('native-export'));
    assert.ok(CONNECTOR_RUNTIME_KINDS.includes('wasi-command'));
  });
});

// ---------------------------------------------------------------------------
// getConnectorHome
// ---------------------------------------------------------------------------

describe('getConnectorHome', () => {
  afterEach(() => {
    delete process.env.STATESET_CONNECTOR_HOME;
  });

  it('returns absolute path from env var when set', () => {
    process.env.STATESET_CONNECTOR_HOME = '/tmp/my-connectors';
    const result = getConnectorHome();
    assert.equal(result, '/tmp/my-connectors');
  });

  it('returns value from options.connectorHome when provided', () => {
    const result = getConnectorHome({ connectorHome: '/tmp/explicit-home' });
    assert.equal(result, '/tmp/explicit-home');
  });

  it('options.connectorHome takes precedence over env var', () => {
    process.env.STATESET_CONNECTOR_HOME = '/tmp/env-home';
    const result = getConnectorHome({ connectorHome: '/tmp/option-home' });
    assert.equal(result, '/tmp/option-home');
  });

  it('falls back to .stateset/connectors in cwd when nothing is set', () => {
    delete process.env.STATESET_CONNECTOR_HOME;
    const result = getConnectorHome();
    assert.ok(result.endsWith(path.join('.stateset', 'connectors')));
  });
});

// ---------------------------------------------------------------------------
// Catalog management — listConnectorMarketplace
// ---------------------------------------------------------------------------

describe('listConnectorMarketplace', () => {
  let connectorHome;

  beforeEach(async () => {
    __resetWasmConnectorState();
    envGuard.save();
    connectorHome = await mkdtemp(path.join(tmpdir(), 'wasm-mkt-list-'));
    process.env.STATESET_CONNECTOR_HOME = connectorHome;
  });

  afterEach(async () => {
    __resetWasmConnectorState();
    envGuard.restore();
    await rm(connectorHome, { recursive: true, force: true });
  });

  it('returns empty list when catalog does not exist', async () => {
    const result = await listConnectorMarketplace({ connectorHome });
    assert.equal(result.success, true);
    assert.equal(result.total, 0);
    assert.deepEqual(result.connectors, []);
  });

  it('returns published connectors', async () => {
    await publishAddConnector(connectorHome);
    const result = await listConnectorMarketplace({ connectorHome });
    assert.equal(result.success, true);
    assert.equal(result.total, 1);
    assert.equal(result.connectors[0].id, 'math.add');
  });

  it('filters by connectorId', async () => {
    await publishAddConnector(connectorHome, { connectorId: 'math.add' });
    await publishAddConnector(connectorHome, { connectorId: 'math.mul', version: '1.0.0',
      actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }] });
    const result = await listConnectorMarketplace({ connectorHome, connectorId: 'math.add' });
    assert.equal(result.total, 1);
    assert.equal(result.connectors[0].id, 'math.add');
  });

  it('filters by tag', async () => {
    await publishAddConnector(connectorHome, { connectorId: 'math.add', tags: ['math', 'core'] });
    await publishAddConnector(connectorHome, { connectorId: 'math.mul', version: '1.0.0',
      actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }], tags: ['other'] });
    const result = await listConnectorMarketplace({ connectorHome, tag: 'math' });
    assert.equal(result.total, 1);
    assert.equal(result.connectors[0].id, 'math.add');
  });

  it('filters by text query matching description', async () => {
    await publishAddConnector(connectorHome, {
      description: 'A connector that performs integer addition.',
    });
    const result = await listConnectorMarketplace({ connectorHome, query: 'integer addition' });
    assert.equal(result.total, 1);
  });

  it('returns empty results when query does not match', async () => {
    await publishAddConnector(connectorHome);
    const result = await listConnectorMarketplace({ connectorHome, query: 'zzznomatchzzz' });
    assert.equal(result.total, 0);
  });

  it('respects limit parameter', async () => {
    // Publish multiple versions
    for (const v of ['1.0.0', '1.1.0', '1.2.0']) {
      await publishAddConnector(connectorHome, { connectorId: 'math.add', version: v, actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }], publisher: 'p', tags: [], description: 'x'.repeat(20) });
    }
    const result = await listConnectorMarketplace({ connectorHome, limit: 2 });
    assert.ok(result.connectors.length <= 2);
  });

  it('returns connectorHome in the result', async () => {
    const result = await listConnectorMarketplace({ connectorHome });
    assert.equal(result.connectorHome, connectorHome);
  });
});

// ---------------------------------------------------------------------------
// publishConnector
// ---------------------------------------------------------------------------

describe('publishConnector', () => {
  let connectorHome;

  beforeEach(async () => {
    __resetWasmConnectorState();
    envGuard.save();
    connectorHome = await mkdtemp(path.join(tmpdir(), 'wasm-mkt-pub-'));
    process.env.STATESET_CONNECTOR_HOME = connectorHome;
  });

  afterEach(async () => {
    __resetWasmConnectorState();
    envGuard.restore();
    await rm(connectorHome, { recursive: true, force: true });
  });

  it('publishes a connector and returns success with entry', async () => {
    const result = await publishAddConnector(connectorHome);
    assert.equal(result.success, true);
    assert.ok(result.connector);
    assert.equal(result.connector.id, 'math.add');
    assert.equal(result.connector.version, '1.0.0');
  });

  it('entry has correct schemaVersion', async () => {
    const result = await publishAddConnector(connectorHome);
    assert.equal(result.connector.schemaVersion, CONNECTOR_SCHEMA_VERSION);
  });

  it('entry has a wasmSha256 hash', async () => {
    const result = await publishAddConnector(connectorHome);
    const expectedHash = sha256Hex(ADD_WASM_BYTES);
    assert.equal(result.connector.wasmSha256, expectedHash);
  });

  it('entry has attestation with deterministic-sha256 when no signing key', async () => {
    const result = await publishAddConnector(connectorHome);
    assert.ok(result.connector.attestation);
    assert.equal(result.connector.attestation.algorithm, 'deterministic-sha256');
  });

  it('entry has attestation with hmac-sha256 when signing key set', async () => {
    process.env.STATESET_CONNECTOR_SIGNING_KEY = 'my-secret-key';
    const result = await publishAddConnector(connectorHome);
    assert.equal(result.connector.attestation.algorithm, 'hmac-sha256');
    assert.equal(result.connector.attestation.signedBy, 'local-signing-key');
  });

  it('entry has safetyAssessment', async () => {
    const result = await publishAddConnector(connectorHome);
    assert.ok(result.connector.safetyAssessment);
    assert.ok(typeof result.connector.safetyAssessment.score === 'number');
  });

  it('entry has tags array', async () => {
    const result = await publishAddConnector(connectorHome, { tags: ['math', 'util'] });
    assert.deepEqual(result.connector.tags, ['math', 'util']);
  });

  it('throws when WASM file does not exist', async () => {
    await assert.rejects(
      () => publishConnector({
        connectorHome,
        connectorId: 'missing.wasm',
        version: '1.0.0',
        wasmPath: '/tmp/does-not-exist-xyz.wasm',
        runtimeKind: 'native-export',
        actions: [],
      }),
      /WASM file does not exist/,
    );
  });

  it('throws on invalid connectorId', async () => {
    const wasmPath = path.join(connectorHome, 'test.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);
    await assert.rejects(
      () => publishConnector({ connectorHome, connectorId: 'INVALID ID!', version: '1.0.0', wasmPath }),
      /Invalid connectorId/,
    );
  });

  it('throws on invalid version', async () => {
    const wasmPath = path.join(connectorHome, 'test.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);
    await assert.rejects(
      () => publishConnector({ connectorHome, connectorId: 'math.add', version: '', wasmPath }),
      /Invalid version/,
    );
  });

  it('throws when connector already exists without force=true', async () => {
    await publishAddConnector(connectorHome);
    await assert.rejects(
      () => publishAddConnector(connectorHome),
      /already exists in catalog/,
    );
  });

  it('overwrites existing connector entry when force=true', async () => {
    await publishAddConnector(connectorHome);
    const wasmPath = path.join(connectorHome, 'math-add-1.0.0.wasm');
    const result = await publishConnector({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      wasmPath,
      runtimeKind: 'native-export',
      actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }],
      force: true,
    });
    assert.equal(result.success, true);
  });

  it('catalog is persisted to disk', async () => {
    await publishAddConnector(connectorHome);
    const catalogPath = path.join(connectorHome, 'catalog.json');
    const raw = await readFile(catalogPath, 'utf8');
    const catalog = JSON.parse(raw);
    assert.equal(catalog.schemaVersion, CONNECTOR_CATALOG_SCHEMA_VERSION);
    assert.ok(Array.isArray(catalog.connectors));
    assert.equal(catalog.connectors.length, 1);
  });

  it('wasi-command runtime with no declared actions gets default run action', async () => {
    const wasmPath = path.join(connectorHome, 'wasi.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);
    const result = await publishConnector({
      connectorHome,
      connectorId: 'my.wasi',
      version: '1.0.0',
      wasmPath,
      runtimeKind: 'wasi-command',
      actions: [],
    });
    assert.equal(result.success, true);
    assert.equal(result.connector.actions.length, 1);
    assert.equal(result.connector.actions[0].name, 'run');
  });

  it('throws on invalid runtime kind', async () => {
    const wasmPath = path.join(connectorHome, 'test.wasm');
    await writeFile(wasmPath, ADD_WASM_BYTES);
    await assert.rejects(
      () => publishConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0', wasmPath, runtimeKind: 'invalid-kind' }),
      /Unsupported runtime kind/,
    );
  });
});

// ---------------------------------------------------------------------------
// Attestation — signConnectorAttestation / verifyConnectorAttestation
// ---------------------------------------------------------------------------

describe('attestation signing and verification', () => {
  let connectorHome;

  beforeEach(async () => {
    __resetWasmConnectorState();
    envGuard.save();
    connectorHome = await mkdtemp(path.join(tmpdir(), 'wasm-mkt-attest-'));
    process.env.STATESET_CONNECTOR_HOME = connectorHome;
  });

  afterEach(async () => {
    __resetWasmConnectorState();
    envGuard.restore();
    await rm(connectorHome, { recursive: true, force: true });
  });

  it('signConnectorAttestation throws when no signing key', async () => {
    await publishAddConnector(connectorHome);
    await assert.rejects(
      () => signConnectorAttestation({ connectorHome, connectorId: 'math.add', version: '1.0.0' }),
      /signing key is required/i,
    );
  });

  it('signConnectorAttestation succeeds with key and updates catalog', async () => {
    await publishAddConnector(connectorHome);
    const result = await signConnectorAttestation({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      signingKey: 'test-secret',
    });
    assert.equal(result.success, true);
    assert.equal(result.connector.attestation.algorithm, 'hmac-sha256');
    assert.ok(result.verification.valid);
  });

  it('signConnectorAttestation records keyId and signedBy when provided', async () => {
    await publishAddConnector(connectorHome);
    const result = await signConnectorAttestation({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      signingKey: 'test-secret',
      keyId: 'key-123',
      signedBy: 'alice',
    });
    assert.equal(result.connector.attestation.keyId, 'key-123');
    assert.equal(result.connector.attestation.signedBy, 'alice');
  });

  it('verifyConnectorAttestation returns valid for unsigned connector', async () => {
    await publishAddConnector(connectorHome);
    const result = await verifyConnectorAttestation({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
    });
    assert.equal(result.success, true);
    assert.equal(result.verification.valid, true);
    assert.equal(result.verification.algorithm, 'deterministic-sha256');
  });

  it('verifyConnectorAttestation returns valid for HMAC-signed connector', async () => {
    await publishAddConnector(connectorHome);
    await signConnectorAttestation({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      signingKey: 'correct-key',
    });
    const result = await verifyConnectorAttestation({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      signingKey: 'correct-key',
    });
    assert.equal(result.verification.valid, true);
    assert.equal(result.verification.algorithm, 'hmac-sha256');
  });

  it('verifyConnectorAttestation returns invalid when wrong HMAC key', async () => {
    await publishAddConnector(connectorHome);
    await signConnectorAttestation({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      signingKey: 'correct-key',
    });
    const result = await verifyConnectorAttestation({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      signingKey: 'wrong-key',
    });
    assert.equal(result.verification.valid, false);
    assert.equal(result.verification.reason, 'signature_mismatch');
  });

  it('verifyConnectorAttestation returns missing_signing_key when HMAC key absent', async () => {
    await publishAddConnector(connectorHome);
    await signConnectorAttestation({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      signingKey: 'correct-key',
    });
    // No key provided — cannot verify HMAC
    const result = await verifyConnectorAttestation({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
    });
    assert.equal(result.verification.valid, false);
    assert.equal(result.verification.reason, 'missing_signing_key');
  });

  it('verifyConnectorAttestation throws when connector not in catalog', async () => {
    await assert.rejects(
      () => verifyConnectorAttestation({ connectorHome, connectorId: 'no.exist', version: '1.0.0' }),
      /not found in the marketplace catalog/,
    );
  });

  it('env var STATESET_CONNECTOR_SIGNING_KEY is used automatically', async () => {
    process.env.STATESET_CONNECTOR_SIGNING_KEY = 'env-key';
    await publishAddConnector(connectorHome);
    // attestation was built with env key at publish time
    const result = await verifyConnectorAttestation({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
    });
    assert.equal(result.verification.valid, true);
    assert.equal(result.verification.algorithm, 'hmac-sha256');
  });
});

// ---------------------------------------------------------------------------
// Safety assessment — assessConnectorSafety
// ---------------------------------------------------------------------------

describe('assessConnectorSafety', () => {
  let connectorHome;

  beforeEach(async () => {
    __resetWasmConnectorState();
    envGuard.save();
    connectorHome = await mkdtemp(path.join(tmpdir(), 'wasm-mkt-safety-'));
    process.env.STATESET_CONNECTOR_HOME = connectorHome;
  });

  afterEach(async () => {
    __resetWasmConnectorState();
    envGuard.restore();
    await rm(connectorHome, { recursive: true, force: true });
  });

  it('returns a safety assessment with score and tier', async () => {
    await publishAddConnector(connectorHome);
    const result = await assessConnectorSafety({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
    });
    assert.equal(result.success, true);
    assert.ok(typeof result.safetyAssessment.score === 'number');
    assert.ok(['trusted', 'moderate', 'high'].includes(result.safetyAssessment.tier));
  });

  it('safety score is between 0 and 100', async () => {
    await publishAddConnector(connectorHome);
    const result = await assessConnectorSafety({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    const score = result.safetyAssessment.score;
    assert.ok(score >= 0 && score <= 100, `Score ${score} out of range`);
  });

  it('safety assessment contains evidence object', async () => {
    await publishAddConnector(connectorHome);
    const result = await assessConnectorSafety({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    const { evidence } = result.safetyAssessment;
    assert.ok(evidence);
    assert.ok(typeof evidence.actionCount === 'number');
    assert.ok(typeof evidence.attestationValid === 'boolean');
  });

  it('higher score for HMAC-signed vs unsigned connector', async () => {
    // unsigned connector
    await publishAddConnector(connectorHome, { connectorId: 'math.add' });
    const unsignedAssess = await assessConnectorSafety({ connectorHome, connectorId: 'math.add', version: '1.0.0' });

    // signed connector
    await publishAddConnector(connectorHome, { connectorId: 'math.mul', version: '1.0.0',
      actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }] });
    await signConnectorAttestation({ connectorHome, connectorId: 'math.mul', version: '1.0.0', signingKey: 'test-secret' });
    const signedAssess = await assessConnectorSafety({ connectorHome, connectorId: 'math.mul', version: '1.0.0', signingKey: 'test-secret' });

    assert.ok(
      signedAssess.safetyAssessment.score > unsignedAssess.safetyAssessment.score,
      `Signed (${signedAssess.safetyAssessment.score}) should exceed unsigned (${unsignedAssess.safetyAssessment.score})`,
    );
  });

  it('recommendation is certify, review, or block', async () => {
    await publishAddConnector(connectorHome);
    const result = await assessConnectorSafety({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    assert.ok(['certify', 'review', 'block'].includes(result.safetyAssessment.recommendation));
  });
});

// ---------------------------------------------------------------------------
// certifyConnector
// ---------------------------------------------------------------------------

describe('certifyConnector', () => {
  let connectorHome;

  beforeEach(async () => {
    __resetWasmConnectorState();
    envGuard.save();
    connectorHome = await mkdtemp(path.join(tmpdir(), 'wasm-mkt-cert-'));
    process.env.STATESET_CONNECTOR_HOME = connectorHome;
  });

  afterEach(async () => {
    __resetWasmConnectorState();
    envGuard.restore();
    await rm(connectorHome, { recursive: true, force: true });
  });

  it('can certify connector after signing attestation', async () => {
    await publishAddConnector(connectorHome);
    await signConnectorAttestation({ connectorHome, connectorId: 'math.add', version: '1.0.0', signingKey: 'key' });
    const result = await certifyConnector({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      signingKey: 'key',
      minSafetyScore: 0,
    });
    assert.equal(result.success, true);
    assert.equal(result.certification.status, 'certified');
  });

  it('certification level is assigned based on safety score', async () => {
    await publishAddConnector(connectorHome);
    await signConnectorAttestation({ connectorHome, connectorId: 'math.add', version: '1.0.0', signingKey: 'k' });
    const result = await certifyConnector({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      signingKey: 'k',
      minSafetyScore: 0,
    });
    assert.ok(['bronze', 'silver', 'gold', 'platinum'].includes(result.certification.level));
  });

  it('custom level overrides auto-assigned level', async () => {
    await publishAddConnector(connectorHome);
    await signConnectorAttestation({ connectorHome, connectorId: 'math.add', version: '1.0.0', signingKey: 'k' });
    const result = await certifyConnector({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      signingKey: 'k',
      minSafetyScore: 0,
      level: 'gold',
    });
    assert.equal(result.certification.level, 'gold');
  });

  it('throws on invalid status value', async () => {
    await publishAddConnector(connectorHome);
    await assert.rejects(
      () => certifyConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0', status: 'not-a-status' }),
      /Unsupported certification status/,
    );
  });

  it('can mark connector as revoked', async () => {
    await publishAddConnector(connectorHome);
    const result = await certifyConnector({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      status: 'revoked',
      force: true,
    });
    assert.equal(result.certification.status, 'revoked');
  });

  it('throws when attestation is not valid without force', async () => {
    // Publish without signing, then manually corrupt attestation
    await publishAddConnector(connectorHome);
    const catalogPath = path.join(connectorHome, 'catalog.json');
    const catalog = JSON.parse(await readFile(catalogPath, 'utf8'));
    catalog.connectors[0].attestation.signature = 'badhash000';
    await writeFile(catalogPath, JSON.stringify(catalog, null, 2));
    await assert.rejects(
      () => certifyConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' }),
      /attestation verification failed/i,
    );
  });

  it('force=true bypasses attestation check', async () => {
    await publishAddConnector(connectorHome);
    const catalogPath = path.join(connectorHome, 'catalog.json');
    const catalog = JSON.parse(await readFile(catalogPath, 'utf8'));
    catalog.connectors[0].attestation.signature = 'badhash000';
    await writeFile(catalogPath, JSON.stringify(catalog, null, 2));
    const result = await certifyConnector({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      status: 'certified',
      force: true,
    });
    assert.equal(result.success, true);
  });
});

// ---------------------------------------------------------------------------
// installConnector / uninstallConnector
// ---------------------------------------------------------------------------

describe('installConnector and uninstallConnector', () => {
  let connectorHome;

  beforeEach(async () => {
    __resetWasmConnectorState();
    envGuard.save();
    connectorHome = await mkdtemp(path.join(tmpdir(), 'wasm-mkt-install-'));
    process.env.STATESET_CONNECTOR_HOME = connectorHome;
  });

  afterEach(async () => {
    __resetWasmConnectorState();
    envGuard.restore();
    await rm(connectorHome, { recursive: true, force: true });
  });

  it('installs a published connector', async () => {
    await publishAddConnector(connectorHome);
    const result = await installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    assert.equal(result.success, true);
    assert.equal(result.connector.id, 'math.add');
    assert.equal(result.connector.version, '1.0.0');
  });

  it('installed connector has modulePath and manifestPath', async () => {
    await publishAddConnector(connectorHome);
    const result = await installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    assert.ok(result.connector.modulePath.endsWith('module.wasm'));
    assert.ok(result.connector.manifestPath.endsWith('manifest.json'));
  });

  it('installed WASM file passes sha256 integrity check', async () => {
    await publishAddConnector(connectorHome);
    const { connector } = await installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    const installedBytes = await readFile(connector.modulePath);
    const actualHash = sha256Hex(installedBytes);
    assert.equal(connector.installedWasmSha256, actualHash);
  });

  it('throws when connector is not in catalog', async () => {
    await assert.rejects(
      () => installConnector({ connectorHome, connectorId: 'no.connector', version: '1.0.0' }),
      /not found in the marketplace catalog/,
    );
  });

  it('throws when already installed and force=false', async () => {
    await publishAddConnector(connectorHome);
    await installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    await assert.rejects(
      () => installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' }),
      /already installed/,
    );
  });

  it('reinstalls when force=true', async () => {
    await publishAddConnector(connectorHome);
    await installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    const result = await installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0', force: true });
    assert.equal(result.success, true);
  });

  it('strict verification throws when attestation invalid', async () => {
    await publishAddConnector(connectorHome);
    const catalogPath = path.join(connectorHome, 'catalog.json');
    const catalog = JSON.parse(await readFile(catalogPath, 'utf8'));
    catalog.connectors[0].attestation.signature = 'corrupted';
    await writeFile(catalogPath, JSON.stringify(catalog, null, 2));
    await assert.rejects(
      () => installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0', verifyStrict: true }),
      /attestation verification failed/i,
    );
  });

  it('requireCertified throws when connector is not certified', async () => {
    await publishAddConnector(connectorHome);
    await assert.rejects(
      () => installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0', requireCertified: true }),
      /certification policy failed/i,
    );
  });

  it('uninstall removes installed version directory', async () => {
    await publishAddConnector(connectorHome);
    await installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    const result = await uninstallConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    assert.equal(result.success, true);
    assert.equal(result.removed.connectorId, 'math.add');
    assert.equal(result.removed.version, '1.0.0');
  });

  it('uninstall throws when connector is not installed', async () => {
    await assert.rejects(
      () => uninstallConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' }),
      /not installed/,
    );
  });

  it('installs latest version when version not specified', async () => {
    await publishAddConnector(connectorHome, { version: '1.0.0', connectorId: 'math.add', actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }] });
    await publishAddConnector(connectorHome, { version: '2.0.0', connectorId: 'math.add', actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }] });
    const result = await installConnector({ connectorHome, connectorId: 'math.add' });
    assert.equal(result.connector.version, '2.0.0');
  });
});

// ---------------------------------------------------------------------------
// listInstalledConnectors / getInstalledConnector
// ---------------------------------------------------------------------------

describe('listInstalledConnectors and getInstalledConnector', () => {
  let connectorHome;

  beforeEach(async () => {
    __resetWasmConnectorState();
    envGuard.save();
    connectorHome = await mkdtemp(path.join(tmpdir(), 'wasm-mkt-lsinstall-'));
    process.env.STATESET_CONNECTOR_HOME = connectorHome;
  });

  afterEach(async () => {
    __resetWasmConnectorState();
    envGuard.restore();
    await rm(connectorHome, { recursive: true, force: true });
  });

  it('returns empty list when nothing installed', async () => {
    const result = await listInstalledConnectors({ connectorHome });
    assert.equal(result.success, true);
    assert.equal(result.total, 0);
    assert.deepEqual(result.connectors, []);
  });

  it('lists installed connectors', async () => {
    await publishAddConnector(connectorHome);
    await installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    const result = await listInstalledConnectors({ connectorHome });
    assert.equal(result.success, true);
    assert.equal(result.total, 1);
    assert.equal(result.connectors[0].id, 'math.add');
  });

  it('filters installed list by connectorId', async () => {
    await publishAddConnector(connectorHome, { connectorId: 'math.add', version: '1.0.0', actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }] });
    await publishAddConnector(connectorHome, { connectorId: 'math.mul', version: '1.0.0', actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }] });
    await installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    await installConnector({ connectorHome, connectorId: 'math.mul', version: '1.0.0' });
    const result = await listInstalledConnectors({ connectorHome, connectorId: 'math.add' });
    assert.equal(result.total, 1);
    assert.equal(result.connectors[0].id, 'math.add');
  });

  it('getInstalledConnector returns manifest for specific version', async () => {
    await publishAddConnector(connectorHome);
    await installConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    const result = await getInstalledConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' });
    assert.equal(result.success, true);
    assert.equal(result.connector.id, 'math.add');
    assert.equal(result.connector.version, '1.0.0');
  });

  it('getInstalledConnector throws when not installed', async () => {
    await assert.rejects(
      () => getInstalledConnector({ connectorHome, connectorId: 'math.add', version: '1.0.0' }),
      /not installed/,
    );
  });
});

// ---------------------------------------------------------------------------
// executeInstalledConnectorAction (native-export)
// ---------------------------------------------------------------------------

describe('executeInstalledConnectorAction — native-export', () => {
  let connectorHome;

  beforeEach(async () => {
    __resetWasmConnectorState();
    envGuard.save();
    connectorHome = await mkdtemp(path.join(tmpdir(), 'wasm-mkt-exec-'));
    process.env.STATESET_CONNECTOR_HOME = connectorHome;
  });

  afterEach(async () => {
    __resetWasmConnectorState();
    envGuard.restore();
    await rm(connectorHome, { recursive: true, force: true });
  });

  async function publishAndInstall(connectorId = 'math.add', version = '1.0.0') {
    await publishAddConnector(connectorHome, { connectorId, version });
    await installConnector({ connectorHome, connectorId, version });
  }

  it('executes native-export add function', async () => {
    await publishAndInstall();
    const result = await executeInstalledConnectorAction({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      action: 'add',
      params: { a: 3, b: 4 },
    });
    assert.equal(result.success, true);
    assert.equal(result.output.value, 7);
  });

  it('result contains action name and connector info', async () => {
    await publishAndInstall();
    const result = await executeInstalledConnectorAction({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      action: 'add',
      params: { a: 0, b: 0 },
    });
    assert.equal(result.action, 'add');
    assert.equal(result.connector.id, 'math.add');
    assert.equal(result.connector.version, '1.0.0');
  });

  it('execution object has elapsedMs', async () => {
    await publishAndInstall();
    const result = await executeInstalledConnectorAction({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      action: 'add',
      params: { a: 1, b: 2 },
    });
    assert.ok(typeof result.execution.elapsedMs === 'number');
    assert.ok(result.execution.elapsedMs >= 0);
  });

  it('throws when connector is not installed', async () => {
    await assert.rejects(
      () => executeInstalledConnectorAction({
        connectorHome,
        connectorId: 'math.add',
        version: '1.0.0',
        action: 'add',
        params: {},
      }),
      /not installed/,
    );
  });

  it('throws when action does not exist', async () => {
    await publishAndInstall();
    await assert.rejects(
      () => executeInstalledConnectorAction({
        connectorHome,
        connectorId: 'math.add',
        version: '1.0.0',
        action: 'nonexistent',
        params: {},
      }),
      /does not expose action/,
    );
  });

  it('strict verification throws on invalid attestation during execute', async () => {
    await publishAndInstall();
    // Corrupt manifest on disk
    const manifestPath = path.join(connectorHome, 'installed', 'math.add', '1.0.0', 'manifest.json');
    const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
    manifest.attestation.signature = 'tampered';
    await writeFile(manifestPath, JSON.stringify(manifest, null, 2));
    await assert.rejects(
      () => executeInstalledConnectorAction({
        connectorHome,
        connectorId: 'math.add',
        version: '1.0.0',
        action: 'add',
        params: { a: 1, b: 2 },
        verifyStrict: true,
      }),
      /attestation verification failed/i,
    );
  });

  it('requireCertified rejects uncertified connector at execute time', async () => {
    await publishAndInstall();
    await assert.rejects(
      () => executeInstalledConnectorAction({
        connectorHome,
        connectorId: 'math.add',
        version: '1.0.0',
        action: 'add',
        params: { a: 1, b: 2 },
        requireCertified: true,
      }),
      /certification policy failed/i,
    );
  });

  it('minSafetyScore rejects connector below threshold at execute time', async () => {
    await publishAndInstall();
    await assert.rejects(
      () => executeInstalledConnectorAction({
        connectorHome,
        connectorId: 'math.add',
        version: '1.0.0',
        action: 'add',
        params: { a: 1, b: 2 },
        minSafetyScore: 99,
      }),
      /safety score/i,
    );
  });

  it('numeric string params are coerced to numbers', async () => {
    await publishAndInstall();
    const result = await executeInstalledConnectorAction({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      action: 'add',
      params: { a: '10', b: '20' },
    });
    assert.equal(result.output.value, 30);
  });

  it('result attestationVerified reflects actual attestation state', async () => {
    await publishAndInstall();
    const result = await executeInstalledConnectorAction({
      connectorHome,
      connectorId: 'math.add',
      version: '1.0.0',
      action: 'add',
      params: { a: 5, b: 5 },
    });
    assert.equal(typeof result.connector.attestationVerified, 'boolean');
  });
});

// ---------------------------------------------------------------------------
// HMAC signature correctness (standalone verification)
// ---------------------------------------------------------------------------

describe('HMAC attestation signature correctness', () => {
  let connectorHome;

  beforeEach(async () => {
    __resetWasmConnectorState();
    envGuard.save();
    connectorHome = await mkdtemp(path.join(tmpdir(), 'wasm-mkt-hmac-'));
    process.env.STATESET_CONNECTOR_HOME = connectorHome;
  });

  afterEach(async () => {
    __resetWasmConnectorState();
    envGuard.restore();
    await rm(connectorHome, { recursive: true, force: true });
  });

  it('HMAC signature in catalog is reproducible with the same key', async () => {
    const signingKey = 'super-secret-hmac-key';
    await publishAddConnector(connectorHome);
    await signConnectorAttestation({ connectorHome, connectorId: 'math.add', version: '1.0.0', signingKey });

    const catalogPath = path.join(connectorHome, 'catalog.json');
    const catalog = JSON.parse(await readFile(catalogPath, 'utf8'));
    const entry = catalog.connectors[0];
    const { signature, payloadHash } = entry.attestation;

    // signature should be a 64-char hex string (SHA-256)
    assert.match(signature, /^[0-9a-f]{64}$/);
    assert.match(payloadHash, /^[0-9a-f]{64}$/);
  });

  it('different signing keys produce different signatures', async () => {
    await publishAddConnector(connectorHome, { connectorId: 'math.a1' });
    await publishAddConnector(connectorHome, { connectorId: 'math.a2', version: '1.0.0',
      actions: [{ name: 'add', exportName: 'add', args: ['a', 'b'] }] });

    await signConnectorAttestation({ connectorHome, connectorId: 'math.a1', version: '1.0.0', signingKey: 'key-alpha' });
    await signConnectorAttestation({ connectorHome, connectorId: 'math.a2', version: '1.0.0', signingKey: 'key-beta' });

    const catalog = JSON.parse(await readFile(path.join(connectorHome, 'catalog.json'), 'utf8'));
    const sig1 = catalog.connectors.find(c => c.id === 'math.a1').attestation.signature;
    const sig2 = catalog.connectors.find(c => c.id === 'math.a2').attestation.signature;
    assert.notEqual(sig1, sig2);
  });
});

// ---------------------------------------------------------------------------
// __resetWasmConnectorState
// ---------------------------------------------------------------------------

describe('__resetWasmConnectorState', () => {
  it('is a function', () => {
    assert.equal(typeof __resetWasmConnectorState, 'function');
  });

  it('can be called multiple times without error', () => {
    __resetWasmConnectorState();
    __resetWasmConnectorState();
  });
});
