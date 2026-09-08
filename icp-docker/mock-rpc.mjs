// Mock EVM JSON-RPC endpoint for the compose stack.
//
// `icp-chain-watcher` closes the settler's chain-mode gap: it polls
// `eth_getLogs` for ICPEscrow.sol events and forwards them to the settler's
// admin endpoint. Exercising that path used to require a real Base Sepolia
// RPC and a deployed contract, so the watcher was covered only by an
// in-test fixture and never ran in the compose stack at all.
//
// This is that fixture promoted to a service: a zero-dependency JSON-RPC
// endpoint serving synthetic — but genuinely ABI-encoded — ICPEscrow logs,
// which the real watcher decodes with the real decoder. It is a test double
// for the *chain*, not for the watcher or the settler; both of those run for
// real against it.
//
// Surface:
//   POST /            JSON-RPC 2.0: eth_blockNumber, eth_getLogs
//   GET  /healthz     { ok, head_block, logs }        (docker HEALTHCHECK)
//   POST /admin/emit  append a log past the current head and re-finalize
//
// Configuration (env vars):
//   PORT                      HTTP port (default 8790)
//   MOCK_RPC_CONTRACT         ICPEscrow address the logs are attributed to
//   MOCK_RPC_ESCROW_ID        bytes32 escrow id used by the seeded event
//   MOCK_RPC_FINALITY_BLOCKS  head − this = finalized head (default 18)
//   MOCK_RPC_SEED             'funded' (default) or 'none'

import { createServer } from 'node:http';

// ---------------------------------------------------------------------------
// Event topic hashes. Kept in sync with the watcher's decoder by importing it
// rather than by copying the hashes — a divergence here would make the mock
// chain emit logs the watcher silently ignores, which is exactly the failure
// this service exists to catch.
// ---------------------------------------------------------------------------
import { EVENT_TOPICS } from '../services/icp-chain-watcher/src/abi-decoder.mjs';

export const DEFAULTS = {
  contractAddress: (process.env.MOCK_RPC_CONTRACT ?? '0x1cb0000000000000000000000000000000e5c0a0').toLowerCase(),
  escrowId:
    process.env.MOCK_RPC_ESCROW_ID ??
    '0xabc1234567890abcdef1234567890abcdef1234567890abcdef1234567890abc',
  buyer: '0x1111111111111111111111111111111111111111',
  merchant: '0x2222222222222222222222222222222222222222',
  quoteHash: '0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef',
  receiptHash: '0xfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeed',
  // 100.000000 USDC in base units (6 decimals).
  amount: '100000000',
  fulfillmentDeadline: 1748000000,
  disputeWindow: 604800,
  seedBlock: 950,
  finalityBlocks: Number(process.env.MOCK_RPC_FINALITY_BLOCKS ?? 18),
  // Head sits far enough past the seeded event that it is already finalized,
  // so the watcher forwards it on its very first tick.
  headLead: 68,
};

// ---------------------------------------------------------------------------
// Solidity ABI encoding — the inverse of services/icp-chain-watcher/src/abi-decoder.mjs
// ---------------------------------------------------------------------------

/** Left-pad a hex string to one 32-byte EVM word. */
const word = (hex) => hex.toLowerCase().replace(/^0x/, '').padStart(64, '0');
/** uintN right-aligned in a word. */
const uintWord = (n) => word(BigInt(n).toString(16));
/** An address as an indexed topic (right-aligned in a word). */
const addressTopic = (addr) => `0x${word(addr)}`;

export class MockChain {
  constructor({
    contractAddress = DEFAULTS.contractAddress,
    escrowId = DEFAULTS.escrowId,
    finalityBlocks = DEFAULTS.finalityBlocks,
    seed = (process.env.MOCK_RPC_SEED ?? 'funded') !== 'none',
  } = {}) {
    this.contractAddress = contractAddress.toLowerCase();
    this.escrowId = escrowId;
    this.finalityBlocks = finalityBlocks;
    this.seed = seed;
    this.reset();
  }

