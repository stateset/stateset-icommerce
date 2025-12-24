#!/usr/bin/env node
/**
 * StateSet Invoices Agent
 *
 * B2B invoice management and accounts receivable specialist.
 *
 * Usage:
 *   stateset-invoices "list all invoices"
 *   stateset-invoices "show overdue invoices"
 *   stateset-invoices --apply "create invoice for order ORD-123"
 *   stateset-invoices --apply "record payment for invoice INV-456"
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
StateSet Invoices Agent - B2B Invoice Management

Usage:
  stateset-invoices [options] "<request>"

Options:
  --apply          Enable write operations (default: preview only)
  --db <path>      Database path (default: ./store.db)
  --json           JSON output
  --model <name>   Claude model to use
  --resume <id>    Resume session
  -h, --help       Show this help

Examples:
  stateset-invoices "list all invoices"
  stateset-invoices "show overdue invoices"
  stateset-invoices "show invoice INV-123"
  stateset-invoices --apply "create invoice for order ORD-456"
  stateset-invoices --apply "send invoice INV-123"
  stateset-invoices --apply "record $500 payment for invoice INV-123"

Payment Terms:
  Net 15, Net 30, Net 45, Net 60
  Due on Receipt
  Custom terms

Invoice Status Flow:
  draft → sent → viewed → partially_paid → paid
                      ↘ overdue → bad_debt
`);
  process.exit(0);
}

const request = positionals.join(' ');

if (!request) {
  console.error('Usage: stateset-invoices [options] "<request>"');
  console.error('Run with --help for more information');
  process.exit(1);
}

try {
  await runAgentLoop({
    agent: 'invoices',
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
