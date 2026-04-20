STATESET DAEMON QUICK START
===========================

The StateSet daemon runs the iCommerce gateway (and autonomous agents) 
as a system service using systemd.

-----------------------------------------------------------------------
QUICK START (User Mode - No Sudo Required)
-----------------------------------------------------------------------

1. Install the daemon:
   stateset-daemon install --user

2. Add your API keys to the env file:
   nano ~/.config/stateset/env
   # Add: ANTHROPIC_API_KEY=sk-ant-...

3. Enable channels in the config:
   nano ~/.config/stateset/gateway.json
   # Set "telegram": { "enabled": true }

4. Start the daemon:
   stateset-daemon start --user

5. Check status:
   stateset-daemon status --user

-----------------------------------------------------------------------
QUICK START (System Mode - Requires Root)
-----------------------------------------------------------------------

1. Install the daemon:
   sudo stateset-daemon install

2. Add your API keys:
   sudo nano /etc/stateset/env
   # Add: ANTHROPIC_API_KEY=sk-ant-...

3. Enable channels:
   sudo nano /etc/stateset/gateway.json

4. Start the daemon:
   sudo stateset-daemon start

5. Check status:
   stateset-daemon status

-----------------------------------------------------------------------
COMMON COMMANDS
-----------------------------------------------------------------------

Start/Stop/Restart:
  stateset-daemon start [--user]
  stateset-daemon stop [--user]
  stateset-daemon restart [--user]

Enable/Disable (Auto-start on boot):
  stateset-daemon enable [--user]
  stateset-daemon disable [--user]

Status and Logs:
  stateset-daemon status [--user]
  stateset-daemon logs 100      # Last 100 lines
  stateset-daemon logs -f       # Follow logs in real-time

Configuration:
  stateset-daemon config [--user]      # Show current config
  stateset-daemon validate [--user]    # Validate config

Health Check:
  stateset-daemon health        # Check if gateway is responding

-----------------------------------------------------------------------
FILE LOCATIONS (System Mode)
-----------------------------------------------------------------------

Config:      /etc/stateset/gateway.json
Env File:    /etc/stateset/env
Data:        /opt/stateset/data
Logs:        /var/log/stateset
App Files:   /opt/stateset

-----------------------------------------------------------------------
FILE LOCATIONS (User Mode)
-----------------------------------------------------------------------

Config:      ~/.config/stateset/gateway.json
Env File:    ~/.config/stateset/env
Data:        ~/.local/share/stateset
Logs:        ~/.local/share/stateset/logs
App Files:   ~/.local/share/stateset/app

-----------------------------------------------------------------------
REMOTE ACCESS (Tailscale)
-----------------------------------------------------------------------

Expose your gateway to your Tailscale network:

1. Setup Tailscale (requires root):
   sudo stateset-daemon tailscale setup

2. Enable HTTPS access within your tailnet:
   stateset-daemon tailscale serve

3. Enable public internet access (小心!):
   stateset-daemon tailscale funnel

4. Check Tailscale status:
   stateset-daemon tailscale status

---------------------------------------------------------------------------
SSH TUNNELS (Alternative Remote Access)
---------------------------------------------------------------------------

Forward tunnel (access remote from local):
  stateset-daemon ssh-tunnel user@remote-host

Reverse tunnel (expose local on remote):
  stateset-daemon ssh-tunnel user@vps -- reverse --persistent --name vps

List all tunnels:
  stateset-daemon ssh-tunnel list

Generate SSH key:
  stateset-daemon ssh-tunnel keygen

---------------------------------------------------------------------------
BACKGROUND AGENTS
---------------------------------------------------------------------------

When the daemon is running, these services are available:

1. Multi-channel messaging gateway (9 supported platforms)
2. HTTP API for programmatic access (port 8080 by default)
3. WebChat UI at http://localhost:8080/chat
4. Autonomous workflows (scheduled jobs, state machines, policies)
5. Plugin system (memory, search, custom extensions)

Agents run autonomously in the background:
- Low stock monitoring
- Abandoned cart recovery
- Subscription renewals
- Pending return alerts
- Revenue milestone tracking

---------------------------------------------------------------------------
UNINSTALL
---------------------------------------------------------------------------

Remove the daemon (preserves data/config):
  stateset-daemon uninstall [--user]

Remove everything (including data):
  sudo rm -rf /etc/stateset /opt/stateset /var/log/stateset
  # Or user mode:
  rm -rf ~/.config/stateset ~/.local/share/stateset

---------------------------------------------------------------------------
MORE HELP
-----------------------------------------------------------------------

Show all commands:
  stateset-daemon --help

Check service logs with journalctl:
  sudo journalctl -u stateset-gateway -f
  # Or user mode:
  journalctl --user -u stateset-gateway -f

Process details:
  ps aux | grep stateset-gateway

---------------------------------------------------------------------------
