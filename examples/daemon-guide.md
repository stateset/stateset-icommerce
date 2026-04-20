# StateSet daemon (Background Service) Guide

The `stateset-daemon` command manages the iCommerce gateway as a **systemd service**, enabling your agents to run automatically in the background without manual intervention.

## 📋 Table of Contents

1. [Overview](#overview)
2. [Quick Start](#quick-start)
3. [System vs User Services](#system-vs-user-services)
4. [Commands Reference](#commands-reference)
5. [Configuration](#configuration)
6. [Remote Access](#remote-access)
7. [Monitoring & Troubleshooting](#monitoring--troubleshooting)

---

## 🎯 Overview

The StateSet daemon provides:

- ✅ **Background execution** - Agents run 24/7 without a terminal
- ✅ **Auto-restart on failure** - Automatically recovers from crashes
- ✅ **Boot-time startup** - Starts automatically when server boots
- ✅ **Log management** - Integrated with systemd journal
- ✅ **Resource limits** - Memory and file descriptor controls
- ✅ **Security hardening** - Runs as dedicated user with restricted permissions
- ✅ **Remote access** - Built-in Tailscale and SSH tunnel support

---

## 🚀 Quick Start

### System-wide Installation (Recommended for Servers)

```bash
# 1. Install the daemon (creates systemd service, directories, config)
sudo stateset-daemon install

# 2. Edit environment variables (add API keys)
sudo nano /etc/stateset/env
# Add: ANTHROPIC_API_KEY=sk-ant-...

# 3. Edit gateway configuration (enable channels)
sudo nano /etc/stateset/gateway.json
# Set "channels.telegram.enabled": true

# 4. Validate configuration
stateset-daemon validate

# 5. Start the daemon
sudo stateset-daemon start

# 6. Check status
stateset-daemon status
```

### User-level Installation (No Sudo)

```bash
# Install as user service (no root required)
stateset-daemon install --user

# Edit config
nano ~/.config/stateset/env
nano ~/.config/stateset/gateway.json

# Start
stateset-daemon start --user

# Check status
stateset-daemon status --user
```

---

## 🔧 System vs User Services

### System Service (`sudo`)

**Location:** `/etc/systemd/system/stateset-gateway.service`  
**User:** `stateset` (dedicated system user)  
**Config:** `/etc/stateset/`  
**Use when:** Production servers, multi-user systems, always-on services

**Pros:**
- Higher security (isolated system user)
- Starts before user login
- Manages all channels uniformly

**Cons:**
- Requires sudo for all operations
- More complex setup

### User Service (No sudo)

**Location:** `~/.config/systemd/user/stateset-gateway.service`  
**User:** Your current user  
**Config:** `~/.config/stateset/`  
**Use when:** Development, personal server, no sudo access

**Pros:**
- No sudo required
- Easier to manage
- Store in home directory

**Cons:**
- Only starts when you log in (unless lingering enabled)
- All processes run as your user

---

## 📚 Commands Reference

### Install & Setup

```bash
# Install system service (requires sudo)
sudo stateset-daemon install

# Install user service (no sudo)
stateset-daemon install --user

# Install with custom config file
sudo stateset-daemon install --config /path/to/gateway.json
```

### Service Control

```bash
# Start the daemon
sudo stateset-daemon start                    # System
stateset-daemon start --user                 # User

# Stop the daemon
sudo stateset-daemon stop
stateset-daemon stop --user

# Restart the daemon
sudo stateset-daemon restart
stateset-daemon restart --user

# Enable auto-start on boot
sudo stateset-daemon enable
stateset-daemon enable --user

# Disable auto-start on boot
sudo stateset-daemon disable
stateset-daemon disable --user
```

### Status & Monitoring

```bash
# Show detailed status
stateset-daemon status

# Show last 100 log lines
stateset-daemon logs 100

# Follow logs in real-time (like tail -f)
stateset-daemon logs -f

# Show current configuration
stateset-daemon config

# Validate configuration file
stateset-daemon validate

# Check health (HTTP endpoint)
stateset-daemon health
```

### Updates & Maintenance

```bash
# Update to latest version
sudo stateset-daemon update
stateset-daemon update --user

# Uninstall (preserves data/config)
sudo stateset-daemon uninstall
```

---

## ⚙️ Configuration

### Directory Structure

**System Service:**
```
/etc/stateset/
├── env                   # Environment variables (API keys)
├── gateway.json          # Gateway configuration
└── tunnels/              # SSH tunnel configs

/opt/stateset/
├── bin/                  # CLI binaries
├── src/                  # Source code
├── skills/               # Skills directory
├── data/                 # Databases, logs
└── node_modules/         # Dependencies

/var/log/stateset/
└── stateset-gateway.log  # Application logs
```

**User Service:**
```
~/.config/stateset/
├── env                   # Environment variables
├── gateway.json          # Gateway configuration
└── tunnels/              # SSH tunnel configs

~/.local/share/stateset/
├── app/                  # Application files
├── data/                 # Databases
└── logs/                 # Logs
```

### Environment Variables (`env` file)

```bash
# Required: AI Provider
ANTHROPIC_API_KEY=sk-ant-...
# OPENAI_API_KEY=sk-...
# GEMINI_API_KEY=...

# Optional: Channel credentials
TELEGRAM_BOT_TOKEN=...
DISCORD_BOT_TOKEN=...
SLACK_BOT_TOKEN=xoxb-...
SLACK_APP_TOKEN=xapp-...
TEAMS_APP_ID=...
TEAMS_APP_PASSWORD=...

# Optional: Additional providers
OPENAI_API_KEY=sk-...
```

### Gateway Configuration (`gateway.json`)

See `cli/deploy/gateway.config.example.json` for full reference.

Key sections:
- `channels` - Enable/disable communication channels
- `shared` - Agent configuration (model, dbPath, etc.)
- `httpGateway` - HTTP API server settings
- `notifications` - Cross-channel routing rules
- `plugins` - Plugin system configuration
- `heartbeat` - Proactive health checks

---

## 🌐 Remote Access

### Tailscale Integration

Tailscale provides secure VPN access to your gateway from anywhere.

```bash
# 1. Setup Tailscale (requires Tailscale installed)
sudo stateset-daemon tailscale setup

# 2. Enable Tailscale Serve (HTTPS within your tailnet)
stateset-daemon tailscale serve

# 3. Enable Tailscale Funnel (public internet - use carefully!)
stateset-daemon tailscale funnel

# 4. Check status
stateset-daemon tailscale status

# 5. Set custom hostname
stateset-daemon tailscale dns my-gateway
```

**What this gives you:**
- `https://my-gateway.tailnet-name.ts.net` - Private HTTPS (only your tailnet)
- `https://my-gateway.ts.net` - Public HTTPS (via Funnel)

### SSH Tunnels

```bash
# Forward tunnel: Access remote gateway locally
stateset-daemon ssh-tunnel user@remote-server

# Reverse tunnel: Expose local gateway on remote VPS
stateset-daemon ssh-tunnel user@vps --reverse --persistent --name vps

# List active tunnels
stateset-daemon ssh-tunnel list

# Stop a tunnel
stateset-daemon ssh-tunnel stop <name-or-pid>

# Generate SSH key for gateway
stateset-daemon ssh-tunnel keygen
```

**Modes:**
- **Forward** (`-L`): `local:port -> remote:port` (access remote locally)
- **Reverse** (`-R`): `remote:port -> local:port` (expose local remotely)

---

## 🔍 Monitoring & Troubleshooting

### Status Dashboard

```bash
stateset-daemon status
```

Output shows:
```
StateSet Gateway Status

  Service:    ● active (running)
  Enabled:    yes
  Mode:       system
  PID:        12345
  Uptime:     2d 5h 30m

  Health:     OK
  Gateway up: 2d 5h 30m

  Tailscale
  Status:     connected
  Hostname:   my-server
  Tailnet:    example.ts.net
  URL:        https://my-server.example.ts.net
  Serve:      active
  Funnel:     active (public internet)

  SSH Tunnels
  Persistent: 1 running

  Memory:     256MB
```

### View Logs

```bash
# Last 50 lines (default)
stateset-daemon logs

# Follow in real-time
stateset-daemon logs -f

# Specific number of lines
stateset-daemon logs 200

# Use journalctl directly for advanced filtering
sudo journalctl -u stateset-gateway -f
sudo journalctl -u stateset-gateway --since "1 hour ago"
sudo journalctl -u stateset-gateway --grep "error"
```

### Systemd Integration

```bash
# Direct systemctl commands (system service)
sudo systemctl status stateset-gateway
sudo systemctl start stateset-gateway
sudo systemctl stop stateset-gateway
sudo systemctl restart stateset-gateway
sudo systemctl enable stateset-gateway
sudo systemctl disable stateset-gateway

# User service
systemctl --user status stateset-gateway
systemctl --user start stateset-gateway
```

### Health Check API

```bash
# Check health endpoint
curl http://localhost:8080/health
# Response: {"status":"ok","timestamp":"2026-01-29T20:00:00.000Z","uptime":12345678}

# Check metrics
curl http://localhost:8080/metrics
# Response: {"messagesReceived":1234,"messagesSent":5678,"..."}
```

### Common Issues

**Service won't start:**
1. Check logs: `stateset-daemon logs -f`
2. Validate config: `stateset-daemon validate`
3. Check env file has required API keys
4. Verify ports are not in use (8080, 3978 for Teams)

**Can't connect to channels:**
1. Verify API tokens are correct in `/etc/stateset/env` or `~/.config/stateset/env`
2. Check channel is enabled in `gateway.json`
3. Restart: `sudo stateset-daemon restart`

**High memory usage:**
```bash
# Current memory
stateset-daemon status  # Shows memory usage

# Restart to free memory
sudo stateset-daemon restart

# Adjust MemoryMax in systemd unit if needed
sudo systemctl edit stateset-gateway
# Add: [Service] MemoryMax=2G
```

---

## 📝 Example Workflows

### Production Deployment

```bash
#!/bin/bash
# deploy-gateway.sh

# 1. Update code
cd /opt/stateset
git pull
npm ci --omit=dev

# 2. Validate config
sudo stateset-daemon validate

# 3. Restart service
sudo stateset-daemon restart

# 4. Wait for startup
sleep 5

# 5. Health check
curl -f http://localhost:8080/health || {
  echo "Health check failed!"
  sudo stateset-daemon logs -f
  exit 1
}

echo "Deployment successful!"
```

### Setup Multi-Channel Gateway

```bash
# 1. Install daemon
sudo stateset-daemon install

# 2. Configure channels
sudo nano /etc/stateset/gateway.json
# Set:
#   "channels.telegram.enabled": true
#   "channels.discord.enabled": true
#   "channels.slack.enabled": true

# 3. Add credentials
sudo nano /etc/stateset/env
# Add:
#   TELEGRAM_BOT_TOKEN=...
#   DISCORD_BOT_TOKEN=...
#   SLACK_BOT_TOKEN=...
#   SLACK_APP_TOKEN=...

# 4. Validate and start
sudo stateset-daemon validate
sudo stateset-daemon start
sudo stateset-daemon enable  # Auto-start on boot
```

### Expose Gateway Publicly (with Tailscale)

```bash
# 1. Install Tailscale (if not installed)
curl -fsSL https://tailscale.com/install.sh | sh

# 2. Authenticate Tailscale
sudo tailscale up

# 3. Setup StateSet Tailscale integration
sudo stateset-daemon tailscale setup

# 4. Enable Funnel for public HTTPS
stateset-daemon tailscale funnel

# 5. Get your public URL
stateset-daemon tailscale status
# Look for: https://my-server.ts.net

# 6. Test it
curl https://my-server.ts.net/health
```

---

## 🔒 Security Best Practices

1. **Use system service in production** - Runs as isolated `stateset` user
2. **Limit API key permissions** - Only grant necessary scopes
3. **Enable rate limiting** - Configure in `middleware` section
4. **Use VPNs** - Tailscale for internal access, avoid exposing publicly
5. **Regular updates** - `sudo stateset-daemon update`
6. **Monitor logs** - `stateset-daemon logs -f` for suspicious activity
7. **Backup config and data** - `/etc/stateset/` and `/opt/stateset/data/`

---

## 📚 Next Steps

- [Gateway Examples](../gateway-examples/) - Code examples for integrations
- [Multi-Agent Examples](../multi-agent/) - Run autonomous agents that interact
- [Configuration Reference](../cli/deploy/gateway.config.example.json) - Full config options
- [README](../../README.md) - Complete documentation

---

## 💡 Tips

- Use `stateset-daemon status` to quickly check if everything is running
- Follow logs with `stateset-daemon logs -f` when debugging
- Validate config changes before restarting: `stateset-daemon validate`
- Enable auto-start on boot for production: `sudo stateset-daemon enable`
- Use Tailscale for secure remote access without open ports
- Set up notifications for system alerts via the `notifications.routes` config
