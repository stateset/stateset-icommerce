import test from 'node:test';
import assert from 'node:assert/strict';
import { pilotScorecard } from './pilot-scorecard.mjs';

const window = { from: '2026-02-01', through: '2026-02-28', asOf: '2026-03-07' };
const store = '12345678-1234-1234-1234-123456789abc';
const row = (id, selected) => ({
  id,
  enrolledOn: '2026-02-02',
  source: 'referral',
  alternatives: ['next-commerce', 'whop'],
  ...(selected && { decision: { on: '2026-02-10', selected, reason: 'workflow-fit' } }),
});
function evidence(override = {}) {
  return {
    schema: 'stateset.storefront-adoption.v1',
    installationId: store,
    source: 'referral',
    enrolledAt: '2026-02-02T00:00:00.000Z',
    observedAt: '2026-02-12T00:00:00.000Z',
    coverage: 'complete',
    scope: 'primary card checkouts placed since enrollment',
    counts: {
      live: 1,
      sandbox: 0,
      unavailable: 0,
      mismatch: 0,
      unchecked: 0,
      otherRails: 0,
      supplemental: 0,
    },
    firstVerifiedSaleAt: '2026-02-10T00:00:00.000Z',
    hoursToFirstVerifiedSale: 192,
    repeatVerifiedSaleAt: null,
    ...override,
  };
}

test('all enrollments survive, including losses, no purchase, undisclosed shortlists and pending decisions', async () => {
  const result = await pilotScorecard(
    [
      {
        ...row('a', 'stateset'),
        installationId: store,
        reviews: [{ reviewedOn: '2026-02-13', report: 'one' }],
      },
      row('b', 'next-commerce'),
      row('c', 'no-purchase'),
      {
        ...row('d', 'stateset'),
        decision: { on: '2026-03-08', selected: 'stateset', reason: 'flexibility' },
      },
      { ...row('e'), alternatives: [] },
      { ...row('f', 'stateset'), enrolledOn: '2026-01-31' },
      row('g', 'stateset'),
    ],
    window,
    async (path) => {
      assert.equal(path, 'one');
      return evidence();
    },
  );
  assert.deepEqual(result.decidedWinRate, { numerator: 2, denominator: 4, percent: 50 });
  assert.deepEqual(result.activatedWinningTeamsPerEnrolled, {
    numerator: 1,
    denominator: 6,
    percent: 16.7,
  });
  assert.equal(result.winsWithoutVerifiedActivation, 1);
  assert.deepEqual(result.evidenceAmongWins, {
    missingReviewedReport: 1,
    incompleteCoverage: 0,
    noQualifyingLiveSale: 0,
    verifiedActivation: 1,
  });
  assert.equal(result.undecided, 2);
  assert.equal(result.noPurchase, 1);
  assert.deepEqual(result.lossesBySelectedAlternative, { 'next-commerce': 1 });
  assert.equal(result.nonWinReasons['workflow-fit'], 2);
  assert.equal(result.amongShortlistsContaining.whop.enrolled, 5);
  assert.equal(result.byAcquisitionSource.referral.enrolled, 6);
});

test('use latest reviewed evidence by the observation deadline and require complete, post-enrollment live activity', async () => {
  const item = {
    ...row('a', 'stateset'),
    installationId: store,
    reviews: [
      { reviewedOn: '2026-02-14', report: 'partial' },
      { reviewedOn: '2026-02-13', report: 'old' },
      { reviewedOn: '2026-03-06', report: 'later' },
    ],
  };
  const reports = {
    old: evidence(),
    partial: evidence({
      coverage: 'partial',
      observedAt: '2026-02-14T00:00:00.000Z',
      counts: { ...evidence().counts, unchecked: 1 },
    }),
    later: evidence({ observedAt: '2026-03-05T12:00:00.000Z' }),
  };
  const check = (asOf) =>
    pilotScorecard([item], { ...window, asOf }, async (path) => reports[path]);
  assert.equal((await check('2026-02-28')).activatedWins, 0);
  assert.equal((await check('2026-03-07')).activatedWins, 1);
  item.reviews = [{ reviewedOn: '2026-02-12', report: 'old' }];
  reports.old = evidence({
    counts: { ...evidence().counts, live: 0, sandbox: 1 },
    firstVerifiedSaleAt: null,
    hoursToFirstVerifiedSale: null,
  });
  assert.equal((await check('2026-03-07')).activatedWins, 0);
  item.enrolledOn = '2026-02-11';
  item.decision.on = '2026-02-12';
  reports.old = evidence();
  assert.equal((await check('2026-03-07')).activatedWins, 0);
});

