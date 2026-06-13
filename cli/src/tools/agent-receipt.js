/**
 * Agent Receipt Tools — expose the verifiable agent-to-agent commerce
 * lifecycle as MCP-callable tools.
 *
 *   agent_receipt_purchase   buyer drives the full purchase flow end-to-end
 *                            (escrow lock → STARK → anchor → delivery)
 *   agent_receipt_status     read on-chain escrow state for an order
 *   agent_receipt_dispute    buyer raises a dispute (escrow → Disputed)
 *   agent_receipt_resolve    operator resolves a dispute → seller or buyer
 *   agent_receipt_release    seller pulls funds after delivery (when not auto-released)
 *
 * The purchase tool shells out to the canonical demo for now; the lifecycle
 * tools call the contracts directly via ethers for sub-second responses.
 */

import crypto from 'node:crypto';
import fs from 'node:fs';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { z } from 'zod';
import { JsonRpcProvider, Wallet, Contract, keccak256, toUtf8Bytes } from 'ethers';

const execFileAsync = promisify(execFile);

const DEMO_PATH =
  process.env.AGENT_RECEIPT_DEMO_PATH || '/home/dom/icommerce-app/ves-demo/agent-receipt.mjs';
const ANVIL_URL = process.env.ANVIL_URL || 'http://localhost:8545';
const BROADCAST_LOG =
  '/home/dom/icommerce-app/set/contracts/broadcast/DeployAgentReceipt.s.sol/84532001/run-latest.json';

// Well-known Anvil/Hardhat default keys, used ONLY in explicit demo/test mode.
// These keys are public and MUST NEVER be used to sign value-bearing actions
// against any real network. In production the corresponding env vars provide a
// real key sourced from a KMS or the agent's local keystore. The Anvil fallback
// is gated behind STATESET_ALLOW_DEMO_KEYS so the tool cannot silently sign with
// a publicly-known private key outside the local demo.
const DEMO_KEYS = {
  operator: '0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d',
  buyer: '0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a',
  seller: '0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6',
};

/** @typedef {'operator' | 'buyer' | 'seller'} SigningRole */

/** Env var that holds the configured key for each signing role. */
const ROLE_ENV_VAR = {
  operator: 'SEQUENCER_KEY',
  buyer: 'BUYER_KEY',
  seller: 'SELLER_KEY',
};

/**
 * Whether the publicly-known Anvil demo keys are permitted to sign.
 * Enabled by setting STATESET_ALLOW_DEMO_KEYS to a truthy value (1/true/yes/on).
 * @returns {boolean}
 */
function demoKeysAllowed() {
  const flag = String(process.env.STATESET_ALLOW_DEMO_KEYS ?? '')
    .trim()
    .toLowerCase();
  return flag === '1' || flag === 'true' || flag === 'yes' || flag === 'on';
}

/**
 * Resolve the private key for a signing role.
 *
 * Prefers a real configured key from the role's env var. Falls back to the
 * well-known Anvil demo key only when STATESET_ALLOW_DEMO_KEYS is set; otherwise
 * throws so the tool can never sign with a publicly-known key outside the demo.
 *
 * @param {SigningRole} role
 * @returns {string} The 0x-prefixed private key to sign with.
 */
export function resolveSigningKey(role) {
  const envVar = ROLE_ENV_VAR[role];
  const configured = envVar ? process.env[envVar] : undefined;
  if (configured && configured.trim()) {
    return configured.trim();
  }
  if (demoKeysAllowed()) {
    return DEMO_KEYS[role];
  }
  throw new Error(
    `No signing key configured for role "${role}". Set ${envVar} to a real ` +
      'private key, or set STATESET_ALLOW_DEMO_KEYS=1 to use the public Anvil ' +
      'demo keys (local demo/testnet only — never on a real network).',
  );
}

