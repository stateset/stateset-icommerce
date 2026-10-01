#!/usr/bin/env python3
"""Exercise the real launcher in isolated repositories with controlled tool output."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest


SCRIPTS = Path(__file__).resolve().parent
MOCK_TOOL = r'''
import os
from pathlib import Path
import signal
import sys
import time

if sys.argv[1:] == ["--version"]:
    if os.environ.get("VERSION_FAIL"):
        sys.exit(19)
    print('tool "test"\\path\nsecond line')
    sys.exit(0)
package = sys.argv[sys.argv.index("-p") + 1]
with Path(os.environ["CALLS"]).open("a") as calls:
    calls.write(package + "\n")
if os.environ.get("WAIT"):
    def stop(_signum, _frame):
        Path(os.environ["STOPPED"]).write_text("stopped")
        sys.exit(0)
    signal.signal(signal.SIGTERM, stop)
    Path(os.environ["READY"]).write_text(str(os.getpid()))
    print("check running", flush=True)
    while True:
        time.sleep(1)
if os.environ.get("MUTATE") and package == "stateset-embedded":
    Path("source.rs").write_text("changed during the drill")
if os.environ.get("FAIL") == package:
    print("intentional fixture failure")
    sys.exit(17)
markers = {
    "stateset-embedded": ["recovery-proof checksum=abc schema=1 migrations=1 bytes=5 integrity=ok"],
    "stateset-migrations": ["migration-proof path=db rollback=3,2 target=1 remigrated=2,3 data_preserved=true integrity=ok"],
    "stateset-http": ["tenant-recovery-proof restart=replayed backup=replayed authorization=enforced orders=1 payments=1 refunds=1 integrity=ok",
                      "tenant-receipt-failure-proof payments=1 reservation=pending"],
}
if not os.environ.get("NO_MARKERS"):
    for marker in markers[package]:
        if os.environ.get("SPLIT_MARKER"):
            marker = marker.replace(" integrity=ok", "\n integrity=ok")
        print(marker)
count = 0 if os.environ.get("ZERO_TESTS") else len(markers[package])
print(f"test result: ok. {count} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out")
'''


class RecoveryEvidenceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="recovery-evidence-test-")
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name)
        self.repo = self.base / "repo"
        scripts = self.repo / "scripts/ci"
        scripts.mkdir(parents=True)
        for name in ("run_operations_recovery_drill.sh", "run_operations_recovery_drill.py"):
            shutil.copy2(SCRIPTS / name, scripts / name)
        (self.repo / "source.rs").write_text("original source")
        self.git("init", "--quiet")
        self.git("add", ".")
        self.git("-c", "user.name=Recovery Test", "-c", "user.email=recovery@example.invalid",
                 "commit", "--quiet", "-m", "fixture")
        self.head = self.git("rev-parse", "HEAD").strip()
        binaries = self.base / "bin"
        binaries.mkdir()
        for name in ("cargo", "rustc"):
            tool = binaries / name
            tool.write_text(f"#!{sys.executable}\n" + MOCK_TOOL)
            tool.chmod(0o755)
        self.output = self.base / "evidence"
        self.calls = self.base / "calls"
        self.env = {key: value for key, value in os.environ.items()
                    if not key.startswith("GITHUB_")}
        self.env.update(PATH=f"{binaries}:{os.environ['PATH']}",
                        RECOVERY_EVIDENCE_DIR=str(self.output), CALLS=str(self.calls),
                        READY=str(self.base / "ready"), STOPPED=str(self.base / "stopped"))
        self.command = ["bash", str(scripts / "run_operations_recovery_drill.sh")]

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.repo, text=True,
                                       stderr=subprocess.PIPE)

    def invoke(self, **environment):
        return subprocess.run(self.command, cwd=self.repo, env={**self.env, **environment},
                              capture_output=True, text=True, timeout=20)

    def report(self):
        return json.loads((self.output / "evidence.json").read_text())

    def assert_success(self, result):
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        report = self.report()
        self.assertEqual(report["schema_version"], 3)
        self.assertEqual(report["result"], "passed")
        self.assertEqual(report["commit_sha"], self.head)
        self.assertTrue(report["source_unchanged"])
        self.assertEqual(report["source_before"], report["source_after"])
        for check in report["checks"].values():
            self.assertEqual(check["result"], "passed")
            self.assertEqual(check["exit_code"], 0)
            self.assertIn(report["invocation_id"], check["log"])
            self.assertEqual(check["log_sha256"],
                             hashlib.sha256((self.output / check["log"]).read_bytes()).hexdigest())
        return report

    def test_success_binds_current_logs_source_and_toolchain(self):
        report = self.assert_success(self.invoke(GITHUB_SHA=self.head,
                                                 GITHUB_RUN_ID='run "quoted"\nnext'))
        self.assertFalse(report["source_before"]["dirty"])
        self.assertIn('"test"\\path\nsecond line', report["toolchain"]["cargo"])
        self.assertEqual(report["github_run_id"], 'run "quoted"\nnext')
        self.assertEqual(self.calls.read_text().splitlines(),
                         ["stateset-embedded", "stateset-migrations", "stateset-http"])

    def test_failed_rerun_replaces_success_without_reusing_logs(self):
        old = self.assert_success(self.invoke())
        result = self.invoke(FAIL="stateset-embedded")
        self.assertNotEqual(result.returncode, 0)
        report = self.report()
        self.assertEqual(report["result"], "failed")
        self.assertNotEqual(report["invocation_id"], old["invocation_id"])
        check = report["checks"]["backup_restore"]
        self.assertEqual(check["exit_code"], 17)
        self.assertEqual(check["result"], "failed")
        self.assertEqual(check["log_sha256"],
                         hashlib.sha256((self.output / check["log"]).read_bytes()).hexdigest())
        for name in ("migration_rollback", "tenant_http_recovery"):
            self.assertEqual(report["checks"][name]["result"], "not_run")
            self.assertIsNone(report["checks"][name]["log_sha256"])
            self.assertFalse((self.output / report["checks"][name]["log"]).exists())

    def test_later_check_failure_preserves_completed_check(self):
        self.assertNotEqual(self.invoke(FAIL="stateset-http").returncode, 0)
        report = self.report()
        self.assertEqual(report["result"], "failed")
        self.assertEqual(report["checks"]["backup_restore"]["result"], "passed")
        self.assertEqual(report["checks"]["migration_rollback"]["result"], "passed")
        self.assertEqual(report["checks"]["tenant_http_recovery"]["result"], "failed")

    def test_missing_marker_fails_even_with_zero_exit(self):
        self.assertNotEqual(self.invoke(NO_MARKERS="1").returncode, 0)
        self.assertIn("proof marker missing", self.report()["error"])

    def test_marker_fragments_on_unrelated_lines_do_not_pass(self):
        self.assertNotEqual(self.invoke(SPLIT_MARKER="1").returncode, 0)
        self.assertIn("proof marker missing", self.report()["error"])

    def test_zero_tests_cannot_pass_even_with_markers(self):
        self.assertNotEqual(self.invoke(ZERO_TESTS="1").returncode, 0)
        self.assertIn("not report enough successful tests", self.report()["error"])

    def test_dirty_tracked_and_untracked_source_affects_fingerprint(self):
        clean = self.assert_success(self.invoke())
        (self.repo / "source.rs").write_text("uncommitted source")
        tracked = self.assert_success(self.invoke())
        self.assertTrue(tracked["source_before"]["dirty"])
        self.assertNotEqual(clean["source_before"]["sha256"], tracked["source_before"]["sha256"])
        (self.repo / "new.rs").write_text("untracked source")
        untracked = self.assert_success(self.invoke())
        self.assertNotEqual(tracked["source_before"]["sha256"], untracked["source_before"]["sha256"])
        self.assertEqual(untracked["source_before"]["file_count"],
                         tracked["source_before"]["file_count"] + 1)

    def test_source_mutation_prevents_success(self):
        self.assertNotEqual(self.invoke(MUTATE="1").returncode, 0)
        report = self.report()
        self.assertEqual(report["result"], "failed")
        self.assertFalse(report["source_unchanged"])
        self.assertTrue(all(check["result"] == "passed" for check in report["checks"].values()))

    def test_wrong_github_sha_fails_before_any_check(self):
        self.assert_success(self.invoke())
        self.calls.unlink()
        self.assertNotEqual(self.invoke(GITHUB_SHA="0" * 40).returncode, 0)
        report = self.report()
        self.assertEqual(report["result"], "failed")
        self.assertEqual(report["commit_sha"], self.head)
        self.assertIn("GITHUB_SHA", report["error"])
        self.assertFalse(self.calls.exists())

    def test_tool_lookup_failure_invalidates_previous_success(self):
        old = self.assert_success(self.invoke())
        self.calls.unlink()
        self.assertNotEqual(self.invoke(VERSION_FAIL="1").returncode, 0)
        report = self.report()
        self.assertEqual(report["result"], "failed")
        self.assertNotEqual(report["invocation_id"], old["invocation_id"])
        self.assertFalse(self.calls.exists())
        self.assertTrue(all(check["result"] == "not_run" for check in report["checks"].values()))

    def test_modes_symlink_targets_and_deleted_files_change_fingerprint(self):
        def fingerprint():
            return self.assert_success(self.invoke())["source_before"]["sha256"]

        hashes = [fingerprint()]
        source = self.repo / "source.rs"
        source.chmod(0o755)
        hashes.append(fingerprint())
        source.unlink()
        hashes.append(fingerprint())
        source.symlink_to("missing-target-one")
        hashes.append(fingerprint())
        source.unlink()
        source.symlink_to("missing-target-two")
        hashes.append(fingerprint())
        self.assertEqual(len(set(hashes)), len(hashes))

    def test_generated_output_in_worktree_is_excluded(self):
        self.output = self.repo / "local-evidence"
        self.env["RECOVERY_EVIDENCE_DIR"] = str(self.output)
        first = self.assert_success(self.invoke())
        second = self.assert_success(self.invoke())
        self.assertFalse(second["source_before"]["dirty"])
        self.assertEqual(first["source_before"], second["source_before"])

    def test_output_cannot_overwrite_tracked_source(self):
        before = (self.repo / "source.rs").read_bytes()
        result = self.invoke(RECOVERY_EVIDENCE_DIR=str(self.repo))
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.repo / "source.rs").read_bytes(), before)
        result = self.invoke(RECOVERY_EVIDENCE_DIR=str(self.repo / "scripts"))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("tracked source", result.stderr)

    def test_cancellation_stops_child_and_lock_protects_live_report(self):
        self.assert_success(self.invoke())
        with tempfile.TemporaryFile(mode="w+") as transcript:
            process = subprocess.Popen(self.command, cwd=self.repo, env={**self.env, "WAIT": "1"},
                                       stdout=transcript, stderr=subprocess.STDOUT)
            try:
                ready = Path(self.env["READY"])
                deadline = time.monotonic() + 10
                while not ready.exists() and process.poll() is None and time.monotonic() < deadline:
                    time.sleep(0.02)
                self.assertTrue(ready.exists(), "fixture check did not start")
                running = self.report()
                self.assertEqual(running["result"], "running")
                rejected = self.invoke()
                self.assertNotEqual(rejected.returncode, 0)
                self.assertIn("another recovery drill", rejected.stderr)
                self.assertEqual(self.report(), running)
                process.send_signal(signal.SIGTERM)
                self.assertEqual(process.wait(timeout=10), 128 + signal.SIGTERM)
                report = self.report()
                self.assertEqual(report["result"], "cancelled")
                self.assertEqual(report["checks"]["backup_restore"]["result"], "cancelled")
                self.assertTrue(Path(self.env["STOPPED"]).exists())
                check = report["checks"]["backup_restore"]
                self.assertEqual(check["log_sha256"],
                                 hashlib.sha256((self.output / check["log"]).read_bytes()).hexdigest())
            finally:
                if process.poll() is None:
                    process.terminate()
                    process.wait(timeout=10)
        self.assert_success(self.invoke())  # Lock was released on cancellation.


if __name__ == "__main__":
    unittest.main(verbosity=2)
