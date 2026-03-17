/**
 * A2A State Checkpoint Service
 *
 * Enables agent state persistence across restarts — solves the duplicate
 * processing problem by tracking which quote/payment IDs have been handled.
 *
 * Features:
 *   - Save/load arbitrary agent state (JSON)
 *   - Track processed quote IDs (Set persistence)
 *   - Arbitrary checkpoint data (cursor positions, last tick times, etc.)
 *   - Atomic writes via temp-file + rename (crash-safe)
 *   - Zero external dependencies (fs/promises only)
 *
 * @example
 * ```javascript
 * const cp = createCheckpointService('/var/data/agents');
 *
 * // Save agent state
 * await cp.save('0xAgent', { balance: 100, lastTick: Date.now() });
 *
 * // Load on restart
 * const state = await cp.load('0xAgent');
 *
 * // Track processed items
 * await cp.saveProcessedIds('0xAgent', new Set(['q-1', 'q-2']));
 * const ids = await cp.loadProcessedIds('0xAgent');
 * // Set { 'q-1', 'q-2' }
 * ```
 */

import { mkdir, writeFile, readFile, rename, readdir, unlink } from 'node:fs/promises';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';

/**
 * Sanitize an agent address for use as a filename.
 * Replaces characters that are unsafe for filenames.
 *
 * @param {string} agentAddress
 * @returns {string} Safe filename segment
 */
function safeFilename(agentAddress) {
  return String(agentAddress).replace(/[^a-zA-Z0-9_-]/g, '_');
}

/**
 * Create a checkpoint service for persisting agent state.
 *
 * @param {string} dataDir - Directory to store checkpoint files
 * @returns {Object} Checkpoint service API
 */
export function createCheckpointService(dataDir) {
  if (!dataDir) {
    throw new Error('dataDir is required');
  }

  let _dirEnsured = false;

  /**
   * Ensure the data directory exists.
   */
  async function ensureDir() {
    if (_dirEnsured) return;
    await mkdir(dataDir, { recursive: true });
    _dirEnsured = true;
  }

  /**
   * Get the file path for an agent's state file.
   * @param {string} agentAddress
   * @param {string} [suffix='state'] - File suffix
   * @returns {string}
   */
  function filePath(agentAddress, suffix = 'state') {
    return join(dataDir, `${safeFilename(agentAddress)}.${suffix}.json`);
  }

  /**
   * Atomically write JSON data to a file.
   * Writes to a temp file then renames — prevents corruption if the process
   * crashes mid-write.
   *
   * @param {string} targetPath - Final file path
   * @param {*} data - Data to serialize as JSON
   */
  async function atomicWrite(targetPath, data) {
    await ensureDir();
    const tmpPath = `${targetPath}.tmp.${randomUUID()}`;
    const json = JSON.stringify(data, null, 2);
    await writeFile(tmpPath, json, 'utf8');
    await rename(tmpPath, targetPath);
  }

  /**
   * Read and parse a JSON file. Returns null if the file does not exist.
   *
   * @param {string} targetPath
   * @returns {Promise<*|null>}
   */
  async function safeRead(targetPath) {
    try {
      const raw = await readFile(targetPath, 'utf8');
      return JSON.parse(raw);
    } catch (err) {
      if (err.code === 'ENOENT') {
        return null;
      }
      throw err;
    }
  }

  /**
   * Save agent state to disk.
   *
   * @param {string} agentAddress - Agent wallet address or ID
   * @param {Object} state - State object to persist
   */
  async function save(agentAddress, state) {
    if (!agentAddress) throw new Error('agentAddress is required');
    const fp = filePath(agentAddress, 'state');
    await atomicWrite(fp, {
      agentAddress,
      state,
      savedAt: new Date().toISOString(),
    });
  }

  /**
   * Load persisted agent state.
   *
   * @param {string} agentAddress
   * @returns {Promise<Object|null>} The saved state object, or null if none exists
   */
  async function load(agentAddress) {
    if (!agentAddress) throw new Error('agentAddress is required');
    const data = await safeRead(filePath(agentAddress, 'state'));
    return data ? data.state : null;
  }

  /**
   * Save processed quote/payment IDs. Converts Set to Array for JSON.
   *
   * @param {string} agentAddress
   * @param {Set<string>} processedIds
   */
  async function saveProcessedIds(agentAddress, processedIds) {
    if (!agentAddress) throw new Error('agentAddress is required');
    if (!(processedIds instanceof Set)) {
      throw new Error('processedIds must be a Set');
    }
    const fp = filePath(agentAddress, 'processed');
    await atomicWrite(fp, {
      agentAddress,
      processedIds: [...processedIds],
      count: processedIds.size,
      savedAt: new Date().toISOString(),
    });
  }

  /**
   * Load processed IDs as a Set.
   *
   * @param {string} agentAddress
   * @returns {Promise<Set<string>>} Set of processed IDs (empty Set if none saved)
   */
  async function loadProcessedIds(agentAddress) {
    if (!agentAddress) throw new Error('agentAddress is required');
    const data = await safeRead(filePath(agentAddress, 'processed'));
    if (!data || !Array.isArray(data.processedIds)) {
      return new Set();
    }
    return new Set(data.processedIds);
  }

  /**
   * Save arbitrary checkpoint data (cursor positions, last tick time, etc.).
   *
   * @param {string} agentAddress
   * @param {Object} checkpoint - Checkpoint data
   */
  async function saveCheckpoint(agentAddress, checkpoint) {
    if (!agentAddress) throw new Error('agentAddress is required');
    const fp = filePath(agentAddress, 'checkpoint');
    await atomicWrite(fp, {
      agentAddress,
      checkpoint,
      savedAt: new Date().toISOString(),
    });
  }

  /**
   * Load checkpoint data.
   *
   * @param {string} agentAddress
   * @returns {Promise<Object|null>} Checkpoint data or null
   */
  async function loadCheckpoint(agentAddress) {
    if (!agentAddress) throw new Error('agentAddress is required');
    const data = await safeRead(filePath(agentAddress, 'checkpoint'));
    return data ? data.checkpoint : null;
  }

  /**
   * List all agent checkpoints in the data directory.
   *
   * @returns {Promise<Array<Object>>} Array of { agentAddress, type, file }
   */
  async function listCheckpoints() {
    await ensureDir();
    let files;
    try {
      files = await readdir(dataDir);
    } catch (err) {
      if (err.code === 'ENOENT') return [];
      throw err;
    }

    const results = [];
    for (const file of files) {
      if (!file.endsWith('.json')) continue;
      // Format: <safe-address>.<type>.json
      const match = file.match(/^(.+)\.(state|processed|checkpoint)\.json$/);
      if (match) {
        results.push({
          agentAddress: match[1],
          type: match[2],
          file,
        });
      }
    }

    return results;
  }

  /**
   * Delete a checkpoint file for an agent.
   *
   * @param {string} agentAddress
   * @param {string} [type='state'] - File type: 'state', 'processed', or 'checkpoint'
   */
  async function deleteCheckpoint(agentAddress, type = 'state') {
    if (!agentAddress) throw new Error('agentAddress is required');
    const fp = filePath(agentAddress, type);
    try {
      await unlink(fp);
    } catch (err) {
      if (err.code === 'ENOENT') return; // already gone
      throw err;
    }
  }

  return {
    save,
    load,
    saveProcessedIds,
    loadProcessedIds,
    saveCheckpoint,
    loadCheckpoint,
    listCheckpoints,
    deleteCheckpoint,
  };
}

export default { createCheckpointService };
