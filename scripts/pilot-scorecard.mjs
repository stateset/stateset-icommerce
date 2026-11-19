import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';
import { latestAdoptionReports } from '../packages/create-stateset-app/templates/storefront/scripts/adoption-cohorts.js';

function day(value) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value || '')) throw new Error('Supply dates as YYYY-MM-DD');
  const millis = Date.parse(value);
  if (!Number.isFinite(millis) || new Date(millis).toISOString().slice(0, 10) !== value)
    throw new Error('Invalid calendar date');
  return value;
}

function label(value) {
  if (typeof value !== 'string' || !/^[a-z0-9][a-z0-9_-]{0,47}$/.test(value))
    throw new Error('Use short lowercase labels for IDs, channels, alternatives and reasons');
  return value;
}

function tally() {
  return {
    enrolled: 0,
    won: 0,
    lostToAlternative: 0,
    noPurchase: 0,
    undecided: 0,
    activatedWins: 0,
  };
}

function rate(numerator, denominator) {
  return {
    numerator,
    denominator,
    percent: denominator ? +((100 * numerator) / denominator).toFixed(1) : null,
  };
}

function reportCounts(counts) {
  return {
    ...counts,
    winsWithoutVerifiedActivation: counts.won - counts.activatedWins,
    decidedWinRate: rate(counts.won, counts.enrolled - counts.undecided),
    activatedWinningTeamsPerEnrolled: rate(counts.activatedWins, counts.enrolled),
  };
}

function add(counts, outcome, activatedWin) {
  counts.enrolled++;
  counts[outcome]++;
  if (activatedWin) counts.activatedWins++;
}

async function latestReview(row, asOf, readReport) {
  const reports = [];
  for (const review of row.reviews ?? []) {
    if (day(review.reviewedOn) > asOf) continue;
    if (!review.report || typeof review.report !== 'string')
      throw new Error('Supply a reviewed report file');
    const report = await readReport(review.report);
    if (report.installationId !== row.installationId || !row.installationId)
      throw new Error('Reviewed report belongs to a different installation');
    if (Date.parse(report.observedAt) >= Date.parse(review.reviewedOn) + 86400000)
      throw new Error('Report was produced after its review date');
    reports.push(report);
  }
  return latestAdoptionReports(reports)[0];
}

export async function pilotScorecard(rows, { from, through, asOf = through }, readReport) {
  if (day(from) > day(through) || day(through) > day(asOf))
    throw new Error('Expected enrollment start <= enrollment end <= observation date');
  if (!Array.isArray(rows)) throw new Error('Supply an array of evaluation rows');

  const totals = tally();
  const evidenceAmongWins = {
    missingReviewedReport: 0,
    incompleteCoverage: 0,
    noQualifyingLiveSale: 0,
    verifiedActivation: 0,
  };
  const ids = new Set(),
    stores = new Set();
  const channels = new Map(),
    alternatives = new Map(),
    losses = new Map(),
    reasons = new Map();
  for (const row of rows) {
    if (ids.has(label(row.id))) throw new Error('Duplicate evaluation ID');
    ids.add(row.id);
    if (row.installationId != null) {
      if (
        !/^[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/i.test(row.installationId) ||
        stores.has(row.installationId.toLowerCase())
      )
        throw new Error('Invalid or duplicate installation ID');
      stores.add(row.installationId.toLowerCase());
    }
    day(row.enrolledOn);
    label(row.source);
    if (
      !Array.isArray(row.alternatives) ||
      new Set(row.alternatives).size !== row.alternatives.length
    )
      throw new Error('Supply a list of distinct alternatives');
    for (const name of row.alternatives)
      if (['stateset', 'no-purchase'].includes(label(name))) throw new Error('Invalid alternative');
    if (row.decision) {
      const { on, selected, reason } = row.decision;
      if (day(on) < row.enrolledOn) throw new Error('Decision precedes enrollment');
      if (!['stateset', 'no-purchase', ...row.alternatives].includes(label(selected)))
        throw new Error('Record the selected platform in the shortlist');
      label(reason);
    }
    if (row.enrolledOn < from || row.enrolledOn > through) continue;
    const decision = row.decision?.on <= asOf ? row.decision : null;
    let outcome = 'undecided';
    if (decision) {
      if (decision.selected === 'stateset') outcome = 'won';
      else if (decision.selected === 'no-purchase') outcome = 'noPurchase';
      else {
        outcome = 'lostToAlternative';
        losses.set(decision.selected, (losses.get(decision.selected) || 0) + 1);
      }
      if (outcome !== 'won') reasons.set(decision.reason, (reasons.get(decision.reason) || 0) + 1);
    }
    const report = await latestReview(row, asOf, readReport);
    const activatedWin =
      outcome === 'won' &&
      report?.coverage === 'complete' &&
      report.counts.live > 0 &&
      Date.parse(report.firstVerifiedSaleAt) >= Date.parse(row.enrolledOn);
    if (outcome === 'won') {
      if (!report) evidenceAmongWins.missingReviewedReport++;
      else if (report.coverage !== 'complete') evidenceAmongWins.incompleteCoverage++;
      else if (activatedWin) evidenceAmongWins.verifiedActivation++;
      else evidenceAmongWins.noQualifyingLiveSale++;
    }
    add(totals, outcome, activatedWin);
    if (!channels.has(row.source)) channels.set(row.source, tally());
    add(channels.get(row.source), outcome, activatedWin);
    for (const name of row.alternatives) {
      if (!alternatives.has(name)) alternatives.set(name, tally());
      add(alternatives.get(name), outcome, activatedWin);
    }
  }
  const breakdown = (groups) =>
    Object.fromEntries([...groups].sort().map(([name, counts]) => [name, reportCounts(counts)]));
  return {
    cohort: 'Voluntary developer-led StateSet evaluations',
    from,
    through,
    asOf,
    ...reportCounts(totals),
    evidenceAmongWins,
    byAcquisitionSource: breakdown(channels),
    amongShortlistsContaining: breakdown(alternatives),
    lossesBySelectedAlternative: Object.fromEntries([...losses].sort()),
    nonWinReasons: Object.fromEntries([...reasons].sort()),
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const { values } = parseArgs({
      options: {
        ledger: { type: 'string' },
        from: { type: 'string' },
        through: { type: 'string' },
        'as-of': { type: 'string' },
      },
    });
    if (!values.ledger)
      throw new Error(
        'Supply --ledger FILE --from YYYY-MM-DD --through YYYY-MM-DD [--as-of YYYY-MM-DD]',
      );
    const readJson = async (path) => JSON.parse(await readFile(path, 'utf8'));
    const result = await pilotScorecard(
      await readJson(values.ledger),
      { ...values, asOf: values['as-of'] },
      (file) => readJson(resolve(dirname(values.ledger), file)),
    );
    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
