#!/usr/bin/env python3
"""Exercise operator CLI permissions/preview against a private receipt-store fixture."""

import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest


BINARY = Path(sys.argv.pop(1)).resolve() if len(sys.argv) > 1 else None


class ReceiptOperatorTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="receipt-operator-test-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.store = self.directory / "store.db"
        with sqlite3.connect(self.store) as conn:
            conn.executescript("""
                CREATE TABLE http_idempotency_keys (
                    tenant TEXT, idempotency_key TEXT, request_fingerprint TEXT,
                    response_status INTEGER, content_type TEXT, response_body BLOB,
                    created_at INTEGER, created_at_sub_ms_ns INTEGER,
                    PRIMARY KEY (tenant, idempotency_key));
                CREATE TABLE audit_log (
                    action TEXT, entity_type TEXT, entity_id TEXT, actor TEXT, details TEXT);
            """)
            conn.execute("INSERT INTO http_idempotency_keys VALUES (?, ?, ?, 0, NULL, ?, ?, ?)",
                         ("tenant:operator", "original-key", "f" * 64, b"", 1700000000000, 123))
        self.resolution = {
            "tenant": "tenant:operator", "idempotency_key": "original-key",
            "request_fingerprint": "f" * 64,
            "reserved_at": "2023-11-14T22:13:20.000000123Z", "response_status": 201,
            "content_type": "application/json", "response_body": list(b'{"amount":"0.10"}'),
            "actor": "operator:verified", "evidence": "incident:provider-receipt-verified",
        }
        self.proposal = self.directory / "resolution.json"
        self.proposal.write_text(json.dumps(self.resolution))

    def invoke(self, *args, store=None):
        return subprocess.run([str(BINARY), str(store or self.store), *args],
                              text=True, capture_output=True, timeout=15)

    def state(self):
        with sqlite3.connect(self.store) as conn:
            receipt = conn.execute("SELECT response_status, response_body FROM http_idempotency_keys").fetchone()
            audit_count = conn.execute("SELECT COUNT(*) FROM audit_log").fetchone()[0]
            return receipt, audit_count

    def test_inspection_is_scoped_and_does_not_migrate(self):
        result = self.invoke("list", "tenant:operator")
        self.assertEqual(result.returncode, 0, result.stderr)
        rows = json.loads(result.stdout)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["reserved_at"], self.resolution["reserved_at"])
        self.assertEqual(json.loads(self.invoke("list", "tenant:other").stdout), [])
        with sqlite3.connect(self.store) as conn:
            self.assertEqual(conn.execute("SELECT COUNT(*) FROM sqlite_master WHERE type='table'").fetchone()[0], 2)
        self.assertEqual(self.state(), ((0, b""), 0))

    def test_preview_and_missing_worker_confirmation_never_write(self):
        result = self.invoke("resolve", str(self.proposal))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Preview", result.stdout)
        rejected = self.invoke("resolve", str(self.proposal), "--apply")
        self.assertNotEqual(rejected.returncode, 0)
        self.assertIn("worker", rejected.stderr)
        self.assertEqual(self.state(), ((0, b""), 0))

    def test_verified_completion_is_audited_and_duplicate_cannot_overwrite(self):
        result = self.invoke("resolve", str(self.proposal), "--apply", "--worker-stopped")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.state(), ((201, b'{"amount":"0.10"}'), 1))
        repeated = self.invoke("resolve", str(self.proposal), "--apply", "--worker-stopped")
        self.assertNotEqual(repeated.returncode, 0)
        self.assertEqual(self.state(), ((201, b'{"amount":"0.10"}'), 1))

    def test_stale_evidence_and_unknown_fields_are_refused(self):
        self.resolution["reserved_at"] = "2023-11-14T22:13:20.000000124Z"
        self.proposal.write_text(json.dumps(self.resolution))
        self.assertNotEqual(self.invoke("resolve", str(self.proposal), "--apply", "--worker-stopped").returncode, 0)
        self.resolution["model_approval"] = True
        self.proposal.write_text(json.dumps(self.resolution))
        self.assertNotEqual(self.invoke("resolve", str(self.proposal)).returncode, 0)
        self.assertEqual(self.state(), ((0, b""), 0))

    def test_failed_required_audit_leaves_receipt_pending(self):
        with sqlite3.connect(self.store) as conn:
            conn.executescript("CREATE TRIGGER reject_audit BEFORE INSERT ON audit_log BEGIN SELECT RAISE(ABORT, 'audit denied'); END;")
        result = self.invoke("resolve", str(self.proposal), "--apply", "--worker-stopped")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("audit denied", result.stderr)
        self.assertEqual(self.state(), ((0, b""), 0))

    def test_missing_store_is_never_created_by_inspection_or_apply(self):
        missing = self.directory / "missing.db"
        self.assertNotEqual(self.invoke("list", "tenant:operator", store=missing).returncode, 0)
        self.assertFalse(missing.exists())
        self.assertNotEqual(self.invoke("resolve", str(self.proposal), "--apply", "--worker-stopped", store=missing).returncode, 0)
        self.assertFalse(missing.exists())


if __name__ == "__main__":
    if BINARY is None or not BINARY.is_file():
        raise SystemExit("usage: test_http_receipts_example.py PATH_TO_HTTP_RECEIPTS_BINARY")
    unittest.main()
