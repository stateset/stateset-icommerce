#!/usr/bin/env node
/**
 * Mock gRPC Sequencer Server
 *
 * A simple mock server for testing the gRPC client without a full PostgreSQL setup.
 *
 * Usage:
 *   node examples/mock-grpc-server.js
 *
 * The server listens on port 8081 and simulates the StateSet Sequencer.
 */

import grpc from '@grpc/grpc-js';
import protoLoader from '@grpc/proto-loader';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROTO_PATH = path.join(__dirname, '..', 'src', 'sync', 'proto', 'sequencer_v2.proto');

// In-memory event store
const events = [];
let sequenceCounter = 0;

// Load proto
const packageDefinition = protoLoader.loadSync(PROTO_PATH, {
  keepCase: true,
  longs: String,
  enums: String,
  defaults: true,
  oneofs: true,
});
const proto = grpc.loadPackageDefinition(packageDefinition);
const sequencerProto = proto.stateset.sequencer.v2;

// ===========================================================================
// SEQUENCER SERVICE IMPLEMENTATION
// ===========================================================================

const sequencerService = {
  // Push events
  push(call, callback) {
    const request = call.request;
    console.log(`📥 Push: ${request.events.length} events from agent ${request.agent_id}`);

    const accepted = [];
    const rejections = [];
    const startSeq = sequenceCounter + 1;

    for (const event of request.events) {
      sequenceCounter++;
      const sequencedEvent = {
        envelope: event,
        sequence_number: sequenceCounter,
        sequenced_at: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
      };
      events.push(sequencedEvent);
      accepted.push(event.event_id);
      console.log(`   ✓ Event ${event.event_id.slice(0, 8)}... → seq ${sequenceCounter}`);
    }

    callback(null, {
      batch_id: `batch-${Date.now()}`,
      events_accepted: accepted.length,
      events_rejected: rejections.length,
      sequence_start: startSeq,
      sequence_end: sequenceCounter,
      head_sequence: sequenceCounter,
      rejections,
      head_state_root: Buffer.alloc(32),
    });
  },

  // Pull events
  pullEvents(call, callback) {
    const request = call.request;
    const fromSeq = Number(request.from_sequence) || 0;
    const limit = Number(request.limit) || 100;

    console.log(`📤 Pull: from seq ${fromSeq}, limit ${limit}`);

    const filtered = events.filter((e) => Number(e.sequence_number) > fromSeq).slice(0, limit);

    callback(null, {
      events: filtered,
      next_sequence: filtered.length > 0 ? Number(filtered[filtered.length - 1].sequence_number) + 1 : fromSeq,
      has_more: filtered.length === limit,
      head_sequence: sequenceCounter,
    });
  },

  // Get sync state
  getSyncState(call, callback) {
    console.log('📊 GetSyncState');
    callback(null, {
      tenant_id: call.request.tenant_id,
      store_id: call.request.store_id,
      head_sequence: sequenceCounter,
      state_root: Buffer.alloc(32),
      timestamp: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
    });
  },

  // Get health
  getHealth(call, callback) {
    callback(null, {
      healthy: true,
      version: 'mock-1.0.0',
      timestamp: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
    });
  },

  // Stream events (server-side streaming)
  streamEvents(call) {
    const request = call.request;
    console.log(`🔄 StreamEvents: from seq ${request.from_sequence}`);

    // Send existing events
    const fromSeq = Number(request.from_sequence) || 0;
    for (const event of events) {
      if (Number(event.sequence_number) > fromSeq) {
        call.write(event);
      }
    }

    // Keep stream open for new events (simplified - just keep alive)
    const interval = setInterval(() => {
      // Heartbeat - in real impl would send new events
    }, 10000);

    call.on('cancelled', () => {
      console.log('🔄 StreamEvents cancelled');
      clearInterval(interval);
    });
  },

  // Bidirectional sync stream
  syncStream(call) {
    console.log('🔄 SyncStream started');

    call.on('data', (message) => {
      if (message.push) {
        // Handle push via stream
        const pushReq = message.push;
        console.log(`📥 Stream Push: ${pushReq.events.length} events`);

        const startSeq = sequenceCounter + 1;
        for (const event of pushReq.events) {
          sequenceCounter++;
          events.push({
            envelope: event,
            sequence_number: sequenceCounter,
            sequenced_at: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
          });
        }

        call.write({
          push_response: {
            batch_id: `batch-${Date.now()}`,
            events_accepted: pushReq.events.length,
            events_rejected: 0,
            sequence_start: startSeq,
            sequence_end: sequenceCounter,
            head_sequence: sequenceCounter,
            rejections: [],
          },
        });
      } else if (message.pull) {
        // Handle pull via stream
        const pullReq = message.pull;
        const fromSeq = Number(pullReq.from_sequence) || 0;
        const limit = Number(pullReq.limit) || 100;

        const filtered = events.filter((e) => Number(e.sequence_number) > fromSeq).slice(0, limit);

        call.write({
          pull_response: {
            events: filtered,
            next_sequence: filtered.length > 0 ? Number(filtered[filtered.length - 1].sequence_number) + 1 : fromSeq,
            has_more: filtered.length === limit,
            head_sequence: sequenceCounter,
          },
        });
      } else if (message.heartbeat) {
        call.write({
          server_heartbeat: {
            timestamp: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
            head_sequence: sequenceCounter,
          },
        });
      } else if (message.ack) {
        console.log(`✓ ACK: sequences ${message.ack.sequence_numbers?.join(', ')}`);
      }
    });

    call.on('end', () => {
      console.log('🔄 SyncStream ended');
      call.end();
    });

    call.on('error', (err) => {
      console.error('🔄 SyncStream error:', err.message);
    });
  },

  // Subscribe to entity
  subscribeEntity(call) {
    const request = call.request;
    console.log(`📌 SubscribeEntity: ${request.entity_type}:${request.entity_id}`);

    // Send matching events
    for (const event of events) {
      if (
        event.envelope.entity_type === request.entity_type &&
        event.envelope.entity_id === request.entity_id
      ) {
        call.write(event);
      }
    }

    // Keep stream open
    call.on('cancelled', () => {
      console.log('📌 SubscribeEntity cancelled');
    });
  },

  // Get entity history
  getEntityHistory(call, callback) {
    const request = call.request;
    const filtered = events.filter(
      (e) =>
        e.envelope.entity_type === request.entity_type &&
        e.envelope.entity_id === request.entity_id
    );

    callback(null, {
      events: filtered,
      current_version: filtered.length,
    });
  },

  // Get inclusion proof (mock)
  getInclusionProof(call, callback) {
    callback(null, {
      included: true,
      proof: {
        merkle_root: Buffer.alloc(32),
        leaf_index: 0,
        proof_hashes: [],
        leaf_count: events.length,
      },
    });
  },

  // Get commitment (mock)
  getCommitment(call, callback) {
    callback(null, {
      batch_id: 'mock-batch',
      merkle_root: Buffer.alloc(32),
      start_sequence: 1,
      end_sequence: sequenceCounter,
      event_count: events.length,
      committed_at: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
    });
  },
};

