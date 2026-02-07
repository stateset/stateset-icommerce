#!/usr/bin/env node

/**
 * stateset-install-service — Install StateSet Gateway as a system service.
 *
 * Auto-detects OS and installs the appropriate service configuration:
 * - Linux: systemd unit file
 * - macOS: launchd plist
 *
 * Usage:
 *   stateset-install-service              # Install service
 *   stateset-install-service --dry-run    # Preview what would be done
 *   stateset-install-service --uninstall  # Remove the service
 */

import { existsSync, mkdirSync, copyFileSync, writeFileSync } from 'node:fs';
import { unlinkSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execSync } from 'node:child_process';
import { platform } from 'node:os';
import { parseArgs } from 'node:util';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DEPLOY_DIR = join(__dirname, '..', 'deploy');

const { values } = parseArgs({
  options: {
    'dry-run': { type: 'boolean', default: false },
    uninstall: { type: 'boolean', default: false },
    json: { type: 'boolean', default: false },
    output: { type: 'string' },
    help: { type: 'boolean', short: 'h', default: false },
  },
  strict: false,
});

const dryRun = values['dry-run'];
const uninstall = values.uninstall;
const outputPath = values.output || null;
const jsonOutput = Boolean(values.json || outputPath);

const os = platform();

const HELP = `
stateset-install-service — Install StateSet Gateway as a system service.

USAGE:
  stateset-install-service              Install service
  stateset-install-service --dry-run    Preview what would be done
  stateset-install-service --uninstall  Remove the service

OPTIONS:
  --dry-run       Show actions without making changes
  --uninstall     Remove the service
  --json          Output actions as JSON
  --output <file> Write JSON output to file (implies --json)
  -h, --help      Show this help
`;

const report = {
  ok: true,
  os,
  mode: uninstall ? 'uninstall' : 'install',
  dryRun,
  steps: [],
};

function writeJson(data) {
  const payload = JSON.stringify(data, null, 2);
  if (outputPath) {
    writeFileSync(outputPath, payload);
    return;
  }
  console.log(payload);
}

function log(msg) {
  if (!jsonOutput) {
    console.log(`  ${msg}`);
  }
}

function recordStep(step) {
  report.steps.push(step);
  if (step.status === 'error') {
    report.ok = false;
  }
}

function run(cmd, label) {
  if (dryRun) {
    log(`[dry-run] ${label || cmd}`);
    recordStep({ action: 'run', command: cmd, label, status: 'dry-run' });
    return;
  }
  log(`$ ${cmd}`);
  try {
    const stdio = jsonOutput ? 'pipe' : 'inherit';
    execSync(cmd, { stdio });
    recordStep({ action: 'run', command: cmd, label, status: 'ok' });
  } catch (err) {
    if (!jsonOutput) {
      console.error(`  Failed: ${err.message}`);
    }
    recordStep({ action: 'run', command: cmd, label, status: 'error', error: err.message });
  }
}

function ensureDir(dir) {
  if (dryRun) {
    log(`[dry-run] mkdir -p ${dir}`);
    recordStep({ action: 'mkdir', target: dir, status: 'dry-run' });
    return;
  }
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
    log(`Created ${dir}`);
    recordStep({ action: 'mkdir', target: dir, status: 'ok' });
  }
}

function copyFile(src, dest) {
  if (dryRun) {
    log(`[dry-run] cp ${src} → ${dest}`);
    recordStep({ action: 'copy', source: src, target: dest, status: 'dry-run' });
    return;
  }
  copyFileSync(src, dest);
  log(`Copied ${dest}`);
  recordStep({ action: 'copy', source: src, target: dest, status: 'ok' });
}

function removeFile(path) {
  if (dryRun) {
    log(`[dry-run] rm ${path}`);
    recordStep({ action: 'remove', target: path, status: 'dry-run' });
    return;
  }
  if (existsSync(path)) {
    unlinkSync(path);
    log(`Removed ${path}`);
    recordStep({ action: 'remove', target: path, status: 'ok' });
  }
}

// ============================================================================
// Linux (systemd)
// ============================================================================

