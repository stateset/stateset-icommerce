#!/usr/bin/env node
/**
 * StateSet Payments Agent
 *
 * Payment processing and refund management specialist.
 *
 * Usage:
 *   stateset-payments "list all payments"
 *   stateset-payments "show payment PAY-123"
 *   stateset-payments --apply "create payment for order ORD-456"
 *   stateset-payments --apply "refund payment PAY-123"
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
StateSet Payments Agent - Payment Processing

Usage:
  stateset-payments [options] "<request>"

Options:
  --apply          Enable write operations (default: preview only)
  --db <path>      Database path (default: ./store.db)
  --json           JSON output
  --model <name>   Claude model to use
  --resume <id>    Resume session
  -h, --help       Show this help

Examples:
  stateset-payments "list all payments"
  stateset-payments "show payment PAY-123"
  stateset-payments "list pending payments"
  stateset-payments --apply "create payment for order ORD-456"
  stateset-payments --apply "complete payment PAY-123"
  stateset-payments --apply "refund payment PAY-123"
  stateset-payments --apply "refund $50 from payment PAY-123"

Payment Methods:
  credit_card    Credit/debit card payment
  ach            Bank transfer
  wallet         Digital wallet (Apple Pay, Google Pay, etc.)
  cash           Cash on delivery
  invoice        B2B invoicing (net terms)

Payment Status Flow:
  pending → processing → completed → refunded
         ↘ failed
`);
  process.exit(0);
}

const request = positionals.join(' ');

if (!request) {
  console.error('Usage: stateset-payments [options] "<request>"');
  console.error('Run with --help for more information');
  process.exit(1);
}

try {
  await runAgentLoop({
    agent: 'payments',
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
