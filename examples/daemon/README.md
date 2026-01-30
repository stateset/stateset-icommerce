# StateSet iCommerce Daemon - Running Agents in Background

The StateSet daemon manages the gateway and runs agents as a **systemd service** on Linux, meaning they run reliably in the background with auto-restart on boot.

---

## Quick Start

### System Mode (requires sudo):

```bash
# 1. Install as systemd service
sudo stateset-daemon install

# 2. Add API keys
sudo nano /etc/stateset/env
# Add: ANTHROPIC_API_KEY=your_key_here

# 3. Configure channels
sudo nano /etc/stateset/gateway.json
# Set "enabled": true for channels you want

# 4. Start the daemon
sudo stateset-daemon start

# 5. Check status
stateset-daemon status
```

### User Mode (no sudo required):

```bash
# 1. Install as user service
stateset-daemon install --user

# 2. Add API keys
nano ~/.config/stateset/env

# 3. Configure channels
nano ~/.config/stateset/gateway.json

# 4. Start the daemon
stateset-daemon start --user

# 5. Check status
stateset-daemon status --user
```

---

## Daemon Commands

### Core Management

```bash
# Install the systemd service
sudo stateset-daemon install              # System mode
stateset-daemon install --user            # User mode

# Start/Stop/Restart
sudo stateset-daemon start                # System mode
stateset-daemon start --user              # User mode
sudo stateset-daemon stop
sudo stateset-daemon restart

# Auto-start on boot
sudo stateset-daemon enable               # Enable auto-start
sudo stateset-daemon disable              # Disable auto-start

# Check status
stateset-daemon status                    # Show detailed status
stateset-daemon logs                      # View last 50 log lines
stateset-daemon logs 100                  # View last 100 lines
stateset-daemon logs -f                   # Follow logs in real-time
```

### Configuration

```bash
# View current config
stateset-daemon config

# Validate configuration
stateset-daemon validate

# Update to latest version
sudo stateset-daemon update

# Health check
stateset-daemon health

# Uninstall (removes service, keeps data)
sudo stateset-daemon uninstall
```

---

## Directory Structure

### System Mode (sudo):

```
/etc/stateset/              # Configuration
  ├── env                  # Environment variables (API keys)
  ├── gateway.json         # Main configuration
  └── tunnels/             # SSH tunnel configs

/opt/stateset/             # Application files
  ├── bin/                 # CLI binaries
  ├── src/                 # Source code
  ├── skills/              # Agent skills
  └── node_modules/        # Dependencies

/var/log/stateset/         # Logs

/opt/stateset/data/        # Persistent data
  ├── store.db            # Commerce database
  └── sessions.db         # Session storage
```

### User Mode:

```
~/.config/stateset/        # Configuration
  ├── env
  └── gateway.json

~/.local/share/stateset/   # Application files and data
  ├── app/
  ├── logs/
  └── data/
```

---

## Configuration

### Environment File (`env`)

```bash
# StateSet Gateway Environment
# Add your API keys here

# Required
ANTHROPIC_API_KEY=sk-ant-...

# Optional - for AI provider fallback
OPENAI_API_KEY=sk-proj-...
GEMINI_API_KEY=...

# Optional - for specific channels
TELEGRAM_BOT_TOKEN=123456:ABC...
DISCORD_BOT_TOKEN=MTAz...
SLACK_BOT_TOKEN=xoxb-...
SLACK_APP_TOKEN=xapp-...
TEAMS_APP_ID=
TEAMS_APP_PASSWORD=
```

### Gateway Configuration (`gateway.json`)

```json
{
  "channels": {
    "telegram": {
      "enabled": true,
      "allowlist": []
    },
    "discord": {
      "enabled": false,
      "allowlist": [],
      "channelFilter": ["general"]
    },
    "webchat": {
      "enabled": true
    }
  },
  "shared": {
    "dbPath": "./data/store.db",
    "allowApply": false,
    "model": "claude-sonnet-4-20250514",
    "agent": "customer-service"
  },
  "httpGateway": {
    "enabled": true,
    "port": 8080
  },
  "autonomousEngine": null,
  "persistSessions": true
}
```

