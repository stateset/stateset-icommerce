import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import * as standalone from '../../src/standalone.js';

describe('standalone', () => {
  it('exports the embedded Commerce constructor', () => {
    assert.equal(typeof standalone.Commerce, 'function');
  });

  it('exports adapter helpers from the standalone surface', () => {
    assert.equal(typeof standalone.getAdapter, 'function');
    assert.equal(typeof standalone.listAdapters, 'function');
  });
});
