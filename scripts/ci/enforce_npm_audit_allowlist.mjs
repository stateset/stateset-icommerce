#!/usr/bin/env node
/**
 * Enforce a committed advisory-id allowlist against `npm audit`.
 *
 * Behavior:
 * - Runs `npm audit --json --audit-level=<level>` in the current working dir.
 * - If there are no findings at that level, exits 0.
 * - Otherwise, every advisory GHSA id must be present in scripts/ci/npm-audit-allowlist.json.
 * - Allowlisted IDs remain allowed when the only reported fix is a semver-major bump.
 *   They fail the moment npm reports a same-major (patched) fix.
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
  return { info: 0, low: 1, moderate: 2, high: 3, critical: 4 }[sev] ?? -1;
}

const { level, allowlist } = parseArgs(process.argv);
console.log(`+ npm audit --audit-level=${level} --json`);
const res = spawnSync('npm', ['audit', `--audit-level=${level}`, '--json'], {
  encoding: 'utf8',
  stdio: ['ignore', 'pipe', 'pipe'],
});
if (res.error) {
  console.error('error: failed to invoke npm audit:', res.error);
  process.exit(1);
}
const output = res.stdout || res.stderr || '';
let report;
try {
  report = JSON.parse(output);
} catch {
  console.error('error: npm audit did not return JSON; refusing to gate on an unparsable response');
  console.error(output.slice(0, 2000));
  process.exit(1);
}

const vulnEntries = Object.entries(report.vulnerabilities || {});
const minRank = severityRank(level);
const relevant = vulnEntries.filter(([_, v]) => severityRank(v.severity) >= minRank);
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

function collectGhsaIdsFromVuln(vuln, report, seenPkgs = new Set()) {
  const ids = new Set();
  if (!vuln) return ids;
  // Some objects may carry a url directly (rare).
  if (vuln.url) {
    for (const id of extractGhsaIdsFromUrl(vuln.url)) ids.add(id);
  }
  const via = Array.isArray(vuln.via) ? vuln.via : [];
  for (const entry of via) {
    if (typeof entry === 'string') {
      // Reference to another package's vulnerability object
      const pkg = entry;
      if (!seenPkgs.has(pkg) && report.vulnerabilities && report.vulnerabilities[pkg]) {
        seenPkgs.add(pkg);
        const nested = collectGhsaIdsFromVuln(report.vulnerabilities[pkg], report, seenPkgs);
        for (const id of nested) ids.add(id);
      }
      continue;
    }
    if (entry && typeof entry === 'object') {
      if (entry.url) {
        for (const id of extractGhsaIdsFromUrl(entry.url)) ids.add(id);
      }
      // Some entries may themselves have a nested `via` field.
      if (Array.isArray(entry.via)) {
        for (const sub of collectGhsaIdsFromVuln(entry, report, seenPkgs)) {
          ids.add(sub);
        }
      }
    }
  }
  return ids;
}

const violations = [];
for (const [pkg, v] of relevant) {
  const uniqueIds = Array.from(collectGhsaIdsFromVuln(v, report));
  if (uniqueIds.length === 0) {
    violations.push({ pkg, reason: 'no GHSA id found in advisory URLs', ids: [] });
    continue;
  }
  for (const id of uniqueIds) {
    if (!allowedIds.has(id)) {
      violations.push({ pkg, ids: [id], reason: 'id not on allowlist' });
    } else {
      // Allowlisted: treat major-only fixes as allowed; fail on same-major fixes.
      const fa = v.fixAvailable;
      let isMajorOnly = false;
      if (fa && typeof fa === 'object') {
        if (Array.isArray(fa)) {
          isMajorOnly = fa.every((f) => f && typeof f === 'object' && f.isSemVerMajor === true);
        } else {
          isMajorOnly = fa.isSemVerMajor === true;
        }
      }
      if (fa && !isMajorOnly) {
        violations.push({ pkg, ids: [id], reason: 'same-major fix available for allowlisted advisory' });
      }
    }
  }
}

if (violations.length > 0) {
  console.error(`error: npm audit ${level} gate failed:`);
  for (const v of violations) {
    console.error(`  - ${v.pkg}: ${v.reason}${v.ids.length ? ` (${v.ids.join(', ')})` : ''}`);
  }
  console.error('note: only IDs listed in scripts/ci/npm-audit-allowlist.json may pass; allowlisted IDs fail once a same-major patch is available');
  process.exit(1);
}

console.log(`npm audit: ${level} (and higher) advisories allowed by committed list`);
process.exit(0);

