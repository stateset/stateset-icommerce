#!/usr/bin/env node

import * as fs from 'node:fs';
import * as path from 'node:path';
import { spawnSync } from 'node:child_process';
import {
  backupStore,
  configureAgent,
  diagnoseOmarchy,
  discoverStore,
  getStoreStatus,
  installMenu,
  installPlugin,
  installUserService,
  loadOmarchyConfig,
  manageUserService,
  remediationForKind,
  saveOmarchyConfig,
  selectAttention,
  uninstallMenu,
  uninstallPlugin,
  validateOperatorConfig,
} from '../src/omarchy.js';
import { runMain } from '../src/graceful-shutdown.js';

const HELP = `
StateSet iCommerce for Omarchy

USAGE:
  stateset-omarchy install [--db PATH] [--force] [--no-enable] [--service]
  stateset-omarchy uninstall [--no-disable]
  stateset-omarchy status [--json] [--db PATH]
  stateset-omarchy doctor [--json] [--db PATH]
  stateset-omarchy dashboard
  stateset-omarchy attention [--kind all|failed-payments|low-stock|pending-returns|pending-orders] [--json]
  stateset-omarchy remediate [--kind failed-payments|low-stock|pending-returns|pending-orders]
  stateset-omarchy configure [--agent all|claude|codex|opencode]
  stateset-omarchy service install [--port 8090] [--no-start]
  stateset-omarchy service status|start|stop|restart|remove [--json]
  stateset-omarchy backup [--output PATH]
  stateset-omarchy agent
  stateset-omarchy db-path

SAFETY:
  Installation configures preview-only MCP writes by default. --apply is accepted
  only with --kernel-policy, --kernel-principal, and --kernel-store-id.
  Use --preview to return an existing installation to preview-only mode.
`;

function option(args, name) {
  const index = args.indexOf(name);
  return index >= 0 ? args[index + 1] : null;
}

function has(args, name) {
  return args.includes(name);
}

function buildConfig(args) {
  const prior = loadOmarchyConfig();
  const dbPath = discoverStore({ dbPath: option(args, '--db') }) || path.resolve('store.db');
  const config = {
    ...prior,
    dbPath,
    profile: option(args, '--profile') || prior.profile || 'core',
    apply: has(args, '--preview') ? false : has(args, '--apply') || prior.apply === true,
    kernelPolicy: option(args, '--kernel-policy')
      ? path.resolve(option(args, '--kernel-policy'))
      : prior.kernelPolicy,
    kernelPrincipal: option(args, '--kernel-principal')
      ? path.resolve(option(args, '--kernel-principal'))
      : prior.kernelPrincipal,
    kernelStoreId: option(args, '--kernel-store-id') || prior.kernelStoreId,
  };
  validateOperatorConfig(config, { checkFiles: config.apply === true });
  return config;
}

function printStatus(status) {
  console.log(`StateSet iCommerce — ${status.message}`);
  console.log(`Store: ${status.dbPath || 'not discovered'}`);
  console.log(`Mode: ${status.mode}`);
  if (status.ok) {
    console.log(
      `Orders ${status.counts.orders} · Customers ${status.counts.customers} · Products ${status.counts.products} · Returns ${status.counts.returns} · Payments ${status.counts.payments}`,
    );
  }
}

function printAttention(report) {
  const total = report.attention.reduce((sum, item) => sum + item.count, 0);
  console.log(
    `StateSet iCommerce — ${total} selected item${total === 1 ? '' : 's'} need attention`,
  );
  console.log(`Store: ${report.dbPath || 'not discovered'}`);
  console.log(`Mode: ${report.mode}`);
  if (report.attention.length === 0) {
    console.log('No matching operational alerts.');
    return;
  }
  for (const item of report.attention) {
    console.log(`\n${item.label}: ${item.count}`);
    for (const sample of report.samples[item.kind] || []) {
      if (item.kind === 'failed-payments') {
        console.log(`  ${sample.paymentNumber || sample.id} · ${sample.status}`);
      } else if (item.kind === 'low-stock') {
        console.log(
          `  ${sample.sku || 'unknown SKU'} · available ${sample.available ?? '?'} · reorder ${sample.reorderPoint ?? '?'}`,
        );
      } else if (item.kind === 'pending-orders') {
        console.log(`  ${sample.orderNumber || sample.id} · ${sample.status}`);
      } else if (item.kind === 'pending-returns') {
        console.log(`  ${sample.returnNumber || sample.id} · ${sample.status}`);
      }
    }
  }
  if (!report.signalsComplete) {
    console.log(`\nUnavailable signals: ${report.unavailableSignals.join(', ')}`);
  }
  console.log('\nUse the Payments, Inventory, Returns, or Orders agent to review and remediate.');
}

function printDoctor(report) {
  console.log(`StateSet Omarchy — ${report.ready ? 'ready' : 'action required'}`);
  for (const check of report.checks) {
    const marker = check.ok ? 'PASS' : check.required ? 'FAIL' : 'WARN';
    console.log(`${marker.padEnd(4)}  ${check.label} · ${check.detail}`);
  }
}

