import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { CLI_VERSION } from './config.js';
import { createCommerce } from './commerce.js';

export const OMARCHY_PLUGIN_ID = 'com.stateset.icommerce';
export const OMARCHY_CONFIG_DIRNAME = 'stateset-omarchy';
const OMARCHY_CONTRACT = JSON.parse(
  fs.readFileSync(fileURLToPath(new URL('../omarchy/contract.json', import.meta.url)), 'utf8'),
);
export const OMARCHY_CAPABILITIES = Object.freeze([...OMARCHY_CONTRACT.capabilities]);
export const OMARCHY_SERVICE_UNIT = 'stateset-icommerce-mcp.service';

const STORE_FILENAMES = ['store.db', 'stateset.db', 'commerce.db'];

function terminalControllerAction(command) {
  if (
    !/^(agent|attention|backup|configure --agent all|dashboard|remediate|service install)$/.test(
      command,
    )
  ) {
    throw new Error(`Unsafe Omarchy menu command: ${command}`);
  }
  return `omarchy-launch-floating-terminal-with-presentation 'stateset-omarchy ${command}'`;
}

const MENU_ENTRIES = {
  stateset: { icon: '󰆼', label: 'Commerce', aliases: ['icommerce', 'stateset'] },
  'stateset.dashboard': {
    icon: '󰨇',
    label: 'Store Dashboard',
    action: terminalControllerAction('dashboard'),
  },
  'stateset.agent': {
    icon: '󰚩',
    label: 'Commerce Agent',
    action: terminalControllerAction('agent'),
  },
  'stateset.attention': {
    icon: '󰀪',
    label: 'Review Store Attention',
    action: terminalControllerAction('attention'),
  },
  'stateset.remediate': {
    icon: '󰁨',
    label: 'Resolve Store Attention',
    action: terminalControllerAction('remediate'),
  },
  'stateset.backup': {
    icon: '󰁯',
    label: 'Back Up Store',
    action: terminalControllerAction('backup'),
  },
  'stateset.configure': {
    icon: '',
    label: 'Configure Agents',
    action: terminalControllerAction('configure --agent all'),
  },
  'stateset.service': {
    icon: '󰒋',
    label: 'Start Local MCP Service',
    action: terminalControllerAction('service install'),
  },
};

export function configPaths(homeDir = os.homedir()) {
  const xdgConfig = process.env.XDG_CONFIG_HOME || path.join(homeDir, '.config');
  return {
    configDir: path.join(xdgConfig, OMARCHY_CONFIG_DIRNAME),
    configFile: path.join(xdgConfig, OMARCHY_CONFIG_DIRNAME, 'config.json'),
    pluginDir: path.join(xdgConfig, 'omarchy', 'plugins', OMARCHY_PLUGIN_ID),
    menuFile: path.join(xdgConfig, 'omarchy', 'extensions', 'omarchy-menu.jsonc'),
    serviceFile: path.join(xdgConfig, 'systemd', 'user', 'stateset-icommerce-mcp.service'),
  };
}

function readJson(file, fallback = null) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch {
    return fallback;
  }
}

