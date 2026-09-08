// Tests for the compose stack's mock EVM RPC.
//
// The chain-watcher has always been tested against a mock RPC that lived
// inside its own test file, so nothing outside `node --test` could drive it.
// `icp-docker/mock-rpc.mjs` promotes that fixture to a real (tiny) service so
// the compose stack can exercise the watcher end-to-end. These tests pin the
// JSON-RPC surface the watcher depends on, and then run the whole chain →
// watcher → settler path in-process so a compose failure has a cheap,
// toolchain-free reproduction.

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { MockChain, createMockRpcServer, DEFAULTS } from '../mock-rpc.mjs';
import { EVENT_TOPICS } from '../../services/icp-chain-watcher/src/abi-decoder.mjs';

// The watcher reads STATE_FILE once, at module load, and defaults it to the
// process CWD. Point it at a temp path *before* that module is evaluated
// (hence the dynamic import) so running this suite from the repo root does
// not drop a cursor file next to the source tree.
const STATE_FILE = join(tmpdir(), `icp-mock-rpc-cursor-${process.pid}.json`);
process.env.STATE_FILE = STATE_FILE;
const { ChainWatcher } = await import('../../services/icp-chain-watcher/src/server.mjs');

const __dirname = dirname(fileURLToPath(import.meta.url));
const SETTLER = resolve(__dirname, '..', '..', 'services', 'settler-stateset', 'src', 'server.mjs');

let rpcServer;
let rpcUrl;
let chain;
let settlerProc;
let settlerUrl;
let stateDir;

async function rpcCall(method, params = []) {
  const r = await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  });
  const j = await r.json();
  if (j.error) throw new Error(j.error.message);
  return j.result;
}

before(async () => {
  stateDir = mkdtempSync(join(tmpdir(), 'icp-mock-rpc-'));

  chain = new MockChain({});
  rpcServer = createMockRpcServer(chain);
  await new Promise((res) => rpcServer.listen(0, '127.0.0.1', res));
  rpcUrl = `http://127.0.0.1:${rpcServer.address().port}`;

  settlerProc = spawn('node', [SETTLER], {
    env: { ...process.env, PORT: '0' },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let buf = '';
  settlerUrl = await new Promise((res, rej) => {
    const onErr = (d) => {
      buf += d.toString('utf8');
      const m = buf.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
      if (m) {
        settlerProc.stderr.off('data', onErr);
        res(m[1]);
      }
    };
    settlerProc.stderr.on('data', onErr);
    setTimeout(() => rej(new Error('settler did not start')), 5000);
  });
});

after(() => {
  if (settlerProc) settlerProc.kill();
  if (rpcServer) rpcServer.close();
  if (stateDir) rmSync(stateDir, { recursive: true, force: true });
  rmSync(STATE_FILE, { force: true });
});

// ---------------------------------------------------------------------------
// JSON-RPC surface
// ---------------------------------------------------------------------------

test('GET /healthz reports ok plus the chain cursor', async () => {
  const r = await fetch(`${rpcUrl}/healthz`);
  assert.equal(r.status, 200);
  const j = await r.json();
  assert.equal(j.ok, true);
  assert.equal(typeof j.head_block, 'number');
  assert.equal(j.logs, 1); // seeded EscrowFunded
});

test('eth_blockNumber returns a hex head above the seeded event + finality', async () => {
  const hex = await rpcCall('eth_blockNumber');
  assert.match(hex, /^0x[0-9a-f]+$/);
  const head = parseInt(hex, 16);
  assert.ok(
    head - DEFAULTS.finalityBlocks >= DEFAULTS.seedBlock,
    `head ${head} must finalize the seeded event at ${DEFAULTS.seedBlock}`,
  );
});

test('eth_getLogs honours the block range', async () => {
  const inRange = await rpcCall('eth_getLogs', [
    { address: DEFAULTS.contractAddress, fromBlock: '0x0', toBlock: '0x3e8' },
  ]);
  assert.equal(inRange.length, 1);
  assert.equal(inRange[0].topics[0], EVENT_TOPICS.EscrowFunded);

  const outOfRange = await rpcCall('eth_getLogs', [
    { address: DEFAULTS.contractAddress, fromBlock: '0x3b7', toBlock: '0x3e8' },
  ]);
  assert.equal(outOfRange.length, 0, 'block 950 (0x3b6) is below fromBlock 0x3b7');
});

test('eth_getLogs filters by contract address', async () => {
  const logs = await rpcCall('eth_getLogs', [
    { address: '0x' + '11'.repeat(20), fromBlock: '0x0', toBlock: '0x3e8' },
  ]);
  assert.equal(logs.length, 0);
});

test('an unknown JSON-RPC method is an error, not a silent null', async () => {
  const r = await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ jsonrpc: '2.0', id: 7, method: 'eth_sendRawTransaction', params: [] }),
  });
  const j = await r.json();
  assert.equal(j.id, 7);
  assert.ok(j.error, 'unimplemented methods must surface as JSON-RPC errors');
});

