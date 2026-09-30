/**
 * Closing a non-conformance report requires a disposition.
 *
 * A closed NCR is the quality record of what was done with the material, so
 * the engine refuses to close one without a disposition. `updateNcr` is how
 * the binding records one; before it existed, `closeNcr` was the only NCR
 * write after create, so an NCR could never be closed from JavaScript.
 */

'use strict';

const assert = require('node:assert/strict');
const { test } = require('node:test');
const { Commerce } = require('../index.js');

async function openNcr(commerce, sku) {
  return commerce.quality.createNcr({
    source: 'internal_audit',
    severity: 'major',
    sku,
    quantityAffected: 5,
    description: 'scratched units',
  });
}

test('closeNcr without a disposition is refused with a validation error', async () => {
  const commerce = new Commerce(':memory:');
  const ncr = await openNcr(commerce, 'NCR-NO-DISP');

  await assert.rejects(commerce.quality.closeNcr(ncr.id), (err) => {
    assert.equal(err.code, 'VALIDATION');
    assert.match(err.message, /disposition/);
    return true;
  });
  await assert.rejects(commerce.quality.updateNcr(ncr.id, { status: 'closed' }), (err) => {
    assert.equal(err.code, 'VALIDATION');
    assert.match(err.message, /disposition/);
    return true;
  });
  const still = await commerce.quality.getNcr(ncr.id);
  assert.equal(still.status, 'Open');
  assert.equal(still.closedAt, undefined);
});

test('updateNcr records a disposition, then closeNcr closes', async () => {
  const commerce = new Commerce(':memory:');
  const ncr = await openNcr(commerce, 'NCR-DISP');

  const updated = await commerce.quality.updateNcr(ncr.id, {
    disposition: 'return_to_vendor',
    dispositionQuantityExact: '4.5',
    rootCause: 'supplier tooling',
  });
  assert.equal(updated.status, 'Open');
  assert.equal(updated.disposition, 'ReturnToVendor');
  assert.equal(updated.dispositionQuantityExact, '4.5');
  assert.equal(updated.rootCause, 'supplier tooling');

  const closed = await commerce.quality.closeNcr(ncr.id);
  assert.equal(closed.status, 'Closed');
  assert.ok(closed.closedAt, 'a closed NCR records when it closed');
  // Re-closing is a no-op.
  assert.equal((await commerce.quality.closeNcr(ncr.id)).status, 'Closed');
});

test('one updateNcr call may set the disposition and close', async () => {
  const commerce = new Commerce(':memory:');
  const ncr = await openNcr(commerce, 'NCR-ONE-CALL');
  const closed = await commerce.quality.updateNcr(ncr.id, { disposition: 'Scrap', status: 'closed' });
  assert.equal(closed.status, 'Closed');
  assert.equal(closed.disposition, 'Scrap');
});

test('updateNcr refuses unknown enum spellings', async () => {
  const commerce = new Commerce(':memory:');
  const ncr = await openNcr(commerce, 'NCR-BAD-ENUM');
  await assert.rejects(
    commerce.quality.updateNcr(ncr.id, { disposition: 'burn' }),
    (err) => err.code === 'VALIDATION' && /Invalid NCR disposition 'burn'/.test(err.message),
  );
  await assert.rejects(
    commerce.quality.updateNcr(ncr.id, { status: 'done' }),
    (err) => err.code === 'VALIDATION' && /Invalid NCR status 'done'/.test(err.message),
  );
});
