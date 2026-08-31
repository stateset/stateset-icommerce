// LangChain quickstart: the same guardrails demo through LangChain-style
// structured tools. The engine, not the prompt, guarantees the agent
// cannot over-refund.
//
// Run: node examples/agents/langchain-guardrails.mjs
import { isMain } from './x402-demo-helpers.mjs';
import { emitSummary } from './embedded-toolkit-runtime.mjs';
import { outcomeLine, receiptOutcome, setupGuardrailsScenario } from './guardrails-demo-helpers.mjs';

// Stand-in for `DynamicStructuredTool` from '@langchain/core/tools'; with
// LangChain installed, pass the real class instead.
class DynamicStructuredTool {
  constructor(config) {
    Object.assign(this, config);
  }
  invoke(input) {
    return this.func(input);
  }
}

export async function runLangChainGuardrailsDemo({ logger = console } = {}) {
  const { commerce, payment, toolkitOptions } = await setupGuardrailsScenario();
  const { createLangChainTools } = await import('../../bindings/node/langchain.mjs');

  const tools = createLangChainTools(commerce, {
    DynamicStructuredTool,
    filter: ['create_refund'],
    allowApply: true,
    toolkitOptions,
  });
  const refundTool = tools.find((tool) => tool.name === 'create_refund');

  const blocked = receiptOutcome(
    await refundTool.invoke({ paymentId: payment.id, amount: 250.0, reason: 'agent mistake' }),
  );
  const lines = [outcomeLine('Over-refund attempt', blocked)];

  const allowed = receiptOutcome(
    await refundTool.invoke({ paymentId: payment.id, amount: 40.0, reason: 'customer request' }),
  );
  lines.push(outcomeLine('Legit $40 refund', allowed));

  const summary = {
    framework: 'langchain',
    overRefundBlocked: blocked.blocked,
    invariantCode: blocked.code,
    legitRefundExecuted: !allowed.blocked,
  };
  emitSummary(summary, lines, logger);
  return summary;
}

if (isMain(import.meta)) {
  runLangChainGuardrailsDemo().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
