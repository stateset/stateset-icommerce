#!/usr/bin/env node

import { ChannelOrchestrator } from 'cli/src/channels/orchestrator.js';

const orchestrator = new ChannelOrchestrator({
  channels: {
    slack: {
      enabled: true
    },
    discord: {
      enabled: true
    },
    telegram: {
      enabled: true
    }
  },
  shared: {
    dbPath: './store.db',
    allowApply: false,
    model: 'gpt-4',
    maxTurns: 10,
    verbose: true
  },
  httpGateway: {
    enabled: true,
    port: 3000,
    apiKeys: [
      { key: 'sk_demo_key', permissions: ['read', 'write'] }
    ]
  },
  notifications: {
    routes: [
      { channel: 'slack', priority: 1, condition: { customer_tier: 'vip' } },
      { channel: 'email', priority: 2 }
    ]
  },
  persistSessions: true,
  middleware: {
    logger: true,
    rateLimiter: {
      windowMs: 60000,
      maxRequests: 100
    }
  }
});

console.log('Starting multi-channel gateway...\n');

orchestrator.start()
  .then(result => {
    console.log('Started channels:', result.started);
    if (result.failed.length > 0) {
      console.log('Failed channels:', result.failed);
    }
    console.log('\nGateway is running. Press Ctrl+C to stop.');
  })
  .catch(err => {
    console.error('Failed to start:', err.message);
    process.exit(1);
  });

process.on('SIGINT', async () => {
  console.log('\n\nShutting down gateway...');
  await orchestrator.shutdown();
  process.exit(0);
});