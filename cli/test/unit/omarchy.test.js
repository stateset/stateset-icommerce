import { afterEach, beforeEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { performance } from 'node:perf_hooks';
import {
  OMARCHY_PLUGIN_ID,
  configPaths,
  configureAgent,
  diagnoseOmarchy,
  discoverStore,
  getOperationalSummary,
  getStoreStatus,
  installMenu,
  installPlugin,
  installUserService,
  manageUserService,
  mcpServerConfig,
  remediationForKind,
  saveOmarchyConfig,
  selectAttention,
  summarizeOperations,
  uninstallMenu,
  uninstallPlugin,
  validateOperatorConfig,
  userServiceUnit,
} from '../../src/omarchy.js';

describe('Omarchy integration', () => {
  let homeDir;
  let projectDir;
  let previousXdgConfig;

  beforeEach(() => {
    homeDir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-omarchy-home-'));
    projectDir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-omarchy-project-'));
    previousXdgConfig = process.env.XDG_CONFIG_HOME;
    delete process.env.XDG_CONFIG_HOME;
  });

  afterEach(() => {
    if (previousXdgConfig === undefined) delete process.env.XDG_CONFIG_HOME;
    else process.env.XDG_CONFIG_HOME = previousXdgConfig;
    fs.rmSync(homeDir, { recursive: true, force: true });
    fs.rmSync(projectDir, { recursive: true, force: true });
  });

  it('discovers a store by walking up from a project directory', () => {
    const store = path.join(projectDir, 'store.db');
    const nested = path.join(projectDir, 'apps', 'storefront');
    fs.mkdirSync(nested, { recursive: true });
    fs.writeFileSync(store, 'fixture');
    assert.equal(discoverStore({ cwd: nested, homeDir }), store);
  });

  it('uses the operator configuration outside the project directory', () => {
    const store = path.join(projectDir, 'production.db');
    saveOmarchyConfig({ dbPath: store, profile: 'core', apply: false }, homeDir);
    assert.equal(discoverStore({ cwd: homeDir, homeDir }), store);
  });

  it('reports a missing configured database without creating it', async () => {
    const store = path.join(projectDir, 'missing.db');
    const status = await getStoreStatus({ dbPath: store, homeDir });
    assert.equal(status.ok, false);
    assert.equal(status.configured, true);
    assert.match(status.message, /does not exist/);
    assert.equal(fs.existsSync(store), false);
  });

  it('reports governed mode only for the configured governed store', async () => {
    const governed = path.join(projectDir, 'governed.db');
    const other = path.join(projectDir, 'other.db');
    saveOmarchyConfig({ dbPath: governed, apply: true }, homeDir);
    assert.equal((await getStoreStatus({ homeDir })).mode, 'governed-apply');
    assert.equal((await getStoreStatus({ dbPath: other, homeDir })).mode, 'preview');
  });

  it('keeps MCP writes in preview mode by default', () => {
    const server = mcpServerConfig({ dbPath: '/stores/shop.db', profile: 'core', apply: false });
    assert.equal(server.command, 'npx');
    assert.ok(server.args.includes('stateset-mcp'));
    assert.ok(server.args.includes('core'));
    assert.equal(server.args.includes('--apply'), false);
  });

  it('creates a loopback read-only systemd user service by default', () => {
    const config = { dbPath: '/stores/shop db/store.db', profile: 'core', apply: false };
    const unit = userServiceUnit(config, { port: 8090 });
    assert.match(unit, /--host" "127\.0\.0\.1/);
    assert.match(unit, /--read-only/);
    assert.doesNotMatch(unit, /--apply/);
    const installed = installUserService(config, { homeDir, start: false });
    assert.equal(fs.readFileSync(installed.file, 'utf8'), unit);
  });

  it('classifies operational attention without exposing customer data', () => {
    const summary = summarizeOperations({
      orders: [
        { id: 'o1', orderNumber: '1001', status: 'processing', customerEmail: 'private@test' },
        { id: 'o2', orderNumber: '1002', status: 'delivered' },
      ],
      payments: [{ id: 'p1', paymentNumber: 'PAY-1', status: 'failed', amount: '9.99' }],
      pendingReturns: [{ id: 'r1' }],
      lowStockItems: [{ sku: 'SKU-1', name: 'Widget', available: '1', reorderPoint: '2' }],
    });
    assert.deepEqual(summary.alerts, {
      pendingOrders: 1,
      failedPayments: 1,
      pendingReturns: 1,
      lowStock: 1,
      total: 4,
    });
    assert.deepEqual(
      summary.attention.map((item) => item.kind),
      ['failed-payments', 'low-stock', 'pending-returns', 'pending-orders'],
    );
    assert.equal(JSON.stringify(summary).includes('private@test'), false);
    assert.equal(JSON.stringify(summary.samples.failedPayments).includes('9.99'), false);
  });

  it('keeps store signals available when one operational query fails', async () => {
    const summary = await getOperationalSummary({
      orders: { list: async () => [{ id: 'o1', status: 'pending' }] },
      payments: { list: async () => [] },
      returns: { listPending: async () => Promise.reject(new Error('unsupported')) },
      analytics: { lowStockItems: async () => [] },
    });
    assert.equal(summary.alerts.pendingOrders, 1);
    assert.equal(summary.signalsComplete, false);
    assert.deepEqual(summary.unavailableSignals, ['pendingReturns']);
  });

  it('selects a sanitized actionable attention report', () => {
    const status = {
      ok: true,
      dbPath: '/stores/shop.db',
      mode: 'preview',
      alerts: { failedPayments: 1, lowStock: 1, total: 2 },
      attention: [
        { kind: 'failed-payments', label: 'Failed payments', count: 1 },
        { kind: 'low-stock', label: 'Low stock', count: 1 },
      ],
      samples: {
        failedPayments: [{ id: 'p1', paymentNumber: 'PAY-1', status: 'failed' }],
        lowStock: [{ sku: 'SKU-1', available: '1', reorderPoint: '2' }],
      },
    };
    const report = selectAttention(status, 'failed-payments');
    assert.equal(report.attention.length, 1);
    assert.deepEqual(report.samples['failed-payments'], status.samples.failedPayments);
    assert.throws(() => selectAttention(status, 'arbitrary-command'), /Unknown attention kind/);
  });

  it('routes each alert category to a preview-only specialist', () => {
    assert.equal(remediationForKind('failed-payments').command, 'stateset-payments');
    assert.equal(remediationForKind('low-stock').command, 'stateset-inventory');
    assert.equal(remediationForKind('pending-returns').command, 'stateset-returns');
    assert.equal(remediationForKind('pending-orders').command, 'stateset-orders');
    assert.throws(() => remediationForKind('shell-command'), /Unknown remediation kind/);
    for (const kind of ['failed-payments', 'low-stock', 'pending-returns', 'pending-orders']) {
      assert.doesNotMatch(remediationForKind(kind).request, /--apply/);
    }
  });

  it('summarizes large operational snapshots within the shell refresh budget', () => {
    const count = 25_000;
    const started = performance.now();
    const summary = summarizeOperations({
      orders: Array.from({ length: count }, (_, index) => ({ id: `o${index}`, status: 'pending' })),
      payments: Array.from({ length: count }, (_, index) => ({
        id: `p${index}`,
        status: 'failed',
      })),
      pendingReturns: Array.from({ length: count }, (_, index) => ({ id: `r${index}` })),
      lowStockItems: Array.from({ length: count }, (_, index) => ({ sku: `SKU-${index}` })),
    });
    const elapsedMs = performance.now() - started;
    assert.equal(summary.alerts.total, count * 4);
    assert.equal(summary.samples.failedPayments.length, 5);
    assert.ok(elapsedMs < 2_000, `large snapshot took ${elapsedMs.toFixed(1)}ms`);
  });

  it('manages the installed systemd user service lifecycle', () => {
    const calls = [];
    const runner = (_command, args) => {
      calls.push(args);
      if (args.includes('is-active')) return { status: 0, stdout: 'active\n', stderr: '' };
      return { status: 0, stdout: '', stderr: '' };
    };
    installUserService(
      { dbPath: '/stores/shop.db', profile: 'core', apply: false },
      { homeDir, start: false },
    );
    assert.deepEqual(manageUserService('status', { homeDir, runner }), {
      action: 'status',
      file: configPaths(homeDir).serviceFile,
      installed: true,
      active: true,
      state: 'active',
    });
    assert.equal(manageUserService('restart', { homeDir, runner }).active, true);
    assert.equal(manageUserService('stop', { homeDir, runner }).active, false);
    assert.equal(manageUserService('remove', { homeDir, runner }).state, 'removed');
    assert.equal(fs.existsSync(configPaths(homeDir).serviceFile), false);
    assert.ok(calls.some((args) => args.includes('restart')));
    assert.ok(calls.some((args) => args.includes('disable')));
  });

  it('reports a non-installed service without invoking systemd', () => {
    const result = manageUserService('status', {
      homeDir,
      runner: () => assert.fail('systemd should not be called'),
    });
    assert.equal(result.installed, false);
    assert.equal(result.state, 'not-installed');
  });

  it('diagnoses target desktop prerequisites and store failures', async () => {
    const store = path.join(projectDir, 'missing.db');
    saveOmarchyConfig({ dbPath: store, apply: false }, homeDir);
    installPlugin({ homeDir, enable: false });
    const runner = (command, args) => {
      if (command === 'sh' && args[1].startsWith('command -v ')) {
        return {
          status: 0,
          stdout: `/usr/bin/${args[1].slice('command -v '.length)}\n`,
          stderr: '',
        };
      }
      return { status: 3, stdout: 'inactive\n', stderr: '' };
    };
    const report = await diagnoseOmarchy({ homeDir, runner });
    assert.equal(report.ready, false);
    assert.equal(report.checks.find((check) => check.id === 'plugin').ok, true);
    assert.equal(report.checks.find((check) => check.id === 'omarchy').ok, true);
    assert.equal(report.checks.find((check) => check.id === 'store').ok, false);
  });

  it('requires a complete operator identity before apply mode', () => {
    assert.throws(
      () => validateOperatorConfig({ apply: true, kernelPolicy: '/policy.json' }),
      /kernelPrincipal, kernelStoreId/,
    );
    assert.doesNotThrow(() =>
      validateOperatorConfig({
        apply: true,
        kernelPolicy: '/policy.json',
        kernelPrincipal: '/principal.json',
        kernelStoreId: 'store:production',
      }),
    );
  });

  it('merges Claude, OpenCode, and Codex project configuration', () => {
    const config = { dbPath: '/stores/shop.db', profile: 'operations', apply: false };
    const claude = configureAgent('claude', config, projectDir);
    const opencode = configureAgent('opencode', config, projectDir);
    const codex = configureAgent('codex', config, projectDir);

    assert.equal(
      JSON.parse(fs.readFileSync(claude)).mcpServers['stateset-commerce'].command,
      'npx',
    );
    assert.deepEqual(
      JSON.parse(fs.readFileSync(opencode)).mcp['stateset-commerce'].command.slice(0, 2),
      ['npx', '-y'],
    );
    assert.match(fs.readFileSync(codex, 'utf8'), /\[mcp_servers\.stateset-commerce\]/);

    configureAgent('codex', { ...config, dbPath: '/stores/second.db' }, projectDir);
    const updatedCodex = fs.readFileSync(codex, 'utf8');
    assert.match(updatedCodex, /second\.db/);
    assert.equal(updatedCodex.match(/\[mcp_servers\.stateset-commerce\]/g).length, 1);
  });

  it('installs a self-contained plugin and Omarchy menu entries', () => {
    const result = installPlugin({ homeDir, enable: false });
    const locations = configPaths(homeDir);
    const manifest = JSON.parse(fs.readFileSync(path.join(result.target, 'manifest.json')));
    assert.equal(result.target, locations.pluginDir);
    assert.equal(manifest.id, OMARCHY_PLUGIN_ID);
    assert.ok(fs.existsSync(path.join(result.target, 'Service.qml')));
    assert.ok(fs.existsSync(path.join(result.target, 'Panel.qml')));

    const menuFile = installMenu(homeDir);
    const menu = JSON.parse(fs.readFileSync(menuFile));
    assert.equal(menu.stateset.label, 'Commerce');
    assert.match(menu['stateset.dashboard'].action, /stateset-omarchy dashboard/);
    assert.match(menu['stateset.dashboard'].action, /command -v stateset-omarchy/);
    assert.match(menu['stateset.attention'].action, /stateset-omarchy attention/);
    assert.match(menu['stateset.remediate'].action, /stateset-omarchy remediate/);
  });

  it('refuses to overwrite an installed plugin without force', () => {
    installPlugin({ homeDir, enable: false });
    assert.throws(() => installPlugin({ homeDir, enable: false }), /use --force/);
    assert.doesNotThrow(() => installPlugin({ homeDir, enable: false, force: true }));
  });

  it('preserves a plugin checkout installed by Omarchy', () => {
    const target = configPaths(homeDir).pluginDir;
    fs.mkdirSync(path.join(target, '.git'), { recursive: true });
    fs.writeFileSync(
      path.join(target, 'manifest.json'),
      `${JSON.stringify({ id: OMARCHY_PLUGIN_ID, version: '1.28.0' })}\n`,
    );
    const sentinel = path.join(target, 'git-managed.txt');
    fs.writeFileSync(sentinel, 'preserve checkout');
    const calls = [];

    const result = installPlugin({
      homeDir,
      force: true,
      runner: (command, args) => {
        calls.push([command, ...args]);
        return { status: 0, stdout: '', stderr: '' };
      },
    });

    assert.equal(result.managedExternally, true);
    assert.equal(result.replaced, false);
    assert.equal(result.enabled, true);
    assert.equal(fs.readFileSync(sentinel, 'utf8'), 'preserve checkout');
    assert.deepEqual(calls, [['omarchy', 'plugin', 'enable', OMARCHY_PLUGIN_ID]]);
  });

  it('rolls back a forced plugin update when activation fails', () => {
    const installed = installPlugin({ homeDir, enable: false });
    const sentinel = path.join(installed.target, 'operator-local.txt');
    fs.writeFileSync(sentinel, 'keep previous plugin');
    assert.throws(
      () =>
        installPlugin({
          homeDir,
          force: true,
          runner: () => ({ status: 1, stdout: '', stderr: 'activation failed' }),
        }),
      /activation failed/,
    );
    assert.equal(fs.readFileSync(sentinel, 'utf8'), 'keep previous plugin');
    assert.equal(
      fs.readdirSync(path.dirname(installed.target)).some((name) => name.includes('.rollback-')),
      false,
    );
  });

  it('uninstalls only StateSet plugin and menu entries', () => {
    const installed = installPlugin({ homeDir, enable: false });
    const menuFile = configPaths(homeDir).menuFile;
    fs.mkdirSync(path.dirname(menuFile), { recursive: true });
    fs.writeFileSync(menuFile, `${JSON.stringify({ existing: { label: 'Keep me' } })}\n`);
    installMenu(homeDir);

    const plugin = uninstallPlugin({ homeDir, disable: false });
    const menu = uninstallMenu(homeDir);

    assert.equal(plugin.removed, true);
    assert.equal(fs.existsSync(installed.target), false);
    assert.equal(menu.removed, true);
    assert.deepEqual(JSON.parse(fs.readFileSync(menuFile, 'utf8')), {
      existing: { label: 'Keep me' },
    });
    assert.deepEqual(uninstallPlugin({ homeDir, disable: false }).removed, false);
    assert.deepEqual(uninstallMenu(homeDir).removed, false);
  });
});
