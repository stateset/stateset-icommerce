#!/usr/bin/env node

/**
 * Example 3: HTTP Gateway API Integration
 * 
 * This example shows how to use the HTTP gateway to integrate iCommerce
 * with any application that can make HTTP requests (web apps, mobile apps,
 * microservices, etc.)
 * 
 * The HTTP gateway provides:
 * - REST API for sending messages and managing sessions
 * - API key authentication
 * - Per-route permission levels
 * - Sandbox mode for testing
 * - Webchat UI embedding
 */

import { spawn } from 'child_process';

// ANSI color codes for terminal output
const colors = {
  reset: '\x1b[0m',
  green: '\x1b[32m',
  blue: '\x1b[34m',
  yellow: '\x1b[33m',
  cyan: '\x1b[36m',
  red: '\x1b[31m'
};

function log(color, msg) {
  console.log(`${colors[color]}${msg}${colors.reset}`);
}

async function runCommand(cmd, description) {
  log('cyan', `\n${description}`);
  log('yellow', `Running: ${cmd}\n`);
  
  const proc = spawn('npm', cmd.split(' '), {
    stdio: 'inherit',
    shell: true
  });

  return new Promise((resolve) => {
    proc.on('close', (code) => {
      if (code === 0) {
        log('green', `✓ Success\n`);
        resolve(true);
      } else {
        log('red', `✗ Failed (exit code ${code})\n`);
        resolve(false);
      }
    });
  });
}

