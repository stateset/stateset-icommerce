/**
 * Unified Sequencer Client Factory
 *
 * Automatically selects between gRPC and REST clients based on configuration.
 * Provides a consistent interface for both transport types.
 */

import { EventEmitter } from 'events';
import { SequencerClient } from './client.js';

// Lazy-load gRPC client to make it optional
let GrpcSequencerClient = null;
const ALLOWED_SEQUENCER_PROTOCOLS = new Set(['grpc:', 'grpcs:', 'http:', 'https:']);

function parseSequencerUrl(url) {
  if (typeof url !== 'string' || !url.trim()) {
    throw new Error('Sequencer URL must be a non-empty string');
  }

  const parsed = new URL(url.trim());
  if (!ALLOWED_SEQUENCER_PROTOCOLS.has(parsed.protocol)) {
    throw new Error(`Unsupported sequencer protocol: ${parsed.protocol}`);
  }
  if (!parsed.hostname) {
    throw new Error('Sequencer URL must include a host');
  }
  return parsed;
}

/**
 * Check if gRPC is available
 * @returns {Promise<boolean>}
 */
async function isGrpcAvailable() {
  try {
    await import('@grpc/grpc-js');
    await import('@grpc/proto-loader');
    return true;
  } catch (err) {
    console.debug('[unified-client] gRPC availability check failed:', err.message || err);
    return false;
  }
}

/**
 * Load gRPC client module
 * @returns {Promise<typeof GrpcSequencerClient>}
 */
async function loadGrpcClient() {
  if (!GrpcSequencerClient) {
    const module = await import('./grpc-client.js');
    GrpcSequencerClient = module.GrpcSequencerClient;
  }
  return GrpcSequencerClient;
}

/**
 * Determine transport type from URL
 * @param {string} url
 * @returns {'grpc' | 'rest'}
 */
function getTransportType(url) {
  const parsed = parseSequencerUrl(url);
  if (parsed.protocol === 'grpc:' || parsed.protocol === 'grpcs:') {
    return 'grpc';
  }
  return 'rest';
}

/**
 * @typedef {Object} UnifiedClientOptions
 * @property {import('./config.js').SyncConfig} config - Sync configuration
 * @property {boolean} [preferGrpc=true] - Prefer gRPC when available
 * @property {boolean} [enableStreaming=true] - Enable streaming features (gRPC only)
 */

/**
 * Unified client that wraps either gRPC or REST client.
 * Extends EventEmitter for streaming event notifications.
 */
export class UnifiedSequencerClient extends EventEmitter {
  /**
   * @param {UnifiedClientOptions} options
   */
  constructor(options) {
    super();
    this.config = options.config;
    this.preferGrpc = options.preferGrpc !== false;
    this.enableStreaming = options.enableStreaming !== false;
    this._client = null;
    this._transport = null;
    this._grpcAvailable = null;
    this._streamActive = false;
  }

  /**
   * Get the underlying transport type
   * @returns {'grpc' | 'rest' | null}
   */
  get transport() {
    return this._transport;
  }

  /**
   * Check if streaming is supported
   * @returns {boolean}
   */
  get supportsStreaming() {
    return this._transport === 'grpc';
  }

  /**
   * Check if connected
   * @returns {boolean}
   */
  isConnected() {
    if (!this._client) return false;
    if (this._transport === 'grpc') {
      return this._client.connected;
    }
    return this._client.isConnected();
  }

  /**
   * Initialize and connect to the sequencer.
   * Automatically selects gRPC or REST based on configuration.
   * @returns {Promise<void>}
   */
  async connect() {
    let requestedTransport;
    try {
      requestedTransport = getTransportType(this.config.sequencerUrl);
    } catch (error) {
      throw new Error(`Invalid sequencer URL: ${error.message}`);
    }

    // Check if gRPC is available
    if (this._grpcAvailable === null) {
      this._grpcAvailable = await isGrpcAvailable();
    }

    // Determine which transport to use
    if (requestedTransport === 'grpc' && this._grpcAvailable && this.preferGrpc) {
      await this._connectGrpc();
    } else if (requestedTransport === 'grpc' && !this._grpcAvailable) {
      console.warn(
        'gRPC requested but not available. Install @grpc/grpc-js and @grpc/proto-loader for gRPC support. Falling back to REST.',
      );
      await this._connectRest();
    } else {
      await this._connectRest();
    }
  }

