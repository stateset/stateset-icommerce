import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';
import { isDeepStrictEqual } from 'node:util';
import { checkedReview, publicKey, validateWorkflow } from './pilot-scorecard-operations.mjs';
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

function reportCounts(counts, embedded = false) {
  return {
    ...counts,
    [embedded ? 'winsWithoutReviewedActivation' : 'winsWithoutVerifiedActivation']:
      counts.won - counts.activatedWins,
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

function cohort(evidenceAmongWins) {
  return {
    totals: tally(),
    evidenceAmongWins,
    channels: new Map(),
    alternatives: new Map(),
    losses: new Map(),
    reasons: new Map(),
    workflows: new Map(),
  };
}

function breakdown(groups, embedded = false) {
  return Object.fromEntries(
    [...groups].sort().map(([name, counts]) => [name, reportCounts(counts, embedded)]),
  );
}

function summarize(group, embedded = false) {
  return {
    ...reportCounts(group.totals, embedded),
    evidenceAmongWins: group.evidenceAmongWins,
    byAcquisitionSource: breakdown(group.channels, embedded),
    amongShortlistsContaining: breakdown(group.alternatives, embedded),
    lossesBySelectedAlternative: Object.fromEntries([...group.losses].sort()),
    nonWinReasons: Object.fromEntries([...group.reasons].sort()),
  };
}

async function reviewedOperation(row, asOf, readReport, authority, receipts, externalOperations) {
  const files = row.operationReview ?? {};
  if (
    !files ||
    typeof files !== 'object' ||
    Array.isArray(files) ||
    Object.keys(files).some((key) => !['merchant', 'maintainer'].includes(key))
  )
    throw new Error('Supply merchant and maintainer private review file paths');
  const evidence = {};
  for (const role of ['merchant', 'maintainer']) {
    if (files[role] === undefined) continue;
    if (typeof files[role] !== 'string' || !files[role])
      throw new Error('Supply private review file paths');
    const record = await readReport(files[role]);
    const claim = checkedReview(row, role, record, authority);
    if (record.reviewedOn <= asOf) evidence[role] = claim;
  }
  if (!evidence.merchant && !evidence.maintainer) return 'noReviewedOperation';
  if (!evidence.merchant) return 'missingMerchantConfirmation';
  if (!evidence.maintainer) return 'missingMaintainerReview';
  if (!isDeepStrictEqual(evidence.merchant, evidence.maintainer))
    throw new Error('Merchant and maintainer confirmed different operations');
  const claim = evidence.merchant;
  if (receipts.has(claim.receiptTag) || externalOperations.has(claim.externalTag))
    throw new Error('Duplicate embedded operation evidence across teams');
  receipts.add(claim.receiptTag);
  externalOperations.add(claim.externalTag);
  if (
    !claim.afterEnrollment ||
    claim.receiptDay < row.enrolledOn ||
    claim.externalDay < row.enrolledOn
  )
    return 'beforeEnrollment';
  return 'humanReviewedActivation';
}

export async function pilotScorecard(
  rows,
  { from, through, asOf = through, embeddedAuthority },
  readReport,
) {
  if (day(from) > day(through) || day(through) > day(asOf))
    throw new Error('Expected enrollment start <= enrollment end <= observation date');
  if (!Array.isArray(rows)) throw new Error('Supply an array of evaluation rows');

  const checkout = cohort({
    missingReviewedReport: 0,
    incompleteCoverage: 0,
    noQualifyingLiveSale: 0,
    verifiedActivation: 0,
  });
  const embedded = cohort({
    noReviewedOperation: 0,
    missingMerchantConfirmation: 0,
    missingMaintainerReview: 0,
    beforeEnrollment: 0,
    humanReviewedActivation: 0,
  });
  const ids = new Set(),
    stores = new Set(),
    merchantKeys = new Set(),
    receipts = new Set(),
    externalOperations = new Set();
  for (const row of rows) {
    if (ids.has(label(row.id))) throw new Error('Duplicate evaluation ID');
    ids.add(row.id);
    const track = row.track ?? 'storefront';
    if (!['storefront', 'embedded-operations'].includes(track))
      throw new Error('Unknown primary pilot track');
    const isEmbedded = track === 'embedded-operations';
    const { totals, evidenceAmongWins, channels, alternatives, losses, reasons, workflows } =
      isEmbedded ? embedded : checkout;
    if (isEmbedded) {
      validateWorkflow(row.workflow);
      if (
        row.enrolledAt !== undefined &&
        (typeof row.enrolledAt !== 'string' ||
          !Number.isFinite(Date.parse(row.enrolledAt)) ||
          new Date(row.enrolledAt).toISOString() !== row.enrolledAt ||
          row.enrolledAt.slice(0, 10) !== row.enrolledOn)
      ) {
        throw new Error('Embedded enrollment timestamp must match its UTC enrollment day');
      }
      if (row.reviews != null || row.installationId != null)
        throw new Error('Use separate evidence for each pilot track');
      if (row.merchantPublicKey != null) {
        publicKey(row.merchantPublicKey);
        if (
          merchantKeys.has(row.merchantPublicKey) ||
          row.merchantPublicKey === embeddedAuthority?.maintainerPublicKey
        )
          throw new Error('Merchant keys must be distinct across teams and reviewers');
        merchantKeys.add(row.merchantPublicKey);
      }
    } else if (row.operationReview != null || row.merchantPublicKey != null)
      throw new Error('Embedded evidence requires an embedded-operations enrollment');

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
    const operationStatus = isEmbedded
      ? await reviewedOperation(
          row,
          asOf,
          readReport,
          embeddedAuthority,
          receipts,
          externalOperations,
        )
      : undefined;
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
    let activatedWin = false;
    if (isEmbedded) {
      if (outcome === 'won') {
        evidenceAmongWins[operationStatus]++;
        activatedWin = operationStatus === 'humanReviewedActivation';
      }
      if (!workflows.has(row.workflow)) workflows.set(row.workflow, tally());
      add(workflows.get(row.workflow), outcome, activatedWin);
    } else {
      const report = await latestReview(row, asOf, readReport);
      activatedWin =
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
    }
    add(totals, outcome, activatedWin);
    if (!channels.has(row.source)) channels.set(row.source, tally());
    add(channels.get(row.source), outcome, activatedWin);
    for (const name of row.alternatives) {
      if (!alternatives.has(name)) alternatives.set(name, tally());
      add(alternatives.get(name), outcome, activatedWin);
    }
  }
  return {
    cohort: 'Voluntary developer-led StateSet evaluations',
    from,
    through,
    asOf,
    ...summarize(checkout),
    embeddedOperations: {
      cohort: 'Embedded operations evaluations',
      activationEvidence: 'merchant-confirmed, maintainer-reviewed production operation',
      ...summarize(embedded, true),
      byWorkflow: breakdown(embedded.workflows, true),
    },
    allTracksEnrolled: checkout.totals.enrolled + embedded.totals.enrolled,
  };
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const { values } = parseArgs({
      options: {
        ledger: { type: 'string' },
        'embedded-authority': { type: 'string' },
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
      {
        ...values,
        asOf: values['as-of'],
        embeddedAuthority: values['embedded-authority']
          ? await readJson(values['embedded-authority'])
          : undefined,
      },
      (file) => readJson(resolve(dirname(values.ledger), file)),
    );
    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
