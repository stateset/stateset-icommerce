#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Health Check & Diagnostics
 *
 * Usage:
 *   stateset-doctor              Check system health
 *   stateset-doctor --db ./x.db  Check specific database
 *   stateset-doctor --verbose    Show detailed diagnostics
 */

import { parseArgs } from 'node:util';
import { RichOutput, ICONS } from '../src/claude-harness.js';
import { CLI_VERSION, DEFAULT_MODEL } from '../src/config.js';
import { Commerce } from '@stateset/embedded';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

const HELP = `
StateSet iCommerce CLI - Health Check & Diagnostics

USAGE:
  stateset-doctor [options]

OPTIONS:
  --db <path>        Path to SQLite database to check (default: ./store.db)
  --verbose, -V      Show detailed diagnostics
  --json             Output as JSON
  --help, -h         Show this help message

CHECKS:
  ✓ API Key         Validates ANTHROPIC_API_KEY is set
  ✓ Database        Tests database connectivity and schema
  ✓ Node.js         Checks Node.js version compatibility
  ✓ Permissions     Verifies file system permissions
  ✓ Dependencies    Checks required packages

EXAMPLES:
  stateset-doctor
  stateset-doctor --db ./production.db
  stateset-doctor --verbose --json
`;

async function checkApiKey() {
  const apiKey = process.env.ANTHROPIC_API_KEY;
  if (!apiKey) {
    return {
      status: 'error',
      message: 'ANTHROPIC_API_KEY not set',
      hint: 'Set your API key: export ANTHROPIC_API_KEY=sk-ant-...'
    };
  }
  if (!apiKey.startsWith('sk-ant-')) {
    return {
      status: 'warning',
      message: 'API key format looks unusual',
      hint: 'Anthropic API keys typically start with sk-ant-'
    };
  }
  return {
    status: 'ok',
    message: `API key configured (${apiKey.slice(0, 10)}...${apiKey.slice(-4)})`
  };
}

async function checkDatabase(dbPath) {
  try {
    // Check if path exists for file-based databases
    if (dbPath !== ':memory:') {
      const dir = path.dirname(path.resolve(dbPath));
      if (!fs.existsSync(dir)) {
        return {
          status: 'error',
          message: `Directory does not exist: ${dir}`,
          hint: `Create the directory: mkdir -p ${dir}`
        };
      }
    }

    // Try to connect
    const commerce = new Commerce(dbPath);

    // Get some basic stats
    const customers = commerce.customers().list({ limit: 1 });
    const orders = commerce.orders().list({ limit: 1 });

    // Get counts (if tables exist)
    let stats = {};
    try {
      stats = {
        customers: commerce.customers().list({}).length,
        orders: commerce.orders().list({}).length,
        products: commerce.products().list({}).length,
        inventory: 'connected'
      };
    } catch {
      stats = { note: 'Could not fetch stats' };
    }

    return {
      status: 'ok',
      message: `Database connected: ${dbPath}`,
      stats
    };
  } catch (error) {
    return {
      status: 'error',
      message: `Database error: ${error.message}`,
      hint: 'Ensure the database file exists and is readable'
    };
  }
}

async function checkNodeVersion() {
  const version = process.versions.node;
  const major = parseInt(version.split('.')[0], 10);

  if (major < 18) {
    return {
      status: 'error',
      message: `Node.js ${version} is too old`,
      hint: 'Upgrade to Node.js 18 or later'
    };
  }
  if (major < 20) {
    return {
      status: 'warning',
      message: `Node.js ${version} works but 20+ recommended`,
      hint: 'Consider upgrading to Node.js 20 LTS'
    };
  }
  return {
    status: 'ok',
    message: `Node.js ${version}`
  };
}

