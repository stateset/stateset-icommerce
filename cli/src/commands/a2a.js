/**
 * A2A Commands Module
 */

import { a2aTools } from '../tools/a2a.js';
import {
  createMetadata,
  createUnknownActionError,
  formatToolResult,
  invokeTool,
  parseJsonArg,
} from '../command-tooling.js';

const AGENT_ACTIONS = new Set([
  'pay',
  'request-payment',
  'pay-request',
  'request-quote',
  'provide-quote',
  'accept-quote',
  'decline-quote',
  'fulfill-quote',
  'get-payment',
  'list-payments',
  'list-payment-requests',
  'list-quotes',
  'balance',
  'counter-quote',
  'revise-quote',
  'create-escrow',
  'fund-escrow',
  'release-escrow',
  'refund-escrow',
  'dispute-escrow',
  'list-escrows',
  'file-dispute',
  'submit-evidence',
  'resolve-dispute',
  'rate-agent',
  'respond-to-feedback',
  'register-service',
  'execute-split-payment',
  'create-conditional-payment',
  'check-payment-conditions',
  'settle-conditional-payment',
]);

const ACTION_DEFS = [
  ['pay', 'a2a_pay', 'Pay another agent'],
  ['request-payment', 'a2a_request_payment', 'Create a payment request'],
  ['pay-request', 'a2a_pay_request', 'Pay a payment request'],
  ['request-quote', 'a2a_request_quote', 'Request a quote'],
  ['provide-quote', 'a2a_provide_quote', 'Provide a quote'],
  ['accept-quote', 'a2a_accept_quote', 'Accept a quote'],
  ['decline-quote', 'a2a_decline_quote', 'Decline a quote'],
  ['fulfill-quote', 'a2a_fulfill_quote', 'Mark a quote fulfilled'],
  ['get-payment', 'a2a_get_payment', 'Get payment details'],
  ['list-payments', 'a2a_list_payments', 'List payments'],
  ['list-payment-requests', 'a2a_list_payment_requests', 'List payment requests'],
  ['list-quotes', 'a2a_list_quotes', 'List quotes'],
  ['balance', 'a2a_get_balance', 'Get payment balance summary'],
  ['discover-agents', 'a2a_discover_agents', 'Discover agents'],
  ['counter-quote', 'a2a_counter_quote', 'Counter a quote'],
  ['revise-quote', 'a2a_revise_quote', 'Revise a quote'],
  ['create-escrow', 'a2a_create_escrow', 'Create an escrow'],
  ['fund-escrow', 'a2a_fund_escrow', 'Fund an escrow'],
  ['release-escrow', 'a2a_release_escrow', 'Release an escrow'],
  ['refund-escrow', 'a2a_refund_escrow', 'Refund an escrow'],
  ['dispute-escrow', 'a2a_dispute_escrow', 'Dispute an escrow'],
  ['get-escrow', 'a2a_get_escrow', 'Get escrow details'],
  ['list-escrows', 'a2a_list_escrows', 'List escrows'],
  ['file-dispute', 'a2a_file_dispute', 'File a dispute'],
  ['submit-evidence', 'a2a_submit_evidence', 'Submit dispute evidence'],
  ['resolve-dispute', 'a2a_resolve_dispute', 'Resolve a dispute'],
  ['get-dispute', 'a2a_get_dispute', 'Get dispute details'],
  ['list-disputes', 'a2a_list_disputes', 'List disputes'],
  ['rate-agent', 'a2a_rate_agent', 'Rate an agent'],
  ['get-reputation', 'a2a_get_reputation', 'Get agent reputation'],
  ['respond-to-feedback', 'a2a_respond_to_feedback', 'Respond to feedback'],
  ['register-service', 'a2a_register_service', 'Register a service'],
  ['list-services', 'a2a_list_services', 'List services'],
  ['get-service', 'a2a_get_service', 'Get service details'],
  ['send-notification', 'a2a_send_notification', 'Send a notification'],
  ['notification-log', 'a2a_list_notification_log', 'List notification log'],
  ['configure-webhooks', 'a2a_configure_webhooks', 'Configure webhooks'],
  ['webhook-dlq', 'a2a_list_webhook_dlq', 'List webhook dead-letter entries'],
  ['quarantine-failed-webhooks', 'a2a_quarantine_failed_webhooks', 'Quarantine failed webhooks'],
  ['replay-dlq-entry', 'a2a_replay_dlq_entry', 'Replay a dead-letter entry'],
  ['purge-dlq', 'a2a_purge_dlq', 'Purge dead-letter entries'],
  ['dlq-count', 'a2a_dlq_count', 'Get dead-letter queue count'],
  ['create-agent-subscription', 'a2a_create_agent_subscription', 'Create an agent subscription'],
  ['pause-agent-subscription', 'a2a_pause_agent_subscription', 'Pause an agent subscription'],
  ['resume-agent-subscription', 'a2a_resume_agent_subscription', 'Resume an agent subscription'],
  ['cancel-agent-subscription', 'a2a_cancel_agent_subscription', 'Cancel an agent subscription'],
  ['get-agent-subscription', 'a2a_get_agent_subscription', 'Get subscription details'],
  ['list-agent-subscriptions', 'a2a_list_agent_subscriptions', 'List agent subscriptions'],
  [
    'process-subscription-billing',
    'a2a_process_subscription_billing',
    'Process subscription billing',
  ],
  ['create-split-payment', 'a2a_create_split_payment', 'Create a split payment'],
  ['execute-split-payment', 'a2a_execute_split_payment', 'Execute a split payment'],
  ['get-split-payment', 'a2a_get_split_payment', 'Get split payment details'],
  ['list-split-payments', 'a2a_list_split_payments', 'List split payments'],
  ['create-conditional-payment', 'a2a_create_conditional_payment', 'Create a conditional payment'],
  ['check-payment-conditions', 'a2a_check_payment_conditions', 'Check payment conditions'],
  ['settle-conditional-payment', 'a2a_settle_conditional_payment', 'Settle a conditional payment'],
  ['subscribe-events', 'a2a_subscribe_events', 'Subscribe to events'],
  ['list-event-subscriptions', 'a2a_list_event_subscriptions', 'List event subscriptions'],
  ['event-history', 'a2a_get_event_history', 'Get event history'],
];

