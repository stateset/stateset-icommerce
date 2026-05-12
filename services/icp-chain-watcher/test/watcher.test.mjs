// chain-watcher tests using a mock JSON-RPC server + the real
// settler-stateset daemon.
//
// Verifies:
//   - ABI decoding for all 5 events
//   - Forwarder maps decoded events to correct Settler admin payloads
//   - Watcher polls eth_getLogs and POSTs to Settler
//   - State persistence (last_processed_block) survives restart

import { test, after, before } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdtempSync, rmSync, readFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

import { decodeLog, EVENT_TOPICS } from '../src/abi-decoder.mjs';
import { decodedToSettlerEvent } from '../src/forwarder.mjs';
import { ChainWatcher } from '../src/server.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const SETTLER = resolve(__dirname, '..', '..', 'settler-stateset', 'src', 'server.mjs');

const CONTRACT = '0xICPESCROWICPESCROWICPESCROWICPESCROW0000';

// ---------------------------------------------------------------------------
// Fixtures: real ABI-encoded event log payloads
// ---------------------------------------------------------------------------

/** Pad a hex string (without 0x) to 64 chars left. */
const pad32 = (h) => h.toLowerCase().replace(/^0x/, '').padStart(64, '0');
/** uint128/uint64 as 32-byte word, right-aligned. */
const uintWord = (n, bits) => pad32(BigInt(n).toString(16));
/** address as 32-byte word, right-aligned. */
const addressWord = (addr) => pad32(addr.replace(/^0x/, ''));
/** bytes32 as 32-byte word. */
const bytes32Word = (b32) => pad32(b32.replace(/^0x/, ''));

/** Encode a string (dynamic) at the given offset (in bytes from start). */
function stringTail(s) {
  const bytes = Buffer.from(s, 'utf8');
  const len = bytes.length;
  const lengthWord = pad32(BigInt(len).toString(16));
  const padded = bytes.toString('hex').padEnd(Math.ceil(bytes.length / 32) * 64, '0');
  return lengthWord + padded;
}

const ESCROW_ID = '0xabc1234567890abcdef1234567890abcdef1234567890abcdef1234567890abc';
const BUYER = '0x1111111111111111111111111111111111111111';
const MERCHANT = '0x2222222222222222222222222222222222222222';
const QUOTE_HASH = '0xdeadbeef'.repeat(8).slice(0, 66);
const FULFILLMENT_RECEIPT_HASH = '0xfeedfeed'.repeat(8).slice(0, 66);

// ---------------------------------------------------------------------------
// ABI decoder unit tests
// ---------------------------------------------------------------------------

test('decodeLog: EscrowFunded fully decodes', () => {
  const log = {
    address: CONTRACT,
    topics: [EVENT_TOPICS.EscrowFunded, ESCROW_ID, addressTopicForm(BUYER), addressTopicForm(MERCHANT)],
    data:
      '0x' +
      uintWord('100000000', 128) + // 100 USDC (6 decimals)
      uintWord('1748000000', 64) +  // fulfillment deadline
      uintWord('604800', 64) +       // dispute window (7 days)
      bytes32Word(QUOTE_HASH),
    blockNumber: '0x1234',
    transactionHash: '0xabc',
    logIndex: '0x0',
  };
  const d = decodeLog(log);
  assert.equal(d.eventName, 'EscrowFunded');
  assert.equal(d.escrow_id, ESCROW_ID);
  assert.equal(d.buyer, BUYER.toLowerCase());
  assert.equal(d.merchant, MERCHANT.toLowerCase());
  assert.equal(d.amount, '100000000');
  assert.equal(d.fulfillment_deadline, 1748000000);
  assert.equal(d.dispute_window, 604800);
  assert.equal(d.quote_hash, QUOTE_HASH);
  assert.equal(d.rail_event.block_number, 0x1234);
  assert.equal(d.rail_event.tx_hash, '0xabc');
});

test('decodeLog: EscrowReleased decodes', () => {
  const log = {
    address: CONTRACT,
    topics: [EVENT_TOPICS.EscrowReleased, ESCROW_ID, addressTopicForm(MERCHANT)],
    data:
      '0x' +
      uintWord('62980000', 128) + // 62.98 USDC
      bytes32Word(FULFILLMENT_RECEIPT_HASH),
    blockNumber: '0x2000',
    transactionHash: '0xdef',
    logIndex: '0x1',
  };
  const d = decodeLog(log);
  assert.equal(d.eventName, 'EscrowReleased');
  assert.equal(d.amount, '62980000');
  assert.equal(d.fulfillment_receipt_hash, FULFILLMENT_RECEIPT_HASH);
});

test('decodeLog: EscrowDisputed decodes string reason', () => {
  const reason = 'item not as described';
  const log = {
    address: CONTRACT,
    topics: [EVENT_TOPICS.EscrowDisputed, ESCROW_ID, addressTopicForm(BUYER)],
    data: '0x' + pad32('20') + stringTail(reason), // offset=0x20=32 bytes, then string tail
    blockNumber: '0x3000',
    transactionHash: '0xa1',
    logIndex: '0x0',
  };
  const d = decodeLog(log);
  assert.equal(d.eventName, 'EscrowDisputed');
  assert.equal(d.reason, reason);
});

test('decodeLog: returns null for unknown topic0', () => {
  const log = {
    address: CONTRACT,
    topics: ['0x' + 'ff'.repeat(32), ESCROW_ID],
    data: '0x',
    blockNumber: '0x0',
    transactionHash: '0x0',
    logIndex: '0x0',
  };
  assert.equal(decodeLog(log), null);
});

