/**
 * Agent board end to end against the mock sequencer.
 *
 * Runs the real CLI (board.mjs) in child processes, configured only through
 * environment variables, against mock-sequencer.mjs on an ephemeral
 * 127.0.0.1 port. No other network access. Agent directories live in a fresh
 * os.tmpdir() directory that is removed afterwards.
 *
 *   node --test examples/agent-board/test/board.test.mjs
 *
 * No CI job runs example tests; this file is standalone (and wired to
 * `npm run test:agent-board` at the repository root).
 */

import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { after, before, describe, test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { SequencerClient } from '../../../cli/src/sync/client.js';
import { validatePushReceipt } from '../../../cli/src/sync/push-receipt.js';
import { startMockSequencer } from '../mock-sequencer.mjs';
import { clientConfig, sequencerSettings } from '../lib/config.mjs';

const BOARD = fileURLToPath(new URL('../board.mjs', import.meta.url));

let mock;
let home;
/** Every byte any board process wrote, for the secret-leak check. */
const transcript = [];
const agents = {};

/** Environment for board.mjs: the caller's, minus any ambient sequencer settings. */
function envFor(target, boardHome, overrides = {}) {
  const env = { ...process.env };
  for (const key of Object.keys(env)) {
    if (key.startsWith('SEQUENCER_') || key.startsWith('BOARD_') || key === 'STATESET_API_KEY') {
      delete env[key];
    }
  }
  return {
    ...env,
    BOARD_HOME: boardHome,
    SEQUENCER_URL: target.url,
    SEQUENCER_TENANT_ID: target.tenantId,
    SEQUENCER_STORE_ID: target.storeId,
    SEQUENCER_PUBLIC_KEY: target.sequencerPublicKey,
    SEQUENCER_API_KEY: target.writeApiKey,
    ...overrides,
  };
}

const baseEnv = (overrides) => envFor(mock, home, overrides);

/** Run board.mjs; resolves { code, stdout, stderr, json }. */
function board(args, envOverrides = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [BOARD, ...args], { env: baseEnv(envOverrides) });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (d) => (stdout += d));
    child.stderr.on('data', (d) => (stderr += d));
    child.on('error', reject);
    child.on('close', (code) => {
      transcript.push(stdout, stderr);
      let json;
      if (args.includes('--json') || args[0] === 'roster') {
        try {
          json = JSON.parse(stdout);
        } catch {
          json = undefined;
        }
      }
      resolve({ code, stdout, stderr, json });
    });
  });
}

async function boardOk(args, env) {
  const result = await board(args, env);
  assert.equal(result.code, 0, `board ${args.join(' ')} failed:\n${result.stderr}${result.stdout}`);
  return result;
}

const asAdmin = () => ({ SEQUENCER_API_KEY: mock.adminApiKey });

async function post(agent, thread, text, extra = []) {
  const { json } = await boardOk([
    'post',
    '--agent',
    agent,
    '--thread',
    thread,
    text,
    ...extra,
    '--json',
  ]);
  assert.equal(json.status, 'sequenced');
  return json.sequenceNumber;
}

async function readThread(thread, extra = [], env) {
  return (await boardOk(['read', '--thread', thread, ...extra, '--json'], env)).json;
}

