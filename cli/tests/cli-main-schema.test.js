import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parseArgs } from 'node:util';
import { getMainCliParseOptions, normalizeMainCliValues } from '../src/cli-schema.js';

describe('main CLI schema parse mapping', () => {
  it('maps dashed flags to internal camel-case keys', () => {
    const parsed = parseArgs({
      args: [
        '--queue-status',
        '--queue-clear',
        '--queue-admin',
        '--queue-lane',
        'lane-123',
        '--no-memory',
        '--treasury-chain',
        'base',
      ],
      options: getMainCliParseOptions(),
      allowPositionals: true,
    });

    const values = normalizeMainCliValues(parsed.values);
    assert.equal(values.queueStatus, true);
    assert.equal(values.queueClear, true);
    assert.equal(values.queueAdmin, true);
    assert.equal(values.queueLane, 'lane-123');
    assert.equal(values.noMemory, true);
    assert.equal(values.treasuryChain, 'base');
  });

  it('inverts --no-color into color=false', () => {
    const parsed = parseArgs({
      args: ['--no-color'],
      options: getMainCliParseOptions(),
      allowPositionals: true,
    });

    const values = normalizeMainCliValues(parsed.values);
    assert.equal(values.color, false);
  });
});
