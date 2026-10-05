#!/usr/bin/env python3
"""Run recovery checks and write evidence for this invocation, including failures."""

from __future__ import annotations

import datetime as dt
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import tempfile
import uuid

ROOT = Path(__file__).resolve().parents[2]
CHECKS = {
    "backup_restore": {
        "command": ["cargo", "test", "--locked", "-p", "stateset-embedded", "--features", "sqlite",
                    "--test", "maintenance_accessor", "backup_then_restore_reproduces_the_data",
                    "--", "--exact", "--nocapture"],
        "markers": [r"recovery-proof [^\r\n]* integrity=ok$"],
        "minimum_tests": 1,
        "assertions": ["manifest checksum verified", "schema and migration count match",
                       "domain data restored", "PRAGMA integrity_check=ok"],
        "log": "backup-restore.log",
    },
    "migration_rollback": {
        "command": ["cargo", "test", "--locked", "-p", "stateset-migrations",
                    "sqlite::tests::migrate_then_rollback_then_remigrate",
                    "--", "--exact", "--nocapture"],
        "markers": [r"migration-proof [^\r\n]* data_preserved=true integrity=ok$"],
        "minimum_tests": 1,
        "assertions": ["file-backed migrate", "rollback order verified", "retained data preserved",
                       "remigrate healthy", "PRAGMA integrity_check=ok"],
        "log": "migration-rollback.log",
    },
    "tenant_http_recovery": {
        "command": ["cargo", "test", "--locked", "-p", "stateset-http", "--test",
                    "durable_commerce_recovery", "--", "--nocapture"],
        "markers": ["tenant-recovery-proof restart=replayed backup=replayed authorization=enforced orders=1 payments=1 refunds=1 integrity=ok",
                    "tenant-receipt-failure-proof payments=1 reservation=pending"],
        "minimum_tests": 2,
        "assertions": ["authenticated order/payment/refund flow", "tenant-scoped durable receipts",
                       "byte-identical replay after restart and backup restore",
                       "authorization enforced before replay", "receipt failure cannot duplicate a payment",
                       "PRAGMA integrity_check=ok"],
        "log": "tenant-http-recovery.log",
    },
}


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def atomic_write(path: Path, content: str) -> None:
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent,
                                         prefix=".evidence-", delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(content)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def save_report(output: Path, report: dict) -> None:
    atomic_write(output / "evidence.json", json.dumps(report, indent=2, sort_keys=True) + "\n")
    source = report.get("source_before") or {}
    lines = ["# Operations recovery evidence", "", f"- Result: **{report['result']}**",
             f"- Invocation: `{report['invocation_id']}`", f"- Started: {report['created_at']}",
             f"- Checkout commit: `{report.get('commit_sha') or 'unavailable'}`",
             f"- Worktree dirty: `{source.get('dirty', 'unavailable')}`",
             f"- Source fingerprint: `{source.get('sha256', 'unavailable')}`",
             f"- Source unchanged: `{report.get('source_unchanged', 'unverified')}`", ""]
    for name, check in report["checks"].items():
        lines.append(f"- {name}: **{check['result']}**; log `{check['log']}`")
    if report.get("error"):
        lines.extend(["", "Failure details are recorded in `evidence.json`."])
    lines.extend(["", "Only logs referenced by this invocation are evidence for this result.",
                  "Local runs do not complete release gates; retain immutable CI artifacts.",
                  "Runbook: `docs/src/guides/operations-recovery.md`.", ""])
    atomic_write(output / "README.md", "\n".join(lines))


def git(*args: str) -> bytes:
    return subprocess.check_output(["git", *args], cwd=ROOT, stderr=subprocess.PIPE)


def is_within(path: Path, directory: Path) -> bool:
    try:
        path.relative_to(directory)
        return True
    except ValueError:
        return False


def source_snapshot(output: Path) -> dict:
    """Hash tracked and non-ignored untracked source bytes, modes, and deletions.

    Generated evidence is excluded. Ignored files and external build inputs are
    outside this fingerprint; this is source provenance, not a reproducible-build claim.
    """
    paths = sorted(set(git("ls-files", "-z", "--cached", "--others", "--exclude-standard").split(b"\0")) - {b""})
    digest = hashlib.sha256()
    count = 0
    for raw in paths:
        path = ROOT / os.fsdecode(raw)
        if is_within(path, output):
            continue
        try:
            info = path.lstat()
        except FileNotFoundError:
            descriptor = b"deleted"
        else:
            if stat.S_ISLNK(info.st_mode):
                descriptor = b"link:" + os.fsencode(os.readlink(path))
            elif stat.S_ISREG(info.st_mode):
                content = hashlib.sha256()
                with path.open("rb") as stream:
                    for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                        content.update(chunk)
                descriptor = f"file:{info.st_mode & 0o111}:{content.hexdigest()}".encode()
            else:
                raise RuntimeError(f"unsupported source entry in fingerprint: {os.fsdecode(raw)}")
        digest.update(len(raw).to_bytes(8, "big") + raw)
        digest.update(len(descriptor).to_bytes(8, "big") + descriptor)
        count += 1
    status_paths = ["."]
    if is_within(output, ROOT):
        status_paths.append(f":(exclude,literal){output.relative_to(ROOT)}")
    return {
        "commit_sha": git("rev-parse", "HEAD").decode().strip(),
        "sha256": digest.hexdigest(),
        "file_count": count,
        "dirty": bool(git("status", "--porcelain", "--untracked-files=all", "--", *status_paths)),
        "scope": "tracked and non-ignored untracked files; excludes evidence output",
    }


