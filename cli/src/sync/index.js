/**
 * StateSet CLI Sync Module
 *
 * Verifiable Event Sync (VES) for local AI agent kernels.
 * Enables offline-first operation with eventual consistency to production.
 * Supports both REST and gRPC transports, with real-time streaming via gRPC.
 */

export { Outbox, createOutbox } from './outbox.js';
export { SyncConfig, loadSyncConfig, saveSyncConfig } from './config.js';
export { SequencerClient, createSequencerClient } from './client.js';
export { GrpcSequencerClient } from './grpc-client.js';
export {
  UnifiedSequencerClient,
  createUnifiedClient,
  checkGrpcAvailability,
} from './unified-client.js';
export { SyncEngine, createSyncEngine, checkGrpcAvailable } from './engine.js';
export { wrapCommerceWithEvents, EventCapture } from './capture.js';
