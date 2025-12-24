#!/usr/bin/env node
/**
 * StateSet Suppliers Agent
 *
 * Supplier management and purchase order specialist.
 *
 * Usage:
 *   stateset-suppliers "list all suppliers"
 *   stateset-suppliers "list purchase orders"
 *   stateset-suppliers --apply "create supplier Acme Corp"
 *   stateset-suppliers --apply "create purchase order for 100 widgets from Acme Corp"
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
StateSet Suppliers Agent - Supplier & Procurement

Usage:
  stateset-suppliers [options] "<request>"

Options:
  --apply          Enable write operations (default: preview only)
  --db <path>      Database path (default: ./store.db)
  --json           JSON output
  --model <name>   Claude model to use
  --resume <id>    Resume session
  -h, --help       Show this help

Examples:
  stateset-suppliers "list all suppliers"
  stateset-suppliers "list purchase orders"
  stateset-suppliers "show low stock items that need reordering"
  stateset-suppliers --apply "create supplier Acme Corp"
  stateset-suppliers --apply "create PO for 100 WIDGET-001 from Acme Corp"
  stateset-suppliers --apply "approve purchase order PO-123"
  stateset-suppliers --apply "send purchase order PO-123 to supplier"

Purchase Order Status Flow:
  draft → approved → sent → partially_received → received
       ↘ cancelled
`);
  process.exit(0);
}

const request = positionals.join(' ');

if (!request) {
  console.error('Usage: stateset-suppliers [options] "<request>"');
  console.error('Run with --help for more information');
  process.exit(1);
}

try {
  await runAgentLoop({
    agent: 'suppliers',
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