class RunInterrupted(Exception):
    def __init__(self, signum: int):
        super().__init__(f"interrupted by signal {signum}")
        self.signum = signum


def interrupted(signum: int, _frame) -> None:
    raise RunInterrupted(signum)


def run_command(command: list[str], log: Path) -> int:
    process = None
    try:
        with log.open("w", encoding="utf-8") as transcript:
            process = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE,
                                       stderr=subprocess.STDOUT, text=True, encoding="utf-8",
                                       errors="replace", start_new_session=True)
            assert process.stdout is not None
            for line in process.stdout:
                transcript.write(line)
                transcript.flush()
                print(line, end="", flush=True)
            return process.wait()
    finally:
        if process is not None:
            if process.poll() is None:
                # Only the process group created for this check is terminated.
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    process.wait()
            if process.stdout is not None:
                process.stdout.close()


def run(output: Path) -> int:
    invocation = str(uuid.uuid4())
    logs = Path("runs") / invocation
    (output / logs).mkdir(parents=True)
    report = {
        "schema_version": 3, "result": "running", "created_at": now(),
        "invocation_id": invocation, "commit_sha": None,
        "requested_commit_sha": os.environ.get("GITHUB_SHA"),
        "github_run_id": os.environ.get("GITHUB_RUN_ID", "local"),
        "github_run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT", "local"),
        "source_before": None, "source_after": None, "source_unchanged": None,
        "toolchain": {}, "runbook": "docs/src/guides/operations-recovery.md",
        "checks": {name: {"result": "not_run", "command": check["command"],
                           "assertions": check["assertions"], "log": str(logs / check["log"]),
                           "log_sha256": None, "exit_code": None}
                   for name, check in CHECKS.items()},
    }
    # Invalidate a previous success before Git, compiler lookup, or any test runs.
    save_report(output, report)
    exit_code = 1
    current = None
    try:
        before = source_snapshot(output)
        report["source_before"] = before
        report["commit_sha"] = before["commit_sha"]
        requested = report["requested_commit_sha"]
        if requested and requested != report["commit_sha"]:
            raise RuntimeError("GITHUB_SHA does not match the checked-out HEAD")
        for tool in ("rustc", "cargo"):
            report["toolchain"][tool] = subprocess.check_output([tool, "--version"], text=True).strip()
        for name, specification in CHECKS.items():
            current = report["checks"][name]
            current["result"] = "running"
            save_report(output, report)
            print(f"Running recovery check: {name}", flush=True)
            log = output / current["log"]
            try:
                current["exit_code"] = run_command(specification["command"], log)
            finally:
                if log.exists():
                    current["log_sha256"] = hashlib.sha256(log.read_bytes()).hexdigest()
            if current["exit_code"] != 0:
                raise RuntimeError(f"{name} exited with status {current['exit_code']}")
            transcript = log.read_text(encoding="utf-8")
            if not all(re.search(marker, transcript, re.MULTILINE)
                       for marker in specification["markers"]):
                raise RuntimeError(f"{name} proof marker missing")
            passed = sum(int(count) for count in re.findall(
                r"^test result: ok\. ([0-9]+) passed; 0 failed;", transcript, re.MULTILINE))
            if passed < specification["minimum_tests"]:
                raise RuntimeError(f"{name} did not report enough successful tests")
            current["result"] = "passed"
            save_report(output, report)
            current = None
        after = source_snapshot(output)
        report["source_after"] = after
        report["source_unchanged"] = (before["sha256"], before["commit_sha"]) == (after["sha256"], after["commit_sha"])
        if not report["source_unchanged"]:
            raise RuntimeError("source changed during the recovery drill; rerun against a stable worktree")
        report["result"] = "passed"
        exit_code = 0
    except RunInterrupted as error:
        report["result"] = "cancelled"
        report["error"] = str(error)
        if current is not None:
            current["result"] = "cancelled"
        exit_code = 128 + error.signum
    except Exception as error:
        report["result"] = "failed"
        report["error"] = str(error)
        if current is not None:
            current["result"] = "failed"
    finally:
        report["finished_at"] = now()
        save_report(output, report)
    print(f"Operations recovery evidence ({report['result']}): {output / 'evidence.json'}", flush=True)
    return exit_code


def main() -> int:
    output = Path(os.environ.get("RECOVERY_EVIDENCE_DIR", ROOT / "artifacts/operations-recovery")).resolve()
    if is_within(ROOT, output):
        raise RuntimeError("evidence output cannot be the source root or its ancestor")
    if is_within(output, ROOT) and git("ls-files", "--", str(output.relative_to(ROOT))):
        raise RuntimeError("evidence output cannot contain tracked source files")
    output.mkdir(parents=True, exist_ok=True)
    # A crashed process releases flock automatically; never delete another run's lock.
    with (output / ".recovery.lock").open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            raise RuntimeError("another recovery drill is using this evidence directory") from None
        signal.signal(signal.SIGTERM, interrupted)
        signal.signal(signal.SIGINT, interrupted)
        return run(output)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
