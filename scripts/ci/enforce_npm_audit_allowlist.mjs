#!/usr/bin/env node
/**
 * Enforce a committed advisory-id allowlist against `npm audit`.
 *
 * Behavior:
 * - Runs `npm audit --json --audit-level=<level>` in the current working dir.
 * - If there are no findings at that level, exits 0.
 * - Otherwise, every advisory GHSA id must be present in scripts/ci/npm-audit-allowlist.json.
 * - Allowlisted IDs remain allowed only while no patched version is published:
 *   the advisory's package must still have its latest registry version inside
 *   the advisory's vulnerable range. npm's `fixAvailable` is not used, since it
 *   reports `true` for chains (Tailwind 3 -> braces) with no patch at all.
 * - Missing, malformed or unsuccessful audit evidence exits 2 (retryable by
 *   the storefront gate); policy violations and unverifiable exceptions exit 1.
 *
 * Usage:
 *   node scripts/ci/enforce_npm_audit_allowlist.mjs --level high
 *   node scripts/ci/enforce_npm_audit_allowlist.mjs --level moderate
 */
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

function parseArgs(argv) {
  const args = { level: 'high', allowlist: null };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if ((a === '--level' || a === '-l') && i + 1 < argv.length) {
      args.level = argv[++i];
    } else if ((a === '--allowlist' || a === '-a') && i + 1 < argv.length) {
      args.allowlist = argv[++i];
    }
  }
  if (!['low', 'moderate', 'high', 'critical'].includes(args.level)) {
    console.error(`error: invalid --level ${args.level} (expected low|moderate|high|critical)`);
    process.exit(2);
  }
  return args;
}

function severityRank(sev) {
  return ['info', 'low', 'moderate', 'high', 'critical'].indexOf(sev);
}

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function unavailable(reason) {
  console.error(`error: npm audit did not return a valid report: ${reason}`);
  process.exit(2);
}

const { level, allowlist } = parseArgs(process.argv);
console.log(`+ npm audit --audit-level=${level} --json`);
const res = spawnSync('npm', ['audit', `--audit-level=${level}`, '--json'], {
  encoding: 'utf8',
  stdio: ['ignore', 'pipe', 'pipe'],
  timeout: 60000,
});
if (res.error) {
  console.error('error: failed to invoke npm audit:', res.error);
  process.exit(2);
}
if (res.signal || ![0, 1].includes(res.status)) {
  unavailable(`unexpected process result (exit ${res.status}, signal ${res.signal})`);
}
const output = res.stdout || res.stderr || '';
let report;
try {
  report = JSON.parse(output);
} catch {
  console.error('error: npm audit did not return JSON; refusing to gate on an unparsable response');
  console.error(output.slice(0, 2000));
  process.exit(2);
}

// npm errors can be valid JSON. An absent vulnerability map is not evidence of
// a clean dependency tree. Check the v2 summary against every package before
// allowing threshold filtering to hide incomplete or unknown-severity data.
if (
  !isRecord(report) ||
  Object.hasOwn(report, 'error') ||
  report.auditReportVersion !== 2 ||
  !isRecord(report.vulnerabilities) ||
  !isRecord(report.metadata?.vulnerabilities)
) {
  unavailable('expected a complete npm audit v2 report without an error');
}
const vulnEntries = Object.entries(report.vulnerabilities);
const counts = { info: 0, low: 0, moderate: 0, high: 0, critical: 0, total: vulnEntries.length };
for (const [pkg, vulnerability] of vulnEntries) {
  if (!isRecord(vulnerability) || severityRank(vulnerability.severity) < 0) {
    unavailable(`invalid vulnerability or severity for ${pkg}`);
  }
  counts[vulnerability.severity]++;
}
for (const [severity, count] of Object.entries(counts)) {
  if (report.metadata.vulnerabilities[severity] !== count) {
    unavailable(`inconsistent ${severity} vulnerability count`);
  }
}
const minRank = severityRank(level);
const relevant = vulnEntries.filter(([_, v]) => severityRank(v.severity) >= minRank);
if (res.status !== Number(relevant.length > 0)) {
  unavailable('process exit status contradicts findings at the requested severity');
}
if (relevant.length === 0) {
  console.log(`npm audit: no ${level} (or higher) advisories found`);
  process.exit(0);
}

const allowlistPath = allowlist ?? path.join(process.cwd(), 'scripts/ci/npm-audit-allowlist.json');
let allow = { allowed: [] };
try {
  allow = JSON.parse(fs.readFileSync(allowlistPath, 'utf8'));
} catch (err) {
  console.error(`error: missing or unreadable allowlist: ${allowlistPath}`);
  throw err;
}
const allowedIds = new Set(allow.allowed || []);

function extractGhsaIdsFromUrl(url) {
  if (typeof url !== 'string') return [];
  const m = url.match(/GHSA-[a-z0-9-]+/gi);
  return m ? Array.from(new Set(m)) : [];
}