const ESCROW_ABI = [
  'function dispute(bytes32 orderId, bytes32 reasonHash)',
  'function resolveDispute(bytes32 orderId, bool inFavorOfSeller)',
  'function release(bytes32 orderId)',
  'function refund(bytes32 orderId)',
  'function statusOf(bytes32) view returns (uint8)',
  'function orders(bytes32) view returns (address buyer, address seller, address token, uint128 amount, uint64 lockedAt, uint64 deliveredAt, uint64 deliveryDeadline, uint64 confirmationWindow, bytes32 deliveryReceiptHash, uint8 status)',
  'function totalLocked(address) view returns (uint256)',
  'function yieldAvailable(address token) view returns (uint256)',
  'function sweepYield(address token, address recipient)',
  'event Disputed(bytes32 indexed orderId, address indexed by, bytes32 reasonHash)',
  'event DisputeResolved(bytes32 indexed orderId, bool inFavorOfSeller, address resolvedBy)',
  'event Released(bytes32 indexed orderId, address to, uint256 amount)',
  'event Refunded(bytes32 indexed orderId, address to, uint256 amount)',
  'event YieldSwept(address indexed token, address indexed recipient, uint256 amount)',
];
const FX_ABI = [
  'function getQuote(bytes32 pair) view returns (uint256 rate, uint64 updatedAt)',
  'function convert(bytes32 pair, uint256 amountIn) view returns (uint256 amountOut, uint256 rate, uint64 updatedAt)',
  'function isFresh(bytes32 pair) view returns (bool)',
];
const STATUS_NAMES = ['None', 'Locked', 'Delivered', 'Disputed', 'Released', 'Refunded'];

let _addressCache = null;
function loadAddresses() {
  if (_addressCache) return _addressCache;
  if (process.env.ORDER_ESCROW && process.env.SSUSD_TOKEN) {
    _addressCache = {
      escrow: process.env.ORDER_ESCROW,
      ssUsd: process.env.SSUSD_TOKEN,
      fx: process.env.FX_ORACLE || null,
    };
    return _addressCache;
  }
  if (!fs.existsSync(BROADCAST_LOG)) {
    throw new Error(`OrderEscrow not deployed. Run setup.sh first: ./setup.sh`);
  }
  const log = JSON.parse(fs.readFileSync(BROADCAST_LOG, 'utf-8'));
  const escrow = log.transactions.find((t) => t.contractName === 'OrderEscrow');
  const token = log.transactions.find((t) => t.contractName === 'MockSsUSD');
  const fxOracle = log.transactions.find((t) => t.contractName === 'FxOracle');
  if (!escrow || !token) throw new Error('OrderEscrow / MockSsUSD not in broadcast log');
  _addressCache = {
    escrow: escrow.contractAddress,
    ssUsd: token.contractAddress,
    fx: fxOracle?.contractAddress || null,
  };
  return _addressCache;
}

let _providerCache = null;
function getProvider() {
  if (!_providerCache) _providerCache = new JsonRpcProvider(ANVIL_URL);
  return _providerCache;
}

function escrowAs(role) {
  const { escrow } = loadAddresses();
  const provider = getProvider();
  const signingRole = role === 'buyer' ? 'buyer' : role === 'seller' ? 'seller' : 'operator';
  const wallet = new Wallet(resolveSigningKey(signingRole), provider);
  return { contract: new Contract(escrow, ESCROW_ABI, wallet), wallet };
}

async function readEscrowOrder(orderIdHash) {
  const { escrow } = loadAddresses();
  const ro = new Contract(escrow, ESCROW_ABI, getProvider());
  const o = await ro.orders(orderIdHash);
  const status = await ro.statusOf(orderIdHash);
  return {
    orderIdHash,
    contract: escrow,
    buyer: o.buyer,
    seller: o.seller,
    token: o.token,
    amount: o.amount.toString(),
    lockedAt: Number(o.lockedAt),
    deliveredAt: Number(o.deliveredAt),
    deliveryDeadline: Number(o.deliveryDeadline),
    confirmationWindow: Number(o.confirmationWindow),
    deliveryReceiptHash: o.deliveryReceiptHash,
    status: STATUS_NAMES[Number(status)] || 'Unknown',
    statusCode: Number(status),
  };
}