---

## Running Autonomous Agents

The daemon can run autonomous agents through the **autonomous engine**:

### Enable Autonomous Engine in Config

```json
{
  "autonomousEngine": {
    "enabled": true,

    "workflows": {
      "enabled": true,
      "autoStart": true
    },

    "jobs": {
      "enabled": true,
      "autoStart": true
    },

    "policies": {
      "enabled": true,
      "autoStart": true
    }
  }
}
```

### What Runs Automatically:

1. **Scheduled Jobs** - Check inventory, process abandoned carts, renew subscriptions
2. **State Machine Workflows** - Multi-agent order fulfillment, return processing
3. **Policy Engine** - Auto-approve returns, trigger restock orders

### Example: Low Stock Monitor

In your gateway config:

```json
{
  "autonomousEngine": {
    "jobs": {
      "enabled": true,
      "templates": [
        {
          "id": "low-stock-check",
          "name": "Low Stock Monitor",
          "schedule": "0 * * * *",
          "action": {
            "agent": "inventory",
            "request": "Check for low stock items and list any products below their reorder point"
          }
        }
      ]
    }
  }
}
```

This agent runs **every hour** in the background!

---

## Remote Access

### Tailscale (VPN)

```bash
# 1. Setup Tailscale proxy
sudo stateset-daemon tailscale setup

# 2. Enable Tailscale Serve (private network access)
stateset-daemon tailscale serve

# 3. Enable Tailscale Funnel (public internet access) - CAUTION!
stateset-daemon tailscale funnel

# 4. Check status
stateset-daemon tailscale status

# 5. Set custom hostname
stateset-daemon tailscale dns my-commerce-gateway
```

**Access URLs:**
- Private: `https://my-commerce-gateway.tailnet-name.ts.net`
- Public: `https://my-commerce-gateway.ts.net` (via Funnel)

### SSH Tunnels

```bash
# Forward tunnel (access remote gateway from local)
stateset-daemon ssh-tunnel user@remote-vps.com

# Reverse tunnel (expose local gateway on remote)
stateset-daemon ssh-tunnel user@remote-vps.com --reverse

# Persistent tunnel (auto-restart on boot)
stateset-daemon ssh-tunnel user@remote-vps.com --reverse --persistent --name production

# List all tunnels
stateset-daemon ssh-tunnel list

# Stop a tunnel
stateset-daemon ssh-tunnel stop production

# Generate SSH key
stateset-daemon ssh-tunnel keygen
```

---

## Monitoring & Debugging

### View Logs

```bash
# Last 50 lines
stateset-daemon logs

# Last 100 lines
stateset-daemon logs 100

# Follow in real-time (Ctrl+C to exit)
stateset-daemon logs -f

# Using journalctl directly
sudo journalctl -u stateset-gateway -f
```

### Check Health

```bash
# Quick health check
stateset-daemon health

# Full status
stateset-daemon status

# Check metrics
curl http://localhost:8080/metrics
```

### Debug Mode

Enable verbose logging in `gateway.json`:

```json
{
  "shared": {
    "verbose": true
  }
}
```

Then restart: `sudo stateset-daemon restart`

---

## Auto-Discovery & Skills

The daemon automatically discovers and loads:

- **Skills** from `~/.stateset/skills/` or `/opt/stateset/skills/`
- **Plugins** from `~/.stateset/plugins/` or `/opt/stateset/src/plugins/`

### Add a Custom Skill

1. Create skill directory:
```bash
mkdir -p ~/.stateset/skills/inventory-alert
```

2. Create skill file:
```js
// ~/.stateset/skills/inventory-alert/index.js
export const skill = {
  name: 'inventory-alert',
  category: 'commerce',
  description: 'Alert when inventory runs low',

  tools: [{
    name: 'check_low_stock',
    description: 'Check products with low inventory',
    handler: async (commerce) => {
      const products = await commerce.products.list();
      return products.filter(p => p.quantity < 10);
    }
  }]
};
```

