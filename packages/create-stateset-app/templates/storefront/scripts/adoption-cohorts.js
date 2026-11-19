import { readFile } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';
import { isDeepStrictEqual } from 'node:util';

const day = 86400000;

// Use one latest voluntarily supplied report per enrolled store. Young cohorts and
// incomplete provider checks have their own counts and never dilute either rate.
export function groupAdoptionReports(reports) {
  const stores = new Map(),
    groups = new Map();
  for (const report of reports) {
    const {
      schema,
      installationId,
      source,
      enrolledAt,
      observedAt,
      coverage,
      firstVerifiedSaleAt,
      repeatVerifiedSaleAt,
    } = report;
    const start = Date.parse(enrolledAt),
      end = Date.parse(observedAt);
    const first = firstVerifiedSaleAt === null ? null : Date.parse(firstVerifiedSaleAt);
    const repeat = repeatVerifiedSaleAt === null ? null : Date.parse(repeatVerifiedSaleAt);
    if (
      schema !== 'stateset.storefront-adoption.v1' ||
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
    const prior = stores.get(installationId);
    if (
      prior &&
      (prior.enrolledAt !== enrolledAt ||
        prior.source !== source ||
        (Date.parse(prior.observedAt) === end && !isDeepStrictEqual(prior, report)))
    )
      throw new Error('Conflicting reports for the same installation');
    if (!prior || Date.parse(prior.observedAt) < end) stores.set(installationId, report);
  }
  for (const row of stores.values()) {
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
