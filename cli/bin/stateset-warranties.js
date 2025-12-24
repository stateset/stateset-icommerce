#!/usr/bin/env node
/**
 * StateSet Warranties Agent
 *
 * Product warranty and claims management specialist.
 *
 * Usage:
 *   stateset-warranties "list all warranties"
 *   stateset-warranties "show warranty claims"
 *   stateset-warranties --apply "create warranty for order ORD-123"
 *   stateset-warranties --apply "file warranty claim for product WIDGET-001"
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
StateSet Warranties Agent - Warranty & Claims Management

Usage:
  stateset-warranties [options] "<request>"

Options:
  --apply          Enable write operations (default: preview only)
  --db <path>      Database path (default: ./store.db)
  --json           JSON output
  --model <name>   Claude model to use
  --resume <id>    Resume session
  -h, --help       Show this help

Examples:
  stateset-warranties "list all warranties"
  stateset-warranties "show active warranties"
  stateset-warranties "list pending warranty claims"
  stateset-warranties --apply "create 1 year warranty for order ORD-123"
  stateset-warranties --apply "file warranty claim for product WIDGET-001"
  stateset-warranties --apply "approve warranty claim CLM-456"

Warranty Status Flow:
  active → claimed → expired
            ↘ processed

Claim Status Flow:
  pending → approved → processed
         ↘ rejected
`);
  process.exit(0);
}

const request = positionals.join(' ');

if (!request) {
  console.error('Usage: stateset-warranties [options] "<request>"');
  console.error('Run with --help for more information');
  process.exit(1);
}

try {
  await runAgentLoop({
    agent: 'warranties',
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