const purchaseInput = z.object({
  sku: z.string().min(1).describe('Catalog SKU to purchase (e.g. "WDG-TITANIUM-1000")'),
  qty: z.number().int().positive().describe('Quantity to purchase'),
  unit_price_usd: z
    .number()
    .positive()
    .describe('Unit price in USD; the demo applies discount/tax/shipping on top'),
  max_total_usd: z
    .number()
    .positive()
    .default(100_000)
    .describe('Policy cap — STARK proof attests order_total ≤ this. Default 100,000.'),
  skip_release: z
    .boolean()
    .default(false)
    .describe(
      'If true, halt the demo after escrow.markDelivered so the caller can drive dispute/release via separate tools. Default false (auto-release).',
    ),
  fee_recipient: z
    .string()
    .regex(/^0x[0-9a-fA-F]{40}$/)
    .optional()
    .describe(
      'Optional marketplace / platform address. When set with fee_bps > 0, the buyer locks via OrderEscrow.lockWithFee() and the platform receives its cut atomically on release.',
    ),
  fee_bps: z
    .number()
    .int()
    .min(0)
    .max(1000)
    .default(0)
    .describe(
      'Marketplace fee in basis points (1 bps = 0.01%). Hard-capped at 1000 = 10% on-chain. Set 0 to skip fee.',
    ),
});

const orderIdInput = z.object({
  order_id_hash: z
    .string()
    .regex(/^0x[0-9a-fA-F]{64}$/, 'must be 0x-prefixed bytes32 hash')
    .describe(
      'The on-chain orderIdHash — found in receipt.escrow.orderIdHash from agent_receipt_purchase.',
    ),
});

const disputeInput = orderIdInput.extend({
  reason: z
    .string()
    .min(1)
    .max(500)
    .describe('Plain-text dispute reason. Hashed and stored on-chain as proof of filing.'),
});

const resolveInput = orderIdInput.extend({
  in_favor_of_seller: z
    .boolean()
    .describe('true = funds release to seller, false = funds refund to buyer.'),
});

