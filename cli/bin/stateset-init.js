#!/usr/bin/env node

/**
 * StateSet Init Command
 *
 * Initialize a new StateSet Commerce database with optional demo data.
 *
 * Usage:
 *   stateset-init                   # Initialize empty database
 *   stateset-init --demo            # Initialize with demo data
 *   stateset-init --demo --db path  # Custom database path
 *   stateset-init --force --demo    # Overwrite existing database
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { parseArgs } from 'node:util';
import { runMain } from '../src/graceful-shutdown.js';

const HELP = `
StateSet Init - Initialize a Commerce Database

USAGE:
  stateset-init [options]

OPTIONS:
  --quickstart        Zero-prompt standalone setup: creates DB with demo data,
                      initializes .stateset/config.json, ready in under 60 seconds
  --demo              Seed realistic demo data (10 customers, 20 products,
                      15 orders, inventory, promotions, subscriptions)
  --db <path>         Database path (default: ./store.db)
  --force             Overwrite existing database
  -q, --quiet         Minimal output
  -h, --help          Show this help message

EXAMPLES:
  stateset-init --quickstart             # Full standalone setup in 60 seconds
  stateset-init --demo                   # Quick start with demo data
  stateset-init --demo --db my-store.db  # Custom database path
  stateset-init                          # Empty database ready for use

After initializing, try:
  stateset "show me all customers"
  stateset "what products are low on stock?"
  stateset "what is my revenue this month?"
`;

async function main() {
  const { values } = parseArgs({
    options: {
      quickstart: { type: 'boolean', default: false },
      demo: { type: 'boolean', default: false },
      db: { type: 'string', default: './store.db' },
      force: { type: 'boolean', default: false },
      quiet: { type: 'boolean', short: 'q', default: false },
      help: { type: 'boolean', short: 'h', default: false },
    },
    allowPositionals: false,
  });

  if (values.help) {
    console.log(HELP);
    return;
  }

  // --quickstart implies --demo and --force for zero-prompt setup
  const isQuickstart = values.quickstart;
  const wantDemo = values.demo || isQuickstart;
  const forceOverwrite = values.force || isQuickstart;
  const quiet = values.quiet;

  const dbPath = path.resolve(values.db);

  // Check for existing database
  if (fs.existsSync(dbPath) && !forceOverwrite) {
    console.error(`Database already exists: ${dbPath}`);
    console.error('Use --force to overwrite, or specify a different path with --db');
    process.exit(1);
  }

  // Remove existing database if --force
  if (fs.existsSync(dbPath) && forceOverwrite) {
    fs.unlinkSync(dbPath);
    if (!quiet) {
      console.log(`Removed existing database: ${dbPath}`);
    }
  }

  // Ensure parent directory exists
  const dir = path.dirname(dbPath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  // Initialize database via Commerce constructor
  const { createRequire } = await import('node:module');
  const require = createRequire(import.meta.url);

  let Commerce;
  try {
    const mod = require('@stateset/embedded');
    Commerce = mod.Commerce || mod.default?.Commerce || mod.default;
  } catch (err) {
    console.error('Failed to load @stateset/embedded:', err.message);
    console.error('Run: npm install @stateset/embedded');
    process.exit(1);
  }

  if (!Commerce) {
    console.error('Failed to resolve Commerce export from @stateset/embedded.');
    process.exit(1);
  }

  const commerce = new Commerce(dbPath);

  if (!quiet) {
    console.log(`Database initialized: ${dbPath}`);
  }

  // Seed demo data if requested (or --quickstart)
  if (wantDemo) {
    const { seedDemoData } = await import('../src/seeds/demo.js');
    await seedDemoData(commerce, { quiet });
  } else if (!quiet) {
    console.log('');
    console.log('Empty database created. To add demo data, run:');
    console.log(`  stateset-init --demo --force --db ${values.db}`);
    console.log('');
  }

  // --quickstart: also create standalone config
  if (isQuickstart) {
    try {
      const { saveStandaloneConfig, DEFAULT_STANDALONE_CONFIG } =
        await import('../src/config/standalone.js');
      saveStandaloneConfig({ ...DEFAULT_STANDALONE_CONFIG, dbPath }, process.cwd());
      if (!quiet) {
        console.log('Standalone config created: .stateset/config.json');
      }
    } catch (err) {
      console.warn('Could not create standalone config:', err.message);
    }

    if (!quiet) {
      console.log('');
      console.log('Quickstart complete! Try:');
      console.log('  stateset "show me all customers"');
      console.log('  stateset "what products are low on stock?"');
      console.log('  stateset "what is my revenue this month?"');
      console.log('');
      console.log('Connect payment webhooks:');
      console.log('  stateset-webhooks --stripe-secret whsec_YOUR_SECRET --port 3000');
      console.log('');
    }
  }
}

runMain('stateset-init', main);
