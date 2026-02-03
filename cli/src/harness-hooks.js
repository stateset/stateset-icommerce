/**
 * Harness Hooks & Plugin Loader
 *
 * Exposes the shared HookRunner and optional plugin loading
 * for harness-level prompt/tool hooks.
 */

import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { getPluginRegistry } from './channels/plugin-api.js';
import { discoverAndLoadPlugins } from './channels/plugin-loader.js';

let _loaded = false;

function getBundledPluginDir() {
  try {
    const __filename = fileURLToPath(import.meta.url);
    const __dirname = path.dirname(__filename);
    return path.join(__dirname, 'plugins');
  } catch {
    return null;
  }
}

export function getHarnessHookRunner() {
  return getPluginRegistry().getHookRunner();
}

/**
 * Load harness plugins once (idempotent).
 */
export async function ensureHarnessPluginsLoaded(options = {}) {
  if (_loaded) return { loaded: true };

  const bundledDir = getBundledPluginDir();
  const opts = {
    bundledDir,
    verbose: options.verbose || false,
  };

  await discoverAndLoadPlugins(opts);
  _loaded = true;
  return { loaded: true };
}
