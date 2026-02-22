import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

// These tests verify the enhanced graceful-shutdown module exports and
// formatting behavior. We cannot easily test process.exit or signal handling
// in unit tests, so we focus on the module API and error formatting.

describe('graceful-shutdown (enhanced)', () => {
  let mod;

  it('module loads without error', async () => {
    mod = await import('../../src/graceful-shutdown.js');
    assert.ok(mod);
  });

  it('exports installShutdownHandlers', async () => {
    mod = await import('../../src/graceful-shutdown.js');
    assert.ok(typeof mod.installShutdownHandlers === 'function');
  });

  it('exports runMain', async () => {
    mod = await import('../../src/graceful-shutdown.js');
    assert.ok(typeof mod.runMain === 'function');
  });

  describe('imports theme', () => {
    it('theme module is loadable', async () => {
      const theme = await import('../../src/theme.js');
      assert.ok(theme.theme);
      assert.ok(typeof theme.theme.error === 'function');
      assert.ok(typeof theme.theme.bold === 'function');
      assert.ok(typeof theme.theme.muted === 'function');
    });
  });

  describe('imports error-hints', () => {
    it('error-hints module is loadable', async () => {
      const hints = await import('../../src/utils/error-hints.js');
      assert.ok(typeof hints.getErrorHint === 'function');
    });

    it('getErrorHint returns hint for known errors', async () => {
      const { getErrorHint } = await import('../../src/utils/error-hints.js');
      const hint = getErrorHint(new Error('ANTHROPIC_API_KEY is not set'));
      assert.ok(hint, 'expected a hint for API key error');
      assert.ok(hint.includes('stateset-config'), `hint should mention stateset-config, got: ${hint}`);
    });

    it('getErrorHint returns null for unknown errors', async () => {
      const { getErrorHint } = await import('../../src/utils/error-hints.js');
      const hint = getErrorHint(new Error('random unrelated error'));
      assert.strictEqual(hint, null);
    });
  });
});
