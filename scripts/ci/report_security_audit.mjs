import fs from 'node:fs';
import path from 'node:path';
import { AUDITS, STEP_COUNTS } from './run_security_audit.mjs';

export const TITLE = 'Security audit: new advisory on master';
export const MARKER = '<!-- stateset-daily-security-audit:v1 -->';

export function readAuditReport({ directory, sha, runUrl, auditResult }) {
  if (!/^[a-f0-9]{40}$/.test(sha || '')) throw new Error('Invalid audited SHA');
  if (
    !/^https:\/\/[a-z0-9.-]+\/[^/]+\/[^/]+\/actions\/runs\/\d+(?:\/attempts\/\d+)?$/i.test(
      runUrl || '',
    )
  )
    throw new Error('Invalid audit run URL');
  let clean = auditResult === 'success';
  const sections = [];
  for (const id of AUDITS) {
    let status = 'incomplete';
    let explanation = '';
    let log = '';
    try {
      log = fs.readFileSync(path.join(directory, `${id}.log`), 'utf8');
      const report = JSON.parse(fs.readFileSync(path.join(directory, `${id}.json`), 'utf8'));
      if (
        report.schemaVersion !== 1 ||
        report.id !== id ||
        report.sha !== sha ||
        typeof report.success !== 'boolean' ||
        !Array.isArray(report.steps) ||
        report.steps.length === 0 ||
        report.steps.length > STEP_COUNTS[id] ||
        (report.success && report.steps.length !== STEP_COUNTS[id]) ||
        !log.trim()
      )
        throw new Error('Invalid, empty, or stale audit evidence');
      status =
        report.success && report.steps.every((step) => step.exitCode === 0 && step.signal === null)
          ? 'passed'
          : 'failed';
    } catch (error) {
      explanation = `Audit evidence is missing or invalid: ${error.message}`;
    }
    clean &&= status === 'passed';
    // Escape dependency output and bound each excerpt after escaping, so even
    // verbose failures stay within GitHub's issue-body limit. Keep the tail:
    // install progress must not displace the actual advisory or error.
    const escaped = log
      .replace(/\x1b\[[0-9;]*m/g, '')
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/@/g, '&#64;');
    sections.push(
      `### ${id}: ${status}\n\n${explanation}\n\n<pre>${escaped.slice(-8500)}</pre>${escaped.length > 8500 ? '\n\nLog truncated; the complete findings are in the run artifacts.' : ''}`,
    );
  }
  const body = `${MARKER}\n\nDaily security audit of master at \`${sha}\`.\n\n[Audit run and complete logs](${runUrl}).\n\n${clean ? 'All audit gates passed.' : 'An advisory or audit infrastructure failure requires review. Incomplete checks never count as recovery.'}\n\n${sections.join('\n\n')}`;
  return { clean, body };
}

export async function reportSecurityAudit({
  github,
  context,
  core,
  directory,
  sha,
  runUrl,
  auditResult,
}) {
  const report = readAuditReport({ directory, sha, runUrl, auditResult });
  const scope = { owner: context.repo.owner, repo: context.repo.repo };
  const issues = await github.paginate(github.rest.issues.listForRepo, {
    ...scope,
    state: 'open',
    per_page: 100,
  });
  const existing = issues.filter(
    (issue) => !issue.pull_request && issue.user?.type === 'Bot' && issue.body?.includes(MARKER),
  );
  if (existing.length > 1)
    throw new Error('Multiple security tracking issues found; refusing to choose one');
  const issue = existing[0];
  if (report.clean) {
    if (issue) {
      // Update the evidence before closing. API failure leaves the issue open.
      await github.rest.issues.update({
        ...scope,
        issue_number: issue.number,
        title: TITLE,
        body: report.body,
      });
      await github.rest.issues.update({
        ...scope,
        issue_number: issue.number,
        state: 'closed',
        state_reason: 'completed',
      });
    }
  } else {
    if (issue)
      await github.rest.issues.update({
        ...scope,
        issue_number: issue.number,
        title: TITLE,
        body: report.body,
      });
    else await github.rest.issues.create({ ...scope, title: TITLE, body: report.body });
    core.setFailed('Master security audit failed; see the tracking issue and run artifacts.');
  }
  return report;
}
