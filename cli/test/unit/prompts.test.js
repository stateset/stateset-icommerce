/**
 * Tests for cli/src/prompts.js
 *
 * Tests non-interactive helpers (isInteractive, interactiveOr).
 * The readline-based prompt/confirm/select functions require TTY
 * mocking which is out of scope for unit tests.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { isInteractive, interactiveOr } from '../../src/prompts.js';

// ---------------------------------------------------------------------------
// isInteractive
// ---------------------------------------------------------------------------

describe('isInteractive', () => {
  it('returns boolean or undefined', () => {
    const result = isInteractive();
    // In TTY mode returns true; in non-TTY returns undefined (short-circuit)
    assert.ok(result === true || result === false || result === undefined);
  });

  it('returns falsy in non-TTY environment (CI)', () => {
    // In CI / node --test, stdin is not a TTY
    // Returns undefined (not false) because undefined && ... short-circuits
    assert.ok(!isInteractive());
  });
});

// ---------------------------------------------------------------------------
// interactiveOr
// ---------------------------------------------------------------------------

describe('interactiveOr', () => {
  it('returns fallback when not interactive', async () => {
    const result = await interactiveOr(async () => 'interactive-value', 'fallback-value');
    assert.equal(result, 'fallback-value');
  });

  it('throws when not interactive and no fallback', async () => {
    await assert.rejects(
      () => interactiveOr(async () => 'x', undefined, 'Missing input'),
      /Missing input/,
    );
  });

  it('throws with default message when no error message provided', async () => {
    await assert.rejects(() => interactiveOr(async () => 'x'), /non-interactive/);
  });

  it('accepts null as valid fallback', async () => {
    const result = await interactiveOr(async () => 'x', null);
    assert.equal(result, null);
  });

  it('accepts empty string as valid fallback', async () => {
    const result = await interactiveOr(async () => 'x', '');
    assert.equal(result, '');
  });

  it('accepts false as valid fallback', async () => {
    const result = await interactiveOr(async () => 'x', false);
    assert.equal(result, false);
  });

  it('accepts 0 as valid fallback', async () => {
    const result = await interactiveOr(async () => 'x', 0);
    assert.equal(result, 0);
  });
});
