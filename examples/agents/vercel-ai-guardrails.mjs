// Vercel AI SDK quickstart: give an agent refund powers, then watch the
// engine refuse an over-refund with a stable invariant code — no prompt
// engineering, the guarantee lives in the database transaction.
//
// Run: node examples/agents/vercel-ai-guardrails.mjs
import { isMain } from './x402-demo-helpers.mjs';
import { emitSummary } from './embedded-toolkit-runtime.mjs';
import { outcomeLine, receiptOutcome, setupGuardrailsScenario } from './guardrails-demo-helpers.mjs';

export async function runVercelAiGuardrailsDemo({ logger = console } = {}) {
  const { commerce, payment, toolkitOptions } = await setupGuardrailsScenario();
  const { createVercelAITools } = await import('../../bindings/node/vercel-ai.mjs');

  // With the real Vercel AI SDK, pass `tool` from the 'ai' package and hand
  // `tools` straight to generateText({ model, tools, ... }).
  const tools = createVercelAITools(commerce, {
    tool: (definition) => definition,
    filter: ['create_refund'],
    allowApply: true,
    toolkitOptions,
  });

  // The agent tries to refund $250 of a $100 payment.
  const blocked = receiptOutcome(
    await tools.create_refund.execute({ paymentId: payment.id, amount: 250.0, reason: 'agent mistake' }),
  );
  const lines = [outcomeLine('Over-refund attempt', blocked)];

  // A legitimate partial refund goes through.
  const allowed = receiptOutcome(
    await tools.create_refund.execute({ paymentId: payment.id, amount: 40.0, reason: 'customer request' }),
  );
  lines.push(outcomeLine('Legit $40 refund', allowed));

  const summary = {
    framework: 'vercel-ai',
    overRefundBlocked: blocked.blocked,
    invariantCode: blocked.code,
    legitRefundExecuted: !allowed.blocked,
  };
  emitSummary(summary, lines, logger);
  return summary;
}

if (isMain(import.meta)) {
  runVercelAiGuardrailsDemo().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
