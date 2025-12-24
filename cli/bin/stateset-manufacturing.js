#!/usr/bin/env node
/**
 * StateSet Manufacturing Agent
 *
 * Bill of Materials (BOM) and work order management specialist.
 *
 * Usage:
 *   stateset-manufacturing "list all BOMs"
 *   stateset-manufacturing "show work order WO-123"
 *   stateset-manufacturing --apply "create a BOM for product WIDGET-001"
 *   stateset-manufacturing --apply "start work order WO-123"
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
StateSet Manufacturing Agent - BOM and Work Order Management

Usage:
  stateset-manufacturing [options] "<request>"

Options:
  --apply          Enable write operations (default: preview only)
  --db <path>      Database path (default: ./store.db)
  --json           JSON output
  --model <name>   Claude model to use
  --resume <id>    Resume session
  -h, --help       Show this help

Examples:
  stateset-manufacturing "list all BOMs"
  stateset-manufacturing "show BOM for product WIDGET-001"
  stateset-manufacturing "list pending work orders"
  stateset-manufacturing --apply "create a BOM for product ASSEMBLY-001"
  stateset-manufacturing --apply "add component PART-A to BOM BOM-123"
  stateset-manufacturing --apply "create work order from BOM BOM-123 for 100 units"
  stateset-manufacturing --apply "start work order WO-456"
  stateset-manufacturing --apply "complete work order WO-456 with 98 units produced"

Manufacturing Concepts:
  BOM (Bill of Materials)  Recipe defining components needed to build a product
  Work Order               Production job to manufacture a quantity of products
  Yield                    Number of finished products produced

BOM Status Flow:
  draft → active → archived

Work Order Status Flow:
  pending → in_progress → completed
         ↘ cancelled
`);
  process.exit(0);
}

const request = positionals.join(' ');

if (!request) {
  console.error('Usage: stateset-manufacturing [options] "<request>"');
  console.error('Run with --help for more information');
  process.exit(1);
}

try {
  await runAgentLoop({
    agent: 'manufacturing',
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