describe('agent board on the mock sequencer', () => {
  before(async () => {
    mock = await startMockSequencer();
    home = fs.mkdtempSync(path.join(os.tmpdir(), 'agent-board-test-'));
  });

  after(async () => {
    await mock?.close();
    if (home) fs.rmSync(home, { recursive: true, force: true });
  });

  test('init gives every agent its own directory, iCommerce store and Ed25519 key', async () => {
    for (const name of ['alice', 'bob', 'carol']) {
      const { json } = await boardOk(['init', '--agent', name, '--json']);
      assert.equal(json.created, true);
      agents[name] = json;
      assert.ok(fs.statSync(path.join(json.dir, 'store.db')).size > 0, 'store.db created');
      assert.ok(fs.existsSync(path.join(json.dir, 'outbox.db')), 'outbox.db created');
      const keyFile = path.join(json.dir, '.stateset', 'keys', json.agentId, 'signing-keys.json');
      assert.equal(fs.statSync(keyFile).mode & 0o777, 0o600, 'signing key is owner-only');
      assert.match(json.publicKey, /^0x[0-9a-f]{64}$/);
    }
    const ids = new Set(Object.values(agents).map((a) => a.agentId));
    const keys = new Set(Object.values(agents).map((a) => a.publicKey));
    const dirs = new Set(Object.values(agents).map((a) => a.dir));
    assert.equal(ids.size, 3);
    assert.equal(keys.size, 3);
    assert.equal(dirs.size, 3);

    const again = await boardOk(['init', '--agent', 'alice', '--json']);
    assert.equal(again.json.created, false, 'init is idempotent');
    assert.equal(again.json.agentId, agents.alice.agentId);
  });

  test('key registration needs an admin key, as on the real sequencer', async () => {
    const denied = await board(['register', '--agent', 'alice']);
    assert.equal(denied.code, 1);
    assert.match(denied.stderr, /403 Admin permission required/);

    for (const name of ['alice', 'bob', 'carol']) {
      await boardOk(['register', '--agent', name], asAdmin());
    }
    const skipped = await boardOk(['register', '--agent', 'alice', '--json'], asAdmin());
    assert.equal(skipped.json.skipped, true, 'a recorded registration is not re-sent');

    const duplicate = await board(['register', '--agent', 'alice', '--force'], asAdmin());
    assert.equal(duplicate.code, 1);
    assert.match(duplicate.stderr, /500 .*already registered/);
  });

  test('three agents post to one thread; every agent reads all posts in sequence order, verified', async () => {
    const first = await post('alice', 'standup', 'alice: starting the inventory audit');
    const second = await post('bob', 'standup', 'bob: taking returns', [
      '--reply-to',
      String(first),
    ]);
    const third = await post('carol', 'standup', 'carol: audit numbers attached', [
      '--state',
      '{"skusChecked":42,"discrepancies":["SKU-7"]}',
      '--ref',
      'audit:2026-11',
      '--store-summary',
    ]);
    assert.ok(first < second && second < third, 'sequence numbers increase');

    for (const reader of ['alice', 'bob', 'carol']) {
      const view = await readThread('standup', ['--agent', reader]);
      assert.deepEqual(
        view.messages.map((m) => m.sequenceNumber),
        [first, second, third],
      );
      assert.deepEqual(
        view.messages.map((m) => m.author),
        ['alice', 'bob', 'carol'],
      );
      assert.deepEqual(
        view.messages.map((m) => m.signer.agentId),
        [agents.alice.agentId, agents.bob.agentId, agents.carol.agentId],
      );
      for (const m of view.messages) {
        assert.equal(m.verified, true, `#${m.sequenceNumber} verified for ${reader}`);
        assert.equal(m.verification.keySource, 'directory');
        assert.equal(m.inclusion.status, 'not_committed');
      }
      assert.equal(view.messages[1].replyTo, first);
      assert.deepEqual(view.messages[2].state.discrepancies, ['SKU-7']);
      assert.deepEqual(view.messages[2].state.store, { customers: 0, orders: 0, products: 0 });
      assert.deepEqual(view.messages[2].refs, ['audit:2026-11']);
    }

    // No --agent, no --thread: the whole stream via the paged pull route.
    const all = (await boardOk(['read', '--json'])).json;
    assert.equal(all.thread, null);
    assert.deepEqual(
      all.messages.filter((m) => m.thread === 'standup').map((m) => m.sequenceNumber),
      [first, second, third],
    );

    const text = (await boardOk(['read', '--thread', 'standup'])).stdout;
    assert.match(text, /3 message\(s\), 3 verified/);
    assert.match(text, /bob \([0-9a-f]{8}\) re #\d+ +\[verified\]/);
  });

  test('after a commitment every message carries a verified inclusion proof', async () => {
    const { json } = await boardOk(['commit', '--json']);
    assert.equal(json.created, true);
    const view = await readThread('standup');
    for (const m of view.messages) {
      assert.equal(m.inclusion.status, 'verified', JSON.stringify(m.inclusion));
      assert.equal(m.inclusion.batchId, json.commitment.batchId);
      assert.equal(
        m.inclusion.leafCount,
        json.commitment.sequenceEnd - json.commitment.sequenceStart + 1,
      );
    }
    const nothing = await boardOk(['commit', '--json']);
    assert.equal(nothing.json.created, false);
  });

  test('a payload changed after signing is marked unverified; the rest still verify', async () => {
    const honest = await post('bob', 'tamper', 'bob: the honest one');
    const victim = await post('alice', 'tamper', 'alice: refund approved for 10.00');
    const rehashed = await post('carol', 'tamper', 'carol: ship it');
    await boardOk(['commit']);

    // A lying sequencer (or corrupted storage) edits the stored payload...
    mock.tamperPayload(victim, { author: 'alice', text: 'alice: refund approved for 9000.00' });
    // ...or edits it and recomputes the payload hash so the hash check passes.
    mock.tamperPayload(rehashed, { author: 'carol', text: 'carol: cancel it' }, { rehash: true });

    const view = await readThread('tamper');
    const bySeq = new Map(view.messages.map((m) => [m.sequenceNumber, m]));
    assert.equal(bySeq.get(honest).verified, true);

    const edited = bySeq.get(victim);
    assert.equal(edited.verified, false);
    assert.equal(edited.verification.reason, 'signature_invalid');
    assert.match(edited.verification.detail, /payload does not match/);
    assert.equal(edited.text, 'alice: refund approved for 9000.00', 'shown, but flagged');

    const forged = bySeq.get(rehashed);
    assert.equal(forged.verified, false);
    assert.equal(forged.verification.reason, 'signature_invalid');
    assert.match(forged.verification.detail, /signature does not verify/);
    // The rehashed event no longer reproduces the committed root, so the
    // sequencer refuses to serve its proof (HTTP 500), as the real one does.
    assert.notEqual(forged.inclusion.status, 'verified');

    assert.deepEqual(view.counts, { total: 3, verified: 1, unverified: 2 });
    const text = (await boardOk(['read', '--thread', 'tamper'])).stdout;
    assert.match(text, /\[UNVERIFIED: signature_invalid - payload does not match/);
  });

  test('an agent whose key was never registered is rejected by the sequencer', async () => {
    await boardOk(['init', '--agent', 'dave']);
    const result = await board([
      'post',
      '--agent',
      'dave',
      '--thread',
      'standup',
      'dave: hello?',
      '--json',
    ]);
    assert.equal(result.code, 1);
    assert.equal(result.json.status, 'rejected');
    assert.match(result.json.reason, /^agent_key_invalid: Agent key not found/);

    const view = await readThread('standup');
    assert.ok(!view.messages.some((m) => m.author === 'dave'), 'never sequenced');

    const who = (await boardOk(['whoami', '--agent', 'dave', '--json'])).json;
    assert.equal(who.outbox.rejected, 1);
    assert.equal(who.outbox.pending, 0, 'a rejection is final, not retried');
  });

  test('if a sequencer sequences an unregistered agent anyway, readers mark it unverified', async () => {
    await boardOk(['init', '--agent', 'erin']);
    mock.setAcceptUnregistered(true);
    try {
      await post('erin', 'rogue', 'erin: trust me');
    } finally {
      mock.setAcceptUnregistered(false);
    }
    const view = await readThread('rogue');
    assert.equal(view.messages.length, 1);
    assert.equal(view.messages[0].verified, false);
    assert.equal(view.messages[0].verification.reason, 'key_unresolved');
  });

  test('without a usable key directory messages are unverified unless a roster is given', async () => {
    const noKey = await readThread('standup', [], { SEQUENCER_PUBLIC_KEY: '' });
    assert.ok(noKey.messages.length >= 3);
    for (const m of noKey.messages) {
      assert.equal(m.verified, false);
      assert.equal(m.verification.reason, 'sequencer_key_not_configured');
    }

    const rosterFile = path.join(home, 'roster.json');
    const roster = await boardOk(['roster', '--agents', 'alice,bob,carol']);
    fs.writeFileSync(rosterFile, roster.stdout);
    assert.equal(roster.json.agents.length, 3);
    assert.ok(!roster.stdout.includes('privateKey'), 'roster holds public keys only');

    mock.setKeyDirectory(false); // a sequencer without stateset-sequencer PR #6
    try {
      const without = await readThread('standup');
      for (const m of without.messages) {
        assert.equal(m.verified, false);
        assert.equal(m.verification.reason, 'key_unresolved');
        assert.match(m.verification.detail, /PR #6/);
      }
      const withRoster = await readThread('standup', ['--roster', rosterFile]);
      for (const m of withRoster.messages) {
        assert.equal(m.verified, true);
        assert.equal(m.verification.keySource, 'roster');
        assert.equal(m.authorMatchesRoster, true);
      }
    } finally {
      mock.setKeyDirectory(true);
    }
  });

  test('configuration errors name the variable, never a value', async () => {
    const missing = await board(['read', '--thread', 'standup'], { SEQUENCER_TENANT_ID: '' });
    assert.equal(missing.code, 1);
    assert.match(missing.stderr, /Missing sequencer configuration: SEQUENCER_TENANT_ID/);

    const plaintext = await board(['read'], { SEQUENCER_URL: 'http://sequencer.example.com' });
    assert.equal(plaintext.code, 1);
    assert.match(plaintext.stderr, /Refusing plaintext http/);

    const configFile = path.join(home, 'board-config.json');
    fs.writeFileSync(
      configFile,
      JSON.stringify({
        SEQUENCER_URL: mock.url,
        SEQUENCER_API_KEY: mock.readApiKey,
        SEQUENCER_TENANT_ID: mock.tenantId,
        SEQUENCER_STORE_ID: mock.storeId,
      }),
    );
    const viaFile = await board(
      ['read', '--thread', 'standup', '--config', configFile, '--no-proofs', '--json'],
      {
        SEQUENCER_API_KEY: '',
        SEQUENCER_URL: '',
      },
    );
    assert.equal(viaFile.code, 0, viaFile.stderr);
    assert.ok(viaFile.json.messages.length >= 3);
  });

  test('no API key ever reaches output or an agent directory', async () => {
    const secrets = [mock.adminApiKey, mock.writeApiKey, mock.readApiKey];
    const output = transcript.join('\n');
    for (const secret of secrets) assert.ok(!output.includes(secret), 'API key printed');

    const files = [];
    const walk = (dir) => {
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) walk(full);
        else files.push(full);
      }
    };
    for (const agent of Object.values(agents)) walk(agent.dir);
    for (const file of files) {
      const bytes = fs.readFileSync(file);
      for (const secret of secrets) assert.ok(!bytes.includes(secret), `API key in ${file}`);
    }
  });
});

describe('CLI sync client against the real wire format', () => {
  let localMock;
  let settings;
  before(async () => {
    localMock = await startMockSequencer();
    settings = sequencerSettings({
      SEQUENCER_URL: localMock.url,
      SEQUENCER_API_KEY: localMock.writeApiKey,
      SEQUENCER_TENANT_ID: localMock.tenantId,
      SEQUENCER_STORE_ID: localMock.storeId,
    });
  });
  after(() => localMock?.close());

  async function seedOneEvent() {
    const soloHome = fs.mkdtempSync(path.join(os.tmpdir(), 'agent-board-unit-'));
    try {
      const run = (args, overrides) =>
        new Promise((resolve) => {
          const child = spawn(process.execPath, [BOARD, ...args], {
            env: envFor(localMock, soloHome, overrides),
          });
          child.on('close', resolve);
        });
      assert.equal(await run(['init', '--agent', 'solo']), 0);
      const admin = { SEQUENCER_API_KEY: localMock.adminApiKey };
      assert.equal(await run(['register', '--agent', 'solo'], admin), 0);
      assert.equal(await run(['post', '--agent', 'solo', '--thread', 'x', 'hello']), 0);
    } finally {
      fs.rmSync(soloHome, { recursive: true, force: true });
    }
  }

  /** The stored wire envelope of event #1 as a CLI push envelope. */
  function storedAsPushEnvelope() {
    const stored = localMock.events()[0];
    return {
      stored,
      envelope: {
        eventId: stored.event_id,
        commandId: stored.command_id ?? null,
        tenantId: stored.tenant_id,
        storeId: stored.store_id,
        entityType: stored.entity_type,
        entityId: stored.entity_id,
        eventType: stored.event_type,
        payload: stored.payload,
        vesVersion: 1,
        payloadKind: 0,
        payloadEncrypted: null,
        payloadPlainHash: stored.payload_plain_hash,
        payloadCipherHash: stored.payload_cipher_hash,
        agentKeyId: stored.agent_key_id,
        agentSignature: stored.agent_signature,
        agentSignatureScheme: 0,
        agentSignatureBundle: null,
        baseVersion: null,
        createdAt: stored.created_at,
        sourceAgent: stored.source_agent_id,
      },
    };
  }

  test('SequencerClient#pull reads the VES stream, not the legacy /api/v1/events route', async () => {
    await seedOneEvent();
    const client = new SequencerClient(clientConfig(settings));
    const page = await client.pull(0, 100);
    assert.equal(page.events.length, 1);
    assert.equal(page.events[0].envelope.sourceAgent, localMock.events()[0].source_agent_id);
    assert.ok(localMock.requests.some((r) => r.path === '/api/v1/ves/events'));
    assert.ok(!localMock.requests.some((r) => r.path === '/api/v1/events'));
  });

  test('an exact replay returns its stored receipt uncounted, and validatePushReceipt settles it', async () => {
    const { stored, envelope } = storedAsPushEnvelope();
    const client = new SequencerClient(clientConfig(settings));
    const response = await client.push({ agentId: stored.source_agent_id, events: [envelope] });
    assert.equal(response.eventsAccepted, 0);
    assert.equal(response.eventsRejected, 0);
    assert.equal(response.receipts.length, 1);

    const pending = [{ eventId: stored.event_id, localSeq: 1 }];
    const [settled] = validatePushReceipt(response, pending);
    assert.equal(settled.rejected, false);
    assert.equal(settled.remoteSeq, stored.sequence_number);
  });

  test('the mock refuses, at ingest, a signature that does not cover the event', async () => {
    const { stored, envelope } = storedAsPushEnvelope();
    const client = new SequencerClient(clientConfig(settings));
    // event_id is in the signing preimage, so the old signature no longer covers it.
    const moved = { ...envelope, eventId: randomUUID(), commandId: randomUUID() };
    const response = await client.push({ agentId: stored.source_agent_id, events: [moved] });
    assert.equal(response.eventsAccepted, 0);
    assert.equal(response.rejections[0].event_id, moved.eventId);
    assert.equal(response.rejections[0].reason, 'invalid_signature');
    const [settled] = validatePushReceipt(response, [{ eventId: moved.eventId, localSeq: 2 }]);
    assert.equal(settled.rejected, true);
    assert.equal(settled.reason, 'invalid_signature');
  });

  test('rejections arrive as snake_case event_id, which validatePushReceipt settles', () => {
    const response = {
      eventsAccepted: 0,
      eventsRejected: 1,
      headSequence: 4,
      rejections: [{ event_id: 'e-1', reason: 'invalid_signature', message: 'bad' }],
    };
    const pending = [{ eventId: 'e-1', localSeq: 7 }];
    const [settled] = validatePushReceipt(response, pending);
    assert.equal(settled.rejected, true);
    assert.equal(settled.reason, 'invalid_signature');
    assert.equal(settled.message, 'bad');
    assert.throws(
      () =>
        validatePushReceipt(
          { eventsAccepted: 0, eventsRejected: 0, headSequence: 4, receipts: [], rejections: [] },
          pending,
        ),
      /Invalid push receipt/,
      'an event that is neither sequenced nor rejected leaves the batch pending',
    );
  });

  test('the client authenticates as ApiKey; the mock refuses an API key sent as Bearer', async () => {
    // Only the API routes: on a shared machine other local processes
    // sometimes probe the ephemeral port (unauthenticated `GET /`).
    const fromClient = localMock.requests.filter((r) => r.path.startsWith('/api/'));
    assert.ok(fromClient.length > 0);
    assert.deepEqual(
      fromClient.filter((r) => r.auth !== 'ApiKey'),
      [],
      'CLI client and board use ApiKey',
    );

    const url = `${localMock.url}/api/v1/ves/head?tenant_id=${localMock.tenantId}&store_id=${localMock.storeId}`;
    const ok = await fetch(url, { headers: { Authorization: `ApiKey ${localMock.readApiKey}` } });
    assert.equal(ok.status, 200);
    const viaHeader = await fetch(url, { headers: { 'x-api-key': localMock.readApiKey } });
    assert.equal(viaHeader.status, 200);
    const bearer = await fetch(url, {
      headers: { Authorization: `Bearer ${localMock.readApiKey}` },
    });
    assert.equal(bearer.status, 401);
    assert.equal((await bearer.json()).code, 'invalid_jwt');
  });
});
