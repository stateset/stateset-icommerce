#!/usr/bin/env node

/**
 * Quick Start: Run StateSet iCommerce Gateway as a Background Service
 *
 * This example shows you how to:
 * 1. Install the StateSet daemon
 * 2. Configure it to run your agent
 * 3. Start it as a background service
 * 4. Check its status and logs
 */

import { execSync } from 'child_process';
import chalk from 'chalk';

console.log(chalk.bold.blue(`
════════════════════════════════════════════════════════════════
   StateSet iCommerce - Run Agents in Background
════════════════════════════════════════════════════════════════
`));

console.log(chalk.yellow(`
Quick Overview
--------------
The StateSet daemon uses systemd to run the gateway and agents
as a background service. This means:
• Your agents keep running even after you close the terminal
• Automatic restart on crashes or system reboot (when enabled)
• Centralized logging via journalctl
• Easy management with 'stateset-daemon' CLI
`));

console.log(chalk.green(`
Step 1: Install the Daemon
────────────────────────────────
`));

console.log(`
# For system-wide installation (requires sudo)
${chalk.cyan('sudo stateset-daemon install')}

# For user-level installation (no sudo required)
${chalk.cyan('stateset-daemon install --user')}
`);

console.log(chalk.green(`
Step 2: Configure Your Agent
────────────────────────────────
`));

console.log(`
The daemon creates these files after installation:

${chalk.cyan('/etc/stateset/env')}         → API keys (ANTHROPIC_API_KEY, etc.)
${chalk.cyan('/etc/stateset/gateway.json')} → Gateway configuration

Edit them:
${chalk.cyan('sudo nano /etc/stateset/env')}
${chalk.cyan('sudo nano /etc/stateset/gateway.json')}
`);

console.log(chalk.green(`
Step 3: Start the Background Service
────────────────────────────────────
`));

console.log(`
# Start the daemon
${chalk.cyan('sudo stateset-daemon start')}

# Enable auto-start on boot
${chalk.cyan('sudo stateset-daemon enable')}
`);

console.log(chalk.green(`
Step 4: Monitor Your Agents
───────────────────────────
`));

console.log(`
# Check if the daemon is running
${chalk.cyan('stateset-daemon status')}

# View logs (last 50 lines)
${chalk.cyan('stateset-daemon logs')}

# Follow logs in real-time (like tail -f)
${chalk.cyan('stateset-daemon logs -f')}
`);

console.log(chalk.green(`
Common Management Commands
──────────────────────────
`));

const commands = [
  { cmd: 'stateset-daemon stop', desc: 'Stop the daemon' },
  { cmd: 'stateset-daemon restart', desc: 'Restart the daemon' },
  { cmd: 'stateset-daemon disable', desc: 'Disable auto-start on boot' },
  { cmd: 'stateset-daemon config', desc: 'Show current configuration' },
  { cmd: 'stateset-daemon validate', desc: 'Validate gateway config' },
  { cmd: 'stateset-daemon health', desc: 'Check gateway health' },
  { cmd: 'stateset-daemon status', desc: 'Show full status with metrics' },
];

for (const c of commands) {
  console.log(`${chalk.cyan(c.cmd.padEnd(45))} ${chalk.dim(c.desc)}`);
}

console.log(chalk.green(`
Example: Minimal Setup for Telegram Agent
───────────────────────────────────────────
`));

console.log(`
# 1. Install daemon
${chalk.cyan('sudo stateset-daemon install')}

# 2. Add your Telegram bot token to env
${chalk.cyan('sudo nano /etc/stateset/env')}
# Add: TELEGRAM_BOT_TOKEN=your_bot_token_here

# 3. Enable Telegram in gateway config
${chalk.cyan('sudo nano /etc/stateset/gateway.json')}
# Change: "telegram": { "enabled": true }

# 4. Start the daemon
${chalk.cyan('sudo stateset-daemon start'}

# 5. Check it's running
${chalk.cyan('stateset-daemon status')}
`);

console.log(chalk.green(`
Advanced: Multiple Agents in Background
──────────────────────────────────────────
`));

console.log(`
You can enable multiple channels in gateway.json:

{
  "channels": {
    "telegram": { "enabled": true },
    "discord": { "enabled": true },
    "slack": { "enabled": true }
  },
  "shared": {
    "agent": "customer-service"
  }
}

The daemon will run ALL enabled channels simultaneously!
Each channel gets its own agent instance, all working in parallel.
`);

console.log(chalk.yellow(`
System Level vs User Level
──────────────────────────
`));

console.log(`
${chalk.bold('System Level (sudo)')}
• Runs as system service
• More isolated and secure
• Requires sudo to manage
• Best for production

${chalk.bold('User Level (--user)')}
• Runs as your user
• No sudo required
• Easier to develop with
• Best for development
`);

console.log(chalk.yellow(`
Remote Access (Optional)
────────────────────────
`));

console.log(`
The daemon supports remote access via Tailscale or SSH tunnels:

${chalk.cyan('# Tailscale - secure VPN access')}
${chalk.cyan('sudo stateset-daemon tailscale setup')}
${chalk.cyan('sudo stateset-daemon tailscale serve')}

${chalk.cyan('# SSH tunnels - expose gateway on remote server')}
${chalk.cyan('stateset-daemon ssh-tunnel user@vps --reverse --persistent')}
`);

console.log(chalk.bold.blue(`
════════════════════════════════════════════════════════════════
   Ready to Run Your Agents in Background!
════════════════════════════════════════════════════════════════
`));

console.log(chalk.dim(`
Next steps:
1. Run: sudo stateset-daemon install
2. Edit /etc/stateset/env with your API keys
3. Edit /etc/stateset/gateway.json to enable channels
4. Run: sudo stateset-daemon start
5. Run: stateset-daemon status to verify

You now have autonomous agents running 24/7! 🤖
`) +
  '\n');

// Try to check if daemon is already installed
try {
  const serviceFile = execSync('ls /etc/systemd/system/stateset-gateway.service 2>/dev/null', { encoding: 'utf8' });
  if (serviceFile) {
    console.log(chalk.green('✓ Daemon already installed at /etc/systemd/system/stateset-gateway.service'));
  }
} catch {
  console.log(chalk.yellow('⚠ Daemon not installed yet. Run "sudo stateset-daemon install" to get started.'));
}