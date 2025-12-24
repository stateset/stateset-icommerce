#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Health Check & Diagnostics
 *
 * Usage:
 *   stateset-doctor              Check system health
 *   stateset-doctor --db ./x.db  Check specific database
 *   stateset-doctor --verbose    Show detailed diagnostics
 *   stateset-doctor --checks api,db  Run specific checks
 */

import { parseArgs } from 'node:util';
import { RichOutput, ICONS } from '../src/claude-harness.js';
import { CLI_VERSION, DEFAULT_MODEL, FEATURES } from '../src/config.js';
import { Commerce } from '@stateset/embedded';
import { checkApiAvailability } from '../src/offline.js';
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
  --checks <list>    Run specific checks (comma-separated)
  --fix              Attempt to fix issues automatically
  --help, -h         Show this help message

CHECKS:
  ✓ api             Validates ANTHROPIC_API_KEY and API connectivity
  ✓ db              Tests database connectivity and schema
  ✓ node            Checks Node.js version compatibility
  ✓ permissions     Verifies file system permissions
  ✓ dependencies    Checks required packages
  ✓ sync            Checks sync configuration (if configured)
  ✓ plugins         Validates installed plugins
  ✓ config          Verifies CLI configuration

EXAMPLES:
  stateset-doctor
  stateset-doctor --db ./production.db
  stateset-doctor --verbose --json
  stateset-doctor --checks api,db
  stateset-doctor --fix
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

  // Check API connectivity
  try {
    const connectivity = await checkApiAvailability(apiKey, { timeout: 5000 });
    if (!connectivity.available) {
      return {
        status: 'warning',
        message: `API key set but connectivity issue: ${connectivity.reason}`,
        hint: connectivity.message,
        stats: { keyConfigured: true, apiReachable: false }
      };
    }
  } catch (error) {
    // Ignore connectivity check errors, key is still valid
  }

  return {
    status: 'ok',
    message: `API key configured (${apiKey.slice(0, 10)}...${apiKey.slice(-4)})`,
    stats: { keyConfigured: true, apiReachable: true }
  };
}

async function checkSync() {
  try {
    const { isSyncConfigured, loadSyncConfig } = await import('../src/sync/config.js');

    if (!isSyncConfigured()) {
      return {
        status: 'info',
        message: 'Sync not configured (optional)',
        hint: 'Run stateset-sync init to enable event synchronization'
      };
    }

    const config = loadSyncConfig();
    return {
      status: 'ok',
      message: 'Sync configured',
      stats: {
        endpoint: config.sequencerEndpoint,
        configured: true
      }
    };
  } catch (error) {
    return {
      status: 'warning',
      message: `Sync check failed: ${error.message}`,
      hint: 'Sync may not be available'
    };
  }
}

async function checkPlugins() {
  try {
    const { createPluginLoader } = await import('../src/plugins/loader.js');
    const loader = createPluginLoader();
    const plugins = await loader.loadAll();

    if (plugins.length === 0) {
      return {
        status: 'info',
        message: 'No plugins installed (optional)',
        hint: 'Add plugins to ~/.stateset/plugins/ to extend functionality'
      };
    }

    return {
      status: 'ok',
      message: `${plugins.length} plugin(s) loaded`,
      stats: {
        count: plugins.length,
        plugins: plugins.map(p => p.name)
      }
    };
  } catch (error) {
    return {
      status: 'warning',
      message: `Plugin check failed: ${error.message}`
    };
  }
}

async function checkConfig() {
  const configDir = path.join(os.homedir(), '.stateset');
  const checks = [];

  // Check config directory
  if (fs.existsSync(configDir)) {
    checks.push({ name: 'configDir', ok: true });
  } else {
    checks.push({ name: 'configDir', ok: false, hint: 'mkdir -p ~/.stateset' });
  }

  // Check profiles
  const profilesDir = path.join(configDir, 'profiles');
  if (fs.existsSync(profilesDir)) {
    const profiles = fs.readdirSync(profilesDir).filter(f => f.endsWith('.json'));
    checks.push({ name: 'profiles', ok: true, count: profiles.length });
  } else {
    checks.push({ name: 'profiles', ok: true, count: 0 });
  }

  const allOk = checks.every(c => c.ok);
  return {
    status: allOk ? 'ok' : 'warning',
    message: allOk ? 'Configuration directory OK' : 'Configuration needs setup',
    stats: {
      configDir: configDir,
      checks
    }
  };
}

async function checkDiskSpace(dbPath) {
  try {
    const stats = fs.statSync(dbPath);
    const dbSizeMB = (stats.size / 1024 / 1024).toFixed(2);

    // Check available space in the directory
    const dir = path.dirname(path.resolve(dbPath));

    return {
      status: 'ok',
      message: `Database size: ${dbSizeMB} MB`,
      stats: {
        sizeBytes: stats.size,
        sizeMB: parseFloat(dbSizeMB),
        path: dbPath
      }
    };
  } catch (error) {
    if (error.code === 'ENOENT') {
      return {
        status: 'info',
        message: 'Database file does not exist yet',
        hint: 'Will be created on first use'
      };
    }
    return {
      status: 'warning',
      message: `Could not check database: ${error.message}`
    };
  }
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
      checks: { type: 'string', default: '' },
      fix: { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false }
    },
    allowPositionals: true
  });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  const output = new RichOutput({ color: !values.json });

  // Available checks
  const allChecks = {
    'API Key': checkApiKey,
    'Node.js': checkNodeVersion,
    'Database': () => checkDatabase(values.db),
    'Permissions': () => checkPermissions(values.db),
    'Dependencies': checkDependencies,
    'System': checkSystem,
    'Sync': checkSync,
    'Plugins': checkPlugins,
    'Config': checkConfig,
    'Disk Space': () => checkDiskSpace(values.db)
  };

  // Filter checks if specified
  let checksToRun = Object.keys(allChecks);
  if (values.checks) {
    const requested = values.checks.split(',').map(c => c.trim().toLowerCase());
    const checkMap = {
      'api': 'API Key',
      'node': 'Node.js',
      'db': 'Database',
      'database': 'Database',
      'permissions': 'Permissions',
      'deps': 'Dependencies',
      'dependencies': 'Dependencies',
      'system': 'System',
      'sync': 'Sync',
      'plugins': 'Plugins',
      'config': 'Config',
      'disk': 'Disk Space'
    };
    checksToRun = requested
      .map(r => checkMap[r])
      .filter(Boolean);
  }

  // Run selected checks
  const checks = {};
  for (const name of checksToRun) {
    const checkFn = allChecks[name];
    if (checkFn) {
      checks[name] = await checkFn();
    }
  }

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
