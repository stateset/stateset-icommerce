#!/usr/bin/env node
/**
 * gRPC Streaming Demo
 *
 * Demonstrates real-time bidirectional sync between AI agents and the StateSet sequencer.
 *
 * This example:
 * 1. Connects to the sequencer via gRPC
 * 2. Creates commerce events (orders, inventory)
 * 3. Streams events to the sequencer in real-time
 * 4. Receives acknowledgments and other agents' events
 *
 * Usage:
 *   # Start the sequencer first (in stateset-sequencer directory):
 *   cargo run
 *
 *   # Then run this demo:
 *   node examples/grpc-streaming-demo.js
 *
 *   # With custom sequencer URL:
 *   SEQUENCER_URL=grpc://localhost:8081 node examples/grpc-streaming-demo.js
 */

import crypto from 'crypto';
import fs from 'fs';
import path from 'path';
import os from 'os';
import Database from 'better-sqlite3';
import { createSyncEngine, SyncConfig } from '../src/sync/index.js';
import { AgentKeyManager } from '../src/sync/keys.js';
import { hexToBuffer } from '../src/sync/crypto.js';

// =============================================================================
// CONFIGURATION
// =============================================================================

const SEQUENCER_URL = process.env.SEQUENCER_URL || 'grpc://localhost:8081';
const TENANT_ID = process.env.TENANT_ID || '00000000-0000-0000-0000-000000000001';
const STORE_ID = process.env.STORE_ID || '00000000-0000-0000-0000-000000000002';
const AGENT_ID = process.env.AGENT_ID || crypto.randomUUID();
const API_KEY = process.env.STATESET_API_KEY || null; // No key needed when AUTH_MODE=disabled

console.log('╔════════════════════════════════════════════════════════════════╗');
console.log('║           StateSet gRPC Streaming Demo                         ║');
console.log('╠════════════════════════════════════════════════════════════════╣');
console.log(`║  Sequencer:  ${SEQUENCER_URL.padEnd(47)}║`);
console.log(`║  Tenant ID:  ${TENANT_ID.padEnd(47)}║`);
console.log(`║  Store ID:   ${STORE_ID.padEnd(47)}║`);
console.log(`║  Agent ID:   ${AGENT_ID.padEnd(47)}║`);
console.log('╚════════════════════════════════════════════════════════════════╝');
console.log();

// =============================================================================
// SYNC ENGINE SETUP
// =============================================================================

/**
 * Create a sync configuration
 */
function createConfig() {
  return new SyncConfig({
    sequencer: {
      url: SEQUENCER_URL,
      tls: SEQUENCER_URL.startsWith('grpcs://'),
      insecure: !SEQUENCER_URL.startsWith('grpcs://'),
    },
    identity: {
      tenantId: TENANT_ID,
      storeId: STORE_ID,
      agentId: AGENT_ID,
    },
    auth: {
      apiKey: API_KEY,
    },
    sync: {
      autoSync: false,
      syncIntervalMs: 5000,
      batchSize: 100,
      retryPolicy: {
        maxRetries: 3,
        baseDelay: 1000,
        maxDelay: 10000,
      },
    },
    local: {
      dbPath: ':memory:',
      outboxRetentionDays: 7,
    },
    keys: {
      keysDir: 'keys',
      autoGenerate: true,
      encryptPayloads: false,
    },
  });
}

/**
 * Create sample commerce events
 */
function createSampleEvents() {
  const now = new Date().toISOString();
  const orderId = `ORD-${Date.now()}`;
  const customerId = `CUST-${Math.floor(Math.random() * 10000)}`;

  return [
    // Order created event
    {
      eventId: crypto.randomUUID(),
      commandId: crypto.randomUUID(),
      entityType: 'order',
      entityId: orderId,
      eventType: 'OrderCreated',
      payload: {
        orderId,
        customerId,
        items: [
          { sku: 'WIDGET-001', name: 'Premium Widget', quantity: 2, price: 29.99 },
          { sku: 'GADGET-002', name: 'Smart Gadget', quantity: 1, price: 49.99 },
        ],
        subtotal: 109.97,
        tax: 9.90,
        total: 119.87,
        currency: 'USD',
        status: 'pending',
      },
      baseVersion: 0,
      createdAt: now,
    },
    // Inventory reserved event
    {
      eventId: crypto.randomUUID(),
      commandId: crypto.randomUUID(),
      entityType: 'inventory',
      entityId: 'WIDGET-001',
      eventType: 'InventoryReserved',
      payload: {
        sku: 'WIDGET-001',
        orderId,
        quantity: 2,
        reservationType: 'order',
        expiresAt: new Date(Date.now() + 30 * 60 * 1000).toISOString(),
      },
      baseVersion: 0,
      createdAt: now,
    },
    // Payment initiated event
    {
      eventId: crypto.randomUUID(),
      commandId: crypto.randomUUID(),
      entityType: 'payment',
      entityId: `PAY-${Date.now()}`,
      eventType: 'PaymentInitiated',
      payload: {
        orderId,
        amount: 119.87,
        currency: 'USD',
        method: 'card',
        provider: 'stripe',
      },
      baseVersion: 0,
      createdAt: now,
    },
  ];
}