test('reject duplicate teams, reused evidence, invalid dates, wrong-store reports, and impossible review times', async () => {
  await assert.rejects(pilotScorecard([row('a'), row('a')], window), /Duplicate/);
  const a = {
    ...row('a', 'stateset'),
    installationId: store,
    reviews: [{ reviewedOn: '2026-02-12', report: 'x' }],
  };
  await assert.rejects(
    pilotScorecard([a, { ...a, id: 'b' }], window, async () => evidence()),
    /duplicate installation/i,
  );
  await assert.rejects(pilotScorecard([row('a', 'other')], window), /shortlist/);
  await assert.rejects(
    pilotScorecard([{ ...row('a'), enrolledOn: '2026-02-31' }], window),
    /calendar date/,
  );
  await assert.rejects(
    pilotScorecard([a], window, async () =>
      evidence({ installationId: store.replace('abc', 'aaa') }),
    ),
    /different installation/,
  );
  await assert.rejects(
    pilotScorecard([a], window, async () => evidence({ observedAt: '2026-03-01T00:00:00.000Z' })),
    /after its review/,
  );
});

test('empty and undecided cohorts report the actual denominator', async () => {
  const empty = await pilotScorecard([], window);
  assert.deepEqual(empty.decidedWinRate, { numerator: 0, denominator: 0, percent: null });
  const open = await pilotScorecard([row('a')], window);
  assert.equal(open.activatedWinningTeamsPerEnrolled.percent, 0);
  assert.equal(open.decidedWinRate.percent, null);
});

import { createHash, generateKeyPairSync, randomBytes, randomUUID } from 'node:crypto';
import { signReview, summarizeOperation } from './pilot-scorecard-operations.mjs';

// Generated, synthetic test data exercises validation only. It is never enrolled as merchant evidence.
function embeddedFixture(team = 'ops', workflow = 'order-cancellation') {
  const matchKey = randomBytes(32),
    merchant = generateKeyPairSync('ed25519'),
    maintainer = generateKeyPairSync('ed25519');
  const encode = (key) => key.publicKey.export({ type: 'spki', format: 'der' }).toString('base64');
  const embeddedAuthority = {
    maintainerPublicKey: encode(maintainer),
    matchingKeyId: createHash('sha256').update(matchKey).digest('hex'),
  };
  const evaluation = {
    ...row(team, 'stateset'),
    track: 'embedded-operations',
    workflow,
    enrolledAt: '2026-02-02T13:00:00.000Z',
    merchantPublicKey: encode(merchant),
    operationReview: { merchant: 'merchant-review', maintainer: 'maintainer-review' },
  };
  const id = randomUUID(),
    timestamp = '2026-02-04T16:30:00.000Z',
    refund = workflow === 'customer-refund';
  const source = {
    enrolledAt: evaluation.enrolledAt,
    provenance: 'live-merchant-operation',
    receiptEnvironment: 'production',
    productionStoreId: 'store:merchant-main',
    receipt: {
      receipt_id: randomUUID(),
      command_type: refund ? 'payments.create_refund' : 'orders.transition',
      status: 'succeeded',
      aggregate_type: refund ? 'refund' : 'order',
      aggregate_id: id,
      audit_hash: 'a'.repeat(64),
      event_ids: [randomUUID()],
      economic_context: { store_id: 'store:merchant-main' },
      result: {
        id,
        status: refund ? 'pending' : 'cancelled',
        customerEmail: 'sensitive@example.com',
      },
      completed_at: timestamp,
    },
    external: {
      system: 'merchant-platform',
      account: 'private-shop-id',
      operationId: 'private-operation-id',
      outcome: refund ? 'refund-completed' : 'order-cancelled',
      completedAt: timestamp,
      environment: 'production',
    },
  };
  const claim = summarizeOperation(team, workflow, source, matchKey);
  const files = {
    'merchant-review': signReview(claim, 'merchant', '2026-02-09', merchant.privateKey),
    'maintainer-review': signReview(claim, 'maintainer', '2026-03-03', maintainer.privateKey),
  };
  const run = (rows = [evaluation], args = {}) =>
    pilotScorecard(rows, { ...window, embeddedAuthority, ...args }, async (file) => files[file]);
  return {
    claim,
    evaluation,
    source,
    embeddedAuthority,
    matchKey,
    merchant,
    maintainer,
    files,
    run,
    encode,
  };
}