const ACTIONS = Object.fromEntries(
  ACTION_DEFS.map(([action, tool, description]) => [
    action,
    {
      tool,
      description,
      args: AGENT_ACTIONS.has(action) ? ['<agentAddress>', '[payloadJson]'] : ['[payloadJson]'],
      parse: (args) => {
        if (AGENT_ACTIONS.has(action)) {
          const [agentAddress, payloadJson] = args;
          if (!agentAddress) throw new Error(`Usage: a2a ${action} <agentAddress> [payloadJson]`);
          return {
            agentAddress,
            params: payloadJson ? parseJsonArg(payloadJson, 'payload') : {},
          };
        }

        const [payloadJson] = args;
        return {
          params: payloadJson ? parseJsonArg(payloadJson, 'payload') : {},
        };
      },
    },
  ]),
);

export const toolActionMap = ACTION_DEFS.map(([action, tool]) => ({ action, tool }));

export async function execute(action, args, context) {
  const config = ACTIONS[action];
  if (!config) throw createUnknownActionError('a2a', ACTIONS, action);

  const { params = {}, agentAddress } = config.parse(args);
  const result = await invokeTool(a2aTools, config.tool, context, params, { agentAddress });
  return formatToolResult(result, context, 'No A2A data found.');
}

export const metadata = createMetadata(
  'a2a',
  ['p2p', 'agentpay'],
  'Agent-to-agent payments, quotes, escrow, disputes, services, subscriptions, and events',
  ACTIONS,
);

export default { execute, metadata };