async function main() {
  const args = process.argv.slice(2);
  const command = args[0] || 'status';
  if (has(args, '--help') || has(args, '-h')) {
    console.log(HELP.trim());
    return;
  }

  if (command === 'install') {
    const config = buildConfig(args);
    const plugin = installPlugin({
      force: has(args, '--force'),
      enable: !has(args, '--no-enable'),
    });
    const configFile = saveOmarchyConfig(config);
    let menuFile = null;
    try {
      menuFile = installMenu();
    } catch (error) {
      console.warn(`Menu integration skipped: ${error.message}`);
    }
    const configured = [];
    for (const agent of ['claude', 'codex', 'opencode']) {
      try {
        configured.push(configureAgent(agent, config));
      } catch (error) {
        console.warn(`${agent} configuration skipped: ${error.message}`);
      }
    }
    console.log(
      plugin.managedExternally
        ? `Using Git-managed StateSet Omarchy plugin at ${plugin.target}`
        : `Installed StateSet Omarchy plugin at ${plugin.target}`,
    );
    console.log(
      plugin.enabled
        ? 'Plugin enabled.'
        : 'Plugin staged; enable it with: omarchy plugin enable com.stateset.icommerce',
    );
    console.log(`Configuration: ${configFile}`);
    if (menuFile) console.log(`Omarchy menu: ${menuFile}`);
    console.log(`Agent configuration: ${configured.join(', ')}`);
    if (has(args, '--service')) {
      const service = installUserService(config, {
        port: option(args, '--port') || 8090,
        start: !has(args, '--no-start'),
      });
      console.log(
        service.started
          ? `MCP service started: http://127.0.0.1:${option(args, '--port') || 8090}/mcp`
          : `MCP service unit: ${service.file}${service.error ? ` (${service.error})` : ''}`,
      );
    }
    return;
  }

  if (command === 'uninstall') {
    const service = manageUserService('status');
    if (service.installed) manageUserService('remove');
    const plugin = uninstallPlugin({ disable: !has(args, '--no-disable') });
    const menu = uninstallMenu();
    console.log(
      plugin.removed
        ? `Removed StateSet Omarchy plugin from ${plugin.target}`
        : `StateSet Omarchy plugin is not installed at ${plugin.target}`,
    );
    if (menu.removed) console.log(`Removed StateSet entries from ${menu.file}`);
    console.log('Store data, StateSet configuration, and agent configuration were retained.');
    return;
  }

  if (command === 'configure') {
    const config = buildConfig(args);
    saveOmarchyConfig(config);
    const requested = option(args, '--agent') || 'all';
    const agents = requested === 'all' ? ['claude', 'codex', 'opencode'] : [requested];
    for (const agent of agents) console.log(`${agent}: ${configureAgent(agent, config)}`);
    return;
  }

  if (command === 'service') {
    const action = args[1] || 'install';
    if (action === 'install') {
      const config = buildConfig(args);
      saveOmarchyConfig(config);
      const service = installUserService(config, {
        port: option(args, '--port') || 8090,
        start: !has(args, '--no-start'),
      });
      console.log(`Service unit: ${service.file}`);
      if (service.started) console.log('Service enabled and started.');
      else if (service.error) console.warn(`Service not started: ${service.error}`);
      return;
    }
    const service = manageUserService(action);
    if (has(args, '--json')) console.log(JSON.stringify(service));
    else {
      console.log(`Service: ${service.state}`);
      console.log(`Unit: ${service.file}`);
    }
    return;
  }

  if (command === 'status' || command === 'dashboard') {
    const status = await getStoreStatus({ dbPath: option(args, '--db') });
    if (has(args, '--json')) console.log(JSON.stringify(status));
    else printStatus(status);
    if (command === 'dashboard' && process.stdout.isTTY) {
      process.stdout.write('\nWrites remain preview-only. Press Enter to close.');
      fs.readSync(0, Buffer.alloc(1), 0, 1, null);
      process.stdout.write('\n');
    }
    return;
  }

  if (command === 'attention') {
    const status = await getStoreStatus({ dbPath: option(args, '--db') });
    if (!status.ok) throw new Error(status.message);
    const report = selectAttention(status, option(args, '--kind') || 'all');
    if (has(args, '--json')) console.log(JSON.stringify(report));
    else printAttention(report);
    return;
  }

  if (command === 'doctor') {
    const report = await diagnoseOmarchy({ dbPath: option(args, '--db') });
    if (has(args, '--json')) console.log(JSON.stringify(report));
    else printDoctor(report);
    if (!report.ready) process.exitCode = 1;
    return;
  }

  if (command === 'remediate') {
    const status = await getStoreStatus({ dbPath: option(args, '--db') });
    if (!status.ok) throw new Error(status.message);
    const requestedKind = option(args, '--kind');
    const kind = requestedKind || status.attention?.[0]?.kind;
    if (!kind) {
      console.log('No operational alerts require remediation.');
      return;
    }
    const remediation = remediationForKind(kind);
    const result = spawnSync(remediation.command, ['--db', status.dbPath, remediation.request], {
      stdio: 'inherit',
    });
    if (result.error) throw result.error;
    process.exitCode = result.status ?? 1;
    return;
  }

  if (command === 'backup') {
    const result = await backupStore({
      dbPath: option(args, '--db'),
      destination: option(args, '--output'),
    });
    console.log(`Backup created: ${result.backupPath}`);
    return;
  }

  if (command === 'db-path') {
    const dbPath = discoverStore({ dbPath: option(args, '--db') });
    if (!dbPath) throw new Error('No iCommerce store found');
    console.log(dbPath);
    return;
  }

  if (command === 'agent') {
    const dbPath = discoverStore({ dbPath: option(args, '--db') });
    if (!dbPath) throw new Error('No iCommerce store found');
    const result = spawnSync('stateset', ['--db', dbPath], { stdio: 'inherit' });
    process.exitCode = result.status ?? 1;
    return;
  }

  throw new Error(`Unknown command: ${command}\n${HELP}`);
}

runMain('stateset-omarchy', main);