export const agentReceiptTools = [
  {
    name: 'agent_receipt_purchase',
    description: [
      'Execute a verifiable agent-to-agent purchase end-to-end:',
      'buyer agent locks ssUSD in OrderEscrow, sequencer commits VES events,',
      'STARK proof attests order_total ≤ policy cap, SetRegistry anchors the',
      'commitment + proof on Set Chain L2, buyer marks delivered, seller',
      'releases. Returns the signed Agent Receipt JSON with on-chain tx hashes.',
      'Requires the local stack (anvil + sequencer + postgres + deployed',
      'contracts) to be running — see /home/dom/icommerce-app/setup.sh.',
    ].join(' '),
    inputSchema: purchaseInput,
    permission: 'write',
    handler: async (args) => {
      const { sku, qty, unit_price_usd, max_total_usd, skip_release, fee_recipient, fee_bps } =
        purchaseInput.parse(args);

      const cliArgs = [
        DEMO_PATH,
        '--json',
        '--sku',
        sku,
        '--qty',
        String(qty),
        '--unit-price',
        String(unit_price_usd),
        '--max-total-usd',
        String(max_total_usd),
        ...(skip_release ? ['--skip-release'] : []),
        ...(fee_recipient ? ['--fee-recipient', fee_recipient] : []),
        ...(fee_bps > 0 ? ['--fee-bps', String(fee_bps)] : []),
      ];

      try {
        const { stdout } = await execFileAsync(process.execPath, cliArgs, {
          maxBuffer: 8 * 1024 * 1024,
          timeout: 120_000,
          env: {
            ...process.env,
            AGENT_RECEIPT_JSON: '1',
          },
        });
        const receipt = JSON.parse(stdout.trim());
        return {
          success: true,
          receipt,
          summary: {
            order: receipt.order.id,
            total_usd: receipt.order.total,
            buyer_wallet: receipt.parties.buyer.wallet,
            seller_wallet: receipt.parties.seller.wallet,
            escrow_status: receipt.escrow.finalStatus,
            order_id_hash: receipt.escrow.orderIdHash,
            anchor_tx: receipt.anchor.anchorTx,
            stark_proof_hash: receipt.starkProof.proofHash,
            marketplace: receipt.escrow.marketplace || null,
          },
        };
      } catch (err) {
        return {
          success: false,
          error: err.message,
          stderr: err.stderr?.toString().slice(-2000),
          stdout: err.stdout?.toString().slice(-2000),
        };
      }
    },
  },

  {
    name: 'agent_receipt_status',
    description:
      'Read the on-chain escrow state for an order. Returns buyer, seller, ' +
      'amount, deadlines, delivery receipt hash, and current status ' +
      '(None / Locked / Delivered / Disputed / Released / Refunded).',
    inputSchema: orderIdInput,
    permission: 'read',
    handler: async (args) => {
      const { order_id_hash } = orderIdInput.parse(args);
      try {
        return { success: true, ...(await readEscrowOrder(order_id_hash)) };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'agent_receipt_dispute',
    description:
      'Buyer raises an on-chain dispute on a Delivered order. Funds freeze in ' +
      'escrow until the operator resolves. The plain-text reason is hashed ' +
      '(keccak256) and stored on-chain as proof of the filing.',
    inputSchema: disputeInput,
    permission: 'write',
    handler: async (args) => {
      const { order_id_hash, reason } = disputeInput.parse(args);
      try {
        const reasonHash = keccak256(toUtf8Bytes(reason));
        const { contract } = escrowAs('buyer');
        const tx = await contract.dispute(order_id_hash, reasonHash, { gasLimit: 200_000n });
        const rcpt = await tx.wait();
        const after = await readEscrowOrder(order_id_hash);
        return {
          success: true,
          tx: rcpt.hash,
          block: rcpt.blockNumber,
          gasUsed: rcpt.gasUsed.toString(),
          reasonHash,
          reason,
          status: after.status,
          order: after,
        };
      } catch (err) {
        return { success: false, error: err.shortMessage || err.message };
      }
    },
  },

  {
    name: 'agent_receipt_resolve',
    description:
      'Operator (sequencer / arbiter) resolves a Disputed order. ' +
      'Routes the locked funds either to the seller (in_favor_of_seller=true) ' +
      'or refunds the buyer (false). Emits DisputeResolved + Released/Refunded.',
    inputSchema: resolveInput,
    permission: 'admin',
    handler: async (args) => {
      const { order_id_hash, in_favor_of_seller } = resolveInput.parse(args);
      try {
        const { contract } = escrowAs('operator');
        const tx = await contract.resolveDispute(order_id_hash, in_favor_of_seller, {
          gasLimit: 200_000n,
        });
        const rcpt = await tx.wait();
        const after = await readEscrowOrder(order_id_hash);
        return {
          success: true,
          tx: rcpt.hash,
          block: rcpt.blockNumber,
          gasUsed: rcpt.gasUsed.toString(),
          inFavorOfSeller: in_favor_of_seller,
          status: after.status,
          order: after,
        };
      } catch (err) {
        return { success: false, error: err.shortMessage || err.message };
      }
    },
  },

  {
    name: 'agent_receipt_fx_quote',
    description:
      'Read a fresh FX quote from the on-chain FxOracle and convert an amount ' +
      'between currencies. Pair format: "BASE/QUOTE", e.g. "EUR/ssUSD" or ' +
      '"JPY/ssUSD". Returns the rate, freshness, and the converted amount. ' +
      'Use this BEFORE locking funds so the agent can verify the rate is ' +
      'fresh and within expected bounds. Pre-seeded pairs at deploy time: ' +
      'EUR/ssUSD, GBP/ssUSD, JPY/ssUSD, MXN/ssUSD.',
    inputSchema: z.object({
      pair: z
        .string()
        .regex(/^[A-Z]{2,8}\/[A-Za-z]{2,8}$/)
        .describe('Currency pair, BASE/QUOTE — e.g. "EUR/ssUSD".'),
      amount_base: z
        .number()
        .nonnegative()
        .default(1)
        .describe('Amount of the base currency to convert. Default 1 (returns the per-unit rate).'),
    }),
    permission: 'read',
    handler: async (args) => {
      const { pair, amount_base } = args;
      try {
        const { fx } = loadAddresses();
        if (!fx) throw new Error('FxOracle not deployed');
        const oracle = new Contract(fx, FX_ABI, getProvider());
        const pairId = keccak256(toUtf8Bytes(pair));
        const fresh = await oracle.isFresh(pairId);
        if (!fresh) {
          return { success: false, error: `quote for ${pair} is stale or unknown`, pair, pairId };
        }
        // Scale amount to 1e18 like the contract expects.
        const amountIn = BigInt(Math.round(amount_base * 1e6)) * 10n ** 12n; // 1e18 = 1e6 * 1e12
        const [amountOut, rate, updatedAt] = await oracle.convert(pairId, amountIn);
        return {
          success: true,
          pair,
          pairId,
          oracle: fx,
          rate: rate.toString(),
          ratePerUnit: Number(rate) / 1e18,
          amountBase: amount_base,
          amountQuote: Number(amountOut) / 1e18,
          updatedAt: Number(updatedAt),
          updatedAtIso: new Date(Number(updatedAt) * 1000).toISOString(),
          fresh: true,
        };
      } catch (err) {
        return { success: false, error: err.shortMessage || err.message };
      }
    },
  },

  {
    name: 'agent_receipt_merchant_statement',
    description:
      'Aggregate every emitted receipt in a directory into a single platform ' +
      'settlement statement: GMV, marketplace fees earned, FX exposure by ' +
      'currency, dispute outcomes, compliance bundle counts, and a sampled ' +
      'on-chain audit pass rate. Optional filters scope the statement to a ' +
      'date range, a specific seller wallet, or a specific buyer wallet — ' +
      'enabling multi-tenant accounting on a single OrderEscrow contract.',
    inputSchema: z.object({
      receipts_dir: z
        .string()
        .optional()
        .describe(
          'Directory containing receipt JSON files. Defaults to /home/dom/icommerce-app/ves-demo.',
        ),
      since_iso: z
        .string()
        .optional()
        .describe('ISO-8601 timestamp; only receipts modified at or after this time are included.'),
      seller_wallet: z
        .string()
        .regex(/^0x[0-9a-fA-F]{40}$/)
        .optional()
        .describe(
          'Filter to one seller wallet for per-seller statements (multi-tenant accounting).',
        ),
      buyer_wallet: z
        .string()
        .regex(/^0x[0-9a-fA-F]{40}$/)
        .optional()
        .describe('Filter to one buyer wallet (e.g. for buyer-side spend reports).'),
    }),
    permission: 'read',
    handler: async (args) => {
      const script = '/home/dom/icommerce-app/ves-demo/merchant-statement-demo.mjs';
      const cliArgs = [script, '--json'];
      if (args.receipts_dir) cliArgs.push('--dir', args.receipts_dir);
      if (args.since_iso) cliArgs.push('--since', args.since_iso);
      if (args.seller_wallet) cliArgs.push('--seller', args.seller_wallet);
      if (args.buyer_wallet) cliArgs.push('--buyer', args.buyer_wallet);
      try {
        const { stdout } = await execFileAsync(process.execPath, cliArgs, {
          maxBuffer: 8 * 1024 * 1024,
          timeout: 180_000,
          env: { ...process.env, MERCHANT_STATEMENT_JSON: '1' },
        });
        const statement = JSON.parse(stdout.trim());
        return {
          success: true,
          statement,
          summary: {
            receiptCount: statement.source.receiptCount,
            schemaValid: statement.schemaValidation.valid,
            schemaInvalid: statement.schemaValidation.invalid,
            gmvUsd: statement.orderActivity.gmvUsd,
            outcomes: statement.orderActivity.outcomes,
            crossBorderTotalSsUsd: statement.crossBorder.totalSsUsd,
            currenciesSeen: Object.keys(statement.crossBorder.byCurrency || {}),
            complianceBundles: statement.compliance.bundlesProduced,
            starkProofs: statement.compliance.proofs,
            auditPassed: statement.auditSample.passed,
            auditDrifted: statement.auditSample.drifted,
            auditFailed: statement.auditSample.failed,
            auditSampled: statement.auditSample.sampled,
          },
        };
      } catch (err) {
        return {
          success: false,
          error: err.message,
          stderr: err.stderr?.toString().slice(-2000),
        };
      }
    },
  },

  {
    name: 'agent_receipt_request_payout',
    description:
      "Initiate a fiat payout from the seller's SSDC balance to their bank " +
      'via the off-ramp bridge. Auto-handles SSDC.approve idempotently, ' +
      "signs a canonical payout-request message with the seller's wallet " +
      'key, POSTs the signed request to the bridge, and returns a ' +
      'Stripe-Treasury-shaped OutboundPayment intent. Requires bridge ' +
      'running on http://localhost:4243 (or BRIDGE_PAYOUT_URL env).',
    inputSchema: z.object({
      role: z
        .enum(['seller', 'buyer'])
        .default('seller')
        .describe(
          'Which signing key to use; "seller" is the default for typical platform-payout flows. The corresponding env var (SELLER_KEY / BUYER_KEY) provides the private key.',
        ),
      amount_usd: z
        .number()
        .positive()
        .describe('USD amount to convert from SSDC and queue for ACH payout.'),
      bank_last4: z
        .string()
        .regex(/^\d{4}$/)
        .describe('Last 4 digits of the recipient bank account, for the OutboundPayment metadata.'),
    }),
    permission: 'write',
    handler: async (args) => {
      const { role, amount_usd, bank_last4 } = args;
      const bridgeUrl = process.env.BRIDGE_PAYOUT_URL || 'http://localhost:4243';

      try {
        // Health check; bail if bridge isn't running so the agent can act.
        const health = await fetch(`${bridgeUrl}/health`).catch(() => null);
        if (!health || !health.ok) {
          return {
            success: false,
            error: `payout bridge not reachable at ${bridgeUrl}`,
            hint: 'Start it with: node /home/dom/icommerce-app/ves-demo/bridge-ssdc-payout.mjs',
          };
        }
        const bridgeState = await health.json();
        const bridgeTreasury = bridgeState.bridge_treasury;
        const ssdcAddr = bridgeState.ssdc;

        const provider = getProvider();
        const network = await provider.getNetwork();
        const wallet = new Wallet(
          resolveSigningKey(role === 'seller' ? 'seller' : 'buyer'),
          provider,
        );

        // Check + auto-approve once
        const SSDC_ABI = [
          'function balanceOf(address) view returns (uint256)',
          'function approve(address, uint256) returns (bool)',
          'function allowance(address, address) view returns (uint256)',
        ];
        const ssdc = new Contract(ssdcAddr, SSDC_ABI, wallet);
        const amountUnits = BigInt(Math.round(amount_usd * 1e6)) * 10n ** 12n; // 18dp
        const balance = await ssdc.balanceOf(wallet.address);
        if (balance < amountUnits) {
          return {
            success: false,
            error: `insufficient SSDC: have ${balance}, need ${amountUnits}`,
            walletBalance: balance.toString(),
            requested: amountUnits.toString(),
          };
        }
        const allowance = await ssdc.allowance(wallet.address, bridgeTreasury);
        let approveTx = null;
        if (allowance < amountUnits) {
          const tx = await ssdc.approve(bridgeTreasury, amountUnits * 10n);
          const rcpt = await tx.wait();
          approveTx = rcpt.hash;
        }

        // Compose canonical message (must match bridge-ssdc-payout.mjs).
        // 16 random bytes -> 32 hex chars, cryptographically secure (replay nonce).
        const nonce = '0x' + crypto.randomBytes(16).toString('hex');
        const issuedAt = Math.floor(Date.now() / 1000);
        const sellerChecked = wallet.address;
        const message = [
          'StateSet SSDC payout request v1',
          `seller:    ${sellerChecked}`,
          `amount:    $${amount_usd.toFixed(2)} USD`,
          `bank:      ****${bank_last4}`,
          `nonce:     ${nonce}`,
          `issuedAt:  ${issuedAt}`,
          `chainId:   ${Number(network.chainId)}`,
        ].join('\n');
        const signature = await wallet.signMessage(message);

        // POST signed request
        const resp = await fetch(`${bridgeUrl}/payout`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            seller: sellerChecked,
            amountUsd: amount_usd,
            bankLast4: bank_last4,
            nonce,
            issuedAt,
            signature,
          }),
        });
        const data = await resp.json();
        if (!resp.ok) {
          return { success: false, error: data.error, status: resp.status, approveTx };
        }
        return {
          success: true,
          approveTx,
          pull_tx: data.pull_tx,
          pull_block: data.pull_block,
          bridge_treasury: data.bridge_treasury,
          payout: data.payout,
          summary: {
            seller: sellerChecked,
            amount_usd,
            bank_last4,
            payout_id: data.payout.id,
            expected_arrival: new Date(data.payout.expected_arrival_date * 1000).toISOString(),
            status: data.payout.status,
          },
        };
      } catch (err) {
        return { success: false, error: err.shortMessage || err.message };
      }
    },
  },

  {
    name: 'agent_receipt_audit',
    description:
      'Independently audit a StateSet commerce receipt against the live chain. ' +
      'Re-verifies on-chain claims (escrow status, registry batch commitment, ' +
      'STARK proof metadata) and — for compliance bundles — runs the ' +
      'Winterfell verifier on every policy proof. Returns a structured ' +
      'pass/fail summary the calling agent can act on. The strongest ' +
      'audit primitive in the stack: any agent can verify any receipt ' +
      'without trusting the producer.',
    inputSchema: z.object({
      receipt_path: z
        .string()
        .min(1)
        .describe(
          'Absolute path to a stateset.*-receipt.v1 or stateset.compliance-bundle.v1 JSON file (e.g. /home/dom/icommerce-app/ves-demo/agent-receipt-ORD-XXX.json).',
        ),
    }),
    permission: 'read',
    handler: async (args) => {
      const { receipt_path } = args;
      const verifierScript = '/home/dom/icommerce-app/ves-demo/verify-receipt.mjs';
      try {
        const { stdout } = await execFileAsync(
          process.execPath,
          [verifierScript, receipt_path, '--json'],
          { maxBuffer: 4 * 1024 * 1024, timeout: 60_000 },
        ).catch((err) => {
          // Non-zero exit is normal when checks fail; keep going to parse JSON.
          if (err.stdout || err.stderr) return { stdout: err.stdout, stderr: err.stderr };
          throw err;
        });
        const raw = (stdout || '').toString();
        const marker = raw.indexOf('\n--JSON--\n');
        const summary = marker >= 0 ? JSON.parse(raw.slice(marker + 10)) : null;
        const transcript = marker >= 0 ? raw.slice(0, marker) : raw;
        return {
          success: true,
          allOk: summary?.allOk ?? false,
          schema: summary?.schema,
          checksPassed: summary?.checksPassed,
          checksTotal: summary?.checksTotal,
          file: summary?.file,
          // Stripped of ANSI for agent consumption.
          // eslint-disable-next-line no-control-regex
          transcript: transcript.replace(/\x1b\[[0-9;]*m/g, ''),
        };
      } catch (err) {
        return {
          success: false,
          error: err.shortMessage || err.message,
          stderr: err.stderr?.toString().slice(-1000),
        };
      }
    },
  },

  {
    name: 'agent_receipt_sweep_yield',
    description:
      'Operator/marketplace sweeps the rebasing yield surplus held by ' +
      'OrderEscrow to a recipient. With the production SSDC stablecoin, ' +
      'this is the T-Bill yield earned by escrowed funds while orders were ' +
      'in flight — a programmable platform revenue stream alongside any ' +
      'BPS fee. Read first via yield_available; positive amount returns ' +
      'the sweep tx, otherwise a no-op.',
    inputSchema: z.object({
      token_address: z
        .string()
        .regex(/^0x[0-9a-fA-F]{40}$/)
        .describe(
          'ERC-20 token to sweep yield for. Use the SSDC proxy address for the rebasing-yield case.',
        ),
      recipient: z
        .string()
        .regex(/^0x[0-9a-fA-F]{40}$/)
        .describe('Where to send the swept yield (marketplace, yield pool, buyer rebate, etc.).'),
    }),
    permission: 'admin',
    handler: async (args) => {
      const { token_address, recipient } = args;
      try {
        const { contract } = escrowAs('operator');
        const ro = new Contract(loadAddresses().escrow, ESCROW_ABI, getProvider());
        const before = await ro.yieldAvailable(token_address);
        if (before === 0n) {
          return {
            success: true,
            swept: false,
            yieldAvailable: '0',
            note: 'no yield surplus to sweep right now',
          };
        }
        const tx = await contract.sweepYield(token_address, recipient, { gasLimit: 200_000n });
        const rcpt = await tx.wait();
        const after = await ro.yieldAvailable(token_address);
        return {
          success: true,
          swept: true,
          tx: rcpt.hash,
          block: rcpt.blockNumber,
          gasUsed: rcpt.gasUsed.toString(),
          token: token_address,
          recipient,
          amountSweptUnits: (before - after).toString(),
          yieldAvailableAfter: after.toString(),
        };
      } catch (err) {
        return { success: false, error: err.shortMessage || err.message };
      }
    },
  },

  {
    name: 'agent_receipt_refund',
    description:
      "Buyer recovers locked funds after the order's deliveryDeadline has " +
      'expired. No dispute, no operator, no platform — purely the safety ' +
      'property of the OrderEscrow primitive. Reverts with DeadlineNotReached ' +
      'if the deadline has not yet passed.',
    inputSchema: orderIdInput,
    permission: 'write',
    handler: async (args) => {
      const { order_id_hash } = orderIdInput.parse(args);
      try {
        const { contract } = escrowAs('buyer');
        const tx = await contract.refund(order_id_hash, { gasLimit: 200_000n });
        const rcpt = await tx.wait();
        const after = await readEscrowOrder(order_id_hash);
        return {
          success: true,
          tx: rcpt.hash,
          block: rcpt.blockNumber,
          gasUsed: rcpt.gasUsed.toString(),
          status: after.status,
          order: after,
        };
      } catch (err) {
        return { success: false, error: err.shortMessage || err.message };
      }
    },
  },

  {
    name: 'agent_receipt_release',
    description:
      'Seller pulls escrowed funds after delivery + confirmation window. ' +
      'Use this when agent_receipt_purchase was called with skip_release=true ' +
      'and there has been no dispute. Routes funds to the seller wallet.',
    inputSchema: orderIdInput,
    permission: 'write',
    handler: async (args) => {
      const { order_id_hash } = orderIdInput.parse(args);
      try {
        const { contract } = escrowAs('seller');
        const tx = await contract.release(order_id_hash, { gasLimit: 200_000n });
        const rcpt = await tx.wait();
        const after = await readEscrowOrder(order_id_hash);
        return {
          success: true,
          tx: rcpt.hash,
          block: rcpt.blockNumber,
          gasUsed: rcpt.gasUsed.toString(),
          status: after.status,
          order: after,
        };
      } catch (err) {
        return { success: false, error: err.shortMessage || err.message };
      }
    },
  },
];
