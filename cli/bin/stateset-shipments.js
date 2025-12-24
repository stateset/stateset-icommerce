#!/usr/bin/env node
/**
 * StateSet Shipments Agent
 *
 * Shipment tracking and delivery management specialist.
 *
 * Usage:
 *   stateset-shipments "list all shipments"
 *   stateset-shipments "show shipment SHIP-123"
 *   stateset-shipments --apply "create shipment for order ORD-456"
 *   stateset-shipments --apply "mark shipment SHIP-123 as delivered"
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
StateSet Shipments Agent - Shipment Tracking

Usage:
  stateset-shipments [options] "<request>"

Options:
  --apply          Enable write operations (default: preview only)
  --db <path>      Database path (default: ./store.db)
  --json           JSON output
  --model <name>   Claude model to use
  --resume <id>    Resume session
  -h, --help       Show this help

Examples:
  stateset-shipments "list all shipments"
  stateset-shipments "show shipment SHIP-123"
  stateset-shipments "list in-transit shipments"
  stateset-shipments --apply "create shipment for order ORD-456 with tracking FEDEX123"
  stateset-shipments --apply "mark shipment SHIP-123 as delivered"

Shipping Carriers:
  FEDEX, UPS, USPS, DHL, and regional carriers

Shipment Status Flow:
  created → shipped → in_transit → delivered
                               ↘ exception
`);
  process.exit(0);
}

const request = positionals.join(' ');

if (!request) {
  console.error('Usage: stateset-shipments [options] "<request>"');
  console.error('Run with --help for more information');
  process.exit(1);
}

try {
  await runAgentLoop({
    agent: 'shipments',
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