async function main() {
  console.log(`
╔═════════════════════════════════════════════════════════╗
║   StateSet iCommerce - HTTP Gateway API Examples       ║
╚═════════════════════════════════════════════════════════╝

The HTTP gateway lets you integrate iCommerce with any
application that can make HTTP requests.

Supported Channels:
  • WebChat           - Self-contained chat UI (/chat)
  • API               - REST endpoints for programmatic access
  • Webhooks          - Receive notifications from iCommerce
  • CORS Proxy        - Enable browser-based integration

`);

  // Check if dependencies are installed
  log('blue', 'Checking installation...');
  const installed = await runCommand('list @stateset/cli', 'Checking if @stateset/cli is installed');
  
  if (!installed) {
    log('yellow', 'Installing @stateset/cli...');
    await runCommand('install -g @stateset/cli', 'Installing CLI globally');
  }

  console.log(`
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  SECTION 1: Starting the HTTP Gateway
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

The HTTP gateway starts automatically when you launch the
gateway daemon. It listens on a configurable port (default: 3000).

`);

  await runCommand('start', 'Starting iCommerce gateway daemon');

  console.log(`
Gateway now running! Access it at:
  → Dashboard:  http://localhost:3000/
  → WebChat:    http://localhost:3000/chat
  → API Docs:   http://localhost:3000/api

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  SECTION 2: API Examples
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

The HTTP gateway provides REST API endpoints for programmatic
access to the commerce engine.

------------------
  Example 2.1: Send a message via HTTP API
------------------

This curl command sends a message to the agent:

curl -X POST http://localhost:3000/api/messages \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer YOUR_API_KEY" \\
  -d '{
    "text": "What products do you have in stock?",
    "sessionId": "user-123"
  }'

Response:
{
  "response": "I found 42 products in stock. Here are the top 5:...",
  "sessionId": "user-123",
  "agent": "commerce"
}

------------------
  Example 2.2: List active sessions
------------------

curl -X GET http://localhost:3000/api/sessions \\
  -H "Authorization: Bearer YOUR_API_KEY"

Response:
{
  "sessions": [
    { "id": "user-123", "channel": "http", "lastActive": "2026-01-29T20:30:00Z" },
    { "id": "user-456", "channel": "webchat", "lastActive": "2026-01-29T20:28:00Z" }
  ]
}

------------------
  Example 2.3: Get session history
------------------

curl -X GET http://localhost:3000/api/sessions/user-123/history \\
  -H "Authorization: Bearer YOUR_API_KEY"

Response:
{
  "messages": [
    { "role": "user", "text": "What products do you have?", "timestamp": "..." },
    { "role": "agent", "text": "I found 42 products...", "timestamp": "..." }
  ]
}

------------------
  Example 2.4: Send a notification to a channel
------------------

curl -X POST http://localhost:3000/api/notify \\
  -H "Authorization: Bearer YOUR_API_KEY" \\
  -d '{
    "channel": "slack",
    "recipient": "U1234567890",
    "message": "Your order #1234 has shipped!"
  }'

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  SECTION 3: WebChat Integration
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

The WebChat channel provides a ready-to-use chat UI that you can
embed in your website or application.

------------------
  Example 3.1: Embed WebChat in your website
------------------

Simply add an iframe to your HTML:

<iframe 
  src="http://localhost:3000/chat"
  width="100%" 
  height="500px"
  frameborder="0"
  title="StateSet Commerce Chat">
</iframe>

------------------
  Example 3.2: Custom WebChat with JavaScript
------------------

<script>
async function sendMessage(text) {
  const response = await fetch('/chat/message', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ 
      text: text,
      sessionId: 'my-custom-session-id'
    })
  });
  return await response.json();
}

// Example usage:
sendMessage('Show me my orders').then(data => {
  console.log(data.response);
});
</script>

------------------
  Example 3.3: Retrieve conversation history
------------------

<script>
async function loadHistory(sessionId) {
  const response = await fetch(\`/chat/history/\${sessionId}\`);
  const data = await response.json();
  return data.messages;
}

// Load on page load:
const sessionId = localStorage.getItem('ss_chat_session');
if (sessionId) {
  loadHistory(sessionId).then(messages => {
    messages.forEach(msg => displayMessage(msg.role, msg.text));
  });
}
</script>

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  SECTION 4: Authentication & Security
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

The HTTP gateway supports API key authentication and
per-route permission levels.

------------------
  Example 4.1: Configure API keys
------------------

Create a gateway.config.json file:

{
  "httpGateway": {
    "port": 3000,
    "apiKeys": [
      {
        "key": "sk_live_1234567890abcdef",
        "permissions": ["read", "write", "admin"]
      },
      {
        "key": "sk_test_0987654321fedcba",
        "permissions": ["read", "write"]
      }
    ]
  }
}

Start the gateway with config:

npm start -- --config gateway.config.json

------------------
  Example 4.2: Use API key in requests
------------------

# Bearer token header
curl -H "Authorization: Bearer sk_live_1234567890abcdef" \\
  http://localhost:3000/api/sessions

# Or query parameter
curl "http://localhost:3000/api/sessions?apiKey=sk_live_1234567890abcdef"

------------------
  Example 4.3: Sandbox mode for testing
------------------

{
  "httpGateway": {
    "sandbox": {
      "enabled": true,
      "variation": "demo"
    }
  }
}

In sandbox mode, all changes are mocked and no actual 
database modifications occur.

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  SECTION 5: Integration Examples
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

------------------
  Example 5.1: Node.js Integration
------------------

const axios = require('axios');

class CommerceClient {
  constructor(baseUrl, apiKey) {
    this.client = axios.create({
      baseURL: baseUrl,
      headers: { 'Authorization': `Bearer ${apiKey}` }
    });
  }

  async sendMessage(text, sessionId) {
    const { data } = await this.client.post('/api/messages', {
      text,
      sessionId
    });
    return data;
  }

  async getOrders(customerId) {
    const { data } = await this.client.post('/api/messages', {
      text: `List orders for customer ${customerId}`,
      sessionId: 'admin'
    });
    return data;
  }
}

// Usage:
const client = new CommerceClient('http://localhost:3000', 'sk_live_...');
const response = await client.sendMessage('Show my orders', 'user-123');

------------------
  Example 5.2: Python Integration
------------------

import requests

class CommerceAPI:
    def __init__(self, base_url, api_key):
        self.base_url = base_url
        self.headers = {'Authorization': f'Bearer {api_key}'}
    
    def send_message(self, text, session_id=None):
        response = requests.post(
            f'{self.base_url}/api/messages',
            json={'text': text, 'sessionId': session_id},
            headers=self.headers
        )
        return response.json()

# Usage:
api = CommerceAPI('http://localhost:3000', 'sk_live_...')
response = api.send_message('Show my orders', 'user-123')

------------------
  Example 5.3: React Component Integration
------------------

import React, { useState, useEffect } from 'react';

function CommerceChat() {
  const [messages, setMessages] = useState([]);
  const [input, setInput] = useState('');
  const [sessionId, setSessionId] = useState('');

  const sendMessage = async () => {
    const response = await fetch('/api/messages', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ text: input, sessionId })
    });
    const data = await response.json();
    setMessages([...messages, 
      { role: 'user', text: input },
      { role: 'agent', text: data.response }
    ]);
    setSessionId(data.sessionId);
    setInput('');
  };

  return (
    <div className="chat">
      <div className="messages">
        {messages.map((msg, i) => (
          <div key={i} className={msg.role}>{msg.text}</div>
        ))}
      </div>
      <input 
        value={input} 
        onChange={e => setInput(e.target.value)}
        placeholder="Type a message..."
      />
      <button onClick={sendMessage}>Send</button>
    </div>
  );
}

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  SECTION 6: Advanced Features
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

------------------
  Example 6.1: Webhook notifications
------------------

Configure notification routes in gateway.config.json:

{
  "notifications": {
    "routes": [
      {
        "event": "order.created",
        "channel": "slack",
        "recipients": ["U123456", "U789012"],
        "template": "New order created: {orderId}"
      },
      {
        "event": "inventory.low",
        "channel": "email",
        "recipients": ["admin@company.com"],
        "template": "Low stock alert: {product}"
      }
    ]
  }
}

------------------
  Example 6.2: Custom middleware
------------------

Add custom processing to incoming requests:

{
  "middleware": {
    "rateLimiter": {
      "windowMs": 60000,
      "maxRequests": 100
    },
    "contentFilter": {
      "enabled": true,
      "blockPatterns": ["spam", "abuse"]
    },
    "languageDetect": true
  }
}

------------------
  Example 6.3: CORS configuration
------------------

Enable cross-origin requests from your frontend:

{
  "httpGateway": {
    "cors": {
      "origin": "https://your-app.com",
      "methods": ["GET", "POST"],
      "credentials": true
    }
  }
}

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Next Steps
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

1. Try the live WebChat at http://localhost:3000/chat
2. Review API documentation at http://localhost:3000/api/docs
3. Configure your gateway settings in gateway.config.json
4. Integrate with your application using the examples above
5. Explore plugin system for custom functionality

For more information:
  • Documentation: https://docs.stateset.com/gateway
  • API Reference: https://docs.stateset.com/api
  • Configuration: https://docs.stateset.com/config

`);
}

main().catch(err => {
  log('red', `Error: ${err.message}`);
  process.exit(1);
});