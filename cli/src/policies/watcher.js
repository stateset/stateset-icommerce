/**
 * Policy File Watcher
 *
 * Watches a directory for policy file changes (.yaml, .yml, .json)
 * and hot-reloads them into a PolicyEngine instance.
 *
 * Uses fs.watch with debouncing to avoid double-reloads.
 */

import fs from 'fs';
import path from 'path';
import { parse as parseYAML } from 'yaml';
import { PolicySet } from './engine.js';

/**
 * Watch a directory for policy file changes and reload into the engine.
 *
 * @param {import('./engine.js').PolicyEngine} engine - The policy engine to reload into
 * @param {string} policiesDir - Absolute path to the policies directory
 * @param {Object} [opts]
 * @param {number} [opts.debounceMs=500] - Debounce interval in milliseconds
 * @param {Function} [opts.onReload] - Callback after successful reload
 * @param {Function} [opts.onError] - Callback on error
 * @returns {{ stop: Function, isWatching: Function }}
 */
export function watchPolicies(engine, policiesDir, opts = {}) {
  const { debounceMs = 500, onReload = null, onError = null } = opts;

  if (!engine) throw new Error('engine is required');
  if (!policiesDir) throw new Error('policiesDir is required');

  // Ensure directory exists
  if (!fs.existsSync(policiesDir)) {
    fs.mkdirSync(policiesDir, { recursive: true });
  }

  let debounceTimer = null;
  let watching = true;

  /**
   * Load all policy files from the directory into the engine.
   * Clears existing policies first to handle deletions.
   */
  function reloadPolicies() {
    try {
      const files = fs
        .readdirSync(policiesDir)
        .filter((f) => f.endsWith('.yaml') || f.endsWith('.yml') || f.endsWith('.json'));

      // Track which policy IDs we loaded
      const loadedIds = new Set();

      for (const file of files) {
        try {
          const filePath = path.join(policiesDir, file);
          const content = fs.readFileSync(filePath, 'utf-8');

          let data;
          if (file.endsWith('.json')) {
            data = JSON.parse(content);
          } else {
            data = parseYAML(content);
          }

          if (!data) continue;

          const policySet = new PolicySet(data);
          engine.registerPolicySet(policySet);
          loadedIds.add(policySet.id);
        } catch (fileError) {
          if (onError) onError(fileError, file);
          if (engine.listenerCount('error') > 0) {
            engine.emit('error', { type: 'watch:file', file, error: fileError });
          }
        }
      }

      engine.emit('reloaded', { fileCount: files.length, policyIds: [...loadedIds] });
      if (onReload) onReload({ fileCount: files.length, policyIds: [...loadedIds] });
    } catch (error) {
      if (onError) onError(error);
      if (engine.listenerCount('error') > 0) {
        engine.emit('error', { type: 'watch:reload', error });
      }
    }
  }

  /**
   * Debounced handler for fs.watch events.
   */
  function handleChange(eventType, filename) {
    if (!watching) return;
    if (!filename) return;

    // Only react to policy files
    if (!filename.endsWith('.yaml') && !filename.endsWith('.yml') && !filename.endsWith('.json')) {
      return;
    }

    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      reloadPolicies();
    }, debounceMs);
  }

  const watcher = fs.watch(policiesDir, { persistent: false }, handleChange);

  watcher.on('error', (error) => {
    if (onError) onError(error);
    engine.emit('error', { type: 'watch:watcher', error });
  });

  return {
    /**
     * Stop watching for changes.
     */
    stop() {
      watching = false;
      if (debounceTimer) {
        clearTimeout(debounceTimer);
        debounceTimer = null;
      }
      watcher.close();
    },

    /**
     * Check if the watcher is still active.
     * @returns {boolean}
     */
    isWatching() {
      return watching;
    },

    /**
     * Force a reload of all policy files.
     */
    reload() {
      reloadPolicies();
    },
  };
}