3. Restart daemon:
```bash
stateset-daemon restart --user
```

The skill is now available to all agents!

---

## Updating & Maintenance

### Update to Latest Version

```bash
# Update daemon and all files
sudo stateset-daemon update

# Restart to apply changes
sudo stateset-daemon restart
```

### Backup Data

```bash
# Backup PostgreSQL database
cp /opt/stateset/data/store.db ~/backup/store.db.$(date +%Y%m%d)

# Backup configuration
sudo cp /etc/stateset/gateway.json ~/backup/gateway.json.$(date +%Y%m%d)
sudo cp /etc/stateset/env ~/backup/env.$(date +%Y%m%d)
```

### Restore from Backup

```bash
# Stop daemon
sudo stateset-daemon stop

# Restore database
sudo cp ~/backup/store.db.20260129 /opt/stateset/data/store.db

# Restore config
sudo cp ~/backup/gateway.json.20260129 /etc/stateset/gateway.json

# Start daemon
sudo stateset-daemon start
```

---

## Troubleshooting

### Service Won't Start

```bash
# Check if service is installed
systemctl status stateset-gateway

# View detailed logs
sudo journalctl -u stateset-gateway -n 100 --no-pager

# Validate config
stateset-daemon validate
```

### Port Already in Use

```bash
# Check what's using port 8080
sudo lsof -i :8080

# Change port in gateway.json
{
  "httpGateway": {
    "port": 3000
  }
}

# Restart
sudo stateset-daemon restart
```

### Agent Not Responding

```bash
# Check if agent is enabled in config
stateset-daemon config | grep agent

# Check logs for errors
stateset-daemon logs -f

# Verify API key is set
cat /etc/stateset/env | grep ANTHROPIC_API_KEY
```

### High Memory Usage

The daemon has a 1GB memory limit by default. To increase:

```bash
# Edit service file
sudo systemctl edit stateset-gateway

# Add:
[Service]
MemoryMax=2G

# Reload and restart
sudo systemctl daemon-reload
sudo systemctl restart stateset-gateway
```

---

## Production Deployment Checklist

- [x] ✅ Install as systemd service: `sudo stateset-daemon install`
- [x] ✅ Add API keys: Edit `/etc/stateset/env`
- [x] ✅ Configure channels: Edit `/etc/stateset/gateway.json`
- [x] ✅ Enable auto-start: `sudo stateset-daemon enable`
- [x] ✅ Set up Tailscale for remote access
- [x] ✅ Configure autonomous agents/jobs
- [x] ✅ Set up log rotation (via logrotate)
- [x] ✅ Enable health monitoring
- [x] ✅ Configure backup routine
- [x] ✅ Test restart: `sudo stateset-daemon restart`

---

## Security Best Practices

1. **Environment Variables** - Never commit `env` files. Set permissions: `chmod 600 /etc/stateset/env`
2. **API Keys** - Use separate keys for dev/staging/production
3. **User Mode** - Use `--user` mode for development (no sudo required)
4. **Allowlists** - Restrict which users can interact on channels:
   ```json
   {
     "channels": {
       "slack": {
         "allowlist": ["U1234...", "U5678..."]
       }
     }
   }
   ```
5. **HTTPS Only** - Use Tailscale Funnel or reverse proxy for public access
6. **Rate Limiting** - Enable in config:
   ```json
   {
     "middleware": {
       "rateLimiter": {
         "windowMs": 60000,
         "max": 100
       }
     }
   }
   ```

---

## Advanced: Custom Service File

For custom configurations, create a drop-in file:

```bash
sudo systemctl edit stateset-gateway
```

Add custom overrides:

```ini
[Service]
# Override environment variables
Environment=CUSTOM_VAR=value

# Override resource limits
MemoryMax=2G
CPUQuota=200%

# Add custom restart delays
RestartSec=5
```

---

## Support & Documentation

- **Full Documentation**: https://docs.stateset.com
- **GitHub Issues**: https://github.com/stateset/stateset-icommerce/issues
- **Community Discord**: https://discord.gg/stateset