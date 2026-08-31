// Custom runtime / Claude Agent SDK quickstart: generic tool descriptors
// ({ name, description, schema, execute }) work with any agent loop.
// Claude Desktop and Claude Code users can get the same tools with zero
// code via MCP: npx -y -p @stateset/cli stateset-mcp --db ./store.db
//
// Run: node examples/agents/custom-runtime-guardrails.mjs
import { isMain } from './x402-demo-helpers.mjs';
import { emitSummary } from './embedded-toolkit-runtime.mjs';
import { outcomeLine, receiptOutcome, setupGuardrailsScenario } from './guardrails-demo-helpers.mjs';

export async function runCustomRuntimeGuardrailsDemo({ logger = console } = {}) {
  const { commerce, payment, toolkitOptions } = await setupGuardrailsScenario();
  const { createToolDescriptors } = await import('../../bindings/node/generic.mjs');

  const descriptors = createToolDescriptors(commerce, {
    filter: ['create_refund'],
    allowApply: true,
    toolkitOptions,
  });
  const refundTool = descriptors.find((descriptor) => descriptor.name === 'create_refund');

  const blocked = receiptOutcome(
    await refundTool.execute({ paymentId: payment.id, amount: 250.0, reason: 'agent mistake' }),
  );
  const lines = [outcomeLine('Over-refund attempt', blocked)];

  const allowed = receiptOutcome(
    await refundTool.execute({ paymentId: payment.id, amount: 40.0, reason: 'customer request' }),
  );
  lines.push(outcomeLine('Legit $40 refund', allowed));

  const summary = {
    framework: 'custom-runtime',
    overRefundBlocked: blocked.blocked,
    invariantCode: blocked.code,
    legitRefundExecuted: !allowed.blocked,
  };
  emitSummary(summary, lines, logger);
  return summary;
}

if (isMain(import.meta)) {
  runCustomRuntimeGuardrailsDemo().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
