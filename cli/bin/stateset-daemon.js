#!/usr/bin/env node

/**
 * StateSet iCommerce Daemon Manager
 *
 * Manages the channel gateway as a systemd service with Tailscale
 * and SSH remote access support.
 *
 * Usage:
 *   stateset-daemon install       Install systemd service
 *   stateset-daemon start         Start the daemon
 *   stateset-daemon stop          Stop the daemon
 *   stateset-daemon restart       Restart the daemon
 *   stateset-daemon enable        Enable auto-start on boot
 *   stateset-daemon disable       Disable auto-start on boot
 *   stateset-daemon status        Show daemon status
 *   stateset-daemon logs [n]      Show last N log lines
 *   stateset-daemon config        Show current config
 *   stateset-daemon validate      Validate gateway config
 *   stateset-daemon update        Update to latest version
 *   stateset-daemon tailscale     Tailscale remote access management
 *   stateset-daemon ssh-tunnel    SSH tunnel management
 *   stateset-daemon health        Check gateway health
 *   stateset-daemon uninstall     Remove systemd service
 */

import { parseArgs } from 'node:util';
import { execSync, spawn } from 'node:child_process';
import { readFileSync, writeFileSync, existsSync, mkdirSync, copyFileSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { homedir } from 'node:os';
import chalk from 'chalk';
import { installShutdownHandlers } from '../src/graceful-shutdown.js';
installShutdownHandlers('stateset-daemon');

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const CLI_ROOT = resolve(__dirname, '..');

// ============================================================================
// Configuration
// ============================================================================

const SERVICE_NAME = 'stateset-gateway';
const TAILSCALE_SERVICE = 'stateset-tailscale';
const SSH_TUNNEL_TEMPLATE = 'stateset-ssh-tunnel@';
const LOG_IDENT = 'stateset-gateway';

const HELP = `
${chalk.bold('StateSet iCommerce Daemon Manager')}

${chalk.dim('USAGE:')}
  stateset-daemon <command> [options]

${chalk.dim('COMMANDS:')}
  ${chalk.yellow('install')}              Install systemd service and create directories
  ${chalk.yellow('start')}                Start the gateway daemon
  ${chalk.yellow('stop')}                 Stop the daemon
  ${chalk.yellow('restart')}              Restart the daemon
  ${chalk.yellow('enable')}               Enable auto-start on boot
  ${chalk.yellow('disable')}              Disable auto-start on boot
  ${chalk.yellow('status')}               Show daemon status
  ${chalk.yellow('logs')} [n]             Show last N log lines (default: 50)
  ${chalk.yellow('config')}               Show current configuration
  ${chalk.yellow('validate')}             Validate gateway configuration
  ${chalk.yellow('update')}               Update to latest version
  ${chalk.yellow('health')}               Check gateway health
  ${chalk.yellow('uninstall')}            Remove systemd service and config

${chalk.dim('TAILSCALE:')}
  ${chalk.yellow('tailscale setup')}      Install Tailscale proxy service
  ${chalk.yellow('tailscale serve')}      Enable Tailscale Serve (tailnet HTTPS)
  ${chalk.yellow('tailscale funnel')}     Enable Tailscale Funnel (public HTTPS)
  ${chalk.yellow('tailscale status')}     Show Tailscale connection details
  ${chalk.yellow('tailscale dns')} <name> Set MagicDNS hostname

${chalk.dim('SSH TUNNELS:')}
  ${chalk.yellow('ssh-tunnel')} <host>    Create forward SSH tunnel
  ${chalk.yellow('ssh-tunnel')} <host> ${chalk.dim('--reverse')}     Reverse tunnel (expose local)
  ${chalk.yellow('ssh-tunnel')} <host> ${chalk.dim('--persistent')}  Install as systemd service
  ${chalk.yellow('ssh-tunnel list')}      List active tunnels
  ${chalk.yellow('ssh-tunnel stop')} <n>  Stop a tunnel by name or PID
  ${chalk.yellow('ssh-tunnel keygen')}    Generate SSH key for gateway
  ${chalk.yellow('ssh-tunnel status')}    Summary of all tunnels

${chalk.dim('OPTIONS:')}
  --config <path>      Config file path
  --port <n>           HTTP gateway port (default: 8080)
  --user               Install/manage as user service (no sudo)
  --follow, -f         Follow logs in real-time
  --json               Output supported commands as JSON
  --output <file>      Write JSON output to file (implies --json)
  --reverse            Create reverse SSH tunnel
  --persistent         Create persistent SSH tunnel via systemd
  --name <n>           Name for persistent tunnel
  --help, -h           Show this help

${chalk.dim('JSON OUTPUT:')}
  Supported commands: status, health, config, validate, logs (non-follow), tailscale status, ssh-tunnel list/status

${chalk.dim('EXAMPLES:')}
  ${chalk.dim('# Install and start')}
  sudo stateset-daemon install
  sudo stateset-daemon start
  stateset-daemon status

  ${chalk.dim('# User-level service (no sudo)')}
  stateset-daemon install --user
  stateset-daemon start --user

  ${chalk.dim('# View logs (follow mode)')}
  stateset-daemon logs -f
  stateset-daemon logs 100

  ${chalk.dim('# Tailscale remote access')}
  sudo stateset-daemon tailscale setup
  stateset-daemon tailscale serve
  stateset-daemon tailscale funnel

  ${chalk.dim('# SSH tunnels')}
  stateset-daemon ssh-tunnel user@remote-host
  stateset-daemon ssh-tunnel user@vps --reverse --persistent --name vps
  stateset-daemon ssh-tunnel list
  stateset-daemon ssh-tunnel keygen
`;

// ============================================================================
// Path Resolution (system vs user services)
// ============================================================================

function getUserPaths(isUserMode) {
  if (!isUserMode) {
    return {
      isUser: false,
      serviceDir: '/etc/systemd/system',
      serviceFile: `/etc/systemd/system/${SERVICE_NAME}.service`,
      tailscaleServiceFile: `/etc/systemd/system/${TAILSCALE_SERVICE}.service`,
      sshTunnelTemplateFile: `/etc/systemd/system/${SSH_TUNNEL_TEMPLATE}.service`,
      configDir: '/etc/stateset',
      dataDir: '/opt/stateset/data',
      appDir: '/opt/stateset',
      logDir: '/var/log/stateset',
      tunnelEnvDir: '/etc/stateset/tunnels',
      systemctl: 'systemctl',
      systemctlFlag: '',
      installTarget: 'multi-user.target',
      serviceUser: 'stateset',
    };
  }

  const home = homedir();
  const serviceDir = join(home, '.config/systemd/user');
  const configDir = join(home, '.config/stateset');
  const dataDir = join(home, '.local/share/stateset');

  return {
    isUser: true,
    serviceDir,
    serviceFile: join(serviceDir, `${SERVICE_NAME}.service`),
    tailscaleServiceFile: join(serviceDir, `${TAILSCALE_SERVICE}.service`),
    sshTunnelTemplateFile: join(serviceDir, `${SSH_TUNNEL_TEMPLATE}.service`),
    configDir,
    dataDir,
    appDir: join(dataDir, 'app'),
    logDir: join(dataDir, 'logs'),
    tunnelEnvDir: join(configDir, 'tunnels'),
    systemctl: 'systemctl --user',
    systemctlFlag: '--user',
    installTarget: 'default.target',
    serviceUser: null,
  };
}

// ============================================================================
// Helpers
// ============================================================================

function run(cmd, opts = {}) {
  try {
    const result = execSync(cmd, {
      encoding: 'utf-8',
      stdio: opts.silent ? 'pipe' : 'inherit',
      ...opts,
    });
    if (result === null || result === undefined) return '';
    return typeof result === 'string' ? result.trim() : String(result).trim();
  } catch (err) {
    if (opts.ignoreError) return '';
    throw err;
  }
}

function runQuiet(cmd) {
  return run(cmd, { silent: true, ignoreError: true });
}

function isRoot() {
  return process.getuid?.() === 0;
}

function requireRoot(action) {
  if (!isRoot()) {
    console.error(`Error: '${action}' requires root privileges. Use sudo.`);
    process.exit(1);
  }
}

function serviceExists(paths) {
  return existsSync(paths.serviceFile);
}

// ============================================================================
// Config Validation
// ============================================================================

function validateConfigDetailed(configPath) {
  const result = {
    path: configPath,
    valid: true,
    errors: [],
    warnings: [],
    channels: { enabled: [], count: 0 },
    httpGateway: null,
    env: { path: join(dirname(configPath), 'env'), exists: false, anthropicKeySet: null },
  };

  if (!existsSync(configPath)) {
    result.valid = false;
    result.errors.push(`Config file not found: ${configPath}`);
    return result;
  }

  try {
    const raw = readFileSync(configPath, 'utf-8');
    const config = JSON.parse(raw);

    if (!config.channels || typeof config.channels !== 'object') {
      result.errors.push('Config error: missing "channels" section');
    }
    if (!config.shared || typeof config.shared !== 'object') {
      result.errors.push('Config error: missing "shared" section');
    }

    const envPath = result.env.path;
    if (existsSync(envPath)) {
      result.env.exists = true;
      const env = readFileSync(envPath, 'utf-8');
      const hasKey = Boolean(env.match(/^ANTHROPIC_API_KEY=\S+/m));
      result.env.anthropicKeySet = hasKey;
      if (!hasKey) {
        result.warnings.push('Warning: ANTHROPIC_API_KEY is not set in env file');
      }
    }

    if (config.channels && typeof config.channels === 'object') {
      const enabledChannels = Object.entries(config.channels)
        .filter(([, c]) => c && c.enabled !== false)
        .map(([key]) => key);
      result.channels.enabled = enabledChannels;
      result.channels.count = enabledChannels.length;
      if (enabledChannels.length === 0) {
        result.warnings.push('Warning: No channels are enabled in config');
      }
    }

    if (config.httpGateway) {
      result.httpGateway = { port: config.httpGateway.port || 8080 };
    }

    result.valid = result.errors.length === 0;
    return result;
  } catch (err) {
    result.valid = false;
    result.errors.push(`Config parse error: ${err.message}`);
    return result;
  }
}

function validateConfig(configPath) {
  const result = validateConfigDetailed(configPath);

  if (result.errors.length > 0) {
    for (const err of result.errors) {
      const prefix = err.startsWith('Config file not found') ? '' : '  ';
      console.error(chalk.red(`${prefix}${err}`));
    }
    return false;
  }

  for (const warning of result.warnings) {
    console.warn(chalk.yellow(`  ${warning}`));
  }

  if (result.channels.count > 0) {
    console.log(
      chalk.green(
        `  ${result.channels.count} channel(s) enabled: ${result.channels.enabled.join(', ')}`,
      ),
    );
  }

  if (result.httpGateway) {
    console.log(chalk.green(`  HTTP gateway: port ${result.httpGateway.port}`));
  }

  console.log(chalk.green('  Config is valid.'));
  return true;
}

// ============================================================================
// JSON Output Helpers
// ============================================================================

function collectStatus(paths, port = 8080) {
  const active = runQuiet(`${paths.systemctl} is-active ${SERVICE_NAME}`);
  const enabled = runQuiet(`${paths.systemctl} is-enabled ${SERVICE_NAME}`);

  const data = {
    service: {
      name: SERVICE_NAME,
      active: active === 'active',
      activeState: active || 'unknown',
      enabled: enabled === 'enabled',
      enabledState: enabled || 'unknown',
      mode: paths.isUser ? 'user' : 'system',
    },
    port,
  };

  if (active === 'active') {
    const pid = runQuiet(`${paths.systemctl} show -p MainPID --value ${SERVICE_NAME}`);
    if (pid && pid !== '0') {
      data.service.pid = pid;
      const uptime = runQuiet(`ps -p ${pid} -o etime= 2>/dev/null`);
      if (uptime) data.service.uptime = uptime.trim();
    }
  }

  const healthBody = runQuiet(`curl -s http://127.0.0.1:${port}/health`);
  if (healthBody) {
    try {
      const health = JSON.parse(healthBody);
      data.health = {
        available: true,
        status: health.status,
        uptimeMs: health.uptime,
        uptimeSec: typeof health.uptime === 'number' ? Math.floor(health.uptime / 1000) : undefined,
        timestamp: health.timestamp,
      };
    } catch {
      data.health = { available: false };
    }
  } else {
    data.health = { available: false };
  }

  const tsStatus = runQuiet('tailscale status --json 2>/dev/null');
  if (tsStatus) {
    try {
      const ts = JSON.parse(tsStatus);
      const hostname = ts.Self?.HostName || null;
      const tailnet = ts.MagicDNSSuffix || null;
      const url = hostname && tailnet ? `https://${hostname}.${tailnet}` : null;

      const serveStatus = runQuiet('tailscale serve status --json 2>/dev/null');
      let serveActive = false;
      if (serveStatus) {
        try {
          const serve = JSON.parse(serveStatus);
          serveActive = Object.keys(serve).length > 0;
        } catch {
          /* ignore */
        }
      }

      const funnelStatus = runQuiet('tailscale funnel status --json 2>/dev/null');
      let funnelActive = false;
      if (funnelStatus) {
        try {
          const funnel = JSON.parse(funnelStatus);
          funnelActive = Object.keys(funnel).length > 0;
        } catch {
          /* ignore */
        }
      }

      data.tailscale = {
        connected: true,
        hostname,
        tailnet,
        url,
        serveActive,
        funnelActive,
      };
    } catch {
      data.tailscale = { connected: false };
    }
  } else {
    data.tailscale = { connected: false };
  }

  const tunnelServices = runQuiet(
    `${paths.systemctl} list-units --type=service --all "${SSH_TUNNEL_TEMPLATE}*" --no-legend --plain 2>/dev/null`,
  );
  const serviceList = tunnelServices ? tunnelServices.split('\n').filter(Boolean) : [];
  const persistentRunning = serviceList.filter((line) => line.includes('running')).length;
  const tunnelProcs = runQuiet('pgrep -c -f "ssh.*-[LR].*127.0.0.1" 2>/dev/null');
  const adhocCount = parseInt(tunnelProcs || '0', 10);

  data.sshTunnels = {
    persistentTotal: serviceList.length,
    persistentRunning,
    adhocCount,
  };

  if (active === 'active') {
    const mem = runQuiet(`${paths.systemctl} show -p MemoryCurrent --value ${SERVICE_NAME}`);
    const bytes = parseInt(mem, 10);
    if (!Number.isNaN(bytes)) {
      data.memory = {
        bytes,
        mb: Math.round(bytes / 1024 / 1024),
      };
    }
  }

  return data;
}

function collectHealth(port = 8080) {
  const url = `http://127.0.0.1:${port}/health`;
  const result = {
    ok: false,
    port,
    url,
    httpStatus: null,
  };

  const status = runQuiet(`curl -s -o /dev/null -w "%{http_code}" ${url}`);
  if (status) {
    const code = parseInt(status, 10);
    result.httpStatus = Number.isNaN(code) ? null : code;
  }

  if (status === '200') {
    const body = runQuiet(`curl -s ${url}`);
    try {
      const data = JSON.parse(body);
      result.ok = true;
      result.health = data;
      if (typeof data.uptime === 'number') {
        result.uptimeSec = Math.floor(data.uptime / 1000);
      }
    } catch (err) {
      result.error = `Health response parse error: ${err.message}`;
    }

    const metricsBody = runQuiet(`curl -s http://127.0.0.1:${port}/metrics`);
    if (metricsBody) {
      try {
        result.metrics = JSON.parse(metricsBody);
      } catch {
        /* ignore */
      }
    }
  } else if (status) {
    result.error = `Health check failed (HTTP ${status})`;
  } else {
    result.error = `Health check failed: Gateway not reachable at ${url}`;
  }

  return result;
}

function collectLogs(lines, paths) {
  const flag = paths.systemctlFlag ? ' ' + paths.systemctlFlag : '';
  const output = runQuiet(
    `journalctl -u ${SERVICE_NAME}${flag} --no-pager -n ${lines} --output cat`,
  );
  const entries = output ? output.split('\n').filter(Boolean) : [];
  return { lines: entries, count: entries.length };
}

function collectConfig(paths) {
  const configPath = join(paths.configDir, 'gateway.json');
  if (!existsSync(configPath)) {
    return { error: `No config file found at ${configPath}`, path: configPath };
  }
  try {
    const config = JSON.parse(readFileSync(configPath, 'utf-8'));
    return { path: configPath, config };
  } catch (err) {
    return { error: `Config parse error: ${err.message}`, path: configPath };
  }
}

function collectTailscaleStatus() {
  const tsStatus = runQuiet('tailscale status --json 2>/dev/null');
  if (!tsStatus) {
    return { connected: false };
  }

  try {
    const ts = JSON.parse(tsStatus);
    const hostname = ts.Self?.HostName || null;
    const tailnet = ts.MagicDNSSuffix || null;
    const url = hostname && tailnet ? `https://${hostname}.${tailnet}` : null;

    let serveActive = false;
    const serveStatus = runQuiet('tailscale serve status --json 2>/dev/null');
    if (serveStatus) {
      try {
        const serve = JSON.parse(serveStatus);
        serveActive = Object.keys(serve).length > 0;
      } catch {
        /* ignore */
      }
    }

    let funnelActive = false;
    const funnelStatus = runQuiet('tailscale funnel status --json 2>/dev/null');
    if (funnelStatus) {
      try {
        const funnel = JSON.parse(funnelStatus);
        funnelActive = Object.keys(funnel).length > 0;
      } catch {
        /* ignore */
      }
    }

    const proxyService = runQuiet(`systemctl is-active ${TAILSCALE_SERVICE}`);

    return {
      connected: true,
      hostname,
      tailnet,
      url,
      serveActive,
      funnelActive,
      proxyService: proxyService || null,
      status: ts,
    };
  } catch {
    return { connected: false, error: 'Unable to parse Tailscale status', raw: tsStatus };
  }
}

function collectSshTunnelList(paths) {
  const services = runQuiet(
    `${paths.systemctl} list-units --type=service --all "${SSH_TUNNEL_TEMPLATE}*" --no-legend 2>/dev/null`,
  );
  const persistent = [];
  if (services && services.trim()) {
    for (const line of services.split('\n').filter(Boolean)) {
      const parts = line.trim().split(/\s+/);
      const name = parts[0];
      const state = parts[3] || 'unknown';
      persistent.push({ name, state, active: state === 'running' });
    }
  }

  const procs = runQuiet('pgrep -a -f "ssh.*-[LR].*127.0.0.1" 2>/dev/null');
  const adhoc = [];
  if (procs && procs.trim()) {
    for (const line of procs.split('\n').filter(Boolean)) {
      const [pid, ...cmdParts] = line.split(' ');
      adhoc.push({ pid: parseInt(pid, 10), command: cmdParts.join(' ') });
    }
  }

  return { persistent, adhoc };
}

function collectSshTunnelStatus(paths) {
  const services = runQuiet(
    `${paths.systemctl} list-units --type=service --all "${SSH_TUNNEL_TEMPLATE}*" --no-legend --plain 2>/dev/null`,
  );
  const serviceList = services ? services.split('\n').filter(Boolean) : [];
  const running = serviceList.filter((line) => line.includes('running')).length;

  const procs = runQuiet('pgrep -c -f "ssh.*-[LR].*127.0.0.1" 2>/dev/null');
  const adhocCount = parseInt(procs || '0', 10);

  const keyPath = join(homedir(), '.ssh', 'stateset_gateway');
  const autossh = runQuiet('which autossh 2>/dev/null');

  return {
    persistent: { configured: serviceList.length, running },
    adhoc: { count: adhocCount },
    sshKey: { path: keyPath, present: existsSync(keyPath) },
    autossh: { available: Boolean(autossh), path: autossh || null },
  };
}

// ============================================================================
// Commands: Core Daemon Management
// ============================================================================

function cmdInstall(opts) {
  const { paths } = opts;

  if (!paths.isUser) {
    requireRoot('install');
  }

  console.log(chalk.bold('Installing StateSet Gateway daemon...\n'));

  if (!paths.isUser) {
    // 1. Create system user
    const userExists = runQuiet('id -u stateset');
    if (!userExists) {
      console.log('Creating stateset user...');
      run('useradd --system --create-home --shell /usr/sbin/nologin stateset');
    } else {
      console.log(chalk.dim('User stateset already exists.'));
    }
  }

  // 2. Create directories
  console.log('Creating directories...');
  const dirs = [paths.configDir, paths.dataDir, paths.appDir, paths.logDir, paths.tunnelEnvDir];
  for (const dir of dirs) {
    mkdirSync(dir, { recursive: true });
  }

  // Ensure systemd user dir exists for user mode
  if (paths.isUser) {
    mkdirSync(paths.serviceDir, { recursive: true });
  }

  // 3. Copy CLI files
  console.log(`Copying CLI files to ${paths.appDir}...`);
  run(`cp -r ${CLI_ROOT}/bin ${paths.appDir}/`);
  run(`cp -r ${CLI_ROOT}/src ${paths.appDir}/`);
  run(`cp -r ${CLI_ROOT}/skills ${paths.appDir}/ 2>/dev/null || true`);
  run(`cp ${CLI_ROOT}/package.json ${paths.appDir}/`);

  if (existsSync(join(CLI_ROOT, 'node_modules'))) {
    console.log('Copying node_modules...');
    run(`cp -r ${CLI_ROOT}/node_modules ${paths.appDir}/`);
  } else {
    console.log('Installing dependencies...');
    run(`cd ${paths.appDir} && npm ci --omit=dev`);
  }

  // 4. Copy config
  const configSrc = opts.config || join(CLI_ROOT, 'deploy', 'gateway.config.example.json');
  const configDest = join(paths.configDir, 'gateway.json');
  if (!existsSync(configDest)) {
    console.log('Creating config file...');
    copyFileSync(configSrc, configDest);
  } else {
    console.log(chalk.dim('Config file already exists, skipping.'));
  }

  // 5. Create env file
  const envFile = join(paths.configDir, 'env');
  if (!existsSync(envFile)) {
    writeFileSync(
      envFile,
      [
        '# StateSet Gateway Environment',
        '# Add your API keys here',
        'ANTHROPIC_API_KEY=',
        '# OPENAI_API_KEY=',
        '# GEMINI_API_KEY=',
        '# TELEGRAM_BOT_TOKEN=',
        '# DISCORD_BOT_TOKEN=',
        '# SLACK_BOT_TOKEN=',
        '# SLACK_APP_TOKEN=',
      ].join('\n') + '\n',
    );
    if (!paths.isUser) {
      run(`chmod 600 ${envFile}`);
    }
    console.log(`Created env file at ${envFile}`);
  }

  // 6. Set ownership (system mode only)
  if (!paths.isUser) {
    run(`chown -R stateset:stateset ${paths.appDir} ${paths.logDir}`);
    run(`chown -R stateset:stateset ${paths.configDir}`);
  }

  // 7. Install systemd service
  console.log('Installing systemd service...');
  const serviceSource = join(CLI_ROOT, 'deploy', 'stateset-gateway.service');
  if (!paths.isUser && existsSync(serviceSource)) {
    copyFileSync(serviceSource, paths.serviceFile);
  } else {
    writeFileSync(paths.serviceFile, generateServiceFile(paths));
  }

  // 8. Copy SSH tunnel template
  const sshTemplateSource = join(CLI_ROOT, 'deploy', `${SSH_TUNNEL_TEMPLATE}.service`);
  if (existsSync(sshTemplateSource)) {
    copyFileSync(sshTemplateSource, paths.sshTunnelTemplateFile);
  } else {
    writeFileSync(paths.sshTunnelTemplateFile, generateSshTunnelTemplate(paths));
  }

  run(`${paths.systemctl} daemon-reload`);
  run(`${paths.systemctl} enable ${SERVICE_NAME}`);

  console.log(`
${chalk.green('Installation complete.')}

Next steps:
  1. Edit ${paths.configDir}/env with your API keys:
     ${paths.isUser ? '' : 'sudo '}nano ${paths.configDir}/env

  2. Edit ${paths.configDir}/gateway.json to enable channels:
     ${paths.isUser ? '' : 'sudo '}nano ${paths.configDir}/gateway.json

  3. Validate config:
     stateset-daemon validate${paths.isUser ? ' --user' : ''}

  4. Start the daemon:
     ${paths.isUser ? '' : 'sudo '}stateset-daemon start${paths.isUser ? ' --user' : ''}

  5. Check status:
     stateset-daemon status${paths.isUser ? ' --user' : ''}
`);
}

function cmdStart(paths) {
  if (!serviceExists(paths)) {
    console.error(
      'Service not installed. Run: ' +
        (paths.isUser ? '' : 'sudo ') +
        'stateset-daemon install' +
        (paths.isUser ? ' --user' : ''),
    );
    process.exit(1);
  }

  // Validate config before starting
  const configPath = join(paths.configDir, 'gateway.json');
  if (existsSync(configPath)) {
    console.log('Validating config...');
    if (!validateConfig(configPath)) {
      console.error(chalk.red('Fix config issues before starting.'));
      process.exit(1);
    }
  }

  console.log('Starting StateSet Gateway...');
  if (paths.isUser) {
    run(`${paths.systemctl} start ${SERVICE_NAME}`);
  } else {
    run(`sudo systemctl start ${SERVICE_NAME}`);
  }
  console.log(chalk.green('Started.') + ' Use "stateset-daemon status" to check.');
}

function cmdStop(paths) {
  if (!serviceExists(paths)) {
    console.log(chalk.yellow('Service not installed. Nothing to stop.'));
    return;
  }
  console.log('Stopping StateSet Gateway...');
  if (paths.isUser) {
    run(`${paths.systemctl} stop ${SERVICE_NAME}`);
  } else {
    run(`sudo systemctl stop ${SERVICE_NAME}`);
  }
  console.log(chalk.green('Stopped.'));
}

function cmdRestart(paths) {
  console.log('Restarting StateSet Gateway...');
  if (paths.isUser) {
    run(`${paths.systemctl} restart ${SERVICE_NAME}`);
  } else {
    run(`sudo systemctl restart ${SERVICE_NAME}`);
  }
  console.log(chalk.green('Restarted.'));
}

function cmdEnable(paths) {
  run(`${paths.systemctl} enable ${SERVICE_NAME}`);
  console.log(chalk.green('Service enabled (starts on boot).'));
}

function cmdDisable(paths) {
  run(`${paths.systemctl} disable ${SERVICE_NAME}`);
  console.log(chalk.yellow('Service disabled (will not start on boot).'));
}

function cmdStatus(paths, port = 8080) {
  console.log(chalk.bold('\nStateSet Gateway Status\n'));

  const active = runQuiet(`${paths.systemctl} is-active ${SERVICE_NAME}`);
  const enabled = runQuiet(`${paths.systemctl} is-enabled ${SERVICE_NAME}`);

  const statusIcon =
    active === 'active'
      ? chalk.green('●') + ' ' + chalk.green('active (running)')
      : chalk.red('○') + ' ' + chalk.red(active || 'not installed');

  console.log(`  ${chalk.dim('Service:')}    ${statusIcon}`);
  console.log(
    `  ${chalk.dim('Enabled:')}    ${enabled === 'enabled' ? chalk.green('yes') : chalk.yellow(enabled || 'unknown')}`,
  );
  console.log(`  ${chalk.dim('Mode:')}       ${paths.isUser ? 'user' : 'system'}`);

  // PID and uptime
  if (active === 'active') {
    const pid = runQuiet(`${paths.systemctl} show -p MainPID --value ${SERVICE_NAME}`);
    if (pid && pid !== '0') {
      console.log(`  ${chalk.dim('PID:')}        ${pid}`);
      const uptime = runQuiet(`ps -p ${pid} -o etime= 2>/dev/null`);
      if (uptime) console.log(`  ${chalk.dim('Uptime:')}     ${uptime.trim()}`);
    }
  }

  // Health check
  try {
    const health = runQuiet(`curl -s http://127.0.0.1:${port}/health`);
    if (health) {
      const data = JSON.parse(health);
      console.log(`  ${chalk.dim('Health:')}     ${chalk.green(data.status)}`);
      console.log(`  ${chalk.dim('Gateway up:')} ${Math.floor(data.uptime / 1000)}s`);
    }
  } catch {
    if (active === 'active') {
      console.log(`  ${chalk.dim('Health:')}     ${chalk.yellow('unavailable')}`);
    }
  }

  // Tailscale status
  const tsStatus = runQuiet('tailscale status --json 2>/dev/null');
  if (tsStatus) {
    try {
      const ts = JSON.parse(tsStatus);
      const hostname = ts.Self?.HostName || 'unknown';
      const tailnetName = ts.MagicDNSSuffix || '';

      console.log(`\n  ${chalk.bold('Tailscale')}`);
      console.log(`  ${chalk.dim('Status:')}     ${chalk.green('connected')}`);
      console.log(`  ${chalk.dim('Hostname:')}   ${chalk.cyan(`${hostname}.${tailnetName}`)}`);
      console.log(
        `  ${chalk.dim('URL:')}        ${chalk.cyan(`https://${hostname}.${tailnetName}`)}`,
      );

      // Check serve/funnel
      const serveStatus = runQuiet('tailscale serve status --json 2>/dev/null');
      if (serveStatus) {
        try {
          const serve = JSON.parse(serveStatus);
          if (Object.keys(serve).length > 0) {
            console.log(`  ${chalk.dim('Serve:')}      ${chalk.green('active')}`);
          }
        } catch {
          /* ignore */
        }
      }

      const funnelStatus = runQuiet('tailscale funnel status --json 2>/dev/null');
      if (funnelStatus) {
        try {
          const funnel = JSON.parse(funnelStatus);
          if (Object.keys(funnel).length > 0) {
            console.log(
              `  ${chalk.dim('Funnel:')}     ${chalk.green('active')} ${chalk.dim('(public internet)')}`,
            );
          }
        } catch {
          /* ignore */
        }
      }
    } catch {
      console.log(`\n  ${chalk.dim('Tailscale:')}  ${chalk.yellow('not connected')}`);
    }
  }

  // SSH tunnel status
  const tunnelServices = runQuiet(
    `${paths.systemctl} list-units --type=service --all "${SSH_TUNNEL_TEMPLATE}*" --no-legend --plain 2>/dev/null`,
  );
  const tunnelProcs = runQuiet('pgrep -c -f "ssh.*-[LR].*127.0.0.1" 2>/dev/null');
  const persistentCount = tunnelServices
    ? tunnelServices.split('\n').filter((l) => l.includes('running')).length
    : 0;
  const adhocCount = parseInt(tunnelProcs || '0', 10);

  if (persistentCount > 0 || adhocCount > 0) {
    console.log(`\n  ${chalk.bold('SSH Tunnels')}`);
    if (persistentCount > 0)
      console.log(`  ${chalk.dim('Persistent:')} ${chalk.green(persistentCount + ' running')}`);
    if (adhocCount > 0)
      console.log(`  ${chalk.dim('Ad-hoc:')}     ${chalk.green(adhocCount + ' process(es)')}`);
  }

  // Memory
  if (active === 'active') {
    const mem = runQuiet(`${paths.systemctl} show -p MemoryCurrent --value ${SERVICE_NAME}`);
    if (mem && mem !== '[not set]') {
      const mb = Math.round(parseInt(mem) / 1024 / 1024);
      console.log(`\n  ${chalk.dim('Memory:')}     ${mb}MB`);
    }
  }

  console.log();
}

function cmdLogs(lines, follow, paths) {
  const flagParts = paths.systemctlFlag ? [paths.systemctlFlag] : [];

  if (follow) {
    const child = spawn('journalctl', ['-u', SERVICE_NAME, ...flagParts, '-f', '--output', 'cat'], {
      stdio: 'inherit',
    });

    process.on('SIGINT', () => {
      child.kill('SIGTERM');
    });
    child.on('exit', (code) => process.exit(code || 0));
  } else {
    const flag = flagParts.length ? ' ' + flagParts.join(' ') : '';
    run(`journalctl -u ${SERVICE_NAME}${flag} --no-pager -n ${lines} --output cat`);
  }
}

function cmdConfig(paths) {
  const configPath = join(paths.configDir, 'gateway.json');
  if (existsSync(configPath)) {
    const config = JSON.parse(readFileSync(configPath, 'utf-8'));
    console.log(chalk.bold('Gateway Configuration:\n'));
    console.log(JSON.stringify(config, null, 2));
  } else {
    console.error(`No config file found at ${configPath}`);
    console.error('Run: ' + (paths.isUser ? '' : 'sudo ') + 'stateset-daemon install');
  }
}

function cmdValidate(paths, configOverride) {
  const configPath = configOverride || join(paths.configDir, 'gateway.json');
  console.log(chalk.bold(`Validating ${configPath}...\n`));
  const ok = validateConfig(configPath);
  process.exit(ok ? 0 : 1);
}

function cmdUpdate(paths) {
  if (!paths.isUser) {
    requireRoot('update');
  }

  console.log(chalk.bold('Updating StateSet Gateway...\n'));

  // Stop if running
  const active = runQuiet(`${paths.systemctl} is-active ${SERVICE_NAME}`);
  if (active === 'active') {
    console.log('Stopping service...');
    run(`${paths.systemctl} stop ${SERVICE_NAME}`);
  }

  // Pull latest
  console.log('Installing latest @stateset/cli...');
  run('npm install -g @stateset/cli@latest');

  // Get new CLI path
  const globalRoot = runQuiet('npm root -g');
  const newCliRoot = join(globalRoot, '@stateset/cli');

  if (existsSync(newCliRoot)) {
    // Copy updated files
    console.log(`Copying updated files to ${paths.appDir}...`);
    for (const dir of ['bin', 'src', 'skills']) {
      run(`cp -r ${newCliRoot}/${dir} ${paths.appDir}/ 2>/dev/null || true`);
    }
    run(`cp ${newCliRoot}/package.json ${paths.appDir}/`);

    // Update deploy files
    const deployDir = join(newCliRoot, 'deploy');
    if (existsSync(deployDir) && !paths.isUser) {
      run(`cp ${deployDir}/*.service ${paths.serviceDir}/ 2>/dev/null || true`);
      run(`${paths.systemctl} daemon-reload`);
    }

    // Install dependencies
    console.log('Updating dependencies...');
    run(`cd ${paths.appDir} && npm ci --omit=dev`);

    // Set ownership
    if (!paths.isUser) {
      run(`chown -R stateset:stateset ${paths.appDir}`);
    }
  }

  // Restart if was running
  if (active === 'active') {
    console.log('Restarting service...');
    run(`${paths.systemctl} start ${SERVICE_NAME}`);
  }

  const newVersion = runQuiet(
    `node -e "console.log(JSON.parse(require('fs').readFileSync('${paths.appDir}/package.json','utf-8')).version)"`,
  );
  console.log(chalk.green(`\nUpdate complete. Version: ${newVersion || 'unknown'}`));
}

function cmdHealth(port = 8080) {
  try {
    const result = runQuiet(
      `curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:${port}/health`,
    );
    if (result === '200') {
      const body = runQuiet(`curl -s http://127.0.0.1:${port}/health`);
      const data = JSON.parse(body);
      console.log(`${chalk.green('●')} Health: ${chalk.green('OK')}`);
      console.log(`  Uptime: ${Math.floor(data.uptime / 1000)}s`);
      console.log(`  Timestamp: ${data.timestamp}`);

      // Also check metrics
      const metrics = runQuiet(`curl -s http://127.0.0.1:${port}/metrics`);
      if (metrics) {
        const m = JSON.parse(metrics);
        console.log(`  Messages: ${m.messagesReceived || 0} received, ${m.messagesSent || 0} sent`);
      }
    } else {
      console.error(`${chalk.red('○')} Health check failed (HTTP ${result})`);
      process.exit(1);
    }
  } catch {
    console.error(
      `${chalk.red('○')} Health check failed: Gateway not reachable at http://127.0.0.1:${port}`,
    );
    process.exit(1);
  }
}

function cmdUninstall(paths) {
  if (!paths.isUser) {
    requireRoot('uninstall');
  }

  console.log(chalk.bold('Uninstalling StateSet Gateway...'));

  // Stop services
  run(`${paths.systemctl} stop ${SERVICE_NAME} 2>/dev/null || true`, {
    silent: true,
    ignoreError: true,
  });
  run(`${paths.systemctl} stop ${TAILSCALE_SERVICE} 2>/dev/null || true`, {
    silent: true,
    ignoreError: true,
  });
  run(`${paths.systemctl} disable ${SERVICE_NAME} 2>/dev/null || true`, {
    silent: true,
    ignoreError: true,
  });
  run(`${paths.systemctl} disable ${TAILSCALE_SERVICE} 2>/dev/null || true`, {
    silent: true,
    ignoreError: true,
  });

  // Stop SSH tunnel services
  const tunnelServices = runQuiet(
    `${paths.systemctl} list-units --type=service --all "${SSH_TUNNEL_TEMPLATE}*" --no-legend --plain 2>/dev/null`,
  );
  if (tunnelServices) {
    for (const line of tunnelServices.split('\n').filter(Boolean)) {
      const svc = line.trim().split(/\s+/)[0];
      run(`${paths.systemctl} stop ${svc} 2>/dev/null || true`, {
        silent: true,
        ignoreError: true,
      });
      run(`${paths.systemctl} disable ${svc} 2>/dev/null || true`, {
        silent: true,
        ignoreError: true,
      });
    }
  }

  // Remove service files
  for (const f of [paths.serviceFile, paths.tailscaleServiceFile, paths.sshTunnelTemplateFile]) {
    if (existsSync(f)) {
      if (paths.isUser) {
        run(`rm ${f}`);
      } else {
        run(`rm ${f}`);
      }
      console.log(`  Removed: ${f}`);
    }
  }

  run(`${paths.systemctl} daemon-reload`);

  console.log(`
${chalk.green('Service removed.')} Data and config preserved at:
  ${paths.configDir}/   (config and env)
  ${paths.appDir}/   (application files)

To remove everything:
  ${paths.isUser ? '' : 'sudo '}rm -rf ${paths.configDir} ${paths.appDir} ${paths.logDir}
`);
}

// ============================================================================
// Commands: Tailscale
// ============================================================================

function cmdTailscale(action, arg, port = 8080) {
  switch (action) {
    case 'setup':
      return tailscaleSetup(port);
    case 'serve':
      return tailscaleServe(arg, port);
    case 'funnel':
      return tailscaleFunnel(arg, port);
    case 'status':
      return tailscaleStatus();
    case 'dns':
      return tailscaleDns(arg);
    default:
      return tailscaleStatus();
  }
}

function tailscaleSetup(port) {
  requireRoot('tailscale setup');

  // Check Tailscale is installed
  const tsInstalled = runQuiet('which tailscale');
  if (!tsInstalled) {
    console.error(chalk.red('Tailscale is not installed.'));
    console.error('Install it: curl -fsSL https://tailscale.com/install.sh | sh');
    process.exit(1);
  }

  // Check authentication status
  const tsStatus = runQuiet('tailscale status --json 2>/dev/null');
  let authenticated = false;
  if (tsStatus) {
    try {
      const ts = JSON.parse(tsStatus);
      authenticated = ts.BackendState === 'Running';
    } catch {
      /* ignore */
    }
  }

  if (!authenticated) {
    console.log('Tailscale is not authenticated. Running: tailscale up');
    run('tailscale up');
  } else {
    console.log(chalk.green('Tailscale is already authenticated.'));
  }

  // Install the Tailscale proxy service
  const tsServiceSource = join(CLI_ROOT, 'deploy', 'stateset-tailscale.service');
  const tsServiceFile = `/etc/systemd/system/${TAILSCALE_SERVICE}.service`;
  if (existsSync(tsServiceSource)) {
    copyFileSync(tsServiceSource, tsServiceFile);
  } else {
    writeFileSync(tsServiceFile, generateTailscaleService());
  }

  run('systemctl daemon-reload');
  run(`systemctl enable ${TAILSCALE_SERVICE}`);

  // Get actual hostname info
  const statusJson = runQuiet('tailscale status --json 2>/dev/null');
  let hostname = '<hostname>';
  let tailnet = '<tailnet>.ts.net';
  if (statusJson) {
    try {
      const s = JSON.parse(statusJson);
      hostname = s.Self?.HostName || hostname;
      tailnet = s.MagicDNSSuffix || tailnet;
    } catch {
      /* ignore */
    }
  }

  console.log(`
${chalk.bold('Tailscale proxy service installed.')}

  ${chalk.dim('Tailnet URL:')}  ${chalk.cyan(`https://${hostname}.${tailnet}`)}

Next steps:
  1. Start the proxy:
     ${chalk.yellow(`sudo systemctl start ${TAILSCALE_SERVICE}`)}

  2. Or use Tailscale Serve directly:
     ${chalk.yellow(`stateset-daemon --port ${port} tailscale serve`)}

  3. For public internet access (Funnel):
     ${chalk.yellow(`stateset-daemon --port ${port} tailscale funnel`)}
`);
}

function tailscaleServe(arg, port) {
  if (arg === 'off' || arg === 'disable') {
    run('tailscale serve off');
    console.log(chalk.green('Tailscale Serve disabled.'));
    return;
  }

  if (arg === 'status') {
    const status = runQuiet('tailscale serve status 2>&1');
    console.log(chalk.bold('Tailscale Serve Status:\n'));
    console.log(status || 'No active serve configuration.');
    return;
  }

  // Enable serve (default action)
  console.log(`Enabling Tailscale Serve on port ${port}...`);
  run(`tailscale serve --bg --https=443 http://127.0.0.1:${port}`);

  const url = getTailscaleUrl();
  console.log(chalk.green('\nTailscale Serve enabled.'));
  console.log(`  ${chalk.dim('Access URL:')} ${chalk.cyan(url)}`);
  console.log(chalk.dim('  (Accessible only within your Tailscale network)'));
}

function tailscaleFunnel(arg, port) {
  if (arg === 'off' || arg === 'disable') {
    run('tailscale funnel off');
    console.log(chalk.green('Tailscale Funnel disabled.'));
    return;
  }

  if (arg === 'status') {
    const status = runQuiet('tailscale funnel status 2>&1');
    console.log(chalk.bold('Tailscale Funnel Status:\n'));
    console.log(status || 'No active funnel configuration.');
    return;
  }

  // Enable funnel
  console.log(`Enabling Tailscale Funnel on port ${port}...`);
  console.log(chalk.yellow('Warning: This exposes your gateway to the public internet.'));
  run(`tailscale funnel --bg --https=443 http://127.0.0.1:${port}`);

  const url = getTailscaleUrl();
  console.log(chalk.green('\nTailscale Funnel enabled.'));
  console.log(`  ${chalk.dim('Public URL:')} ${chalk.cyan(url)}`);
  console.log(chalk.dim('  (Accessible from the public internet)'));
}

function tailscaleStatus() {
  const tsStatus = runQuiet('tailscale status --json 2>/dev/null');
  if (!tsStatus) {
    console.log(chalk.yellow('Tailscale is not running or not installed.'));
    console.log('\nTo install: curl -fsSL https://tailscale.com/install.sh | sh');
    return;
  }

  try {
    const ts = JSON.parse(tsStatus);
    const self = ts.Self || {};
    const tailnet = ts.MagicDNSSuffix || '';

    console.log(chalk.bold('\nTailscale Status\n'));
    console.log(
      `  ${chalk.dim('State:')}       ${ts.BackendState === 'Running' ? chalk.green(ts.BackendState) : chalk.yellow(ts.BackendState || 'unknown')}`,
    );
    console.log(`  ${chalk.dim('Hostname:')}    ${self.HostName || 'unknown'}`);
    console.log(`  ${chalk.dim('Tailnet:')}     ${tailnet}`);
    console.log(`  ${chalk.dim('IP:')}          ${(self.TailscaleIPs || []).join(', ')}`);
    console.log(`  ${chalk.dim('DNS Name:')}    ${chalk.cyan(`${self.HostName}.${tailnet}`)}`);
    console.log(
      `  ${chalk.dim('HTTPS URL:')}   ${chalk.cyan(`https://${self.HostName}.${tailnet}`)}`,
    );

    // Check serve status
    const serveStatus = runQuiet('tailscale serve status --json 2>/dev/null');
    if (serveStatus) {
      try {
        const serve = JSON.parse(serveStatus);
        const hasServe = Object.keys(serve).length > 0;
        console.log(
          `  ${chalk.dim('Serve:')}       ${hasServe ? chalk.green('active') : chalk.dim('inactive')}`,
        );
      } catch {
        /* ignore */
      }
    }

    // Check funnel status
    const funnelStatus = runQuiet('tailscale funnel status --json 2>/dev/null');
    if (funnelStatus) {
      try {
        const funnel = JSON.parse(funnelStatus);
        const hasFunnel = Object.keys(funnel).length > 0;
        console.log(
          `  ${chalk.dim('Funnel:')}      ${hasFunnel ? chalk.green('active') + ' ' + chalk.dim('(public internet)') : chalk.dim('inactive')}`,
        );
      } catch {
        /* ignore */
      }
    }

    // List peers
    const peers = ts.Peer ? Object.values(ts.Peer) : [];
    if (peers.length > 0) {
      console.log(`\n  ${chalk.bold('Connected Peers')} (${peers.length})`);
      for (const peer of peers.slice(0, 10)) {
        const online = peer.Online ? chalk.green('online') : chalk.dim('offline');
        console.log(`    ${peer.HostName}: ${online} (${(peer.TailscaleIPs || [])[0] || '?'})`);
      }
      if (peers.length > 10) {
        console.log(chalk.dim(`    ... and ${peers.length - 10} more`));
      }
    }

    // Proxy service status
    const tsActive = runQuiet(`systemctl is-active ${TAILSCALE_SERVICE}`);
    console.log(
      `\n  ${chalk.dim('Proxy Service:')} ${tsActive === 'active' ? chalk.green(tsActive) : chalk.yellow(tsActive || 'not installed')}`,
    );
  } catch {
    console.log('Tailscale status (raw):');
    run('tailscale status', { ignoreError: true });
  }

  console.log();
}

function tailscaleDns(hostname) {
  if (!hostname) {
    // Show current DNS name
    const tsStatus = runQuiet('tailscale status --json 2>/dev/null');
    if (tsStatus) {
      try {
        const ts = JSON.parse(tsStatus);
        const self = ts.Self || {};
        console.log(
          `Current hostname: ${chalk.cyan(`${self.HostName}.${ts.MagicDNSSuffix || ''}`)}`,
        );
      } catch {
        /* ignore */
      }
    }
    console.log(`\nTo change hostname:`);
    console.log(`  stateset-daemon tailscale dns <new-hostname>`);
    return;
  }

  console.log(`Setting Tailscale hostname to: ${hostname}`);
  run(`tailscale set --hostname=${hostname}`);
  console.log(chalk.green(`Hostname set to ${hostname}`));
}

function getTailscaleUrl() {
  const statusJson = runQuiet('tailscale status --json 2>/dev/null');
  if (statusJson) {
    try {
      const s = JSON.parse(statusJson);
      const hostname = s.Self?.HostName;
      const tailnet = s.MagicDNSSuffix;
      if (hostname && tailnet) return `https://${hostname}.${tailnet}`;
    } catch {
      /* ignore */
    }
  }
  return 'https://<hostname>.<tailnet>.ts.net';
}

// ============================================================================
// Commands: SSH Tunnels
// ============================================================================

function cmdSshTunnel(host, port, opts, paths) {
  // Subcommand dispatch
  if (host === 'list') return sshTunnelList(paths);
  if (host === 'stop') return sshTunnelStop(opts.name || positionals[2], paths);
  if (host === 'keygen') return sshKeyGen();
  if (host === 'status') return sshTunnelStatus(paths);

  if (!host) {
    console.error('Usage: stateset-daemon ssh-tunnel <user@host> [options]');
    console.error('       stateset-daemon ssh-tunnel list');
    console.error('       stateset-daemon ssh-tunnel stop <name>');
    console.error('       stateset-daemon ssh-tunnel keygen');
    console.error('       stateset-daemon ssh-tunnel status');
    process.exit(1);
  }

  if (opts.persistent) return sshTunnelPersistent(host, port, opts.reverse, opts.name, paths);
  if (opts.reverse) return sshTunnelReverse(host, port);

  // Default: forward tunnel
  return sshTunnelForward(host, port);
}

function sshTunnelForward(host, port = 8080) {
  console.log(chalk.bold(`Creating forward SSH tunnel to ${host}...`));
  console.log(`  ${chalk.dim('Local:')}  http://localhost:${port}`);
  console.log(`  ${chalk.dim('Remote:')} ${host}:${port}`);
  console.log(`  ${chalk.dim('Mode:')}   Forward (access remote from local)\n`);

  const child = spawn(
    'ssh',
    [
      '-N',
      '-L',
      `${port}:127.0.0.1:${port}`,
      host,
      '-o',
      'ServerAliveInterval=60',
      '-o',
      'ServerAliveCountMax=3',
      '-o',
      'ExitOnForwardFailure=yes',
    ],
    { stdio: 'inherit' },
  );

  child.on('error', (err) => {
    console.error(`SSH error: ${err.message}`);
    process.exit(1);
  });
  child.on('exit', (code) => {
    console.log(`SSH tunnel closed (exit code: ${code})`);
    process.exit(code || 0);
  });

  process.on('SIGINT', () => {
    console.log('\nClosing SSH tunnel...');
    child.kill('SIGTERM');
  });
}

function sshTunnelReverse(host, port = 8080) {
  console.log(chalk.bold(`Creating reverse SSH tunnel to ${host}...`));
  console.log(`  ${chalk.dim('Local:')}  http://localhost:${port}`);
  console.log(`  ${chalk.dim('Remote:')} ${host}:${port}`);
  console.log(`  ${chalk.dim('Mode:')}   Reverse (expose local gateway on remote server)\n`);

  const child = spawn(
    'ssh',
    [
      '-N',
      '-R',
      `${port}:127.0.0.1:${port}`,
      host,
      '-o',
      'ServerAliveInterval=60',
      '-o',
      'ServerAliveCountMax=3',
      '-o',
      'ExitOnForwardFailure=yes',
      '-o',
      'GatewayPorts=yes',
    ],
    { stdio: 'inherit' },
  );

  child.on('error', (err) => {
    console.error(`SSH error: ${err.message}`);
    process.exit(1);
  });
  child.on('exit', (code) => {
    console.log(`Reverse SSH tunnel closed (exit code: ${code})`);
    process.exit(code || 0);
  });

  process.on('SIGINT', () => {
    console.log('\nClosing reverse SSH tunnel...');
    child.kill('SIGTERM');
  });
}

function sshTunnelPersistent(host, port = 8080, reverse = false, name, paths) {
  if (!paths.isUser) {
    requireRoot('persistent ssh-tunnel');
  }

  const tunnelName = name || host.replace(/[^a-zA-Z0-9-]/g, '-');
  const instanceName = `${SSH_TUNNEL_TEMPLATE}${tunnelName}`;

  // Ensure template is installed
  if (!existsSync(paths.sshTunnelTemplateFile)) {
    const templateSource = join(CLI_ROOT, 'deploy', `${SSH_TUNNEL_TEMPLATE}.service`);
    if (existsSync(templateSource)) {
      copyFileSync(templateSource, paths.sshTunnelTemplateFile);
    } else {
      writeFileSync(paths.sshTunnelTemplateFile, generateSshTunnelTemplate(paths));
    }
    run(`${paths.systemctl} daemon-reload`);
  }

  // Generate environment file for this tunnel instance
  mkdirSync(paths.tunnelEnvDir, { recursive: true });

  const envFile = join(paths.tunnelEnvDir, `${tunnelName}.env`);
  const mode = reverse ? 'reverse' : 'forward';
  const sshFlag = reverse ? `-R ${port}:127.0.0.1:${port}` : `-L ${port}:127.0.0.1:${port}`;

  writeFileSync(
    envFile,
    [
      `# SSH Tunnel: ${tunnelName}`,
      `# Created: ${new Date().toISOString()}`,
      `SSH_HOST=${host}`,
      `SSH_PORT_FLAG=${sshFlag}`,
      `LOCAL_PORT=${port}`,
      `TUNNEL_MODE=${mode}`,
    ].join('\n') + '\n',
  );

  run(`${paths.systemctl} daemon-reload`);
  run(`${paths.systemctl} enable ${instanceName}`);
  run(`${paths.systemctl} start ${instanceName}`);

  console.log(chalk.green(`\nPersistent SSH tunnel '${tunnelName}' installed and started.`));
  console.log(`  ${chalk.dim('Service:')}  ${instanceName}`);
  console.log(`  ${chalk.dim('Host:')}     ${host}`);
  console.log(`  ${chalk.dim('Port:')}     ${port}`);
  console.log(`  ${chalk.dim('Mode:')}     ${mode}`);
  console.log(`  ${chalk.dim('Env file:')} ${envFile}`);
  console.log(`\n${chalk.dim('Manage:')}`);
  console.log(`  ${paths.isUser ? '' : 'sudo '}${paths.systemctl} status ${instanceName}`);
  console.log(`  ${paths.isUser ? '' : 'sudo '}${paths.systemctl} stop ${instanceName}`);
  console.log(`  ${paths.isUser ? '' : 'sudo '}${paths.systemctl} restart ${instanceName}`);
}

function sshTunnelList(paths) {
  console.log(chalk.bold('\nSSH Tunnels\n'));

  // Check systemd tunnel services
  const services = runQuiet(
    `${paths.systemctl} list-units --type=service --all "${SSH_TUNNEL_TEMPLATE}*" --no-legend 2>/dev/null`,
  );
  if (services && services.trim()) {
    console.log(chalk.dim('Persistent (systemd):'));
    for (const line of services.split('\n').filter(Boolean)) {
      const parts = line.trim().split(/\s+/);
      const name = parts[0];
      const sub = parts[3] || 'unknown';
      const icon = sub === 'running' ? chalk.green('●') : chalk.red('○');
      console.log(`  ${icon} ${name} (${sub})`);
    }
  } else {
    console.log(chalk.dim('No persistent tunnels configured.'));
  }

  // Check ad-hoc ssh tunnel processes
  const procs = runQuiet('pgrep -a -f "ssh.*-[LR].*127.0.0.1" 2>/dev/null');
  if (procs && procs.trim()) {
    console.log(chalk.dim('\nAd-hoc (process):'));
    for (const line of procs.split('\n').filter(Boolean)) {
      const pid = line.split(' ')[0];
      const cmd = line.split(' ').slice(1).join(' ').slice(0, 80);
      console.log(`  ${chalk.green('●')} PID ${pid}: ${cmd}`);
    }
  }

  console.log();
}

function sshTunnelStop(name, paths) {
  if (!name) {
    console.error('Usage: stateset-daemon ssh-tunnel stop <name>');
    console.error('       Use "stateset-daemon ssh-tunnel list" to see active tunnels.');
    process.exit(1);
  }

  // Try stopping systemd service first
  const serviceName = `${SSH_TUNNEL_TEMPLATE}${name}`;
  const active = runQuiet(`${paths.systemctl} is-active ${serviceName}`);
  if (active === 'active') {
    if (paths.isUser) {
      run(`${paths.systemctl} stop ${serviceName}`);
    } else {
      run(`sudo systemctl stop ${serviceName}`);
    }
    console.log(chalk.green(`Stopped persistent tunnel: ${name}`));
    return;
  }

  // Try killing by PID
  const pid = parseInt(name, 10);
  if (!isNaN(pid)) {
    try {
      process.kill(pid, 'SIGTERM');
      console.log(chalk.green(`Killed tunnel process: ${pid}`));
    } catch {
      console.error(`Could not kill process ${pid}`);
    }
    return;
  }

  console.error(`Tunnel "${name}" not found. Use "stateset-daemon ssh-tunnel list".`);
}

function sshKeyGen() {
  const keyPath = join(homedir(), '.ssh', 'stateset_gateway');

  if (existsSync(keyPath)) {
    console.log(`SSH key already exists at: ${chalk.cyan(keyPath)}`);
    console.log(`Public key: ${keyPath}.pub`);
    const pubKey = runQuiet(`cat ${keyPath}.pub`);
    if (pubKey) {
      console.log(`\n${pubKey}`);
      console.log(chalk.dim("\nAdd this key to the remote server's ~/.ssh/authorized_keys"));
    }
    return;
  }

  // Ensure .ssh directory exists
  mkdirSync(join(homedir(), '.ssh'), { recursive: true });

  console.log('Generating SSH key for StateSet Gateway...');
  run(`ssh-keygen -t ed25519 -C "stateset-gateway" -f ${keyPath} -N ""`);

  const pubKey = runQuiet(`cat ${keyPath}.pub`);
  console.log(chalk.green('\nSSH key generated.'));
  console.log(`  ${chalk.dim('Private:')} ${keyPath}`);
  console.log(`  ${chalk.dim('Public:')}  ${keyPath}.pub`);

  if (pubKey) {
    console.log(`\nPublic key:\n${pubKey}`);
    console.log(`\nAdd this key to the remote server:`);
    console.log(chalk.yellow(`  ssh-copy-id -i ${keyPath} user@remote-host`));
  }
}

function sshTunnelStatus(paths) {
  console.log(chalk.bold('\nSSH Tunnel Status\n'));

  // Count persistent tunnels
  const services = runQuiet(
    `${paths.systemctl} list-units --type=service --all "${SSH_TUNNEL_TEMPLATE}*" --no-legend --plain 2>/dev/null`,
  );
  const serviceList = services ? services.split('\n').filter(Boolean) : [];
  const running = serviceList.filter((l) => l.includes('running')).length;

  console.log(
    `  ${chalk.dim('Persistent tunnels:')} ${serviceList.length} configured, ${running} running`,
  );

  // Count ad-hoc
  const procs = runQuiet('pgrep -c -f "ssh.*-[LR].*127.0.0.1" 2>/dev/null');
  console.log(`  ${chalk.dim('Ad-hoc tunnels:')}     ${procs || '0'} process(es)`);

  // Check SSH key
  const keyPath = join(homedir(), '.ssh', 'stateset_gateway');
  console.log(
    `  ${chalk.dim('SSH key:')}            ${existsSync(keyPath) ? chalk.green('present') : chalk.yellow('not generated')}`,
  );

  // Check autossh availability
  const autossh = runQuiet('which autossh 2>/dev/null');
  console.log(
    `  ${chalk.dim('autossh:')}            ${autossh ? chalk.green('available') : chalk.yellow('not installed (optional)')}`,
  );

  console.log();
}

// ============================================================================
// Service File Generators
// ============================================================================

function generateServiceFile(paths) {
  const userLines = paths.isUser
    ? ''
    : `User=${paths.serviceUser}
Group=${paths.serviceUser}
`;

  const envFileLine = paths.isUser
    ? `EnvironmentFile=-${paths.configDir}/env`
    : `EnvironmentFile=-/etc/stateset/env
EnvironmentFile=-/opt/stateset/.env`;

  const securityLines = paths.isUser
    ? ''
    : `
# Security hardening
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=${paths.dataDir} ${paths.logDir}
PrivateTmp=yes`;

  return `[Unit]
Description=StateSet iCommerce Channel Gateway
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
${userLines}WorkingDirectory=${paths.appDir}
Environment=NODE_ENV=production
${envFileLine}
ExecStart=/usr/bin/node ${paths.appDir}/bin/stateset-channels.js --config ${paths.configDir}/gateway.json
ExecReload=/bin/kill -s HUP $MAINPID
Restart=on-failure
RestartSec=10
StartLimitBurst=5
StartLimitIntervalSec=60
LimitNOFILE=65536
MemoryMax=1G${securityLines}
StandardOutput=journal
StandardError=journal
SyslogIdentifier=${LOG_IDENT}

[Install]
WantedBy=${paths.installTarget}
`;
}

function generateTailscaleService() {
  return `[Unit]
Description=StateSet Tailscale Reverse Proxy
After=stateset-gateway.service tailscaled.service
Requires=stateset-gateway.service

[Service]
Type=simple
ExecStart=/usr/bin/tailscale serve --bg --https=443 http://127.0.0.1:8080
ExecStop=/usr/bin/tailscale serve off
Restart=on-failure
RestartSec=10

StandardOutput=journal
StandardError=journal
SyslogIdentifier=stateset-tailscale

[Install]
WantedBy=multi-user.target
`;
}

function generateSshTunnelTemplate(paths) {
  const userLines =
    paths && !paths.isUser
      ? `User=${paths.serviceUser}
Group=${paths.serviceUser}
`
      : '';

  const keyPath =
    paths && !paths.isUser
      ? '/home/stateset/.ssh/stateset_gateway'
      : join(homedir(), '.ssh', 'stateset_gateway');

  return `[Unit]
Description=StateSet SSH Tunnel (%i)
After=network-online.target stateset-gateway.service
Wants=network-online.target

[Service]
Type=simple
${userLines}EnvironmentFile=${paths ? paths.tunnelEnvDir : '/etc/stateset/tunnels'}/%i.env
Environment=AUTOSSH_GATETIME=0
Environment=AUTOSSH_POLL=60
ExecStart=/bin/bash -c 'if command -v autossh >/dev/null 2>&1; then exec autossh -M 0 -N \${SSH_PORT_FLAG} \${SSH_HOST} -o ServerAliveInterval=60 -o ServerAliveCountMax=3 -o ExitOnForwardFailure=yes -o StrictHostKeyChecking=accept-new -i ${keyPath}; else exec ssh -N \${SSH_PORT_FLAG} \${SSH_HOST} -o ServerAliveInterval=60 -o ServerAliveCountMax=3 -o ExitOnForwardFailure=yes -o StrictHostKeyChecking=accept-new -i ${keyPath}; fi'
Restart=on-failure
RestartSec=15
StartLimitBurst=5
StartLimitIntervalSec=120
StandardOutput=journal
StandardError=journal
SyslogIdentifier=stateset-ssh-tunnel-%i

[Install]
WantedBy=${paths ? paths.installTarget : 'multi-user.target'}
`;
}

// ============================================================================
// Main
// ============================================================================

const { values, positionals } = parseArgs({
  options: {
    config: { type: 'string' },
    port: { type: 'string', default: '8080' },
    user: { type: 'boolean', default: false },
    follow: { type: 'boolean', short: 'f', default: false },
    json: { type: 'boolean', default: false },
    output: { type: 'string' },
    reverse: { type: 'boolean', default: false },
    persistent: { type: 'boolean', default: false },
    name: { type: 'string' },
    help: { type: 'boolean', short: 'h', default: false },
  },
  allowPositionals: true,
});

if (values.help || positionals.length === 0) {
  console.log(HELP);
  process.exit(0);
}

const command = positionals[0];
const port = parseInt(values.port, 10);
const paths = getUserPaths(values.user);
const outputPath = values.output || null;
if (outputPath) {
  values.json = true;
}
const isJsonOutput = values.json;
const writeJson = (data) => {
  const payload = JSON.stringify(data, null, 2);
  if (outputPath) {
    writeFileSync(outputPath, payload);
    return;
  }
  console.log(payload);
};
const emitError = (message, code = 1) => {
  if (isJsonOutput) {
    writeJson({ error: message });
  } else {
    console.error(message);
  }
  process.exit(code);
};

if (Number.isNaN(port)) {
  emitError(`Invalid port: ${values.port}`);
}

if (isJsonOutput) {
  const subcommand = positionals[1];
  const supportedCommands = new Set([
    'status',
    'health',
    'config',
    'validate',
    'logs',
    'tailscale',
    'ssh-tunnel',
    'ssh',
  ]);

  if (!supportedCommands.has(command)) {
    emitError(
      'JSON output is supported for: status, health, config, validate, logs (non-follow), tailscale status, ssh-tunnel list/status.',
    );
  }

  if (command === 'logs' && values.follow) {
    emitError('JSON output is not supported with --follow.');
  }

  if (command === 'tailscale' && subcommand !== 'status') {
    emitError('JSON output is only supported for "tailscale status".');
  }

  if (
    (command === 'ssh-tunnel' || command === 'ssh') &&
    subcommand !== 'list' &&
    subcommand !== 'status'
  ) {
    emitError('JSON output is only supported for "ssh-tunnel list" and "ssh-tunnel status".');
  }
}

switch (command) {
  case 'install':
    cmdInstall({ config: values.config, paths });
    break;
  case 'start':
    cmdStart(paths);
    break;
  case 'stop':
    cmdStop(paths);
    break;
  case 'restart':
    cmdRestart(paths);
    break;
  case 'enable':
    cmdEnable(paths);
    break;
  case 'disable':
    cmdDisable(paths);
    break;
  case 'status':
    if (isJsonOutput) {
      writeJson(collectStatus(paths, port));
    } else {
      cmdStatus(paths, port);
    }
    break;
  case 'logs':
    if (isJsonOutput) {
      const lines = parseInt(positionals[1], 10) || 50;
      writeJson(collectLogs(lines, paths));
    } else {
      cmdLogs(parseInt(positionals[1], 10) || 50, values.follow, paths);
    }
    break;
  case 'config':
    if (isJsonOutput) {
      const config = collectConfig(paths);
      if (config.error) {
        emitError(config.error);
      }
      writeJson(config);
    } else {
      cmdConfig(paths);
    }
    break;
  case 'validate':
    if (isJsonOutput) {
      const configPath = values.config || join(paths.configDir, 'gateway.json');
      const result = validateConfigDetailed(configPath);
      writeJson(result);
      process.exit(result.valid ? 0 : 1);
    } else {
      cmdValidate(paths, values.config);
    }
    break;
  case 'update':
    cmdUpdate(paths);
    break;
  case 'tailscale':
    if (isJsonOutput) {
      writeJson(collectTailscaleStatus());
    } else {
      cmdTailscale(positionals[1], positionals[2], port);
    }
    break;
  case 'ssh-tunnel':
  case 'ssh':
    if (isJsonOutput) {
      if (positionals[1] === 'list') {
        writeJson(collectSshTunnelList(paths));
      } else {
        writeJson(collectSshTunnelStatus(paths));
      }
    } else {
      cmdSshTunnel(positionals[1], port, values, paths);
    }
    break;
  case 'health':
    if (isJsonOutput) {
      const result = collectHealth(port);
      writeJson(result);
      if (!result.ok) {
        process.exit(1);
      }
    } else {
      cmdHealth(port);
    }
    break;
  case 'uninstall':
    cmdUninstall(paths);
    break;
  default:
    if (isJsonOutput) {
      emitError(`Unknown command: ${command}`);
    } else {
      console.error(`Unknown command: ${command}`);
      console.error('Run stateset-daemon --help for usage');
      process.exit(1);
    }
}
