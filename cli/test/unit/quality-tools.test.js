/**
 * Quality tool definitions: `close_ncr` records a disposition before closing.
 *
 * The engine refuses to close a non-conformance report that has no
 * disposition, and `close_ncr` was the only NCR write after create -- so the
 * tool takes an optional `disposition` (and `dispositionQuantity`), records it
 * through `quality.updateNcr`, then closes. Without one it closes as before,
 * and the engine's refusal reaches the caller.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

import { NCR_DISPOSITIONS, qualityTools } from '../../src/tools/quality.js';
import { bindingClassMethods, toolModuleCalls } from '../helpers/binding-class-methods.js';

const byName = Object.fromEntries(qualityTools.map((t) => [t.name, t]));
const closeNcr = byName.close_ncr;
const parse = (params) => z.object(closeNcr.inputSchema).safeParse(params);

const NCR_ID = '3f2504e0-4f89-41d3-9a0c-0305e82c3301';

/** A `commerce.quality` double that records calls and applies the engine's rule. */
function fakeCommerce({ disposition = null } = {}) {
  const calls = [];
  const state = { status: 'Open', disposition };
  return {
    calls,
    commerce: {
      quality: {
        async updateNcr(id, input) {
          calls.push(['updateNcr', id, input]);
          if (input.disposition) state.disposition = input.disposition;
          return { id, ...state };
        },
        async closeNcr(id) {
          calls.push(['closeNcr', id]);
          if (!state.disposition) {
            const err = new Error(
              `Failed to close NCR: Validation error: Cannot close NCR (${id}): it has no disposition`,
            );
            err.code = 'VALIDATION';
            throw err;
          }
          state.status = 'Closed';
          return { id, ...state };
        },
      },
    },
  };
}

describe('close_ncr -- schema', () => {
  it('takes an optional disposition from the engine enum and an exact quantity', () => {
    assert.deepEqual(Object.keys(closeNcr.inputSchema).sort(), [
      'disposition',
      'dispositionQuantity',
      'ncrId',
    ]);
    assert.ok(parse({ ncrId: NCR_ID }).success);
    for (const disposition of NCR_DISPOSITIONS) {
      assert.ok(parse({ ncrId: NCR_ID, disposition }).success, disposition);
    }
    assert.ok(parse({ ncrId: NCR_ID, disposition: 'scrap', dispositionQuantity: '2.5' }).success);
    assert.equal(parse({ ncrId: NCR_ID, disposition: 'burn' }).success, false);
    assert.equal(
      parse({ ncrId: NCR_ID, disposition: 'scrap', dispositionQuantity: 2.5 }).success,
      false,
      'a quantity is an exact string, never a float',
    );
    assert.equal(
      parse({ ncrId: NCR_ID, disposition: 'scrap', dispositionQuantity: '-1' }).success,
      false,
    );
  });

  it('says a disposition is required', () => {
    assert.match(closeNcr.description, /disposition/i);
    assert.match(closeNcr.description, /requires/i);
  });

  it('calls only methods the binding’s Quality class declares', () => {
    const methods = bindingClassMethods('Quality');
    const called = toolModuleCalls('quality.js', 'quality');
    assert.ok(called.has('updateNcr') && called.has('closeNcr'), `found ${[...called]}`);
    for (const method of called) {
      assert.ok(methods.has(method), `commerce.quality.${method} is not on the binding`);
    }
  });
});

describe('close_ncr -- handler', () => {
  it('without --apply previews and writes nothing', async () => {
    const { commerce, calls } = fakeCommerce();
    const result = await closeNcr.handler({
      commerce,
      params: { ncrId: NCR_ID, disposition: 'scrap' },
      allowApply: false,
    });
    assert.notEqual(result.success, true);
    assert.deepEqual(calls, []);
  });

  it('with a disposition: records it through updateNcr, then closes', async () => {
    const { commerce, calls } = fakeCommerce();
    const result = await closeNcr.handler({
      commerce,
      params: { ncrId: NCR_ID, disposition: 'return_to_vendor', dispositionQuantity: '4.5' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.ncr.status, 'Closed');
    assert.deepEqual(calls, [
      ['updateNcr', NCR_ID, { disposition: 'return_to_vendor', dispositionQuantityExact: '4.5' }],
      ['closeNcr', NCR_ID],
    ]);
  });

  it('without a disposition: closes an NCR that already has one', async () => {
    const { commerce, calls } = fakeCommerce({ disposition: 'Scrap' });
    const result = await closeNcr.handler({
      commerce,
      params: { ncrId: NCR_ID },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.deepEqual(calls, [['closeNcr', NCR_ID]]);
  });

  it('without a disposition on an NCR that has none: the engine refusal surfaces', async () => {
    const { commerce, calls } = fakeCommerce();
    await assert.rejects(
      closeNcr.handler({ commerce, params: { ncrId: NCR_ID }, allowApply: true }),
      (err) => err.code === 'VALIDATION' && /disposition/.test(err.message),
    );
    assert.deepEqual(calls, [['closeNcr', NCR_ID]]);
  });

  it('a quantity without a disposition is refused before any write', async () => {
    const { commerce, calls } = fakeCommerce();
    const result = await closeNcr.handler({
      commerce,
      params: { ncrId: NCR_ID, dispositionQuantity: '1' },
      allowApply: true,
    });
    assert.equal(result.success, false);
    assert.match(result.error, /requires disposition/);
    assert.deepEqual(calls, []);
  });
});
