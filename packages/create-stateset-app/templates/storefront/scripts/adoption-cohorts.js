import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { isDeepStrictEqual } from 'node:util';

const day = 86400000;

function canonicalTime(value) {
  if (typeof value !== 'string') return NaN;
  const time = Date.parse(value);
  return Number.isFinite(time) && new Date(time).toISOString() === value ? time : NaN;
}

// Use one latest voluntarily supplied report per enrolled store. Young cohorts and
// incomplete provider checks have their own counts and never dilute either rate.
export function latestAdoptionReports(reports) {
  const stores = new Map();
  for (const report of reports) {
    const {
      schema,
      installationId,
      source,
      enrolledAt,
      observedAt,
      coverage,
      scope,
      counts,
      hoursToFirstVerifiedSale,
      firstVerifiedSaleAt,
      repeatVerifiedSaleAt,
    } = report;
    const start = canonicalTime(enrolledAt),
      end = canonicalTime(observedAt);
    const first = firstVerifiedSaleAt === null ? null : canonicalTime(firstVerifiedSaleAt);
    const repeat = repeatVerifiedSaleAt === null ? null : canonicalTime(repeatVerifiedSaleAt);
    if (
      schema !== 'stateset.storefront-adoption.v1' ||
      typeof installationId !== 'string' ||
      typeof source !== 'string' ||
      !/^[a-f\d]{8}(?:-[a-f\d]{4}){3}-[a-f\d]{12}$/i.test(installationId) ||
      !/^[a-z0-9][a-z0-9_-]{0,47}$/.test(source || '') ||
      !['complete', 'partial'].includes(coverage) ||
      !Number.isFinite(start) ||
      !Number.isFinite(end) ||
      end < start ||
      (first !== null && (!Number.isFinite(first) || first < start || first > end)) ||
      (repeat !== null &&
        (!Number.isFinite(repeat) || first === null || repeat < first + 7 * day || repeat > end))
    )
      throw new Error('Invalid storefront adoption report');
    const fields = [
      'live',
      'sandbox',
      'unchecked',
      'unavailable',
      'mismatch',
      'otherRails',
      'supplemental',
    ];
    if (
      scope !== 'primary card checkouts placed since enrollment' ||
      !fields.every((field) => Number.isSafeInteger(counts?.[field]) && counts[field] >= 0) ||
      coverage !==
        (counts.unchecked || counts.mismatch || counts.unavailable ? 'partial' : 'complete') ||
      (first === null) !== (counts.live === 0) ||
      (repeat !== null && counts.live < 2) ||
      hoursToFirstVerifiedSale !== (first === null ? null : (first - start) / 3600000)
    ) {
      throw new Error('Invalid storefront adoption report');
    }
    const row = {
      schema,
      installationId,
      source,
      enrolledAt,
      observedAt,
      coverage,
      scope,
      counts: Object.fromEntries(fields.map((key) => [key, counts[key]])),
      firstVerifiedSaleAt,
      hoursToFirstVerifiedSale,
      repeatVerifiedSaleAt,
    };
    const prior = stores.get(installationId);
    if (
      prior &&
      (prior.enrolledAt !== enrolledAt ||
        prior.source !== source ||
        (Date.parse(prior.observedAt) === end && !isDeepStrictEqual(prior, row)))
    )
      throw new Error('Conflicting reports for the same installation');
    if (!prior || Date.parse(prior.observedAt) < end) stores.set(installationId, row);
  }
  return [...stores.values()];
}

export function groupAdoptionReports(reports) {
  const groups = new Map();
  for (const row of latestAdoptionReports(reports)) {
    const key = row.source;
    const cohort = groups.get(key) || {
      source: key,
      submittedStores: 0,
      incompleteStores: 0,
      activation7d: { eligible: 0, achieved: 0 },
      repeat30d: { eligible: 0, achieved: 0 },
    };
    groups.set(key, cohort);
    cohort.submittedStores++;
    if (row.coverage !== 'complete') {
      cohort.incompleteStores++;
      continue;
    }
    const start = Date.parse(row.enrolledAt),
      age = Date.parse(row.observedAt) - start;
    const first = Date.parse(row.firstVerifiedSaleAt),
      repeat = Date.parse(row.repeatVerifiedSaleAt);
    if (age >= 7 * day) {
      cohort.activation7d.eligible++;
      if (first < start + 7 * day) cohort.activation7d.achieved++;
    }
    if (age >= 30 * day) {
      cohort.repeat30d.eligible++;
      if (repeat < start + 30 * day) cohort.repeat30d.achieved++;
    }
  }
  return [...groups.values()].sort((a, b) => a.source.localeCompare(b.source));
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try {
    const files = process.argv.slice(2);
    if (!files.length) throw new Error('Supply one or more adoption-report JSON files');
    const reports = await Promise.all(
      files.map(async (file) => JSON.parse(await readFile(file, 'utf8'))),
    );
    console.log(JSON.stringify(groupAdoptionReports(reports), null, 2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
