/**
 * EDI documents API tests for @stateset/embedded Node.js bindings.
 *
 * Trading-partner document tracking: 850/855/856/810 lifecycle, filtering,
 * status transitions, and the aggregate summary. Also covers the
 * generalLedger.listPeriods binding used by the admin close page.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

test('EdiDocuments: full lifecycle', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('ediDocuments API exists', () => {
    assert.ok(commerce.ediDocuments, 'ediDocuments API should exist');
  });

  let po;
  await t.test('create defaults to an inbound pending document', async () => {
    po = await commerce.ediDocuments.create({
      documentType: '850',
      partner: 'ACME-RETAIL',
      reference: 'PO-1001',
      payload: 'ST*850*0001~BEG*00*SA*PO-1001~',
    });
    assert.ok(po.id);
    assert.equal(po.documentType, '850');
    assert.equal(po.direction, 'inbound');
    assert.equal(po.status, 'pending');
    assert.equal(po.partner, 'ACME-RETAIL');
    assert.equal(po.reference, 'PO-1001');
    assert.ok(po.createdAt);
    assert.ok(po.updatedAt);
  });

  let asn;
  await t.test('create accepts an explicit outbound direction', async () => {
    asn = await commerce.ediDocuments.create({
      documentType: '856',
      direction: 'outbound',
      partner: 'ACME-RETAIL',
      reference: 'PO-1001',
    });
    assert.equal(asn.direction, 'outbound');
    assert.equal(asn.status, 'pending');
  });

  await t.test('create rejects an invalid direction', async () => {
    await assert.rejects(
      commerce.ediDocuments.create({ documentType: '810', direction: 'sideways' }),
      /Invalid EDI direction/
    );
  });

  await t.test('get finds the document; missing id resolves null', async () => {
    const found = await commerce.ediDocuments.get(po.id);
    assert.ok(found);
    assert.equal(found.id, po.id);
    assert.equal(found.payload, 'ST*850*0001~BEG*00*SA*PO-1001~');

    const missing = await commerce.ediDocuments.get('00000000-0000-0000-0000-000000000000');
    assert.equal(missing, null);

    await assert.rejects(commerce.ediDocuments.get('not-a-uuid'), /Invalid UUID/);
  });

  await t.test('list returns both documents and honors filters', async () => {
    const all = await commerce.ediDocuments.list();
    assert.equal(all.length, 2);

    const inbound = await commerce.ediDocuments.list({ direction: 'inbound' });
    assert.equal(inbound.length, 1);
    assert.equal(inbound[0].id, po.id);

    const asns = await commerce.ediDocuments.list({ documentType: '856' });
    assert.equal(asns.length, 1);
    assert.equal(asns[0].id, asn.id);

    const byPartner = await commerce.ediDocuments.list({ partner: 'ACME-RETAIL' });
    assert.equal(byPartner.length, 2);

    const limited = await commerce.ediDocuments.list({ limit: 1 });
    assert.equal(limited.length, 1);

    await assert.rejects(commerce.ediDocuments.list({ status: 'bogus' }), /Invalid EDI status/);
  });

  await t.test('setStatus transitions and records error detail', async () => {
    const processed = await commerce.ediDocuments.setStatus(po.id, 'processed');
    assert.equal(processed.status, 'processed');
    assert.equal(processed.errorMessage ?? null, null);

    const errored = await commerce.ediDocuments.setStatus(
      asn.id,
      'error',
      'Missing mandatory segment: BSN'
    );
    assert.equal(errored.status, 'error');
    assert.equal(errored.errorMessage, 'Missing mandatory segment: BSN');

    const errorDocs = await commerce.ediDocuments.list({ status: 'error' });
    assert.equal(errorDocs.length, 1);
    assert.equal(errorDocs[0].id, asn.id);

    await assert.rejects(commerce.ediDocuments.setStatus(po.id, 'bogus'), /Invalid EDI status/);
  });

  await t.test('summary aggregates counts by status and type', async () => {
    const summary = await commerce.ediDocuments.summary();
    assert.equal(summary.total, 2);

    const statusCounts = Object.fromEntries(summary.byStatus.map((c) => [c.key, c.count]));
    assert.equal(statusCounts.processed, 1);
    assert.equal(statusCounts.error, 1);

    const typeCounts = Object.fromEntries(summary.byType.map((c) => [c.key, c.count]));
    assert.equal(typeCounts['850'], 1);
    assert.equal(typeCounts['856'], 1);
  });
});

test('GeneralLedger: listPeriods', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('returns created periods and honors filters', async () => {
    const gl = commerce.generalLedger;
    assert.equal(typeof gl.listPeriods, 'function', 'listPeriods binding should exist');

    assert.deepEqual(await gl.listPeriods(), []);

    const period = await gl.createPeriod({
      periodName: '2026-07',
      fiscalYear: 2026,
      periodNumber: 7,
      startDate: '2026-07-01',
      endDate: '2026-07-31',
    });

    const all = await gl.listPeriods();
    assert.equal(all.length, 1);
    assert.equal(all[0].id, period.id);
    assert.equal(all[0].periodName, '2026-07');
    assert.equal(all[0].startDate, '2026-07-01');
    assert.equal(all[0].endDate, '2026-07-31');

    const byYear = await gl.listPeriods({ fiscalYear: 2026 });
    assert.equal(byYear.length, 1);

    const otherYear = await gl.listPeriods({ fiscalYear: 2027 });
    assert.equal(otherYear.length, 0);

    const byStatus = await gl.listPeriods({ status: period.status });
    assert.equal(byStatus.length, 1);

    await assert.rejects(gl.listPeriods({ status: 'bogus' }), /Invalid period status/);
  });
});
