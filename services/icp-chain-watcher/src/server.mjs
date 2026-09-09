// icp-chain-watcher — observes ICPEscrow events on an EVM chain and forwards
// them to a running settler-stateset daemon as /admin/escrow/event POSTs.
//
// Configuration (env vars):
//   RPC_URL              — EVM JSON-RPC endpoint (e.g. https://sepolia.base.org)
//   CONTRACT_ADDRESS     — deployed ICPEscrow contract address (0x...)
//   SETTLER_URL          — settler-stateset HTTP base URL
//   START_BLOCK          — first block to scan from (default: latest - 1000)
//   POLL_INTERVAL_MS     — polling cadence in ms (default: 12000 — one Base block)
//   FINALITY_BLOCKS      — wait this many blocks before forwarding events (default: 18)
//   LOG_BATCH_MAX_BLOCKS — max block range per eth_getLogs call (default: 500)
//   STATE_FILE           — persist last-processed block here (default: ./.icp-chain-watcher-state.json)
//
// Health endpoint:
//   GET /healthz — 200 { ok: true, ... } while the last poll succeeded (or none
//                   has run yet), 503 { ok: false, last_poll: { ok, error, at } }
//                   once a poll fails. Liveness alone is not the property worth
//                   probing: a watcher whose RPC has gone away forwards nothing
//                   and the settler silently misses every on-chain event.

import { createServer } from 'node:http';
import { readFileSync, writeFileSync, existsSync } from 'node:fs';

import { RpcClient } from './rpc.mjs';
import { decodeLog, EVENT_TOPICS } from './abi-decoder.mjs';
import { decodedToSettlerEvent, postToSettler } from './forwarder.mjs';

const RPC_URL = process.env.RPC_URL ?? '';
const CONTRACT_ADDRESS = (process.env.CONTRACT_ADDRESS ?? '').toLowerCase();
const SETTLER_URL = process.env.SETTLER_URL ?? '';
const POLL_INTERVAL_MS = Number(process.env.POLL_INTERVAL_MS ?? 12000);
const FINALITY_BLOCKS = Number(process.env.FINALITY_BLOCKS ?? 18);
const LOG_BATCH_MAX_BLOCKS = Number(process.env.LOG_BATCH_MAX_BLOCKS ?? 500);
const STATE_FILE = process.env.STATE_FILE ?? './.icp-chain-watcher-state.json';
const HTTP_PORT = Number(process.env.PORT ?? 8789);

// ---------------------------------------------------------------------------
// State persistence (last processed block — restart-safe)
// ---------------------------------------------------------------------------

function loadState() {
  if (!existsSync(STATE_FILE)) return { last_processed_block: null };
  try {
    return JSON.parse(readFileSync(STATE_FILE, 'utf8'));
  } catch (_) {
    return { last_processed_block: null };
  }
}

function saveState(state) {
  writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
}

// ---------------------------------------------------------------------------
// Watcher
// ---------------------------------------------------------------------------

export class ChainWatcher {
  constructor({ rpcUrl, contractAddress, settlerUrl, startBlock = null }) {
    if (!rpcUrl) throw new Error('rpcUrl required');
    if (!contractAddress) throw new Error('contractAddress required');
    if (!settlerUrl) throw new Error('settlerUrl required');
    this.rpc = new RpcClient(rpcUrl);
    this.contractAddress = contractAddress.toLowerCase();
    this.settlerUrl = settlerUrl;
    this.startBlock = startBlock;
    this.state = loadState();
    this.metrics = {
      events_seen: 0,
      events_forwarded: 0,
      errors: 0,
      last_block: null,
      // null until the first poll completes; then { ok, error, at }.
      last_poll: null,
    };
    this._stopped = false;
    this._eventTopics = Object.values(EVENT_TOPICS);
  }

  /**
   * Run one polling cycle, recording its outcome for `/healthz`.
   *
   * Rethrows whatever `#poll()` throws so the caller's error accounting is
   * unchanged; the outcome is recorded either way.
   *
   * @returns {Promise<{ok: boolean, error: string|null}>}
   */
  async tick() {
    try {
      const outcome = await this.#poll();
      this.#notePoll(outcome.ok, outcome.error);
      return outcome;
    } catch (err) {
      this.#notePoll(false, err.message);
      throw err;
    }
  }

