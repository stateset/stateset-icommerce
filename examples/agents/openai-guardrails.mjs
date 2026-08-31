// OpenAI (Responses/Agents) quickstart: the guardrails demo through
// OpenAI-format tool calls. The engine refuses the over-refund inside the
// database transaction and returns a sealed receipt with a stable code.
//
// Run: node examples/agents/openai-guardrails.mjs
import { isMain } from './x402-demo-helpers.mjs';
import { emitSummary } from './embedded-toolkit-runtime.mjs';
import { outcomeLine, receiptOutcome, setupGuardrailsScenario } from './guardrails-demo-helpers.mjs';

export async function runOpenAiGuardrailsDemo({ logger = console } = {}) {
  const { commerce, payment, toolkitOptions } = await setupGuardrailsScenario();
  const { executeOpenAIToolCall } = await import('../../bindings/node/openai.mjs');

  const callRefund = async (callId, amount, reason) =>
    executeOpenAIToolCall(
      commerce,
      {
        call_id: callId,
        function: {
          name: 'create_refund',
          arguments: JSON.stringify({ paymentId: payment.id, amount, reason }),
        },
      },
      { allowApply: true, toolkitOptions },
    );

  const blockedCall = await callRefund('call_1', 250.0, 'agent mistake');
  const blocked = receiptOutcome(blockedCall.result);
  const lines = [outcomeLine('Over-refund attempt', blocked)];

  const allowedCall = await callRefund('call_2', 40.0, 'customer request');
  const allowed = receiptOutcome(allowedCall.result);
  lines.push(outcomeLine('Legit $40 refund', allowed));

  const summary = {
    framework: 'openai',
    overRefundBlocked: blocked.blocked,
    invariantCode: blocked.code,
    legitRefundExecuted: !allowed.blocked,
  };
  emitSummary(summary, lines, logger);
  return summary;
}

if (isMain(import.meta)) {
  runOpenAiGuardrailsDemo().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