async function checkPermissions(dbPath) {
  try {
    const resolvedPath = path.resolve(dbPath);
    const dir = path.dirname(resolvedPath);

    // Check directory is writable
    fs.accessSync(dir, fs.constants.W_OK);

    // If file exists, check it's readable/writable
    if (fs.existsSync(resolvedPath)) {
      fs.accessSync(resolvedPath, fs.constants.R_OK | fs.constants.W_OK);
    }

    return {
      status: 'ok',
      message: 'File permissions OK'
    };
  } catch (error) {
    return {
      status: 'error',
      message: `Permission denied: ${error.message}`,
      hint: 'Check file and directory permissions'
    };
  }
}

async function checkDependencies() {
  const required = ['@anthropic-ai/claude-agent-sdk', '@stateset/embedded', 'zod'];
  const missing = [];

  for (const pkg of required) {
    try {
      await import(pkg);
    } catch {
      missing.push(pkg);
    }
  }

  if (missing.length > 0) {
    return {
      status: 'error',
      message: `Missing packages: ${missing.join(', ')}`,
      hint: 'Run: npm install'
    };
  }
  return {
    status: 'ok',
    message: 'All dependencies installed'
  };
}

async function checkSystem() {
  return {
    status: 'ok',
    message: 'System info',
    stats: {
      platform: process.platform,
      arch: process.arch,
      memory: `${Math.round(os.totalmem() / 1024 / 1024 / 1024)}GB`,
      cpus: os.cpus().length,
      cliVersion: CLI_VERSION,
      defaultModel: DEFAULT_MODEL
    }
  };
}

async function main() {
  const { values } = parseArgs({
    options: {
      db: { type: 'string', default: './store.db' },
      verbose: { type: 'boolean', short: 'V', default: false },
      json: { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false }
    },
    allowPositionals: true
  });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  const output = new RichOutput({ color: !values.json });

  // Run all checks
  const checks = {
    'API Key': await checkApiKey(),
    'Node.js': await checkNodeVersion(),
    'Database': await checkDatabase(values.db),
    'Permissions': await checkPermissions(values.db),
    'Dependencies': await checkDependencies(),
    'System': await checkSystem()
  };

  if (values.json) {
    console.log(JSON.stringify({
      healthy: Object.values(checks).every(c => c.status !== 'error'),
      checks,
      timestamp: new Date().toISOString()
    }, null, 2));
    process.exit(0);
  }

  // Pretty output
  console.log(`\n${ICONS.analytics} ${output.bold('StateSet iCommerce Health Check')}`);
  console.log(`   CLI v${CLI_VERSION}\n`);

  let hasErrors = false;
  let hasWarnings = false;

  for (const [name, result] of Object.entries(checks)) {
    let icon, color;
    switch (result.status) {
      case 'ok':
        icon = output.green('✓');
        color = 'green';
        break;
      case 'warning':
        icon = output.yellow('⚠');
        color = 'yellow';
        hasWarnings = true;
        break;
      case 'error':
        icon = output.red('✗');
        color = 'red';
        hasErrors = true;
        break;
    }

    console.log(`${icon} ${output.bold(name)}: ${result.message}`);

    if (result.hint && (values.verbose || result.status !== 'ok')) {
      console.log(`   ${output.dim('└─')} ${output.dim(result.hint)}`);
    }

    if (values.verbose && result.stats) {
      for (const [key, value] of Object.entries(result.stats)) {
        console.log(`   ${output.dim('│')}  ${output.dim(key)}: ${value}`);
      }
    }
  }

  // Summary
  console.log();
  if (hasErrors) {
    console.log(output.red('✗ Some checks failed. Please fix the errors above.'));
    process.exit(1);
  } else if (hasWarnings) {
    console.log(output.yellow('⚠ All critical checks passed, but there are warnings.'));
    process.exit(0);
  } else {
    console.log(output.green('✓ All checks passed. System is healthy!'));
    process.exit(0);
  }
}

main().catch(error => {
  console.error('Doctor check failed:', error.message);
  process.exit(1);
});