  /**
   * Connect using gRPC
   * @private
   */
  async _connectGrpc() {
    const GrpcClient = await loadGrpcClient();

    // Parse URL for gRPC
    const url = parseSequencerUrl(this.config.sequencerUrl);
    const host = url.port ? `${url.hostname}:${url.port}` : `${url.hostname}:50051`;

    const creds = this.config.getCredentials();

    this._client = new GrpcClient({
      url: host,
      tenantId: this.config.tenantId,
      storeId: this.config.storeId,
      agentId: this.config.agentId,
      tls: url.protocol === 'grpcs:',
      allowInsecureTransport: this.config.sequencer?.insecure === true,
      securityProfile: this.config.securityProfile ?? this.config.sync?.securityProfile,
      apiKey: creds.apiKey,
      jwtToken: creds.jwt,
      retryPolicy: this.config.retryPolicy,
    });

    // Forward events from gRPC client
    this._client.on('connected', () => this.emit('connected'));
    this._client.on('disconnected', () => this.emit('disconnected'));
    this._client.on('error', (err) => this.emit('error', err));
    this._client.on('event', (event) => this.emit('event', event));
    this._client.on('push-ack', (ack) => this.emit('push-ack', ack));
    this._client.on('sync-state', (state) => this.emit('sync-state', state));

    await this._client.connect();
    this._transport = 'grpc';
  }

  /**
   * Connect using REST
   * @private
   */
  async _connectRest() {
    this._client = new SequencerClient(this.config);
    await this._client.connect();
    this._transport = 'rest';
    this.emit('connected');
  }

  /**
   * Disconnect from the sequencer
   * @returns {Promise<void>}
   */
  async disconnect() {
    this.stopStreaming();

    if (this._client) {
      if (this._transport === 'grpc') {
        this._client.disconnect();
      } else {
        await this._client.disconnect();
      }
      this._client = null;
    }

    this._transport = null;
    this.emit('disconnected');
  }

  // ===========================================================================
  // PUSH OPERATIONS
  // ===========================================================================

  /**
   * Push events to the sequencer
   * @param {Object} batch
   * @param {string} batch.agentId
   * @param {Array} batch.events
   * @returns {Promise<Object>} Ingest receipt
   */
  async push(batch) {
    if (this._transport === 'grpc') {
      const response = await this._client.pushEvents(batch.events);
      return {
        batchId: response.batch_id,
        eventsAccepted: Number(response.events_accepted || 0),
        eventsRejected: Number(response.events_rejected || 0),
        sequenceStart: Number(response.sequence_start || 0),
        sequenceEnd: Number(response.sequence_end || 0),
        headSequence: Number(response.head_sequence || 0),
        rejections: (response.rejections || []).map((r) => ({
          eventId: r.event_id,
          reason: r.reason,
        })),
      };
    }
    return this._client.push(batch);
  }

  /**
   * Push with automatic retry
   * @param {Object} batch
   * @param {number} [maxRetries]
   * @returns {Promise<Object>}
   */
  async pushWithRetry(batch, maxRetries) {
    if (this._transport === 'grpc') {
      // gRPC client has built-in retry via reconnection
      return this.push(batch);
    }
    return this._client.pushWithRetry(batch, maxRetries);
  }

  // ===========================================================================
  // PULL OPERATIONS
  // ===========================================================================

  /**
   * Pull events from the sequencer
   * @param {number} fromSequence
   * @param {number} [limit=100]
   * @returns {Promise<Object>}
   */
  async pull(fromSequence, limit = 100) {
    if (this._transport === 'grpc') {
      return this._client.pullEvents({
        fromSequence,
        limit,
      });
    }
    return this._client.pull(fromSequence, limit);
  }

