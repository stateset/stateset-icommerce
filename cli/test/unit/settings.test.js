/**
 * Unit tests for settings.js
 */

import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert';
import {
  DEFAULT_AGENT_SETTINGS,
  loadAgentSettings,
  resetAgentSettingsCache,
} from '../../src/settings.js';

// ===========================================================================
// DEFAULT_AGENT_SETTINGS
// ===========================================================================

describe('DEFAULT_AGENT_SETTINGS', () => {
  it('has expected top-level keys', () => {
    const keys = Object.keys(DEFAULT_AGENT_SETTINGS);
    assert.ok(keys.includes('agent'));
    assert.ok(keys.includes('model'));
    assert.ok(keys.includes('provider'));
    assert.ok(keys.includes('guardrails'));
    assert.ok(keys.includes('contextGuard'));
    assert.ok(keys.includes('retry'));
    assert.ok(keys.includes('memory'));
    assert.ok(keys.includes('plugins'));
    assert.ok(keys.includes('privacy'));
  });

  it('agent defaults to customer-service', () => {
    assert.strictEqual(DEFAULT_AGENT_SETTINGS.agent.default, 'customer-service');
  });

  it('provider defaults to claude', () => {
    assert.strictEqual(DEFAULT_AGENT_SETTINGS.provider.default, 'claude');
  });

  it('contextGuard has numeric thresholds', () => {
    const cg = DEFAULT_AGENT_SETTINGS.contextGuard;
    assert.strictEqual(typeof cg.warningThreshold, 'number');
    assert.strictEqual(typeof cg.compactThreshold, 'number');
    assert.strictEqual(typeof cg.abortThreshold, 'number');
    assert.ok(cg.warningThreshold < cg.compactThreshold);
    assert.ok(cg.compactThreshold < cg.abortThreshold);
  });

  it('retry has retryableErrors array', () => {
    assert.ok(Array.isArray(DEFAULT_AGENT_SETTINGS.retry.retryableErrors));
    assert.ok(DEFAULT_AGENT_SETTINGS.retry.retryableErrors.length > 0);
    assert.ok(DEFAULT_AGENT_SETTINGS.retry.retryableErrors.includes('429'));
  });

  it('privacy defaults redactLogs to true', () => {
    assert.strictEqual(DEFAULT_AGENT_SETTINGS.privacy.redactLogs, true);
  });
});

// ===========================================================================
// loadAgentSettings
// ===========================================================================

describe('loadAgentSettings', () => {
  afterEach(() => {
    resetAgentSettingsCache();
  });

  it('returns defaults when no settings files exist', () => {
    const settings = loadAgentSettings({}, { reload: true });
    assert.strictEqual(settings.agent.default, 'customer-service');
    assert.strictEqual(settings.provider.default, 'claude');
  });

  it('applies overrides on top of defaults', () => {
    const settings = loadAgentSettings({ provider: { default: 'openai' } }, { reload: true });
    assert.strictEqual(settings.provider.default, 'openai');
    // Other defaults preserved
    assert.strictEqual(settings.agent.default, 'customer-service');
  });

  it('deep-merges overrides without clobbering sibling keys', () => {
    const settings = loadAgentSettings({ retry: { maxRetries: 5 } }, { reload: true });
    assert.strictEqual(settings.retry.maxRetries, 5);
    // Sibling keys preserved
    assert.strictEqual(settings.retry.enabled, true);
    assert.ok(Array.isArray(settings.retry.retryableErrors));
  });

  it('uses cached settings on subsequent calls', () => {
    const first = loadAgentSettings({}, { reload: true });
    const second = loadAgentSettings();
    // Both should have same defaults
    assert.strictEqual(first.agent.default, second.agent.default);
  });

  it('overrides can add new keys', () => {
    const settings = loadAgentSettings({ custom: { myFlag: true } }, { reload: true });
    assert.strictEqual(settings.custom.myFlag, true);
  });

  it('scalar override replaces entire value', () => {
    const settings = loadAgentSettings({ memory: { enabled: true } }, { reload: true });
    assert.strictEqual(settings.memory.enabled, true);
    // Sibling should be preserved via deep merge
    assert.strictEqual(settings.memory.useMarkdown, true);
  });
});

// ===========================================================================
// resetAgentSettingsCache
// ===========================================================================

describe('resetAgentSettingsCache', () => {
  it('forces reload on next loadAgentSettings call', () => {
    const first = loadAgentSettings({ provider: { default: 'openai' } }, { reload: true });
    assert.strictEqual(first.provider.default, 'openai');

    resetAgentSettingsCache();
    // Without override, should return to defaults
    const second = loadAgentSettings({}, { reload: true });
    assert.strictEqual(second.provider.default, 'claude');
  });

  it('can be called multiple times safely', () => {
    assert.doesNotThrow(() => {
      resetAgentSettingsCache();
      resetAgentSettingsCache();
      resetAgentSettingsCache();
    });
  });
});