test('embedded decisions and two-person operations have their own denominators and cutoff', async () => {
  const { evaluation, run, claim } = embeddedFixture();
  const entry = (team, decision) => ({
    ...row(team, decision),
    track: 'embedded-operations',
    workflow: 'customer-refund',
  });
  const rows = [
    evaluation,
    entry('other', 'whop'),
    entry('abandoned', 'no-purchase'),
    entry('open'),
    row('checkout', 'stateset'),
  ];
  const report = await run(rows),
    embedded = report.embeddedOperations;
  assert.equal(report.allTracksEnrolled, 5);
  assert.equal(report.enrolled, 1);
  assert.equal(report.activatedWins, 0);
  assert.deepEqual(embedded.decidedWinRate, { numerator: 1, denominator: 3, percent: 33.3 });
  assert.deepEqual(embedded.activatedWinningTeamsPerEnrolled, {
    numerator: 1,
    denominator: 4,
    percent: 25,
  });
  assert.equal(embedded.byWorkflow['order-cancellation'].activatedWins, 1);
  assert.equal(embedded.amongShortlistsContaining.whop.enrolled, 4);
  assert.equal(embedded.evidenceAmongWins.humanReviewedActivation, 1);
  assert.equal(embedded.winsWithoutVerifiedActivation, undefined);
  assert.doesNotMatch(
    JSON.stringify(claim),
    /sensitive|private-shop|private-operation|receipt_id|@/,
  );
  assert.doesNotMatch(JSON.stringify(report), /receiptTag|operationId|PublicKey|signature/);
  const early = (await run(rows, { asOf: '2026-02-28' })).embeddedOperations;
  assert.equal(early.activatedWins, 0);
  assert.equal(early.evidenceAmongWins.missingMaintainerReview, 1);
  assert.equal(
    (await run([{ ...evaluation, operationReview: { maintainer: 'maintainer-review' } }]))
      .embeddedOperations.evidenceAmongWins.missingMerchantConfirmation,
    1,
  );
  assert.equal(
    (await run([{ ...evaluation, operationReview: undefined }])).embeddedOperations
      .evidenceAmongWins.noReviewedOperation,
    1,
  );
  const before = embeddedFixture();
  before.source.enrolledAt = before.evaluation.enrolledAt = '2026-02-04T16:31:00.000Z';
  before.evaluation.enrolledOn = '2026-02-04';
  const earlyClaim = summarizeOperation(
    'ops',
    before.evaluation.workflow,
    before.source,
    before.matchKey,
  );
  for (const role of ['merchant', 'maintainer'])
    before.files[role + '-review'] = signReview(
      earlyClaim,
      role,
      '2026-02-09',
      before[role].privateKey,
    );
  assert.equal((await before.run()).embeddedOperations.evidenceAmongWins.beforeEnrollment, 1);
});

