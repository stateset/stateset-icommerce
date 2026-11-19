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