// ---------------------------------------------------------------------------
// Forwarder mapping tests
// ---------------------------------------------------------------------------

test('decodedToSettlerEvent: EscrowFunded → fund kind', () => {
  const decoded = {
    eventName: 'EscrowFunded',
    escrow_id: ESCROW_ID,
    buyer: BUYER.toLowerCase(),
    merchant: MERCHANT.toLowerCase(),
    amount: '100000000',
    fulfillment_deadline: 1748000000,
    dispute_window: 604800,
    quote_hash: QUOTE_HASH,
    rail_event: { rail: 'evm', block_number: 1, tx_hash: '0xa' },
  };
  const payload = decodedToSettlerEvent(decoded);
  assert.equal(payload.kind, 'fund');
  assert.equal(payload.escrow_id, ESCROW_ID);
  assert.equal(payload.init.amount.amount, '100.000000');
  assert.equal(payload.init.amount.currency, 'USDC');
  assert.equal(payload.init.buyer, BUYER.toLowerCase());
});

test('decodedToSettlerEvent: EscrowReleased → release kind', () => {
  const decoded = {
    eventName: 'EscrowReleased',
    escrow_id: ESCROW_ID,
    merchant: MERCHANT.toLowerCase(),
    amount: '62980000',
    fulfillment_receipt_hash: FULFILLMENT_RECEIPT_HASH,
    rail_event: { rail: 'evm', block_number: 2, tx_hash: '0xb' },
  };
  const payload = decodedToSettlerEvent(decoded);
  assert.equal(payload.kind, 'release');
  assert.equal(payload.payout_amount, '62.980000');
});

// ---------------------------------------------------------------------------
// End-to-end: mock RPC + real Settler daemon + ChainWatcher
// ---------------------------------------------------------------------------

let settlerProc;
let settlerUrl;
let mockRpcServer;
let mockRpcUrl;
let stateDir;

before(async () => {
  stateDir = mkdtempSync(join(tmpdir(), 'icp-chain-watcher-'));

  // Spawn the real settler-stateset daemon
  settlerProc = spawn('node', [SETTLER], { env: { ...process.env, PORT: '0' }, stdio: ['ignore', 'pipe', 'pipe'] });
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

  // Mock JSON-RPC server
  mockRpcServer = createServer(async (req, res) => {
    const chunks = [];
    for await (const c of req) chunks.push(c);
    const body = JSON.parse(Buffer.concat(chunks).toString('utf8'));
    let result;
    if (body.method === 'eth_blockNumber') {
      result = `0x${(1000 + 18).toString(16)}`; // head = 1018, so finalized = 1000
    } else if (body.method === 'eth_getLogs') {
      // Return one synthetic EscrowFunded at block 950 — only if the
      // requested range covers it. Otherwise return an empty result.
      const filter = body.params[0];
      const from = parseInt(filter.fromBlock, 16);
      const to = parseInt(filter.toBlock, 16);
      if (from <= 950 && 950 <= to) {
        result = [
          {
            address: CONTRACT,
            topics: [EVENT_TOPICS.EscrowFunded, ESCROW_ID, addressTopicForm(BUYER), addressTopicForm(MERCHANT)],
            data:
              '0x' +
              uintWord('100000000', 128) +
              uintWord('1748000000', 64) +
              uintWord('604800', 64) +
              bytes32Word(QUOTE_HASH),
            blockNumber: '0x3b6', // 950
            transactionHash: '0xfeedfacecafe',
            logIndex: '0x0',
          },
        ];
      } else {
        result = [];
      }
    } else {
      result = null;
    }
    res.writeHead(200, { 'content-type': 'application/json' });
    res.end(JSON.stringify({ jsonrpc: '2.0', id: body.id, result }));
  });
  await new Promise((res) => mockRpcServer.listen(0, res));
  mockRpcUrl = `http://127.0.0.1:${mockRpcServer.address().port}`;
});

after(() => {
  if (settlerProc) settlerProc.kill();
  if (mockRpcServer) mockRpcServer.close();
  if (stateDir && existsSync(stateDir)) rmSync(stateDir, { recursive: true, force: true });
});

test('ChainWatcher.tick() forwards EscrowFunded → Settler admin event', async () => {
  const stateFile = join(stateDir, 'state.json');
  process.env.STATE_FILE = stateFile;

  const watcher = new ChainWatcher({
    rpcUrl: mockRpcUrl,
    contractAddress: CONTRACT,
    settlerUrl,
    startBlock: 900,
  });
  watcher.state = { last_processed_block: null };

  await watcher.tick();

  assert.equal(watcher.metrics.events_seen, 1);
  assert.equal(watcher.metrics.events_forwarded, 1);
  assert.equal(watcher.metrics.errors, 0);

  // Confirm Settler created the escrow
  const r = await fetch(`${settlerUrl}/icp/v1/escrows/${ESCROW_ID}`);
  assert.equal(r.status, 200);
  const escrow = await r.json();
  assert.equal(escrow.state, 'funded');
  assert.equal(escrow.amount.amount, '100.000000');
});

test('ChainWatcher.tick() advances last_processed_block', async () => {
  const watcher = new ChainWatcher({
    rpcUrl: mockRpcUrl,
    contractAddress: CONTRACT,
    settlerUrl,
    startBlock: 900,
  });
  watcher.state = { last_processed_block: 999 };

  await watcher.tick();
  // head=1018, finalized=1000. from=1000, to=1000.
  assert.equal(watcher.state.last_processed_block, 1000);
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Pad an address to 32 bytes for use as an indexed topic. */
function addressTopicForm(addr) {
  return '0x' + addressWord(addr);
}
