import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

describe('package exports', () => {
  it('exposes the standalone self-reference export', async () => {
    const standalone = await import('@stateset/cli/standalone');

    assert.equal(typeof standalone.Commerce, 'function');
    assert.equal(typeof standalone.getAdapter, 'function');
  });

  it('exposes the embedded agent toolkit self-reference export', async () => {
    const toolkit = await import('@stateset/cli/agent-toolkit');

    assert.equal(typeof toolkit.createEmbeddedAgentToolkit, 'function');
    assert.equal(typeof toolkit.createEmbeddedAgentKit, 'function');
    assert.equal(typeof toolkit.default, 'function');
  });
});
