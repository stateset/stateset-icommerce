/**
 * Agent Receipt Commands Module
 *
 * On-chain purchase receipts for the x402 agent demo flows: escrowed
 * purchases, status lookups, disputes, FX quotes, statements, and payouts.
 */

import { agentReceiptTools } from '../tools/agent-receipt.js';
import {
  createMetadata,
  createUnknownActionError,
  formatToolResult,
  invokeTool,
  parseOptionalBoolean,
  parseOptionalNumber,
} from '../command-tooling.js';

const ACTIONS = {
  purchase: {
    tool: 'agent_receipt_purchase',
    description: 'Buy a SKU with escrowed funds and an on-chain receipt',
    args: ['<sku>', '<qty>', '<unitPriceUsd>', '[maxTotalUsd]', '[skipRelease]'],
    parse: ([sku, qtyRaw, unitPriceRaw, maxTotalRaw, skipReleaseRaw]) => {
      if (!sku || !qtyRaw || !unitPriceRaw) {
        throw new Error(
          'Usage: agent-receipt purchase <sku> <qty> <unitPriceUsd> [maxTotalUsd] [skipRelease]',
        );
      }
      return {
        params: {
          sku,
          qty: parseOptionalNumber(qtyRaw, 'qty must be a number'),
          unit_price_usd: parseOptionalNumber(unitPriceRaw, 'unitPriceUsd must be a number'),
          max_total_usd: parseOptionalNumber(maxTotalRaw, 'maxTotalUsd must be a number'),
          skip_release: parseOptionalBoolean(skipReleaseRaw, 'skipRelease must be true or false'),
        },
      };
    },
  },
  status: {
    tool: 'agent_receipt_status',
    description: 'Look up the on-chain state of an order receipt',
    args: ['<orderIdHash>'],
    parse: ([orderIdHash]) => {
      if (!orderIdHash) throw new Error('Usage: agent-receipt status <orderIdHash>');
      return { params: { order_id_hash: orderIdHash } };
    },
  },
  dispute: {
    tool: 'agent_receipt_dispute',
    description: 'File a dispute against an escrowed order',
    args: ['<orderIdHash>', '<reason>'],
    parse: ([orderIdHash, ...reasonParts]) => {
      const reason = reasonParts.join(' ').trim();
      if (!orderIdHash || !reason) {
        throw new Error('Usage: agent-receipt dispute <orderIdHash> <reason>');
      }
      return { params: { order_id_hash: orderIdHash, reason } };
    },
  },
  resolve: {
    tool: 'agent_receipt_resolve',
    description: 'Resolve a disputed order (admin)',
    args: ['<orderIdHash>', '<inFavorOfSeller>'],
    parse: ([orderIdHash, inFavorRaw]) => {
      if (!orderIdHash || inFavorRaw === undefined) {
        throw new Error('Usage: agent-receipt resolve <orderIdHash> <true|false>');
      }
      return {
        params: {
          order_id_hash: orderIdHash,
          in_favor_of_seller: parseOptionalBoolean(
            inFavorRaw,
            'inFavorOfSeller must be true or false',
          ),
        },
      };
    },
  },
  'fx-quote': {
    tool: 'agent_receipt_fx_quote',
    description: 'Get an FX quote for a currency pair',
    args: ['<pair>', '[amountBase]'],
    parse: ([pair, amountRaw]) => {
      if (!pair) throw new Error('Usage: agent-receipt fx-quote <BASE/QUOTE> [amountBase]');
      return {
        params: {
          pair,
          amount_base: parseOptionalNumber(amountRaw, 'amountBase must be a number'),
        },
      };
    },
  },
  'merchant-statement': {
    tool: 'agent_receipt_merchant_statement',
    description: 'Summarize stored receipts into a merchant statement',
    args: ['[receiptsDir]', '[sinceIso]', '[sellerWallet]', '[buyerWallet]'],
    parse: ([receiptsDir, sinceIso, sellerWallet, buyerWallet]) => ({
      params: {
        receipts_dir: receiptsDir,
        since_iso: sinceIso,
        seller_wallet: sellerWallet,
        buyer_wallet: buyerWallet,
      },
    }),
  },
  'request-payout': {
    tool: 'agent_receipt_request_payout',
    description: 'Request a fiat payout against settled receipts',
    args: ['<amountUsd>', '<bankLast4>', '[role]'],
    parse: ([amountRaw, bankLast4, role]) => {
      if (!amountRaw || !bankLast4) {
        throw new Error('Usage: agent-receipt request-payout <amountUsd> <bankLast4> [role]');
      }
      return {
        params: {
          amount_usd: parseOptionalNumber(amountRaw, 'amountUsd must be a number'),
          bank_last4: bankLast4,
          role,
        },
      };
    },
  },
  audit: {
    tool: 'agent_receipt_audit',
    description: 'Verify a stored receipt file against on-chain state',
    args: ['<receiptPath>'],
    parse: ([receiptPath]) => {
      if (!receiptPath) throw new Error('Usage: agent-receipt audit <receiptPath>');
      return { params: { receipt_path: receiptPath } };
    },
  },
  'sweep-yield': {
    tool: 'agent_receipt_sweep_yield',
    description: 'Sweep accrued escrow yield to a recipient (admin)',
    args: ['<tokenAddress>', '<recipient>'],
    parse: ([tokenAddress, recipient]) => {
      if (!tokenAddress || !recipient) {
        throw new Error('Usage: agent-receipt sweep-yield <tokenAddress> <recipient>');
      }
      return { params: { token_address: tokenAddress, recipient } };
    },
  },
  refund: {
    tool: 'agent_receipt_refund',
    description: 'Refund an escrowed order to the buyer',
    args: ['<orderIdHash>'],
    parse: ([orderIdHash]) => {
      if (!orderIdHash) throw new Error('Usage: agent-receipt refund <orderIdHash>');
      return { params: { order_id_hash: orderIdHash } };
    },
  },
  release: {
    tool: 'agent_receipt_release',
    description: 'Release escrowed funds to the seller',
    args: ['<orderIdHash>'],
    parse: ([orderIdHash]) => {
      if (!orderIdHash) throw new Error('Usage: agent-receipt release <orderIdHash>');
      return { params: { order_id_hash: orderIdHash } };
    },
  },
};

export const toolActionMap = Object.entries(ACTIONS).map(([action, config]) => ({
  action,
  tool: config.tool,
}));

export async function execute(action, args, context) {
  const config = ACTIONS[action];
  if (!config) throw createUnknownActionError('agent-receipt', ACTIONS, action);

  const { params = {} } = config.parse(args);
  const result = await invokeTool(agentReceiptTools, config.tool, context, params);
  return formatToolResult(result, context, 'No receipt data found.');
}

export const metadata = createMetadata(
  'agent-receipt',
  ['receipts', 'ar'],
  'On-chain agent purchase receipts: escrow, disputes, FX, payouts',
  ACTIONS,
);

export default { execute, metadata };