function collectGhsaFindingsFromVuln(vuln, report, seenPkgs = new Set()) {
  /** @type {{ id: string, fixAvailable: any }[]} */
  const findings = [];
  if (!vuln) return findings;
  // If this vuln itself carries a URL with GHSA ids, pair them with this vuln's fixAvailable.
  if (vuln.url) {
    for (const id of extractGhsaIdsFromUrl(vuln.url)) {
      findings.push({ id, fixAvailable: vuln.fixAvailable });
    }
  }
  const via = Array.isArray(vuln.via) ? vuln.via : [];
  for (const entry of via) {
    if (typeof entry === 'string') {
      // Reference to another package's vulnerability object
      const pkg = entry;
      if (!seenPkgs.has(pkg) && report.vulnerabilities && report.vulnerabilities[pkg]) {
        seenPkgs.add(pkg);
        const nested = collectGhsaFindingsFromVuln(report.vulnerabilities[pkg], report, seenPkgs);
        findings.push(...nested);
      }
      continue;
    }
    if (entry && typeof entry === 'object') {
      if (entry.url) {
        for (const id of extractGhsaIdsFromUrl(entry.url)) {
          // Prefer entry.fixAvailable if present; otherwise inherit from this vuln.
          findings.push({ id, fixAvailable: entry.fixAvailable ?? vuln.fixAvailable });
        }
      }
      // Some entries may themselves have a nested `via` field.
      if (Array.isArray(entry.via)) {
        findings.push(...collectGhsaFindingsFromVuln(entry, report, seenPkgs));
      }
    }
  }
  return findings;
}

/** Advisory objects (with name + range) for `id` reachable from `vuln`. */
function advisoriesFor(vuln, report, id, seenPkgs = new Set(), out = new Map()) {
  for (const entry of Array.isArray(vuln?.via) ? vuln.via : []) {
    if (typeof entry === 'string') {
      if (!seenPkgs.has(entry) && report.vulnerabilities?.[entry]) {
        seenPkgs.add(entry);
        advisoriesFor(report.vulnerabilities[entry], report, id, seenPkgs, out);
      }
    } else if (
      entry &&
      extractGhsaIdsFromUrl(entry.url).includes(id) &&
      entry.name &&
      entry.range
    ) {
      out.set(`${entry.name}@${entry.range}`, { name: entry.name, range: entry.range });
    }
  }
  return [...out.values()];
}

const viewCache = new Map();
function npmView(args) {
  const key = args.join(' ');
  if (!viewCache.has(key)) {
    const r = spawnSync('npm', ['view', ...args, '--json'], { encoding: 'utf8', timeout: 60000 });
    if (r.error || r.signal || r.status !== 0) {
      console.error(`error: npm view ${key} failed; refusing to allow without registry data`);
      console.error((r.stderr || '').slice(0, 500));
      process.exit(1);
    }
    try {
      viewCache.set(key, JSON.parse(r.stdout));
    } catch {
      console.error(
        `error: npm view ${key} did not return JSON; refusing an unverifiable exception`,
      );
      process.exit(1);
    }
  }
  return viewCache.get(key);
}

/** Latest published version when it is outside `range` (a patch exists), else null. */
function patchedVersionPublished(name, range) {
  const latest = npmView([name, 'version']);
  const vulnerable = [npmView([`${name}@${range}`, 'version'])].flat();
  const isVersion = (value) =>
    typeof value === 'string' &&
    /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(value);
  if (!isVersion(latest) || vulnerable.length === 0 || !vulnerable.every(isVersion)) {
    console.error(
      `error: npm view returned invalid version data for ${name}; refusing an unverifiable exception`,
    );
    process.exit(1);
  }
  return vulnerable.includes(latest) ? null : latest;
}

const violations = [];
for (const [pkg, v] of relevant) {
  const findings = collectGhsaFindingsFromVuln(v, report);
  const uniqueIds = [...new Set(findings.map((f) => f.id))];
  if (uniqueIds.length === 0) {
    violations.push({ pkg, reason: 'no GHSA id found in advisory URLs', ids: [] });
    continue;
  }
  for (const id of uniqueIds) {
    if (!allowedIds.has(id)) {
      violations.push({ pkg, ids: [id], reason: 'id not on allowlist' });
    } else {
      // npm's `fixAvailable` is not trustworthy here: for the Tailwind 3
      // chain it reports `true` although no patched braces exists. Ask the
      // registry instead: the allowlisted advisory stays allowed only while
      // the package's latest published version is still inside the
      // advisory's vulnerable range. Once a patched release ships, fail.
      const advisories = advisoriesFor(v, report, id);
      if (advisories.length === 0) {
        violations.push({ pkg, ids: [id], reason: 'missing advisory package/range evidence' });
      }
      for (const advisory of advisories) {
        const patched = patchedVersionPublished(advisory.name, advisory.range);
        if (patched) {
          violations.push({
            pkg,
            ids: [id],
            reason: `patched ${advisory.name}@${patched} is published (vulnerable ${advisory.range})`,
          });
        }
      }
    }
  }
}

if (violations.length > 0) {
  console.error(`error: npm audit ${level} gate failed:`);
  for (const v of violations) {
    console.error(`  - ${v.pkg}: ${v.reason}${v.ids.length ? ` (${v.ids.join(', ')})` : ''}`);
  }
  console.error(
    'note: only IDs listed in scripts/ci/npm-audit-allowlist.json may pass; allowlisted IDs fail once a patched version is published',
  );
  process.exit(1);
}

console.log(`npm audit: ${level} (and higher) advisories allowed by committed list`);
process.exit(0);
