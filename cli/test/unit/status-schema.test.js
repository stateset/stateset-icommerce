/**
 * src/utils/status-schema.js: status parameters that refuse a left-out value
 * with what to do instead, and filters that keep older spellings working as
 * aliases of the stored value.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

import { resolveStatusAlias, statusEnum, statusFilter } from '../../src/utils/status-schema.js';
import { zodToInputJsonSchema } from '../../src/mcp/zod-json-schema.js';

describe('statusEnum', () => {
  const schema = z.object({
    status: statusEnum(['confirmed', 'processing'], { shipped: 'call ship_order.' }),
  });

  it('accepts the offered values and advertises only those', () => {
    assert.ok(schema.safeParse({ status: 'confirmed' }).success);
    assert.deepEqual(zodToInputJsonSchema(schema).properties.status.enum, [
      'confirmed',
      'processing',
    ]);
  });

  it('refuses a left-out value with what to do instead', () => {
    const { error } = schema.safeParse({ status: 'shipped' });
    assert.equal(error.issues[0].message, '"shipped" is not accepted here: call ship_order.');
    // Anything else gets the ordinary message listing the choices.
    assert.match(schema.safeParse({ status: 'bogus' }).error.issues[0].message, /expected one of/);
  });
});

describe('statusFilter', () => {
  const schema = z.object({
    status: statusFilter(['paid', 'failed'], {
      subject: 'settlement',
      aliases: { completed: 'paid' },
      redirects: { pending: 'none is ever pending.' },
    }),
  });

  it('is optional, accepts stored values and aliases, and says which is which', () => {
    assert.ok(schema.safeParse({}).success);
    assert.ok(schema.safeParse({ status: 'paid' }).success);
    assert.ok(schema.safeParse({ status: 'completed' }).success);
    const json = zodToInputJsonSchema(schema).properties.status;
    assert.deepEqual(json.enum, ['paid', 'failed', 'completed']);
    assert.equal(
      json.description,
      'Filter by settlement status: paid, failed. Also accepted for compatibility: completed = paid.',
    );
    assert.match(
      schema.safeParse({ status: 'pending' }).error.issues[0].message,
      /none is ever pending/,
    );
  });

  it('maps an alias to the stored value', () => {
    assert.equal(resolveStatusAlias('completed', { completed: 'paid' }), 'paid');
    assert.equal(resolveStatusAlias('failed', { completed: 'paid' }), 'failed');
    assert.equal(resolveStatusAlias(undefined, { completed: 'paid' }), undefined);
  });
});
