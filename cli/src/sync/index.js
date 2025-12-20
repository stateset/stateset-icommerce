/**
 * StateSet CLI Sync Module
 *
 * Verifiable Event Sync (VES) for local AI agent kernels.
 * Enables offline-first operation with eventual consistency to production.
 */

export { Outbox, createOutbox } from './outbox.js';
export { SyncConfig, loadSyncConfig, saveSyncConfig } from './config.js';
export { SequencerClient, createSequencerClient } from './client.js';
export { SyncEngine, createSyncEngine } from './engine.js';
export { wrapCommerceWithEvents, EventCapture } from './capture.js';