  /**
   * Pull events as async iterator
   * @param {number} fromSequence
   * @returns {AsyncIterable<Object>}
   */
  async *pullStream(fromSequence) {
    if (this._transport === 'grpc') {
      let cursor = fromSequence;
      let hasMore = true;

      while (hasMore) {
        const result = await this._client.pullEvents({
          fromSequence: cursor,
          limit: 100,
        });

        for (const event of result.events) {
          yield event;
        }

        cursor = result.nextSequence;
        hasMore = result.hasMore;
      }
    } else {
      yield* this._client.pullStream(fromSequence);
    }
  }

  // ===========================================================================
  // SYNC STATE
  // ===========================================================================

  /**
   * Get the current head sequence
   * @returns {Promise<Object>}
   */
  async getHead() {
    if (this._transport === 'grpc') {
      return this._client.getSyncState();
    }
    return this._client.getHead();
  }

  /**
   * Get entity event history
   * @param {string} entityType
   * @param {string} entityId
   * @returns {Promise<Array>}
   */
  async getEntityHistory(entityType, entityId) {
    if (this._transport === 'grpc') {
      const result = await this._client.getEntityHistory(entityType, entityId);
      return result.events;
    }
    return this._client.getEntityHistory(entityType, entityId);
  }

  // ===========================================================================
  // COMMITMENTS & PROOFS
  // ===========================================================================

  /**
   * Get a batch commitment
   * @param {string} batchId
   * @returns {Promise<Object|null>}
   */
  async getCommitment(batchId) {
    if (this._transport === 'grpc') {
      try {
        return await this._client.getCommitment({ batchId });
      } catch (err) {
        if (err.code === 5) return null; // NOT_FOUND
        throw err;
      }
    }
    return this._client.getCommitment(batchId);
  }

  /**
   * Verify inclusion proof
   * @param {Object} envelope
   * @param {Object} proof
   * @param {string} expectedRoot
   * @returns {boolean}
   */
  verifyInclusion(envelope, proof, expectedRoot) {
    // Verification is client-side only, works for both transports
    if (this._transport === 'rest') {
      return this._client.verifyInclusion(envelope, proof, expectedRoot);
    }
    // For gRPC, use the REST client's verification logic
    const restClient = new SequencerClient(this.config);
    return restClient.verifyInclusion(envelope, proof, expectedRoot);
  }

  /**
   * Verify a receipt signature against a known sequencer public key.
   * @param {Object} receipt - Receipt or sequenced event with receipt fields.
   * @param {Buffer|string|Object} sequencerPublicKey - Sequencer public key or bundle.
   * @returns {boolean}
   */
  verifyReceiptSignature(receipt, sequencerPublicKey) {
    if (this._client?.verifyReceiptSignature) {
      return this._client.verifyReceiptSignature(receipt, sequencerPublicKey);
    }
    // Fallback: create a REST client for verification logic
    const restClient = new SequencerClient(this.config);
    return restClient.verifyReceiptSignature(receipt, sequencerPublicKey);
  }

  // ===========================================================================
  // KEY MANAGEMENT
  // ===========================================================================

  /**
   * Register agent public key
   * @param {Object} keyRegistration
   * @returns {Promise<Object>}
   */
  async registerAgentKey(keyRegistration) {
    if (this._transport === 'grpc') {
      return this._client.registerAgentKey({
        agentId: keyRegistration.agentId,
        keyId: keyRegistration.keyId,
        keyType: 1, // SIGNING
        publicKey: Buffer.from(keyRegistration.publicKey, 'hex'),
        validFrom: keyRegistration.validFrom ? new Date(keyRegistration.validFrom) : undefined,
        validTo: keyRegistration.validTo ? new Date(keyRegistration.validTo) : undefined,
      });
    }
    return this._client.registerAgentKey(keyRegistration);
  }

  /**
   * Get agent's registered keys
   * @param {string} agentId
   * @returns {Promise<Array>}
   */
  async getAgentKeys(agentId) {
    if (this._transport === 'grpc') {
      const result = await this._client.getAgentKeys(agentId);
      return result.keys.map((k) => ({
        keyId: k.keyId,
        publicKey: k.publicKey.toString('hex'),
        status: k.status === 1 ? 'active' : k.status === 2 ? 'revoked' : 'expired',
        createdAt: k.createdAt?.toISOString(),
        validFrom: k.validFrom?.toISOString(),
        validTo: k.validTo?.toISOString(),
      }));
    }
    return this._client.getAgentKeys(agentId);
  }

