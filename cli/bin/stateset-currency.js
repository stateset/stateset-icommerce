#!/usr/bin/env node
/**
 * StateSet Currency Agent
 *
 * Multi-currency support and exchange rate management specialist.
 *
 * Usage:
 *   stateset-currency "list exchange rates"
 *   stateset-currency "convert 100 USD to EUR"
 *   stateset-currency --apply "set exchange rate USD to EUR at 0.92"
 *   stateset-currency --apply "enable currencies USD, EUR, GBP"
 */

import { runAgentLoop, AGENTS } from '../src/claude-harness.js';
import { parseArgs } from 'node:util';

const options = {
  apply: { type: 'boolean', default: false },
  db: { type: 'string', default: './store.db' },
  json: { type: 'boolean', default: false },
  model: { type: 'string' },
  resume: { type: 'string' },
  help: { type: 'boolean', short: 'h', default: false }
};

const { values, positionals } = parseArgs({ options, allowPositionals: true });

if (values.help) {
  console.log(`
StateSet Currency Agent - Multi-Currency Management

Usage:
  stateset-currency [options] "<request>"

Options:
  --apply          Enable write operations (default: preview only)
  --db <path>      Database path (default: ./store.db)
  --json           JSON output
  --model <name>   Claude model to use
  --resume <id>    Resume session
  -h, --help       Show this help

Examples:
  stateset-currency "list exchange rates"
  stateset-currency "get exchange rate from USD to EUR"
  stateset-currency "convert 100 USD to EUR"
  stateset-currency "what currencies are enabled?"
  stateset-currency --apply "set exchange rate USD to EUR at 0.92"
  stateset-currency --apply "set base currency to EUR"
  stateset-currency --apply "enable currencies USD, EUR, GBP, JPY"

Common Currencies:
  USD, EUR, GBP, JPY, CAD, AUD
  Many others supported
`);
  process.exit(0);
}

const request = positionals.join(' ');

if (!request) {
  console.error('Usage: stateset-currency [options] "<request>"');
  console.error('Run with --help for more information');
  process.exit(1);
}

try {
  await runAgentLoop({
    agent: 'currency',
    request,
    applyMode: values.apply,
    dbPath: values.db,
    jsonOutput: values.json,
    model: values.model,
    sessionId: values.resume
  });
} catch (error) {
  console.error('Error:', error.message);
  process.exit(1);
}