  #notePoll(ok, error) {
    this.metrics.last_poll = { ok, error: error ?? null, at: new Date().toISOString() };
  }

  /** Scan new blocks for events and forward them. */
  async #poll() {
    const head = await this.rpc.blockNumber();
    const finalized = head - FINALITY_BLOCKS;
    // Chain shallower than the finality depth: nothing is confirmed yet, which
    // is a healthy state, not a failure.
    if (finalized < 0) return { ok: true, error: null };

    let from = this.state.last_processed_block !== null
      ? this.state.last_processed_block + 1
      : (this.startBlock ?? Math.max(0, head - 1000));
    if (from > finalized) {
      this.metrics.last_block = finalized;
      return { ok: true, error: null }; // nothing new past finality
    }
    const to = Math.min(finalized, from + LOG_BATCH_MAX_BLOCKS - 1);

    const logs = await this.rpc.getLogs({
      address: this.contractAddress,
      topics: [this._eventTopics],
      fromBlock: from,
      toBlock: to,
    });

    for (const log of logs) {
      this.metrics.events_seen++;
      const decoded = decodeLog(log);
      if (!decoded) continue;
      const payload = decodedToSettlerEvent(decoded);
      if (!payload) continue;
      try {
        await postToSettler(this.settlerUrl, payload);
        this.metrics.events_forwarded++;
      } catch (err) {
        this.metrics.errors++;
        const reason = `forward error for ${decoded.eventName} ${decoded.escrow_id}: ${err.message}`;
        process.stderr.write(`${reason}\n`);
        // Don't advance past failures — retry next tick. The chain was
        // reachable but the settler is now behind, so this poll did not
        // succeed and /healthz must say so.
        return { ok: false, error: reason };
      }
    }

    this.state.last_processed_block = to;
    saveState(this.state);
    this.metrics.last_block = to;
    return { ok: true, error: null };
  }

  /** Long-running poll loop. */
  async run(intervalMs = POLL_INTERVAL_MS) {
    while (!this._stopped) {
      try {
        await this.tick();
      } catch (err) {
        this.metrics.errors++;
        process.stderr.write(`tick error: ${err.message}\n`);
      }
      await sleep(intervalMs);
    }
  }

  stop() {
    this._stopped = true;
  }
}

function sleep(ms) {
  return new Promise((res) => setTimeout(res, ms));
}

// ---------------------------------------------------------------------------
// HTTP health endpoint
// ---------------------------------------------------------------------------

export function startHealthServer(watcher, port = HTTP_PORT) {
  const server = createServer((req, res) => {
    if (req.method === 'GET' && req.url === '/healthz') {
      // Healthy while the last poll succeeded, or before any poll has run
      // (start-up grace). A 503 is what makes docker/Kubernetes act on a
      // watcher that can no longer see the chain.
      const lastPoll = watcher.metrics.last_poll;
      const ok = lastPoll === null || lastPoll.ok === true;
      res.writeHead(ok ? 200 : 503, { 'content-type': 'application/json' });
      res.end(JSON.stringify({
        ok,
        ...watcher.metrics,
        last_processed_block: watcher.state.last_processed_block,
      }));
      return;
    }
    res.writeHead(404);
    res.end();
  });
  server.listen(port, () => {
    const addr = server.address();
    process.stderr.write(`icp-chain-watcher health on http://127.0.0.1:${addr.port}\n`);
  });
  return server;
}

// ---------------------------------------------------------------------------
// Main (only runs when invoked directly, not when imported for tests)
// ---------------------------------------------------------------------------

if (import.meta.url === `file://${process.argv[1]}`) {
  if (!RPC_URL || !CONTRACT_ADDRESS || !SETTLER_URL) {
    process.stderr.write('FATAL: RPC_URL, CONTRACT_ADDRESS, and SETTLER_URL env vars are required\n');
    process.exit(2);
  }
  const watcher = new ChainWatcher({
    rpcUrl: RPC_URL,
    contractAddress: CONTRACT_ADDRESS,
    settlerUrl: SETTLER_URL,
    startBlock: process.env.START_BLOCK ? Number(process.env.START_BLOCK) : null,
  });
  const healthServer = startHealthServer(watcher);
  process.stderr.write(`icp-chain-watcher: rpc=${RPC_URL} contract=${CONTRACT_ADDRESS} settler=${SETTLER_URL}\n`);

  // Graceful shutdown: cursor state is already persisted after every poll,
  // so stopping cleanly just means closing the health listener and exiting
  // before the next poll begins.
  for (const signal of ['SIGTERM', 'SIGINT']) {
    process.on(signal, () => {
      process.stderr.write(`icp-chain-watcher: ${signal} received, shutting down\n`);
      healthServer.close(() => process.exit(0));
      setTimeout(() => process.exit(0), 3000).unref();
    });
  }

  watcher.run().catch((err) => {
    process.stderr.write(`fatal: ${err.message}\n`);
    process.exit(1);
  });
}