function installSystemd() {
  if (!jsonOutput) {
    console.log('\n[StateSet] Installing systemd service...\n');
  }

  const serviceFile = join(DEPLOY_DIR, 'stateset-gateway.service');
  const logrotateFile = join(DEPLOY_DIR, 'logrotate.d', 'stateset-gateway');
  const serviceDest = '/etc/systemd/system/stateset-gateway.service';
  const logrotateDest = '/etc/logrotate.d/stateset-gateway';

  // Create stateset user
  try {
    execSync('id stateset', { stdio: 'ignore' });
    log('User "stateset" already exists.');
    recordStep({ action: 'user-check', target: 'stateset', status: 'ok' });
  } catch {
    run(
      'useradd --system --no-create-home --shell /usr/sbin/nologin stateset',
      'Create stateset user',
    );
  }

  // Create directories
  ensureDir('/opt/stateset/data');
  ensureDir('/etc/stateset');
  ensureDir('/var/log/stateset');

  // Set ownership
  run('chown -R stateset:stateset /opt/stateset', 'Set /opt/stateset ownership');
  run('chown -R stateset:stateset /var/log/stateset', 'Set /var/log/stateset ownership');

  // Copy service file
  copyFile(serviceFile, serviceDest);

  // Copy logrotate config
  if (existsSync(logrotateFile)) {
    copyFile(logrotateFile, logrotateDest);
  }

  // Copy example config if none exists
  const configDest = '/etc/stateset/gateway.json';
  if (!existsSync(configDest)) {
    const exampleConfig = join(DEPLOY_DIR, 'gateway.config.example.json');
    if (existsSync(exampleConfig)) {
      copyFile(exampleConfig, configDest);
      log('Copied example config — edit /etc/stateset/gateway.json before starting.');
    }
  }

  // Reload systemd
  run('systemctl daemon-reload', 'Reload systemd');

  if (!jsonOutput) {
    console.log('\n[StateSet] Installation complete.');
    console.log('  Next steps:');
    console.log('  1. Edit /etc/stateset/gateway.json');
    console.log('  2. Add API keys to /etc/stateset/env');
    console.log('  3. sudo systemctl enable --now stateset-gateway');
    console.log('');
  }
}

function uninstallSystemd() {
  if (!jsonOutput) {
    console.log('\n[StateSet] Uninstalling systemd service...\n');
  }

  run('systemctl stop stateset-gateway 2>/dev/null || true', 'Stop service');
  run('systemctl disable stateset-gateway 2>/dev/null || true', 'Disable service');
  removeFile('/etc/systemd/system/stateset-gateway.service');
  removeFile('/etc/logrotate.d/stateset-gateway');
  run('systemctl daemon-reload', 'Reload systemd');

  if (!jsonOutput) {
    console.log(
      '\n[StateSet] Service uninstalled. Data in /opt/stateset and /etc/stateset left intact.\n',
    );
  }
}

// ============================================================================
// macOS (launchd)
// ============================================================================

function installLaunchd() {
  if (!jsonOutput) {
    console.log('\n[StateSet] Installing launchd service...\n');
  }

  const plistFile = join(DEPLOY_DIR, 'com.stateset.gateway.plist');
  const plistDest = '/Library/LaunchDaemons/com.stateset.gateway.plist';

  // Create directories
  ensureDir('/usr/local/lib/stateset');
  ensureDir('/usr/local/etc/stateset');
  ensureDir('/usr/local/var/log/stateset');

  // Copy plist
  copyFile(plistFile, plistDest);

  // Copy example config if none exists
  const configDest = '/usr/local/etc/stateset/gateway.json';
  if (!existsSync(configDest)) {
    const exampleConfig = join(DEPLOY_DIR, 'gateway.config.example.json');
    if (existsSync(exampleConfig)) {
      copyFile(exampleConfig, configDest);
      log('Copied example config — edit /usr/local/etc/stateset/gateway.json before starting.');
    }
  }

  if (!jsonOutput) {
    console.log('\n[StateSet] Installation complete.');
    console.log('  Next steps:');
    console.log('  1. Copy stateset files to /usr/local/lib/stateset/');
    console.log('  2. Edit /usr/local/etc/stateset/gateway.json');
    console.log('  3. sudo launchctl load /Library/LaunchDaemons/com.stateset.gateway.plist');
    console.log('');
  }
}

function uninstallLaunchd() {
  if (!jsonOutput) {
    console.log('\n[StateSet] Uninstalling launchd service...\n');
  }

  const plistDest = '/Library/LaunchDaemons/com.stateset.gateway.plist';

  run(`launchctl unload ${plistDest} 2>/dev/null || true`, 'Unload service');
  removeFile(plistDest);

  if (!jsonOutput) {
    console.log('\n[StateSet] Service uninstalled. Data in /usr/local/etc/stateset left intact.\n');
  }
}

// ============================================================================
// Main
// ============================================================================

function main() {
  if (values.help) {
    console.log(HELP);
    return;
  }

  if (dryRun) {
    if (!jsonOutput) {
      console.log('\n[StateSet] Dry-run mode — no changes will be made.\n');
    }
  }

  if (os === 'linux') {
    if (uninstall) {
      uninstallSystemd();
    } else {
      installSystemd();
    }
  } else if (os === 'darwin') {
    if (uninstall) {
      uninstallLaunchd();
    } else {
      installLaunchd();
    }
  } else {
    const message = `Unsupported OS: ${os}. Only Linux (systemd) and macOS (launchd) are supported.`;
    if (!jsonOutput) {
      console.error(message);
    }
    recordStep({ action: 'error', status: 'error', error: message });
  }

  if (jsonOutput) {
    writeJson(report);
  }

  if (!report.ok) {
    process.exit(1);
  }
}

import { runMain } from '../src/graceful-shutdown.js';
runMain('stateset-install-service', main);
