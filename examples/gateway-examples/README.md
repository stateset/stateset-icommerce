# StateSet iCommerce Gateway Examples

This directory contains practical examples of how to use the StateSet iCommerce gateway system.

## Overview

The iCommerce gateway provides a **multi-channel messaging interface** that connects AI commerce agents to various communication platforms including Slack, Discord, Telegram, WhatsApp, Microsoft Teams, Matrix, iMessage, Google Chat, and HTTP/WebChat.

## Available Examples

| Example | Description | Difficulty |
|---------|-------------|------------|
| **1-multi-channel.js** | Launch multiple gateways simultaneously with a single config | Beginner |
| **2-http-gateway.js** | Use the HTTP gateway for API integrations | Beginner |
| **3-rich-messages.js** | Send rich, interactive cards with buttons and actions | Intermediate |
| **4-notifications.js** | Send proactive notifications to channels | Intermediate |
| **5-plugins.js** | Extend gateways with custom plugins | Advanced |

## Quick Start

### 1. Single Channel (Slack)

```bash
export SLACK_BOT_TOKEN='xoxb-...'
export SLACK_APP_TOKEN='xapp-...'

node 1-multi-channel.js
```

### 2. HTTP Gateway

```bash
# Start the gateway on port 3000
node 2-http-gateway.js

# In another terminal, make requests:
curl http://localhost:3000/api/messages \
  -H "Content-Type: application/json" \
  -d '{"text": "Show my orders"}'
```

### 3. WebChat UI

```bash
# Start the gateway with webchat
node 1-multi-channel.js --config webchat-only.config.json

# Open in browser:
open http://localhost:3000/chat
```

## Configuration File Format

```json
{
  "shared": {
    "dbPath": "./store.db",
    "allowApply": false,
    "model": "gpt-4"
  },
  "channels": {
    "slack": {
      "enabled": true
    },
    "discord": {
      "enabled": true,
      "allowlist": ["123456789012345678"]
    },
    "webchat": {
      "enabled": true
    }
  },
  "httpGateway": {
    "enabled": true,
    "port": 3000,
    "apiKeys": [
      { "key": "sk_live_demo", "permissions": ["read", "write"] }
    ]
  },
  "notifications": {
    "routes": {
      "order_placed": ["slack"],
      "payment_failed": ["discord"]
    }
  }
}
```

## API Endpoints (HTTP Gateway)

### Send a message
```bash
POST /api/messages
Content-Type: application/json
Authorization: Bearer sk_live_...

{
  "text": "Show my orders",
  "sessionId": "user-123"
}
```

### Get session info
```bash
GET /api/sessions/:id
```

### Send notification
```bash
POST /api/notifications
Content-Type: application/json
Authorization: Bearer sk_live_...

{
  "channel": "slack",
  "target": "user-id-123",
  "message": "Your order has shipped!"
}
```

## Authentication

The HTTP gateway supports API key authentication:

```javascript
// In your gateway config:
{
  "httpGateway": {
    "apiKeys": [
      {
        "key": "sk_live_...",
        "permissions": ["read", "write"],
        "description": "Production API key"
      },
      {
        "key": "sk_test_...",
        "permissions": ["read"],
        "description": "Read-only test key"
      }
    ]
  }
}
```

Usage:
```bash
# Bearer token header
Authorization: Bearer sk_live_...

# Or query parameter
/api/messages?apiKey=sk_live_...
```

## Rich Messages

Send interactive cards with buttons:

```javascript
// Rich message format
{
  "title": "Order #1234",
  "description": "Your order is ready for review",
  "fields": [
    { "name": "Total", "value": "$99.99" },
    { "name": "Status", "value": "Pending" }
  ],
  "buttons": [
    { "label": "View Order", "action": "/order 1234" },
    { "label": "Support", "url": "https://support.example.com" }
  ]
}
```

## Supported Channels

| Channel | Requirements | Bot Driver |
|---------|--------------|------------|
| **Slack** | `SLACK_BOT_TOKEN`, `SLACK_APP_TOKEN` | Socket Mode |
| **Discord** | `DISCORD_TOKEN` | Bot API |
| **Telegram** | `TELEGRAM_BOT_TOKEN` | Bot API |
| **WhatsApp** | `WHATSAPP_TOKEN` | Cloud API |
| **Signal** | Signal CLI + bridge | Custom |
| **Google Chat** | OAuth credentials | Bot API |
| **Microsoft Teams** | `TEAMS_APP_ID`, `TEAMS_APP_PASSWORD` | Bot Framework |
| **Matrix** | Matrix homeserver URL + access token | Matrix Client |
| **iMessage** | BlueBubbles server | Custom bridge |
| **WebChat** | None (HTTP-based) | Built-in |

## Features

### Persistent Sessions
Sessions are stored in SQLite and survive gateway restarts:
```json
{
  "sessionDbPath": "./sessions.db",
  "persistSessions": true
}
```

### Middleware
Add rate limiting, content filtering, logging, etc:
```json
{
  "middleware": {
    "rateLimiter": {
      "maxRequests": 100,
      "windowMs": 60000
    },
    "contentFilter": {
      "profanity": true,
      "spam": true
    },
    "logger": true,
    "languageDetect": true
  }
}
```

### Multi-Agent Support
Route different channels to different agents:
```json
{
  "channels": {
    "slack": {
      "agent": "orders"
    },
    "discord": {
      "agent": "support"
    }
  }
}
```

### Identity Linking
Link user identities across channels:
```json
{
  "identityDbPath": "./identities.db"
}
```

## Running the Examples

```bash
# Make sure you're in the examples/gateway-examples directory
cd examples/gateway-examples

# Install dependencies (if needed)
npm install axios

# Make examples executable
chmod +x *.js

# Run an example
node 1-multi-channel.js
```

## Troubleshooting

### Slack: "Invalid auth" error
- Ensure `SLACK_BOT_TOKEN` starts with `xoxb-`
- Ensure `SLACK_APP_TOKEN` starts with `xapp-`
- Check Socket Mode is enabled in your Slack app

### Discord: "Privileged Intents" required
- Enable "Server Members Intent" and "Message Content Intent" in Discord Developer Portal

### HTTP Gateway: Port already in use
- Change the port in config or use `httpGateway.port` setting

### "Database locked" error
- Ensure only one gateway instance is running per database
- Check file permissions on SQLite DB files

## Next Steps

- Read the full documentation in `../../README.md`
- Check out multi-agent examples in `../multi-agent/`
- Explore autonomous workflows in `../../cli/src/autonomous/`
- Review the source code in `../../cli/src/channels/`

## License

These examples are part of StateSet iCommerce. See the main project LICENSE file.