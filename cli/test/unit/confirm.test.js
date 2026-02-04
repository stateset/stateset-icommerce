/**
 * Unit tests for confirm utils
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { createConfirmHandler } from '../../src/utils/confirm.js';

describe('confirm utils', () => {
  it('assumeYes returns true', async () => {
    const handler = createConfirmHandler({ assumeYes: true });
    const ok = await handler({ operation: 'test' });
    assert.strictEqual(ok, true);
  });

  it('nonInteractive returns false', async () => {
    const handler = createConfirmHandler({ nonInteractive: true });
    const ok = await handler({ operation: 'test' });
    assert.strictEqual(ok, false);
  });

  it('uses provided confirmPrompt', async () => {
    let called = false;
    const handler = createConfirmHandler({
      confirmPrompt: async () => {
        called = true;
        return true;
      }
    });
    const ok = await handler({ operation: 'test' });
    assert.strictEqual(called, true);
    assert.strictEqual(ok, true);
  });
});
