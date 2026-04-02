#!/usr/bin/env node

import { runExactHttpFlowDemo } from './x402-exact-http-flow.mjs';
import { runLocalIntentFlowDemo } from './x402-local-intent-flow.mjs';
import { runCreditLedgerFlowDemo } from './x402-credit-ledger-flow.mjs';
import { isMain, printSection } from './x402-demo-helpers.mjs';

export async function runAllX402AgentDemoFlows() {
  printSection('StateSet Agent x402 Demo Pack');
  await runExactHttpFlowDemo();
  await runLocalIntentFlowDemo();
  await runCreditLedgerFlowDemo();
}

if (isMain(import.meta)) {
  runAllX402AgentDemoFlows().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}

