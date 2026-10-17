#!/usr/bin/env python3
"""Source-bound, fail-closed evidence lifecycle for the storefront gate."""
import hashlib
import json
import os
from pathlib import Path
import sys
from datetime import datetime, timezone

from run_operations_recovery_drill import source_snapshot


def now():
    return datetime.now(timezone.utc).isoformat()


def save(output, report):
    temporary = output / '.evidence.json.tmp'
    temporary.write_text(json.dumps(report, indent=2) + '\n')
    temporary.replace(output / 'evidence.json')


def begin(output, invocation):
    output.mkdir(parents=True, exist_ok=True)
    report = {'schema_version': 2, 'result': 'running', 'invocation_id': invocation,
              'started_at': now(), 'level': 'not_started', 'source_before': None,
              'source_after': None, 'source_unchanged': None, 'exit_code': None}
    # Invalidate old success before Git or source inspection can fail.
    save(output, report)
    report['source_before'] = source_snapshot(output)
    requested = os.environ.get('GITHUB_SHA')
    if requested and requested != report['source_before']['commit_sha']:
        raise RuntimeError('GITHUB_SHA does not match the checked-out source')
    save(output, report)


def load_owned(output, invocation):
    report = json.loads((output / 'evidence.json').read_text())
    if report.get('invocation_id') != invocation:
        raise RuntimeError('refusing to overwrite evidence from another invocation')
    return report


def bind_log(output, report):
    log = output / 'golden-path.log'
    report['log'] = 'golden-path.log'
    report['log_sha256'] = hashlib.sha256(log.read_bytes()).hexdigest() if log.exists() else None


def finish(output, invocation):
    report = load_owned(output, invocation)
    if report.get('result') != 'running' or not report.get('source_before'):
        raise RuntimeError('storefront evidence was not initialized')
    report['source_after'] = source_snapshot(output)
    before, after = report['source_before'], report['source_after']
    report['source_unchanged'] = (before['sha256'], before['commit_sha']) == (after['sha256'], after['commit_sha'])
    bind_log(output, report)
    save(output, report)
    if not report['source_unchanged']:
        raise RuntimeError('source changed during storefront validation; rerun against a stable worktree')
    if report.get('level') not in ('scaffold', 'install-seed-doctor-journey-typecheck-build'):
        raise RuntimeError('unknown or unfinished storefront validation level')
    if report['level'] != 'scaffold':
        if os.environ.get('GITHUB_ACTIONS') == 'true' and report.get('native_build') == 'reused':
            raise RuntimeError('CI cannot claim a fresh native build from a reused binary')
        transcript = (output / 'golden-path.log').read_text()
        if 'storefront-configuration-proof mode=launch result=passed' not in transcript:
            raise RuntimeError('launch configuration proof is missing')
        journeys = []
        for line in transcript.splitlines():
            try:
                value = json.loads(line)
            except ValueError:
                continue
            if isinstance(value, dict) and value.get('settlement') == 'simulated-no-funds-transferred':
                journeys.append(value)
        if len(journeys) != 1 or journeys[0].get('result') != 'passed' or journeys[0].get('externalProvidersVerified') is not False:
            raise RuntimeError('native merchant journey proof is missing or ambiguous')
        report['merchant_journey'] = journeys[0]
    report.update(result='passed', exit_code=0, finished_at=now())
    save(output, report)


def fail(output, invocation, code):
    report = load_owned(output, invocation)
    report.update(result='failed', exit_code=code or 1, finished_at=now())
    bind_log(output, report)
    save(output, report)


if __name__ == '__main__':
    try:
        action, directory = sys.argv[1:3]
        invocation = os.environ['GOLDEN_PATH_INVOCATION_ID']
        destination = Path(directory).resolve()
        if action == 'begin':
            begin(destination, invocation)
        elif action == 'finish':
            finish(destination, invocation)
        elif action == 'fail':
            fail(destination, invocation, int(sys.argv[3]))
        else:
            raise RuntimeError('unknown evidence action')
    except Exception as error:
        print(f'Storefront evidence error: {error}', file=sys.stderr)
        sys.exit(1)
