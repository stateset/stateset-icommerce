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

import { existsSync, mkdirSync, copyFileSync, chmodSync, readFileSync } from 'node:fs';
import { unlinkSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execSync } from 'node:child_process';
import { platform } from 'node:os';

const __dirname = dirname(fileURLToPath(import.meta.url));
const DEPLOY_DIR = join(__dirname, '..', 'deploy');

const args = process.argv.slice(2);
const dryRun = args.includes('--dry-run');
const uninstall = args.includes('--uninstall');

const os = platform();

function log(msg) {
  console.log(`  ${msg}`);
}

function run(cmd, label) {
  if (dryRun) {
    log(`[dry-run] ${label || cmd}`);
    return;
  }
  log(`$ ${cmd}`);
  try {
    execSync(cmd, { stdio: 'inherit' });
  } catch (err) {
    console.error(`  Failed: ${err.message}`);
  }
}

function ensureDir(dir) {
  if (dryRun) {
    log(`[dry-run] mkdir -p ${dir}`);
    return;
  }
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
    log(`Created ${dir}`);
  }
}

function copyFile(src, dest) {
  if (dryRun) {
    log(`[dry-run] cp ${src} → ${dest}`);
    return;
  }
  copyFileSync(src, dest);
  log(`Copied ${dest}`);
}

function removeFile(path) {
  if (dryRun) {
    log(`[dry-run] rm ${path}`);
    return;
  }
  if (existsSync(path)) {
    unlinkSync(path);
    log(`Removed ${path}`);
  }
}

// ============================================================================
// Linux (systemd)
// ============================================================================

function installSystemd() {
  console.log('\n[StateSet] Installing systemd service...\n');

  const serviceFile = join(DEPLOY_DIR, 'stateset-gateway.service');
  const logrotateFile = join(DEPLOY_DIR, 'logrotate.d', 'stateset-gateway');
  const serviceDest = '/etc/systemd/system/stateset-gateway.service';
  const logrotateDest = '/etc/logrotate.d/stateset-gateway';

  // Create stateset user
  try {
    execSync('id stateset', { stdio: 'ignore' });
    log('User "stateset" already exists.');
  } catch {
    run('useradd --system --no-create-home --shell /usr/sbin/nologin stateset', 'Create stateset user');
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

  console.log('\n[StateSet] Installation complete.');
  console.log('  Next steps:');
  console.log('  1. Edit /etc/stateset/gateway.json');
  console.log('  2. Add API keys to /etc/stateset/env');
  console.log('  3. sudo systemctl enable --now stateset-gateway');
  console.log('');
}

function uninstallSystemd() {
  console.log('\n[StateSet] Uninstalling systemd service...\n');

  run('systemctl stop stateset-gateway 2>/dev/null || true', 'Stop service');
  run('systemctl disable stateset-gateway 2>/dev/null || true', 'Disable service');
  removeFile('/etc/systemd/system/stateset-gateway.service');
  removeFile('/etc/logrotate.d/stateset-gateway');
  run('systemctl daemon-reload', 'Reload systemd');

  console.log('\n[StateSet] Service uninstalled. Data in /opt/stateset and /etc/stateset left intact.\n');
}

// ============================================================================
// macOS (launchd)
// ============================================================================

function installLaunchd() {
  console.log('\n[StateSet] Installing launchd service...\n');

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

  console.log('\n[StateSet] Installation complete.');
  console.log('  Next steps:');
  console.log('  1. Copy stateset files to /usr/local/lib/stateset/');
  console.log('  2. Edit /usr/local/etc/stateset/gateway.json');
  console.log('  3. sudo launchctl load /Library/LaunchDaemons/com.stateset.gateway.plist');
  console.log('');
}

function uninstallLaunchd() {
  console.log('\n[StateSet] Uninstalling launchd service...\n');

  const plistDest = '/Library/LaunchDaemons/com.stateset.gateway.plist';

  run(`launchctl unload ${plistDest} 2>/dev/null || true`, 'Unload service');
  removeFile(plistDest);

  console.log('\n[StateSet] Service uninstalled. Data in /usr/local/etc/stateset left intact.\n');
}

// ============================================================================
// Main
// ============================================================================

function main() {
  if (dryRun) {
    console.log('\n[StateSet] Dry-run mode — no changes will be made.\n');
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
    console.error(`Unsupported OS: ${os}. Only Linux (systemd) and macOS (launchd) are supported.`);
    process.exit(1);
  }
}

main();