  // ===========================================================================
  // STREAMING (gRPC only)
  // ===========================================================================

  /**
   * Start real-time event streaming.
   * Only available with gRPC transport.
   * @param {Object} [options]
   * @param {number} [options.fromSequence] - Starting sequence
   * @param {string[]} [options.entityTypeFilter] - Filter by entity types
   * @param {string[]} [options.eventTypeFilter] - Filter by event types
   * @returns {boolean} Whether streaming was started
   */
  startStreaming(options = {}) {
    if (this._transport !== 'grpc') {
      console.warn('Streaming requires gRPC transport');
      return false;
    }

    if (this._streamActive) {
      return true;
    }

    this._client.startEventStream(options);
    this._streamActive = true;
    return true;
  }

  /**
   * Start bidirectional sync stream.
   * Only available with gRPC transport.
   * @returns {boolean} Whether sync stream was started
   */
  startSyncStream() {
    if (this._transport !== 'grpc') {
      console.warn('Sync stream requires gRPC transport');
      return false;
    }

    if (this._streamActive) {
      return true;
    }

    this._client.startSyncStream();
    this._streamActive = true;
    return true;
  }

  /**
   * Push events via the sync stream.
   * Only available with gRPC transport and active sync stream.
   * @param {Array} events
   * @returns {boolean}
   */
  pushViaStream(events) {
    if (this._transport !== 'grpc' || !this._streamActive) {
      return false;
    }
    this._client.pushEventsViaStream(events);
    return true;
  }

  /**
   * Pull events via the sync stream.
   * Only available with gRPC transport and active sync stream.
   * @param {Object} options
   * @returns {boolean}
   */
  pullViaStream(options = {}) {
    if (this._transport !== 'grpc' || !this._streamActive) {
      return false;
    }
    this._client.pullEventsViaStream(options);
    return true;
  }

  /**
   * Acknowledge received events (for sync stream)
   * @param {number[]} sequenceNumbers
   * @returns {boolean}
   */
  ackEvents(sequenceNumbers) {
    if (this._transport !== 'grpc' || !this._streamActive) {
      return false;
    }
    this._client.ackEvents(sequenceNumbers);
    return true;
  }

  /**
   * Subscribe to a specific entity's events.
   * Only available with gRPC transport.
   * @param {string} entityType
   * @param {string} entityId
   * @param {Function} callback - Called for each event
   * @returns {Object|null} Subscription handle or null if not supported
   */
  subscribeEntity(entityType, entityId, callback) {
    if (this._transport !== 'grpc') {
      return null;
    }

    const stream = this._client.subscribeEntity(entityType, entityId);
    this._client.on('entity-event', callback);

    return {
      unsubscribe: () => {
        stream.cancel();
        this._client.off('entity-event', callback);
      },
    };
  }

  /**
   * Stop streaming
   */
  stopStreaming() {
    if (this._transport === 'grpc' && this._client) {
      if (this._client.eventStream) {
        this._client.eventStream.cancel();
        this._client.eventStream = null;
      }
      if (this._client.syncStream) {
        this._client.syncStream.end();
        this._client.syncStream = null;
      }
    }
    this._streamActive = false;
  }

  /**
   * Check if streaming is active
   * @returns {boolean}
   */
  isStreaming() {
    return this._streamActive;
  }

  /**
   * Register an event listener for streaming events
   * @param {Function} callback
   */
  onEvent(callback) {
    this.on('event', callback);
  }

  /**
   * Remove event listener
   * @param {Function} callback
   */
  offEvent(callback) {
    this.off('event', callback);
  }
}

/**
 * Create a unified sequencer client
 * @param {import('./config.js').SyncConfig} config
 * @param {Object} [options]
 * @param {boolean} [options.preferGrpc=true]
 * @param {boolean} [options.enableStreaming=true]
 * @returns {UnifiedSequencerClient}
 */
export function createUnifiedClient(config, options = {}) {
  return new UnifiedSequencerClient({
    config,
    ...options,
  });
}

/**
 * Check if gRPC dependencies are installed
 * @returns {Promise<boolean>}
 */
export async function checkGrpcAvailability() {
  return isGrpcAvailable();
}