test('POST /admin/emit appends a log past the previous head and re-finalizes', async () => {
  const before = chain.head;
  const r = await fetch(`${rpcUrl}/admin/emit`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ event: 'EscrowDisputed', reason: 'probe' }),
  });
  assert.equal(r.status, 200);
  const j = await r.json();
  assert.ok(j.block > before, 'emitted block must be beyond the old head');
  assert.equal(chain.head - DEFAULTS.finalityBlocks, j.block, 'new head must finalize it');

  const logs = await rpcCall('eth_getLogs', [
    {
      address: DEFAULTS.contractAddress,
      fromBlock: `0x${j.block.toString(16)}`,
      toBlock: `0x${j.block.toString(16)}`,
    },
  ]);
  assert.equal(logs.length, 1);
  assert.equal(logs[0].topics[0], EVENT_TOPICS.EscrowDisputed);

  chain.reset();
});

// ---------------------------------------------------------------------------
// End-to-end: mock chain → ChainWatcher → real settler daemon
// ---------------------------------------------------------------------------

test('chain → watcher → settler settles an escrow and yields a signed receipt', async () => {
  chain.reset();

  const watcher = new ChainWatcher({
    rpcUrl,
    contractAddress: DEFAULTS.contractAddress,
    settlerUrl,
    startBlock: DEFAULTS.seedBlock - 50,
  });
  watcher.state = { last_processed_block: null };

  // 1. The seeded EscrowFunded reaches the settler through the watcher.
  await watcher.tick();
  assert.equal(watcher.metrics.events_forwarded, 1, 'watcher forwarded the funded event');
  assert.equal(watcher.metrics.errors, 0);

  const funded = await fetch(`${settlerUrl}/icp/v1/escrows/${DEFAULTS.escrowId}`).then((r) =>
    r.json(),
  );
  assert.equal(funded.state, 'funded');
  assert.equal(funded.amount.currency, 'USDC');
  assert.equal(funded.amount.amount, '100.000000');

  // 2. Fulfillment is off-chain evidence — ICPEscrow.sol emits no event for
  //    it, so the merchant backend drives this transition, not the watcher.
  const ful = await fetch(`${settlerUrl}/admin/escrow/event`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ escrow_id: DEFAULTS.escrowId, kind: 'fulfill' }),
  });
  assert.equal(ful.status, 200);

  // 3. The on-chain release is observed and forwarded, settling the escrow.
  await fetch(`${rpcUrl}/admin/emit`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ event: 'EscrowReleased', amount: '100000000' }),
  });
  const releaseLog = chain.logs[chain.logs.length - 1];
  await watcher.tick();
  assert.equal(watcher.metrics.events_forwarded, 2, 'watcher forwarded the release event');
  assert.equal(watcher.metrics.errors, 0);

  const settled = await fetch(`${settlerUrl}/icp/v1/escrows/${DEFAULTS.escrowId}`).then((r) =>
    r.json(),
  );
  assert.equal(settled.state, 'released');

  // The settler's signed record must trace back to the chain log the watcher
  // observed — otherwise "settled through the watcher" is unverified.
  const released = settled.events.find((e) => e.to_state === 'released');
  assert.ok(released, 'settler recorded a released event');
  assert.ok(released.settler_signature?.sig, 'released event is settler-signed');
  assert.equal(released.trigger.kind, 'rail-released');
  assert.equal(released.trigger.rail_event.tx_hash, releaseLog.transactionHash);
  assert.equal(
    released.trigger.rail_event.block_number,
    parseInt(releaseLog.blockNumber, 16),
  );
});