export function discoverStore({ dbPath, cwd = process.cwd(), homeDir = os.homedir() } = {}) {
  if (dbPath) return path.resolve(cwd, dbPath);
  if (process.env.STATESET_DB_PATH) return path.resolve(cwd, process.env.STATESET_DB_PATH);
  if (process.env.DB_PATH) return path.resolve(cwd, process.env.DB_PATH);

  const configured = readJson(configPaths(homeDir).configFile);
  if (configured?.dbPath) return path.resolve(configured.dbPath);

  let directory = path.resolve(cwd);
  while (true) {
    for (const filename of STORE_FILENAMES) {
      const candidate = path.join(directory, filename);
      if (fs.existsSync(candidate)) return candidate;
    }
    const parent = path.dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  return null;
}

export function loadOmarchyConfig(homeDir = os.homedir()) {
  return readJson(configPaths(homeDir).configFile, {}) || {};
}

export function saveOmarchyConfig(config, homeDir = os.homedir()) {
  const locations = configPaths(homeDir);
  fs.mkdirSync(locations.configDir, { recursive: true, mode: 0o700 });
  fs.writeFileSync(locations.configFile, `${JSON.stringify(config, null, 2)}\n`, { mode: 0o600 });
  return locations.configFile;
}

const PENDING_ORDER_STATUSES = new Set(['pending', 'confirmed', 'processing', 'on_hold']);
const FAILED_PAYMENT_STATUSES = new Set(['failed', 'declined']);

export function summarizeOperations({
  orders = [],
  payments = [],
  pendingReturns = [],
  lowStockItems = [],
} = {}) {
  const pendingOrders = orders.filter((order) =>
    PENDING_ORDER_STATUSES.has(String(order?.status || '').toLowerCase()),
  );
  const failedPayments = payments.filter((payment) =>
    FAILED_PAYMENT_STATUSES.has(String(payment?.status || '').toLowerCase()),
  );
  const alerts = {
    pendingOrders: pendingOrders.length,
    failedPayments: failedPayments.length,
    pendingReturns: pendingReturns.length,
    lowStock: lowStockItems.length,
  };
  alerts.total = Object.values(alerts).reduce((total, count) => total + count, 0);
  return {
    alerts,
    attention: [
      {
        kind: 'failed-payments',
        label: 'Failed payments',
        count: alerts.failedPayments,
        severity: 'critical',
      },
      { kind: 'low-stock', label: 'Low stock', count: alerts.lowStock, severity: 'warning' },
      {
        kind: 'pending-returns',
        label: 'Pending returns',
        count: alerts.pendingReturns,
        severity: 'warning',
      },
      {
        kind: 'pending-orders',
        label: 'Pending orders',
        count: alerts.pendingOrders,
        severity: 'info',
      },
    ].filter((item) => item.count > 0),
    samples: {
      failedPayments: failedPayments.slice(0, 5).map((payment) => ({
        id: payment.id,
        paymentNumber: payment.paymentNumber,
        status: payment.status,
      })),
      lowStock: lowStockItems.slice(0, 5).map((item) => ({
        sku: item.sku,
        name: item.name,
        available: item.available,
        reorderPoint: item.reorderPoint,
      })),
      pendingOrders: pendingOrders.slice(0, 5).map((order) => ({
        id: order.id,
        orderNumber: order.orderNumber,
        status: order.status,
      })),
      pendingReturns: pendingReturns.slice(0, 5).map((returnValue) => ({
        id: returnValue.id,
        returnNumber: returnValue.returnNumber,
        status: returnValue.status,
      })),
    },
  };
}

export async function getOperationalSummary(commerce) {
  const names = ['orders', 'payments', 'pendingReturns', 'lowStockItems'];
  const results = await Promise.allSettled([
    commerce.orders.list(),
    commerce.payments.list(),
    commerce.returns.listPending(),
    commerce.analytics.lowStockItems(),
  ]);
  const values = Object.fromEntries(
    results.map((result, index) => [
      names[index],
      result.status === 'fulfilled' && Array.isArray(result.value) ? result.value : [],
    ]),
  );
  const unavailableSignals = results
    .map((result, index) => (result.status === 'rejected' ? names[index] : null))
    .filter(Boolean);
  return {
    ...summarizeOperations(values),
    signalsComplete: unavailableSignals.length === 0,
    unavailableSignals,
  };
}

export async function getStoreStatus(options = {}) {
  const dbPath = discoverStore(options);
  const config = loadOmarchyConfig(options.homeDir);
  const governedStore =
    config.apply === true &&
    Boolean(config.dbPath) &&
    Boolean(dbPath) &&
    path.resolve(config.dbPath) === path.resolve(dbPath);
  const base = {
    schemaVersion: OMARCHY_CONTRACT.schemaVersion,
    controllerVersion: CLI_VERSION,
    capabilities: [...OMARCHY_CAPABILITIES],
    ok: false,
    configured: Boolean(dbPath),
    dbPath,
    mode: governedStore ? 'governed-apply' : 'preview',
    counts: { orders: 0, customers: 0, products: 0, returns: 0, payments: 0 },
    checkedAt: new Date().toISOString(),
  };

  if (!dbPath) return { ...base, message: 'No iCommerce store found' };
  if (!fs.existsSync(dbPath)) {
    return { ...base, configured: true, message: 'Configured store does not exist' };
  }

  try {
    const commerce = createCommerce(dbPath);
    const [orders, customers, products, returns, payments, operations] = await Promise.all([
      commerce.orders.count(),
      commerce.customers.count(),
      commerce.products.count(),
      commerce.returns.count(),
      commerce.payments.count(),
      getOperationalSummary(commerce),
    ]);
    return {
      ...base,
      ok: true,
      counts: { orders, customers, products, returns, payments },
      ...operations,
      sizeBytes: fs.statSync(dbPath).size,
      message:
        operations.alerts.total > 0
          ? `Store ready · ${operations.alerts.total} need attention`
          : !operations.signalsComplete
            ? 'Store ready · some operational signals unavailable'
            : 'Store ready',
    };
  } catch (error) {
    return { ...base, message: error instanceof Error ? error.message : String(error) };
  }
}

const ATTENTION_KINDS = new Set([
  'all',
  'failed-payments',
  'low-stock',
  'pending-returns',
  'pending-orders',
]);

export function selectAttention(status, kind = 'all') {
  const selected = String(kind || 'all').toLowerCase();
  if (!ATTENTION_KINDS.has(selected)) {
    throw new Error(`Unknown attention kind: ${kind}`);
  }
  const attention = (status.attention || []).filter(
    (item) => selected === 'all' || item.kind === selected,
  );
  const sampleKeys = {
    'failed-payments': 'failedPayments',
    'low-stock': 'lowStock',
    'pending-returns': 'pendingReturns',
    'pending-orders': 'pendingOrders',
  };
  const samples = Object.fromEntries(
    attention.map((item) => [item.kind, status.samples?.[sampleKeys[item.kind]] || []]),
  );
  return {
    ok: status.ok === true,
    dbPath: status.dbPath || null,
    mode: status.mode || 'preview',
    alerts: status.alerts || {},
    attention,
    samples,
    signalsComplete: status.signalsComplete !== false,
    unavailableSignals: status.unavailableSignals || [],
  };
}

export function remediationForKind(kind) {
  const remediations = {
    'failed-payments': {
      command: 'stateset-payments',
      request: 'Review failed payments and recommend safe next actions.',
    },
    'low-stock': {
      command: 'stateset-inventory',
      request: 'Review low-stock items and recommend replenishment actions.',
    },
    'pending-returns': {
      command: 'stateset-returns',
      request: 'Review pending returns and recommend safe next actions.',
    },
    'pending-orders': {
      command: 'stateset-orders',
      request: 'Review pending orders and recommend safe fulfillment actions.',
    },
  };
  const remediation = remediations[String(kind || '').toLowerCase()];
  if (!remediation) throw new Error(`Unknown remediation kind: ${kind}`);
  return remediation;
}

export function mcpServerConfig(config) {
  const args = [
    '-y',
    '-p',
    '@stateset/cli',
    'stateset-mcp',
    '--db',
    config.dbPath,
    '--profile',
    config.profile || 'core',
  ];
  if (config.apply === true) {
    args.push(
      '--apply',
      '--kernel-policy',
      config.kernelPolicy,
      '--kernel-principal',
      config.kernelPrincipal,
      '--kernel-store-id',
      config.kernelStoreId,
    );
  }
  return { command: 'npx', args };
}

function systemdQuote(value) {
  const text = String(value);
  if (/[\r\n\0]/.test(text)) throw new Error('Systemd arguments may not contain control lines');
  return `"${text.replace(/%/g, '%%').replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

export function userServiceUnit(config, { port = 8090 } = {}) {
  validateOperatorConfig(config);
  const parsedPort = Number.parseInt(String(port), 10);
  if (!Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65535) {
    throw new Error(`Invalid MCP service port: ${port}`);
  }
  const args = [
    'npx',
    '-y',
    '-p',
    '@stateset/cli',
    'stateset-mcp-http',
    '--host',
    '127.0.0.1',
    '--port',
    String(parsedPort),
    '--db',
    config.dbPath,
    '--profile',
    config.profile || 'core',
  ];
  if (config.apply === true) {
    args.push(
      '--apply',
      '--kernel-policy',
      config.kernelPolicy,
      '--kernel-principal',
      config.kernelPrincipal,
      '--kernel-store-id',
      config.kernelStoreId,
    );
  } else {
    args.push('--read-only');
  }
  return `[Unit]\nDescription=StateSet iCommerce MCP for Omarchy\nAfter=network-online.target\n\n[Service]\nType=simple\nExecStart=/usr/bin/env ${args.map(systemdQuote).join(' ')}\nRestart=on-failure\nRestartSec=5\nNoNewPrivileges=true\nPrivateTmp=true\nUMask=0077\n\n[Install]\nWantedBy=default.target\n`;
}

export function installUserService(
  config,
  { homeDir = os.homedir(), port = 8090, start = true, runner = spawnSync } = {},
) {
  const file = configPaths(homeDir).serviceFile;
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, userServiceUnit(config, { port }), { mode: 0o644 });
  if (!start) return { file, started: false };
  const reload = runner('systemctl', ['--user', 'daemon-reload'], { encoding: 'utf8' });
  if (reload.status !== 0) return { file, started: false, error: reload.stderr?.trim() };
  const enable = runner('systemctl', ['--user', 'enable', '--now', OMARCHY_SERVICE_UNIT], {
    encoding: 'utf8',
  });
  return {
    file,
    started: enable.status === 0,
    error: enable.status === 0 ? null : enable.stderr?.trim(),
  };
}

export function manageUserService(action, { homeDir = os.homedir(), runner = spawnSync } = {}) {
  const file = configPaths(homeDir).serviceFile;
  const installed = fs.existsSync(file);
  if (action === 'status') {
    if (!installed)
      return { action, file, installed: false, active: false, state: 'not-installed' };
    const result = runner('systemctl', ['--user', 'is-active', OMARCHY_SERVICE_UNIT], {
      encoding: 'utf8',
    });
    const state = String(result.stdout || result.stderr || 'unknown').trim() || 'unknown';
    return { action, file, installed: true, active: result.status === 0, state };
  }

  if (!['start', 'stop', 'restart', 'remove'].includes(action)) {
    throw new Error(`Unknown service action: ${action}`);
  }
  if (!installed) throw new Error(`Service is not installed: ${file}`);

  if (action === 'remove') {
    const disable = runner('systemctl', ['--user', 'disable', '--now', OMARCHY_SERVICE_UNIT], {
      encoding: 'utf8',
    });
    if (disable.status !== 0 && disable.status !== 1) {
      throw new Error(disable.stderr?.trim() || 'Unable to disable StateSet MCP service');
    }
    fs.unlinkSync(file);
    const reload = runner('systemctl', ['--user', 'daemon-reload'], { encoding: 'utf8' });
    if (reload.status !== 0) {
      throw new Error(reload.stderr?.trim() || 'Unable to reload systemd user units');
    }
    return { action, file, installed: false, active: false, state: 'removed' };
  }

  const result = runner('systemctl', ['--user', action, OMARCHY_SERVICE_UNIT], {
    encoding: 'utf8',
  });
  if (result.status !== 0) {
    throw new Error(result.stderr?.trim() || `Unable to ${action} StateSet MCP service`);
  }
  return {
    action,
    file,
    installed: true,
    active: action !== 'stop',
    state: action === 'stop' ? 'inactive' : 'active',
  };
}

export async function diagnoseOmarchy({ homeDir = os.homedir(), dbPath, runner = spawnSync } = {}) {
  const locations = configPaths(homeDir);
  const status = await getStoreStatus({ homeDir, dbPath });
  const commandAvailable = (command) => {
    const result = runner('sh', ['-c', `command -v ${command}`], { encoding: 'utf8' });
    return result.status === 0;
  };
  const manifest = readJson(path.join(locations.pluginDir, 'manifest.json'));
  const service = manageUserService('status', { homeDir, runner });
  const checks = [
    {
      id: 'store',
      label: 'Commerce store is readable',
      required: true,
      ok: status.ok === true,
      detail: status.message,
    },
    {
      id: 'config',
      label: 'Omarchy integration is configured',
      required: true,
      ok: fs.existsSync(locations.configFile),
      detail: locations.configFile,
    },
    {
      id: 'plugin',
      label: 'Shell plugin is installed',
      required: true,
      ok: manifest?.id === OMARCHY_PLUGIN_ID,
      detail: locations.pluginDir,
    },
    {
      id: 'omarchy',
      label: 'Omarchy CLI is available',
      required: true,
      ok: commandAvailable('omarchy'),
      detail: 'omarchy',
    },
    {
      id: 'notifications',
      label: 'Desktop notifications are available',
      required: false,
      ok: commandAvailable('notify-send'),
      detail: 'notify-send',
    },
    {
      id: 'service',
      label: 'Background MCP service is active',
      required: false,
      ok: service.active,
      detail: service.state,
    },
  ];
  return {
    ready: checks.filter((check) => check.required).every((check) => check.ok),
    checks,
    status,
    service,
    checkedAt: new Date().toISOString(),
  };
}

export function validateOperatorConfig(config, { checkFiles = false } = {}) {
  if (config.apply !== true) return;
  const missing = ['kernelPolicy', 'kernelPrincipal', 'kernelStoreId'].filter(
    (key) => !config[key],
  );
  if (missing.length > 0) {
    throw new Error(`Apply mode requires operator-owned ${missing.join(', ')}`);
  }
  if (checkFiles) {
    for (const key of ['kernelPolicy', 'kernelPrincipal']) {
      if (!fs.existsSync(config[key]))
        throw new Error(`${key} file does not exist: ${config[key]}`);
    }
  }
}

function writeJsonMerged(file, mutate) {
  let value = {};
  if (fs.existsSync(file)) {
    value = readJson(file);
    if (!value || Array.isArray(value) || typeof value !== 'object') {
      throw new Error(`Refusing to modify non-JSON configuration: ${file}`);
    }
  }
  mutate(value);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
  return file;
}

export function configureAgent(agent, config, cwd = process.cwd()) {
  const server = mcpServerConfig(config);
  if (agent === 'claude' || agent === 'generic') {
    return writeJsonMerged(path.join(cwd, '.mcp.json'), (value) => {
      value.mcpServers ||= {};
      value.mcpServers['stateset-commerce'] = server;
    });
  }
  if (agent === 'opencode') {
    return writeJsonMerged(path.join(cwd, 'opencode.json'), (value) => {
      value.mcp ||= {};
      value.mcp['stateset-commerce'] = {
        type: 'local',
        command: [server.command, ...server.args],
        enabled: true,
      };
    });
  }
  if (agent === 'codex') {
    const file = path.join(cwd, '.codex', 'config.toml');
    const marker = '[mcp_servers.stateset-commerce]';
    const existing = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : '';
    fs.mkdirSync(path.dirname(file), { recursive: true });
    const block = [
      marker,
      `command = ${JSON.stringify(server.command)}`,
      `args = ${JSON.stringify(server.args)}`,
    ];
    const lines = existing.split('\n');
    const start = lines.findIndex((line) => line.trim() === marker);
    if (start >= 0) {
      let end = start + 1;
      while (end < lines.length && !/^\s*\[[^\]]+\]\s*$/.test(lines[end])) end += 1;
      lines.splice(start, end - start, ...block);
      fs.writeFileSync(file, `${lines.join('\n').replace(/\n+$/, '')}\n`);
    } else {
      const prefix = existing && !existing.endsWith('\n') ? `${existing}\n` : existing;
      fs.writeFileSync(file, `${prefix}${block.join('\n')}\n`);
    }
    return file;
  }
  throw new Error(`Unknown agent target: ${agent}`);
}

export function installMenu(homeDir = os.homedir()) {
  const file = configPaths(homeDir).menuFile;
  return writeJsonMerged(file, (value) => Object.assign(value, MENU_ENTRIES));
}

export function uninstallMenu(homeDir = os.homedir()) {
  const file = configPaths(homeDir).menuFile;
  if (!fs.existsSync(file)) return { file, removed: false, fileRemoved: false };
  const value = readJson(file);
  if (!value || Array.isArray(value) || typeof value !== 'object') {
    throw new Error(`Refusing to modify non-JSON configuration: ${file}`);
  }
  let removed = false;
  for (const key of Object.keys(MENU_ENTRIES)) {
    if (Object.hasOwn(value, key)) {
      delete value[key];
      removed = true;
    }
  }
  if (!removed) return { file, removed: false, fileRemoved: false };
  if (Object.keys(value).length === 0) {
    fs.unlinkSync(file);
    return { file, removed: true, fileRemoved: true };
  }
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
  return { file, removed: true, fileRemoved: false };
}

export function bundledPluginDir() {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../omarchy');
}

export function installPlugin({
  homeDir = os.homedir(),
  force = false,
  enable = true,
  runner = spawnSync,
} = {}) {
  const target = configPaths(homeDir).pluginDir;
  if (fs.existsSync(target)) {
    if (fs.existsSync(path.join(target, '.git'))) {
      const manifest = readJson(path.join(target, 'manifest.json'));
      if (manifest?.id !== OMARCHY_PLUGIN_ID) {
        throw new Error(`Git-managed plugin at ${target} has an unexpected manifest`);
      }
      let enabled = false;
      if (enable) {
        const result = runner('omarchy', ['plugin', 'enable', OMARCHY_PLUGIN_ID], {
          encoding: 'utf8',
        });
        enabled = result.status === 0;
        if (!enabled) {
          throw new Error(result.stderr?.trim() || 'Unable to enable StateSet Omarchy plugin');
        }
      }
      return { target, enabled, replaced: false, managedExternally: true };
    }
    if (!force) throw new Error(`Plugin already exists: ${target} (use --force to update)`);
  }
  fs.mkdirSync(path.dirname(target), { recursive: true });
  const staged = `${target}.install-${process.pid}-${Date.now()}`;
  const rollback = `${target}.rollback-${process.pid}-${Date.now()}`;
  const replacing = fs.existsSync(target);
  try {
    fs.cpSync(bundledPluginDir(), staged, { recursive: true, errorOnExist: true });
    const manifest = readJson(path.join(staged, 'manifest.json'));
    if (manifest?.id !== OMARCHY_PLUGIN_ID) throw new Error('Bundled Omarchy manifest is invalid');
    if (replacing) fs.renameSync(target, rollback);
    fs.renameSync(staged, target);

    let enabled = false;
    if (enable) {
      const result = runner('omarchy', ['plugin', 'enable', OMARCHY_PLUGIN_ID], {
        encoding: 'utf8',
      });
      enabled = result.status === 0;
      if (replacing && !enabled) {
        throw new Error(
          result.stderr?.trim() || 'Unable to enable updated StateSet Omarchy plugin',
        );
      }
    }
    if (replacing) fs.rmSync(rollback, { recursive: true, force: true });
    return { target, enabled, replaced: replacing, managedExternally: false };
  } catch (error) {
    fs.rmSync(staged, { recursive: true, force: true });
    if (fs.existsSync(rollback)) {
      fs.rmSync(target, { recursive: true, force: true });
      fs.renameSync(rollback, target);
    }
    throw error;
  }
}

export function uninstallPlugin({
  homeDir = os.homedir(),
  disable = true,
  runner = spawnSync,
} = {}) {
  const target = configPaths(homeDir).pluginDir;
  if (!fs.existsSync(target)) return { target, removed: false, disabled: false };
  let disabled = false;
  if (disable) {
    const result = runner('omarchy', ['plugin', 'disable', OMARCHY_PLUGIN_ID], {
      encoding: 'utf8',
    });
    disabled = result.status === 0;
  }
  fs.rmSync(target, { recursive: true });
  return { target, removed: true, disabled };
}

export async function backupStore({ dbPath, destination, cwd, homeDir } = {}) {
  const resolved = discoverStore({ dbPath, cwd, homeDir });
  if (!resolved || !fs.existsSync(resolved)) throw new Error('No iCommerce store found');
  const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
  const backupPath = path.resolve(
    destination || path.join(path.dirname(resolved), 'backups', `store-${timestamp}.db`),
  );
  fs.mkdirSync(path.dirname(backupPath), { recursive: true });
  const commerce = createCommerce(resolved);
  const report = await commerce.maintenance.backup(backupPath);
  return { backupPath, report };
}