// =============================================================================
// DEMO EXECUTION
// =============================================================================

async function runDemo() {
  console.log('📦 Setting up sync engine...\n');

  // Create temporary directory for keys
  const tempDir = path.join(os.tmpdir(), `stateset-demo-${Date.now()}`);
  const configDir = path.join(tempDir, '.stateset');
  const keysDir = path.join(configDir, 'keys');
  fs.mkdirSync(keysDir, { recursive: true });
  console.log(`🔑 Using temp directory: ${tempDir}\n`);

  // Create in-memory database
  const db = new Database(':memory:');

  // Create key manager and generate keys
  const keyManager = new AgentKeyManager(configDir);
  console.log('🔐 Generating agent signing keys...');
  const signingKey = await keyManager.generateSigningKey(AGENT_ID);
  console.log(`   Key ID: ${signingKey.keyId}`);
  console.log(`   Public Key: ${signingKey.publicKey.slice(0, 16)}...`);
  console.log();

  // Create sync configuration
  const config = createConfig();

  // Create sync engine with gRPC enabled
  const engine = createSyncEngine({
    db,
    config,
    preferGrpc: true,
    enableStreaming: true,
    configDir,
    keyManager,
  });

  // Set up event handlers
  engine.on('connected', (info) => {
    console.log(`✅ Connected via ${info.transport.toUpperCase()}`);
  });

  engine.on('error', (err) => {
    console.error(`❌ Error: ${err.message}`);
  });

  engine.on('event', (event) => {
    console.log(`📥 Received event: ${event.eventType} (seq: ${event.sequenceNumber})`);
  });

  engine.on('push:ack', (ack) => {
    console.log(`✅ Push acknowledged: ${ack.events_accepted || ack.eventsAccepted} events accepted`);
  });

  engine.on('streamingStarted', (info) => {
    console.log(`🔄 Streaming started (mode: ${info.mode})`);
  });

  engine.on('streamingStopped', () => {
    console.log('⏹️  Streaming stopped');
  });

  try {
    // Initialize the engine
    console.log('🔌 Connecting to sequencer...');
    await engine.initialize();

    // Check capabilities
    const capabilities = engine.getCapabilities();
    console.log('\n📊 Engine Capabilities:');
    console.log(`   Transport: ${capabilities.transport || 'not connected'}`);
    console.log(`   Streaming: ${capabilities.streaming ? 'yes' : 'no'}`);
    console.log(`   Bidirectional Sync: ${capabilities.bidirectionalSync ? 'yes' : 'no'}`);
    console.log();

    // Check connection status
    const status = await engine.getStatus();
    console.log('📈 Sync Status:');
    console.log(`   Connected: ${status.connected}`);
    console.log(`   Transport: ${status.transport || 'none'}`);
    console.log(`   Remote Head: ${status.remoteHead || 0}`);
    console.log(`   Pending: ${status.pending}`);
    console.log();

    if (!status.connected) {
      console.log('⚠️  Not connected to sequencer. Demonstrating offline mode...\n');
    }

    // Start streaming if gRPC is available
    if (engine.supportsStreaming() && status.connected) {
      console.log('🚀 Starting bidirectional streaming sync...');
      const streamStarted = engine.startStreamingSync({ bidirectional: true });

      if (streamStarted) {
        console.log('✅ Stream started successfully\n');

        // Give stream time to initialize
        await sleep(500);
      }
    }

    // Create sample events
    console.log('📝 Creating sample commerce events...\n');
    const events = createSampleEvents();

    for (const event of events) {
      console.log(`   → ${event.eventType} for ${event.entityType}:${event.entityId}`);

      // Add event to outbox
      await engine.outbox.append({
        ...event,
        tenantId: TENANT_ID,
        storeId: STORE_ID,
        sourceAgent: AGENT_ID,
        vesVersion: 1,
        payloadKind: 0,
        agentKeyId: 0,
        agentSignature: '',
        payloadPlainHash: crypto.createHash('sha256')
          .update(JSON.stringify(event.payload))
          .digest('hex'),
        payloadCipherHash: '0'.repeat(64),
      });
    }

    console.log(`\n📤 Added ${events.length} events to outbox`);

    // Get outbox stats
    const stats = engine.outbox.getStats();
    console.log(`   Pending: ${stats.pending}, Synced: ${stats.synced}\n`);

    // Push events
    if (status.connected) {
      if (engine.isStreaming()) {
        console.log('🔄 Pushing events via bidirectional stream...');

        // Get pending events
        const pending = engine.outbox.getPending(100);
        const streamEvents = pending.map(e => ({
          eventId: e.eventId,
          commandId: e.commandId,
          entityType: e.entityType,
          entityId: e.entityId,
          eventType: e.eventType,
          payload: e.payload,
          baseVersion: e.baseVersion,
          createdAt: e.createdAt,
          // VES v1.0 fields (handles 0x prefix via hexToBuffer)
          payloadHash: e.payloadPlainHash ? hexToBuffer(e.payloadPlainHash) : null,
          payloadCipherHash: e.payloadCipherHash ? hexToBuffer(e.payloadCipherHash) : null,
          agentKeyId: e.agentKeyId,
          signature: e.agentSignature ? hexToBuffer(e.agentSignature) : null,
        }));

        // Wait for acknowledgment with timeout
        const ackPromise = new Promise((resolve, reject) => {
          const timeout = setTimeout(() => {
            reject(new Error('Push acknowledgment timeout'));
          }, 5000);

          engine.once('push:ack', (ack) => {
            clearTimeout(timeout);
            resolve(ack);
          });
        });

        engine.pushViaStream(streamEvents);
        console.log('✅ Events sent via stream');

        try {
          const ack = await ackPromise;
          const accepted = ack.events_accepted ?? ack.eventsAccepted ?? 0;
          const rejected = ack.events_rejected ?? ack.eventsRejected ?? 0;
          console.log(`✅ Received acknowledgment: ${accepted} accepted, ${rejected} rejected`);
          console.log(`   Batch ID: ${ack.batch_id || ack.batchId}`);
          console.log(`   Sequence range: ${ack.sequence_start || ack.sequenceStart}-${ack.sequence_end || ack.sequenceEnd}`);
          console.log(`   Head sequence: ${ack.head_sequence || ack.headSequence}`);
          if (ack.rejections && ack.rejections.length > 0) {
            console.log('   Rejections:');
            for (const r of ack.rejections) {
              console.log(`     - ${r.event_id || r.eventId}: ${r.reason} - ${r.message}`);
            }
          }
        } catch (e) {
          console.log(`⚠️  ${e.message} - events may still be processing`);
        }

        // Brief pause for stream to settle
        await sleep(500);
      } else {
        console.log('📤 Pushing events via standard push...');
        const pushResult = await engine.push();
        console.log(`   Result: ${pushResult.success ? 'success' : 'failed'}`);
        console.log(`   Pushed: ${pushResult.pushed}, Rejected: ${pushResult.rejected}`);
      }
    } else {
      console.log('📥 Events queued for later sync (offline mode)');
    }

    // Pull any remote events
    if (status.connected && !engine.isStreaming()) {
      console.log('\n📥 Pulling remote events...');
      const pullResult = await engine.pull();
      console.log(`   Pulled: ${pullResult.pulled}, Applied: ${pullResult.applied}`);
    }

    // Show final status
    console.log('\n📊 Final Status:');
    const finalStatus = await engine.getStatus();
    console.log(`   Connected: ${finalStatus.connected}`);
    console.log(`   Streaming: ${finalStatus.streaming}`);
    console.log(`   Pending: ${finalStatus.pending}`);
    console.log(`   Buffered Events: ${finalStatus.bufferedEvents}`);

    // Get recent events from buffer
    const recentEvents = engine.getRecentEvents(10);
    if (recentEvents.length > 0) {
      console.log(`\n📜 Recent Events (${recentEvents.length}):`);
      for (const e of recentEvents) {
        console.log(`   ${e.sequenceNumber}: ${e.eventType} (${e.entityType}:${e.entityId})`);
      }
    }

    // Clean up
    console.log('\n🧹 Cleaning up...');
    if (engine.isStreaming()) {
      engine.stopStreamingSync();
    }
    await engine.shutdown();
    db.close();

    // Remove temp directory
    fs.rmSync(tempDir, { recursive: true, force: true });

    console.log('✅ Demo complete!\n');

  } catch (error) {
    console.error('\n❌ Demo failed:', error.message);
    console.error(error.stack);
    process.exit(1);
  }
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

// Run the demo
runDemo().catch(console.error);