  /** Restore the seeded genesis state — one EscrowFunded at `seedBlock`. */
  reset() {
    this.logs = [];
    this.head = DEFAULTS.seedBlock + DEFAULTS.headLead;
    this._txCounter = 0;
    if (this.seed) {
      this.logs.push(this.#buildLog('EscrowFunded', {}, DEFAULTS.seedBlock));
    }
  }

  /**
   * Append an event at `head + 1` and advance the head so the new block is
   * finalized. The watcher only reads up to `head − finalityBlocks`, so
   * without the advance it would never see it.
   *
   * @param {string} event  one of the EVENT_TOPICS keys
   * @param {object} fields overrides (amount, reason, ...)
   * @returns {{block: number, head: number}}
   */
  emit(event, fields = {}) {
    const block = this.head + 1;
    this.logs.push(this.#buildLog(event, fields, block));
    this.head = block + this.finalityBlocks;
    return { block, head: this.head };
  }

  /** eth_getLogs, honouring address / block-range / topic0 filters. */
  getLogs(filter = {}) {
    const from = parseBlockTag(filter.fromBlock, 0, this.head);
    const to = parseBlockTag(filter.toBlock, this.head, this.head);
    const addr = filter.address ? String(filter.address).toLowerCase() : null;
    const wanted = flattenTopic0(filter.topics);

    return this.logs.filter((log) => {
      const n = parseInt(log.blockNumber, 16);
      if (n < from || n > to) return false;
      if (addr && log.address.toLowerCase() !== addr) return false;
      if (wanted && !wanted.has(log.topics[0].toLowerCase())) return false;
      return true;
    });
  }

  #buildLog(event, fields, block) {
    const topic0 = EVENT_TOPICS[event];
    if (!topic0) throw new Error(`unknown event '${event}'`);
    const amount = fields.amount ?? DEFAULTS.amount;
    const base = {
      address: this.contractAddress,
      blockNumber: `0x${block.toString(16)}`,
      transactionHash: `0x${(++this._txCounter).toString(16).padStart(64, '0')}`,
      logIndex: '0x0',
    };

    switch (event) {
      case 'EscrowFunded':
        return {
          ...base,
          topics: [
            topic0,
            this.escrowId,
            addressTopic(DEFAULTS.buyer),
            addressTopic(DEFAULTS.merchant),
          ],
          data:
            '0x' +
            uintWord(amount) +
            uintWord(fields.fulfillment_deadline ?? DEFAULTS.fulfillmentDeadline) +
            uintWord(fields.dispute_window ?? DEFAULTS.disputeWindow) +
            word(fields.quote_hash ?? DEFAULTS.quoteHash),
        };
      case 'EscrowReleased':
        return {
          ...base,
          topics: [topic0, this.escrowId, addressTopic(DEFAULTS.merchant)],
          data: '0x' + uintWord(amount) + word(fields.receipt_hash ?? DEFAULTS.receiptHash),
        };
      case 'EscrowRefunded':
        return {
          ...base,
          topics: [topic0, this.escrowId, addressTopic(DEFAULTS.buyer)],
          // head: amount (word 0), reason offset (word 1) → 2 head words.
          data: '0x' + uintWord(amount) + encodeStringTail(fields.reason ?? 'refunded on chain', 2),
        };
      case 'EscrowDisputed':
        return {
          ...base,
          topics: [topic0, this.escrowId, addressTopic(DEFAULTS.buyer)],
          // head: reason offset only (word 0) → 1 head word.
          data: '0x' + encodeStringTail(fields.reason ?? 'item not as described', 1),
        };
      case 'EscrowResolved':
        return {
          ...base,
          topics: [topic0, this.escrowId, addressTopic(DEFAULTS.merchant)],
          data: '0x' + uintWord(amount) + word(fields.decision_hash ?? DEFAULTS.receiptHash),
        };
      default:
        throw new Error(`unhandled event '${event}'`);
    }
  }
}

/**
 * Encode a dynamic `string` argument as Solidity does: an OFFSET in the
 * argument's head slot, then the length and the UTF-8 bytes in the tail,
 * padded to the next 32-byte boundary.
 *
 * The offset is measured in bytes from the start of `data`, so it depends on
 * how many head words precede the tail — `headWords × 32`. `EscrowDisputed`
 * has the string as its only argument (1 head word, offset 32); on
 * `EscrowRefunded` it follows a uint128 amount (2 head words, offset 64).
 * Hard-coding 32 makes the decoder read the offset word itself as a length
 * and hand back 31 NUL bytes instead of the reason, so callers MUST pass the
 * real head-word count.
 *
 * @param {string} s          the string argument
 * @param {number} headWords  total head words in this event's data section
 */
function encodeStringTail(s, headWords) {
  if (!Number.isInteger(headWords) || headWords < 1) {
    throw new Error(`encodeStringTail: headWords must be a positive integer, got ${headWords}`);
  }
  const bytes = Buffer.from(s, 'utf8');
  const padded = bytes.toString('hex').padEnd(Math.ceil(bytes.length / 32) * 64, '0');
  return uintWord(headWords * 32) + uintWord(bytes.length) + padded;
}

function parseBlockTag(tag, fallback, head) {
  if (tag === undefined || tag === null) return fallback;
  if (tag === 'earliest') return 0;
  if (tag === 'latest' || tag === 'pending' || tag === 'finalized' || tag === 'safe') return head;
  if (typeof tag === 'number') return tag;
  return parseInt(tag, 16);
}

/** eth_getLogs `topics` is an array-of-(string|array); position 0 is topic0. */
function flattenTopic0(topics) {
  if (!Array.isArray(topics) || topics.length === 0) return null;
  const t0 = topics[0];
  if (t0 === null || t0 === undefined) return null;
  const list = Array.isArray(t0) ? t0 : [t0];
  return new Set(list.map((t) => String(t).toLowerCase()));
}

// ---------------------------------------------------------------------------
// HTTP server
// ---------------------------------------------------------------------------

async function readJson(req) {
  const chunks = [];
  for await (const c of req) chunks.push(c);
  if (chunks.length === 0) return null;
  try {
    return JSON.parse(Buffer.concat(chunks).toString('utf8'));
  } catch (_) {
    return null;
  }
}

function send(res, status, body) {
  res.writeHead(status, { 'content-type': 'application/json' });
  res.end(JSON.stringify(body));
}

export function createMockRpcServer(chain) {
  return createServer(async (req, res) => {
    const url = (req.url ?? '/').split('?')[0];

    if (req.method === 'GET' && url === '/healthz') {
      return send(res, 200, { ok: true, head_block: chain.head, logs: chain.logs.length });
    }

    if (req.method === 'POST' && url === '/admin/emit') {
      const body = (await readJson(req)) ?? {};
      try {
        const { block, head } = chain.emit(body.event ?? 'EscrowFunded', body);
        return send(res, 200, { ok: true, block, head });
      } catch (err) {
        return send(res, 400, { ok: false, error: err.message });
      }
    }

    if (req.method === 'POST') {
      const body = await readJson(req);
      if (!body) return send(res, 400, { jsonrpc: '2.0', id: null, error: { code: -32700, message: 'parse error' } });
      const id = body.id ?? null;
      try {
        switch (body.method) {
          case 'eth_blockNumber':
            return send(res, 200, { jsonrpc: '2.0', id, result: `0x${chain.head.toString(16)}` });
          case 'eth_getLogs':
            return send(res, 200, { jsonrpc: '2.0', id, result: chain.getLogs(body.params?.[0] ?? {}) });
          case 'eth_chainId':
            return send(res, 200, { jsonrpc: '2.0', id, result: '0x14a34' }); // base-sepolia
          default:
            return send(res, 200, {
              jsonrpc: '2.0',
              id,
              error: { code: -32601, message: `method not supported by mock-rpc: ${body.method}` },
            });
        }
      } catch (err) {
        return send(res, 200, { jsonrpc: '2.0', id, error: { code: -32000, message: err.message } });
      }
    }

    res.writeHead(404);
    res.end();
  });
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

if (import.meta.url === `file://${process.argv[1]}`) {
  const port = Number(process.env.PORT ?? 8790);
  const chain = new MockChain({});
  const server = createMockRpcServer(chain);
  server.listen(port, () => {
    process.stderr.write(
      `icp-docker mock-rpc: contract=${chain.contractAddress} head=${chain.head} ` +
        `logs=${chain.logs.length} on http://0.0.0.0:${server.address().port}\n`,
    );
  });
  for (const signal of ['SIGTERM', 'SIGINT']) {
    process.on(signal, () => {
      server.close(() => process.exit(0));
      setTimeout(() => process.exit(0), 3000).unref();
    });
  }
}
