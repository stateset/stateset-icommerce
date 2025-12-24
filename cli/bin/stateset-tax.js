#!/usr/bin/env node
/**
 * StateSet Tax Agent
 *
 * Tax calculation and compliance specialist.
 *
 * Usage:
 *   stateset-tax "calculate tax for order shipping to California"
 *   stateset-tax "what's the tax rate for New York?"
 *   stateset-tax "list tax jurisdictions"
 *   stateset-tax --apply "create tax exemption for customer abc123"
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
StateSet Tax Agent - Tax Calculation & Compliance

Usage:
  stateset-tax [options] "<request>"

Options:
  --apply          Enable write operations (default: preview only)
  --db <path>      Database path (default: ./store.db)
  --json           JSON output
  --model <name>   Claude model to use
  --resume <id>    Resume session
  -h, --help       Show this help

Examples:
  stateset-tax "calculate tax for order shipping to California"
  stateset-tax "what's the tax rate for New York?"
  stateset-tax "list tax jurisdictions"
  stateset-tax "get tax info for Texas"
  stateset-tax "what are the EU VAT rates?"
  stateset-tax "calculate tax for cart CART-123456"
  stateset-tax "show customer tax exemptions"
  stateset-tax --apply "create tax exemption for customer abc123 - resale certificate"

Tax Jurisdictions:
  US: State + County + City taxes (nexus-based)
  EU: VAT (Value Added Tax)
  CA: GST/HST/PST
  Other regions supported
`);
  process.exit(0);
}

const request = positionals.join(' ');

if (!request) {
  console.error('Usage: stateset-tax [options] "<request>"');
  console.error('Run with --help for more information');
  process.exit(1);
}

try {
  await runAgentLoop({
    agent: 'tax',
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