test('both reviews bind the same redacted case, trusted keys, enrollment, role and live origin', async () => {
  const f = embeddedFixture('refund-team', 'customer-refund'),
    { evaluation, files, claim, merchant, run, maintainer } = f;
  assert.equal((await run()).embeddedOperations.activatedWins, 1);
  await assert.rejects(run([evaluation], { embeddedAuthority: undefined }), /does not match/);
  const impostor = generateKeyPairSync('ed25519');
  files['maintainer-review'] = signReview(claim, 'maintainer', '2026-03-03', impostor.privateKey);
  await assert.rejects(run(), /signature/);
  files['maintainer-review'] = signReview(claim, 'maintainer', '2026-03-03', merchant.privateKey);
  await assert.rejects(run(), /signature/);
  await assert.rejects(
    run([{ ...evaluation, merchantPublicKey: f.encode(maintainer) }]),
    /distinct/,
  );
  files['maintainer-review'] = signReview(
    { ...claim, externalTag: 'a'.repeat(64) },
    'maintainer',
    '2026-03-03',
    maintainer.privateKey,
  );
  await assert.rejects(run(), /different operations/);
  files['maintainer-review'] = signReview(claim, 'maintainer', '2026-03-03', maintainer.privateKey);
  files['maintainer-review'].reviewedOn = '2026-02-10';
  await assert.rejects(run(), /signature/);
  files['maintainer-review'] = files['merchant-review'];
  await assert.rejects(run(), /role/);
  assert.throws(
    () =>
      signReview({ ...claim, origin: 'fixture' }, 'merchant', '2026-02-09', merchant.privateKey),
    /Invalid live/,
  );
  assert.throws(
    () =>
      signReview(
        { ...claim, email: 'private@example.com' },
        'merchant',
        '2026-02-09',
        merchant.privateKey,
      ),
    /fields/,
  );
  assert.throws(() => signReview(claim, 'merchant', '2026-02-01', merchant.privateKey), /precedes/);
});

test('unapplied, sandbox, fixture, unrelated, and unfinished inputs cannot produce an eligible claim', () => {
  const { source, matchKey } = embeddedFixture(),
    prepare = (value) => summarizeOperation('ops', 'order-cancellation', value, matchKey);
  for (const overrides of [
    { provenance: 'fixture' },
    { receiptEnvironment: 'sandbox' },
    { productionStoreId: 'unrelated' },
    { external: { ...source.external, environment: 'sandbox' } },
    { external: { ...source.external, outcome: 'pending' } },
    { receipt: { ...source.receipt, status: 'preview' } },
    { receipt: { ...source.receipt, event_ids: [] } },
    { receipt: { ...source.receipt, result: { ...source.receipt.result, status: 'confirmed' } } },
    {
      receipt: { ...source.receipt, economic_context: { store_id: 'store:launch-drill' } },
      productionStoreId: 'store:launch-drill',
    },
  ])
    assert.throws(() => prepare({ ...source, ...overrides }), /Require/);
});

test('one enrollment per team, role key and observed operation, even across enrollment windows', async () => {
  const { evaluation, files, run, claim, encode, maintainer } = embeddedFixture(),
    other = generateKeyPairSync('ed25519');
  await assert.rejects(run([evaluation, row(evaluation.id)]), /Duplicate/);
  await assert.rejects(run([evaluation, { ...evaluation, id: 'other' }]), /keys must be distinct/);
  const otherRow = {
    ...evaluation,
    id: 'previous',
    enrolledOn: '2026-01-20',
    enrolledAt: '2026-01-20T13:00:00.000Z',
    merchantPublicKey: encode(other),
    operationReview: { merchant: 'other-merchant', maintainer: 'other-maintainer' },
  };
  for (const retain of ['receiptTag', 'externalTag']) {
    const second = {
      ...claim,
      team: otherRow.id,
      enrolledAt: otherRow.enrolledAt,
      receiptTag: '0'.repeat(64),
      externalTag: '1'.repeat(64),
      [retain]: claim[retain],
    };
    files['other-merchant'] = signReview(second, 'merchant', '2026-02-09', other.privateKey);
    files['other-maintainer'] = signReview(
      second,
      'maintainer',
      '2026-03-03',
      maintainer.privateKey,
    );
    await assert.rejects(run([otherRow, evaluation]), /Duplicate embedded operation/);
  }
  await assert.rejects(run([{ ...evaluation, installationId: store }]), /separate evidence/);
  await assert.rejects(run([{ ...evaluation, track: 'storefront' }]), /requires an embedded/);
});
