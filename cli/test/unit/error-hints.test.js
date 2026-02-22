import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { getErrorHint } from '../../src/utils/error-hints.js';

describe('getErrorHint', () => {
  it('returns hint for @stateset/embedded load failure', () => {
    const hint = getErrorHint(new Error('Failed to load @stateset/embedded: MODULE_NOT_FOUND'));
    assert.ok(hint);
    assert.ok(hint.includes('npm install'));
  });

  it('returns hint for missing API key', () => {
    const hint = getErrorHint(new Error('ANTHROPIC_API_KEY is not set'));
    assert.ok(hint);
    assert.ok(hint.includes('stateset-config set-key'));
  });

  it('returns hint for authentication failure', () => {
    const hint = getErrorHint(new Error('401 Unauthorized: invalid API key'));
    assert.ok(hint);
    assert.ok(hint.includes('set-key'));
  });

  it('returns hint for database not found', () => {
    const hint = getErrorHint(new Error('ENOENT: no such file or directory, open store.db'));
    assert.ok(hint);
    assert.ok(hint.includes('stateset-init'));
  });

  it('returns hint for connection refused', () => {
    const hint = getErrorHint(new Error('ECONNREFUSED 127.0.0.1:443'));
    assert.ok(hint);
    assert.ok(hint.includes('internet'));
  });

  it('returns hint for rate limiting', () => {
    const hint = getErrorHint(new Error('429 Too Many Requests'));
    assert.ok(hint);
    assert.ok(hint.includes('retry'));
  });

  it('returns hint for overloaded API', () => {
    const hint = getErrorHint(new Error('503 Service Unavailable - overloaded'));
    assert.ok(hint);
    assert.ok(hint.includes('overloaded'));
  });

  it('returns hint for permission denied', () => {
    const hint = getErrorHint(new Error('EACCES: permission denied'));
    assert.ok(hint);
    assert.ok(hint.includes('permissions'));
  });

  it('returns hint for --apply requirement', () => {
    const hint = getErrorHint(new Error('This operation requires --apply'));
    assert.ok(hint);
    assert.ok(hint.includes('--apply'));
  });

  it('returns null for unknown errors', () => {
    const hint = getErrorHint(new Error('something completely unrelated'));
    assert.equal(hint, null);
  });

  it('handles string input', () => {
    const hint = getErrorHint('ECONNREFUSED');
    assert.ok(hint);
  });

  it('is case insensitive', () => {
    const hint = getErrorHint(new Error('failed to load @STATESET/EMBEDDED'));
    assert.ok(hint);
  });

  it('returns hint for fetch failures', () => {
    const hint = getErrorHint(new Error('fetch failed: network error'));
    assert.ok(hint);
  });

  it('returns hint for budget exceeded', () => {
    const hint = getErrorHint(new Error('Budget exceeded: spending limit reached'));
    assert.ok(hint);
    assert.ok(hint.includes('budget'));
  });
});
