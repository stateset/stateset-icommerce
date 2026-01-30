#!/usr/bin/env node

import { spawn } from 'child_process';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const log = (color, msg) => {
  const colors = {
    reset: '\x1b[0m',
    green: '\x1b[32m',
    blue: '\x1b[34m',
    yellow: '\x1b[33m',
    red: '\x1b[31m'
  };
  console.log(`${colors[color]}${msg}${colors.reset}`);
};

const runExample = async (name, script) => {
  log('blue', `\n═══════════════════════════════════════════════════`);
  log('blue', `Running: ${name}`);
  log('blue', `═══════════════════════════════════════════════════`);

  return new Promise((resolve, reject) => {
    const proc = spawn('node', [script], {
      cwd: __dirname,
      stdio: 'inherit',
      env: process.env
    });

    proc.on('close', (code) => {
      if (code === 0) {
        log('green', `✓ ${name} completed`);
        resolve();
      } else {
        log('red', `✗ ${name} failed`);
        reject(new Error(`Exit code ${code}`));
      }
    });
  });
};

const main = async () => {
  console.log(`
╔═════════════════════════════════════════════════════════╗
║     Multi-Agent iCommerce Examples Runner              ║
╚═════════════════════════════════════════════════════════╝
  `);

  const args = process.argv.slice(2);
  const example = args[0];

  const examples = [
    { name: '1. Scheduled Jobs (Independent Agents)', script: './1-scheduled-agents.js' },
    { name: '2. State Machine Workflow (Agent Handoff)', script: './2-workflow-handoff.js' },
    { name: '3. Policy-Driven Chain Reaction', script: './3-policy-chain.js' },
    { name: 'All Examples', script: null }
  ];

  if (example === 'all' || !example) {
    log('yellow', 'Running all examples...\n');
    
    for (const { name, script } of examples) {
      if (script) {
        try {
          await runExample(name, script);
        } catch (err) {
          log('red', `Skipping ${name}`);
        }
      }
    }
    
    log('green', '\n✓ All examples completed!');
  } else {
    const idx = parseInt(example) - 1;
    if (idx >= 0 && idx < examples.length - 1) {
      const { name, script } = examples[idx];
      await runExample(name, script);
    } else {
      log('red', 'Invalid example number');
      log('yellow', 'Usage: node run.js [1|2|3|all]');
      process.exit(1);
    }
  }
};

main().catch(err => {
  log('red', `Fatal error: ${err.message}`);
  process.exit(1);
});