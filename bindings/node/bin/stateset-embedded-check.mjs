#!/usr/bin/env node

import { parseArgs } from 'node:util';
import { createCheckReport, runEmbeddedCheck } from '../self-test.mjs';

const HELP = `Usage: stateset-embedded-check [--json] [--help]

Verify the installed native engine with an offline, in-memory order, exact-decimal
payment, idempotent retry, and refund. No API keys, network calls, or files needed.
This checks local engine state only, not payment providers or external settlement.

  --json      Print one JSON report (schemaVersion: 1)
  --help, -h  Show this help without loading the native engine

Exit codes: 0 = passed, 1 = check failed, 2 = invalid arguments.
`;

function printReport(report, json) {
  if (json) {
    console.log(JSON.stringify(report));
    return;
  }
  console.log(`StateSet embedded ${report.version}: ${report.ok ? 'PASS' : 'FAIL'}`);
  console.log('Local engine only; payment providers and external settlement were not checked.');
  for (const check of report.checks) {
    console.log(`  ${check.status === 'passed' ? 'PASS' : 'FAIL'} ${check.id}`);
    if (check.message) console.log(`    ${check.code}: ${check.message}`);
    if (check.hint) console.log(`    ${check.hint}`);
  }
}

let values;
try {
  ({ values } = parseArgs({
    options: { json: { type: 'boolean' }, help: { type: 'boolean', short: 'h' } },
    allowPositionals: false,
  }));
} catch (error) {
  const report = createCheckReport();
  report.checks.push({
    id: 'arguments',
    status: 'failed',
    code: error.code,
    message: error.message,
    hint: 'Use stateset-embedded-check --help. No database path or provider options are accepted.',
  });
  printReport(report, process.argv.slice(2).includes('--json'));
  process.exitCode = 2;
}

if (values?.help) {
  console.log(HELP);
} else if (values) {
  const report = await runEmbeddedCheck();
  printReport(report, values.json);
  process.exitCode = report.ok ? 0 : 1;
}