// ===========================================================================
// KEY MANAGEMENT SERVICE IMPLEMENTATION
// ===========================================================================

const agentKeys = new Map();

const keyManagementService = {
  registerAgentKey(call, callback) {
    const request = call.request;
    const key = `${request.agent_id}:${request.key_id}`;
    agentKeys.set(key, {
      ...request,
      status: 1, // ACTIVE
      created_at: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
    });
    console.log(`🔑 RegisterAgentKey: ${request.agent_id} key ${request.key_id}`);
    callback(null, {
      success: true,
      message: 'Key registered',
      registered_at: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
    });
  },

  getAgentKeys(call, callback) {
    const request = call.request;
    const keys = [];
    for (const [k, v] of agentKeys) {
      if (k.startsWith(request.agent_id + ':')) {
        keys.push(v);
      }
    }
    callback(null, { keys });
  },

  revokeAgentKey(call, callback) {
    const request = call.request;
    const key = `${request.agent_id}:${request.key_id}`;
    if (agentKeys.has(key)) {
      agentKeys.get(key).status = 2; // REVOKED
    }
    callback(null, {
      success: true,
      revoked_at: { seconds: Math.floor(Date.now() / 1000), nanos: 0 },
    });
  },
};

// ===========================================================================
// START SERVER
// ===========================================================================

function main() {
  const server = new grpc.Server();

  server.addService(sequencerProto.Sequencer.service, sequencerService);
  server.addService(sequencerProto.KeyManagement.service, keyManagementService);

  const port = process.env.GRPC_PORT || 8081;

  server.bindAsync(`0.0.0.0:${port}`, grpc.ServerCredentials.createInsecure(), (err, boundPort) => {
    if (err) {
      console.error('Failed to start server:', err);
      process.exit(1);
    }

    console.log('╔════════════════════════════════════════════════════════════════╗');
    console.log('║           StateSet Mock gRPC Sequencer                         ║');
    console.log('╠════════════════════════════════════════════════════════════════╣');
    console.log(`║  Listening on:  0.0.0.0:${String(boundPort).padEnd(39)}║`);
    console.log('║  Services:      Sequencer, KeyManagement                       ║');
    console.log('╚════════════════════════════════════════════════════════════════╝');
    console.log('\n📡 Ready to receive events...\n');
  });
}

main();
