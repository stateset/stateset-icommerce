/**
 * x402 Commands Module
 */

import { x402Tools } from '../tools/x402.js';
import {
  createMetadata,
  createUnknownActionError,
  formatToolResult,
  invokeTool,
  parseJsonArg,
} from '../command-tooling.js';

const ACTION_DEFS = [
  ['create-payment-intent', 'x402_create_payment_intent', 'Create a payment intent'],
  ['sign-intent', 'x402_sign_intent', 'Sign a payment intent'],
  ['get-intent', 'x402_get_intent', 'Get payment intent details'],
  ['list-intents', 'x402_list_intents', 'List payment intents'],
  ['settle-intent-onchain', 'x402_settle_intent_onchain', 'Settle an intent on-chain'],
  ['execute-agent-payment', 'x402_execute_agent_payment', 'Execute an end-to-end agent payment'],
  [
    'record-incoming-settlement',
    'x402_record_incoming_settlement',
    'Record an incoming settlement',
  ],
  ['mark-settled', 'x402_mark_settled', 'Mark an intent settled'],
  ['get-next-nonce', 'x402_get_next_nonce', 'Get the next payer nonce'],
  ['credit-balance', 'x402_credit_balance', 'Get credit balance'],
  ['get-credit-account', 'x402_get_credit_account', 'Get credit account'],
  ['credit-deposit', 'x402_credit_deposit', 'Deposit credit'],
  ['credit-debit', 'x402_credit_debit', 'Debit credit'],
  ['credit-transactions', 'x402_credit_transactions', 'List credit transactions'],
];

const ACTIONS = Object.fromEntries(
  ACTION_DEFS.map(([action, tool, description]) => [
    action,
    {
      tool,
      description,
      args: ['<payloadJson>'],
      parse: ([payloadJson]) => {
        if (!payloadJson) throw new Error(`Usage: x402 ${action} <payloadJson>`);
        return { params: parseJsonArg(payloadJson, 'payload') };
      },
    },
  ]),
);

export const toolActionMap = ACTION_DEFS.map(([action, tool]) => ({ action, tool }));

export async function execute(action, args, context) {
  const config = ACTIONS[action];
  if (!config) throw createUnknownActionError('x402', ACTIONS, action);

  const { params = {}, agentAddress } = config.parse(args);
  const result = await invokeTool(x402Tools, config.tool, context, params, { agentAddress });
  return formatToolResult(result, context, 'No x402 data found.');
}

export const metadata = createMetadata(
  'x402',
  ['xpay', 'credit-ledger'],
  'x402 intents, settlement, and credit ledger commands',
  ACTIONS,
);

export default { execute, metadata };
