/**
 * Unit tests for channels/plugin-config.js — PluginConfigState, singletons
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {
  PluginConfigState,
  getPluginConfigState,
  resetPluginConfigState,
} from '../../src/channels/plugin-config.js';

// ===========================================================================
// resolve — Enable Resolution
// ===========================================================================

describe('PluginConfigState.resolve', () => {
  it('returns false when global disabled', () => {
    const state = new PluginConfigState({ globalEnabled: false });
    const res = state.resolve('my-plugin');
    assert.equal(res.enabled, false);
    assert.ok(res.reason.includes('globally disabled'));
  });

  it('returns false when plugin is in deny list', () => {
    const state = new PluginConfigState({ deny: ['bad-plugin'] });
    const res = state.resolve('bad-plugin');
    assert.equal(res.enabled, false);
    assert.ok(res.reason.includes('deny'));
  });

  it('deny list takes precedence over explicit enable', () => {
    const state = new PluginConfigState({
      deny: ['my-plugin'],
      entries: { 'my-plugin': { enabled: true } },
    });
    const res = state.resolve('my-plugin');
    assert.equal(res.enabled, false);
  });

  it('returns true for explicitly enabled plugin', () => {
    const state = new PluginConfigState({
      entries: { 'my-plugin': { enabled: true } },
    });
    const res = state.resolve('my-plugin');
    assert.equal(res.enabled, true);
    assert.ok(res.reason.includes('explicitly enabled'));
  });

  it('returns false for explicitly disabled plugin', () => {
    const state = new PluginConfigState({
      entries: { 'my-plugin': { enabled: false } },
    });
    const res = state.resolve('my-plugin');
    assert.equal(res.enabled, false);
    assert.ok(res.reason.includes('explicitly disabled'));
  });

  it('allow list present + plugin in list => true', () => {
    const state = new PluginConfigState({ allow: ['my-plugin'] });
    const res = state.resolve('my-plugin');
    assert.equal(res.enabled, true);
    assert.ok(res.reason.includes('allow list'));
  });

  it('allow list present + plugin NOT in list => false', () => {
    const state = new PluginConfigState({ allow: ['other-plugin'] });
    const res = state.resolve('my-plugin');
    assert.equal(res.enabled, false);
    assert.ok(res.reason.includes('not in allow list'));
  });

  it('manifest enabledByDefault => true', () => {
    const state = new PluginConfigState();
    const res = state.resolve('my-plugin', { enabledByDefault: true });
    assert.equal(res.enabled, true);
    assert.ok(res.reason.includes('manifest'));
  });

  it('default (no rules) => enabled', () => {
    const state = new PluginConfigState();
    const res = state.resolve('my-plugin');
    assert.equal(res.enabled, true);
    assert.ok(res.reason.includes('default'));
  });
});

// ===========================================================================
// isEnabled / getDisableReason
// ===========================================================================

describe('PluginConfigState.isEnabled / getDisableReason', () => {
  it('isEnabled delegates to resolve', () => {
    const state = new PluginConfigState({ deny: ['bad'] });
    assert.equal(state.isEnabled('bad'), false);
    assert.equal(state.isEnabled('good'), true);
  });

  it('getDisableReason returns reason for disabled plugin', () => {
    const state = new PluginConfigState({ deny: ['bad'] });
    const reason = state.getDisableReason('bad');
    assert.ok(reason.includes('deny'));
  });

  it('getDisableReason returns empty string for enabled plugin', () => {
    const state = new PluginConfigState();
    const reason = state.getDisableReason('my-plugin');
    assert.equal(reason, '');
  });
});

// ===========================================================================
// enable / disable / resetToDefault
// ===========================================================================

describe('PluginConfigState mutations', () => {
  it('enable sets enabled=true and removes from deny', () => {
    const state = new PluginConfigState({ deny: ['my-plugin'] });
    state.enable('my-plugin');
    assert.equal(state.isEnabled('my-plugin'), true);
  });

  it('disable sets enabled=false', () => {
    const state = new PluginConfigState();
    state.disable('my-plugin');
    assert.equal(state.isEnabled('my-plugin'), false);
  });

  it('resetToDefault removes the enabled override', () => {
    const state = new PluginConfigState();
    state.disable('my-plugin');
    assert.equal(state.isEnabled('my-plugin'), false);
    state.resetToDefault('my-plugin');
    // Falls through to default => enabled
    assert.equal(state.isEnabled('my-plugin'), true);
  });

  it('resetToDefault on non-existent entry does not throw', () => {
    const state = new PluginConfigState();
    assert.doesNotThrow(() => state.resetToDefault('nonexistent'));
  });
});

// ===========================================================================
// setConfig / getConfig
// ===========================================================================

describe('PluginConfigState config', () => {
  it('setConfig stores plugin-specific config', () => {
    const state = new PluginConfigState();
    state.setConfig('my-plugin', { key: 'value', num: 42 });
    const config = state.getConfig('my-plugin');
    assert.deepEqual(config, { key: 'value', num: 42 });
  });

  it('getConfig returns empty object for unconfigured plugin', () => {
    const state = new PluginConfigState();
    const config = state.getConfig('unknown-plugin');
    assert.deepEqual(config, {});
  });

  it('setConfig replaces previous config', () => {
    const state = new PluginConfigState();
    state.setConfig('my-plugin', { a: 1 });
    state.setConfig('my-plugin', { b: 2 });
    assert.deepEqual(state.getConfig('my-plugin'), { b: 2 });
  });
});

// ===========================================================================
// setAllowList / setDenyList / setGlobalEnabled
// ===========================================================================

describe('PluginConfigState list management', () => {
  it('setAllowList restricts to listed plugins', () => {
    const state = new PluginConfigState();
    assert.equal(state.isEnabled('other'), true);
    state.setAllowList(['only-this']);
    assert.equal(state.isEnabled('only-this'), true);
    assert.equal(state.isEnabled('other'), false);
  });

  it('setDenyList blocks listed plugins', () => {
    const state = new PluginConfigState();
    state.setDenyList(['blocked']);
    assert.equal(state.isEnabled('blocked'), false);
    assert.equal(state.isEnabled('allowed'), true);
  });

  it('setGlobalEnabled toggles master switch', () => {
    const state = new PluginConfigState();
    assert.equal(state.isEnabled('any'), true);
    state.setGlobalEnabled(false);
    assert.equal(state.isEnabled('any'), false);
    state.setGlobalEnabled(true);
    assert.equal(state.isEnabled('any'), true);
  });
});

// ===========================================================================
// listEntries
// ===========================================================================

describe('PluginConfigState.listEntries', () => {
  it('returns entries with enable state', () => {
    const state = new PluginConfigState();
    state.enable('a');
    state.disable('b');
    state.setConfig('a', { x: 1 });
    const entries = state.listEntries();
    assert.ok(entries.length >= 2);

    const entryA = entries.find((e) => e.id === 'a');
    assert.ok(entryA);
    assert.equal(entryA.enabled, true);
    assert.equal(entryA.hasConfig, true);

    const entryB = entries.find((e) => e.id === 'b');
    assert.ok(entryB);
    assert.equal(entryB.enabled, false);
    assert.equal(entryB.hasConfig, false);
  });
});

// ===========================================================================
// toJSON
// ===========================================================================

describe('PluginConfigState.toJSON', () => {
  it('serializes full state', () => {
    const state = new PluginConfigState({
      allow: ['a'],
      deny: ['b'],
      entries: { c: { enabled: true, config: { x: 1 } } },
    });
    const json = state.toJSON();
    assert.equal(json.globalEnabled, true);
    assert.deepEqual(json.allow, ['a']);
    assert.deepEqual(json.deny, ['b']);
    assert.equal(json.entries.c.enabled, true);
    assert.deepEqual(json.entries.c.config, { x: 1 });
  });

  it('serializes empty state', () => {
    const state = new PluginConfigState();
    const json = state.toJSON();
    assert.equal(json.globalEnabled, true);
    assert.deepEqual(json.allow, []);
    assert.deepEqual(json.deny, []);
    assert.deepEqual(json.entries, {});
  });
});

// ===========================================================================
// Persistence — round-trip with tmp file
// ===========================================================================

describe('PluginConfigState persistence', () => {
  let tmpDir;
  let statePath;

  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'plugin-config-test-'));
    statePath = path.join(tmpDir, 'plugin-state.json');
  });

  afterEach(() => {
    try {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    } catch {
      // ignore cleanup errors
    }
  });

  it('persists state to disk and reloads', () => {
    // Create state and persist
    const state1 = new PluginConfigState({ statePath });
    state1.enable('plugin-a');
    state1.disable('plugin-b');
    state1.setConfig('plugin-a', { mode: 'fast' });
    state1.setDenyList(['plugin-c']);

    // Verify file was written
    assert.ok(fs.existsSync(statePath));

    // Reload from disk
    const state2 = new PluginConfigState({ statePath });
    assert.equal(state2.isEnabled('plugin-a'), true);
    assert.equal(state2.isEnabled('plugin-b'), false);
    assert.deepEqual(state2.getConfig('plugin-a'), { mode: 'fast' });
    assert.equal(state2.isEnabled('plugin-c'), false);
  });

  it('handles missing state file gracefully', () => {
    const nonExistent = path.join(tmpDir, 'does-not-exist.json');
    const state = new PluginConfigState({ statePath: nonExistent });
    // Should not throw, should use defaults
    assert.equal(state.isEnabled('any'), true);
  });
});

// ===========================================================================
// getPluginConfigState / resetPluginConfigState — singleton
// ===========================================================================

describe('singleton management', () => {
  afterEach(() => {
    resetPluginConfigState();
  });

  it('getPluginConfigState returns a PluginConfigState instance', () => {
    const state = getPluginConfigState();
    assert.ok(state instanceof PluginConfigState);
  });

  it('getPluginConfigState returns the same instance on repeated calls', () => {
    const a = getPluginConfigState();
    const b = getPluginConfigState();
    assert.equal(a, b);
  });

  it('resetPluginConfigState clears the singleton', () => {
    const a = getPluginConfigState();
    resetPluginConfigState();
    const b = getPluginConfigState();
    assert.notEqual(a, b);
  });
});
