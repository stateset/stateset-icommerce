#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Update Manager
 *
 * Usage:
 *   stateset-update status
 *   stateset-update check --json
 *   stateset-update apply --yes
 */

import { parseArgs } from 'node:util';
import { spawnSync } from 'node:child_process';
import { CLI_VERSION } from '../src/config.js';
import { runMain } from '../src/graceful-shutdown.js';

const PACKAGE_NAME = '@stateset/cli';
const DEFAULT_TIMEOUT_MS = 10000;
const SUPPORTED_COMMANDS = new Set(['status', 'check', 'apply']);

const HELP = `
StateSet iCommerce CLI - Update Manager

USAGE:
  stateset-update [command] [options]

COMMANDS:
  status              Show current version and check latest (default)
  check               Alias of status
  apply               Install latest or selected version

OPTIONS:
  --json              Output JSON
  --yes, -y           Execute update immediately (apply only)
  --channel <name>    Release channel/tag (default: latest)
  --tag <version>     Exact package version/tag
  --timeout <ms>      npm lookup timeout in ms (default: 10000)
  --help, -h          Show this help message
  --version, -v       Show version

EXAMPLES:
  stateset update
  stateset update status --json
  stateset update apply --tag 0.7.14 --yes
`;

function parseTimeout(value) {
  if (value === undefined) return DEFAULT_TIMEOUT_MS;
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error('--timeout must be a positive integer');
  }
  return parsed;
}

function normalizeVersion(version) {
  return String(version || '')
    .trim()
    .replace(/^v/i, '')
    .split('-')[0];
}

function compareVersions(a, b) {
  const av = normalizeVersion(a)
    .split('.')
    .map((n) => Number.parseInt(n, 10) || 0);
  const bv = normalizeVersion(b)
    .split('.')
    .map((n) => Number.parseInt(n, 10) || 0);
  const length = Math.max(av.length, bv.length);
  for (let i = 0; i < length; i++) {
    const left = av[i] ?? 0;
    const right = bv[i] ?? 0;
    if (left > right) return 1;
    if (left < right) return -1;
  }
  return 0;
}

function fetchLatestVersion(reference, timeoutMs) {
  const query = `${PACKAGE_NAME}@${reference}`;
  const result = spawnSync('npm', ['view', query, 'version', '--json'], {
    encoding: 'utf-8',
    timeout: timeoutMs,
  });

  if (result.error && result.status === null) {
    return {
      version: null,
      error: `npm lookup failed: ${result.error.message}`,
      command: `npm view ${query} version --json`,
    };
  }

  if (result.status !== 0) {
    const stderr = (result.stderr || '').trim();
    return {
      version: null,
      error: stderr || `npm exited with status ${result.status}`,
      command: `npm view ${query} version --json`,
    };
  }

  const raw = (result.stdout || '').trim();
  if (!raw) {
    return {
      version: null,
      error: 'npm returned empty version response',
      command: `npm view ${query} version --json`,
    };
  }

  try {
    const parsed = JSON.parse(raw);
    const version = Array.isArray(parsed) ? parsed[parsed.length - 1] : parsed;
    return { version: String(version), error: null, command: `npm view ${query} version --json` };
  } catch {
    return {
      version: raw.replace(/^"+|"+$/g, ''),
      error: null,
      command: `npm view ${query} version --json`,
    };
  }
}

function renderHumanStatus(report) {
  console.log(`Current: ${report.currentVersion}`);
  if (report.error) {
    console.log(`Latest:  unavailable (${report.error})`);
    console.log(`Hint:    Run '${report.lookupCommand}' when network access is available.`);
    return;
  }

  console.log(`Latest:  ${report.latestVersion} (${report.reference})`);
  if (report.updateAvailable) {
    console.log(`Update available: yes`);
    console.log(`Install with: npm install -g ${PACKAGE_NAME}@${report.latestVersion}`);
  } else {
    console.log('Update available: no (already up to date)');
  }
}

function emitJson(payload) {
  console.log(JSON.stringify(payload, null, 2));
}

function performApply(report) {
  if (!report.latestVersion) {
    throw new Error('Unable to resolve a target version. Run `stateset-update status` first.');
  }

  const packageRef = `${PACKAGE_NAME}@${report.latestVersion}`;
  const installResult = spawnSync('npm', ['install', '-g', packageRef], {
    encoding: 'utf-8',
  });

  if (installResult.stdout) process.stdout.write(installResult.stdout);
  if (installResult.stderr) process.stderr.write(installResult.stderr);

  if (installResult.error && installResult.status === null) {
    throw new Error(`Update failed: ${installResult.error.message}`);
  }
  if (installResult.status !== 0) {
    throw new Error(`Update failed with exit code ${installResult.status}`);
  }

  return {
    installedVersion: report.latestVersion,
    installCommand: `npm install -g ${PACKAGE_NAME}@${report.latestVersion}`,
  };
}

async function main() {
  const { values, positionals } = parseArgs({
    options: {
      json: { type: 'boolean', default: false },
      yes: { type: 'boolean', short: 'y', default: false },
      channel: { type: 'string' },
      tag: { type: 'string' },
      timeout: { type: 'string' },
      help: { type: 'boolean', short: 'h', default: false },
      version: { type: 'boolean', short: 'v', default: false },
    },
    allowPositionals: true,
  });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  if (values.version) {
    console.log(`@stateset/cli v${CLI_VERSION}`);
    process.exit(0);
  }

  const command = (positionals[0] || 'status').toLowerCase();
  if (!SUPPORTED_COMMANDS.has(command)) {
    throw new Error(
      `Unknown command '${command}'. Expected one of: ${Array.from(SUPPORTED_COMMANDS).join(', ')}`,
    );
  }

  const timeoutMs = parseTimeout(values.timeout);
  const reference = values.tag || values.channel || 'latest';
  const latest = fetchLatestVersion(reference, timeoutMs);
  const updateAvailable =
    !latest.error && latest.version
      ? compareVersions(normalizeVersion(latest.version), normalizeVersion(CLI_VERSION)) > 0
      : false;

  const report = {
    package: PACKAGE_NAME,
    currentVersion: CLI_VERSION,
    latestVersion: latest.version,
    updateAvailable,
    reference,
    lookupCommand: latest.command,
    checkedAt: new Date().toISOString(),
    error: latest.error,
  };

  if (command === 'apply') {
    const installCommand = report.latestVersion
      ? `npm install -g ${PACKAGE_NAME}@${report.latestVersion}`
      : null;

    if (values.json) {
      if (!values.yes) {
        emitJson({
          ...report,
          action: 'preview-install',
          installCommand,
        });
        process.exit(0);
      }

      if (!report.latestVersion) {
        throw new Error(`Cannot apply update: ${report.error || 'no target version available'}`);
      }

      const installResult = performApply(report);
      emitJson({
        ...report,
        action: 'installed',
        installCommand,
        ...installResult,
      });
      process.exit(0);
    }

    if (!report.latestVersion) {
      throw new Error(`Cannot apply update: ${report.error || 'no target version available'}`);
    }

    if (!values.yes) {
      console.log(`Ready to install ${report.latestVersion}.`);
      console.log(`Run with --yes to execute, or run manually:\n  ${installCommand}`);
      process.exit(0);
    }

    performApply(report);
    console.log(`Updated successfully to ${report.latestVersion}.`);
    process.exit(0);
  }

  if (values.json) {
    emitJson(report);
  } else {
    renderHumanStatus(report);
  }
}

runMain('stateset-update', main);
