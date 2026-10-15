#!/usr/bin/env python3
"""Coverage gates fail closed on partial, invalid, or insufficient evidence."""

import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("db_coverage", Path(__file__).with_name("check_db_coverage.py"))
coverage = importlib.util.module_from_spec(spec)
spec.loader.exec_module(coverage)


def record(name, covered=99, count=100):
    return {"filename": str(coverage.DB_ROOT / name),
            "summary": {"lines": {"covered": covered, "count": count, "percent": 100}}}


class CoverageGateTests(unittest.TestCase):
    def report(self):
        return {"data": [{"files": [record(name) for name in sorted(coverage.REQUIRED_FILES)]}]}

    def test_measured_counts_override_reported_percent(self):
        report = self.report()
        report["data"][0]["files"] = [record(name, 89 if name == "src/maintenance.rs" else 99)
                                         for name in sorted(coverage.REQUIRED_FILES)]
        results = coverage.evaluate(report)
        self.assertFalse(next(item for item in results if item["name"] == "src/maintenance.rs")["passed"])

    def test_exact_floor_passes(self):
        report = self.report()
        for item in report["data"][0]["files"]:
            name = Path(item["filename"]).relative_to(coverage.DB_ROOT).as_posix()
            item["summary"]["lines"]["covered"] = int(
                coverage.CRITICAL_FILES.get(name, coverage.MINIMUM_LINES))
        self.assertTrue(all(result["passed"] for result in coverage.evaluate(report)))

    def test_global_floor_accepts_boundary_and_rejects_one_line_below(self):
        for delta, passed in ((0, True), (-1, False)):
            report = self.report()
            files = report["data"][0]["files"]
            covered = sum(item["summary"]["lines"]["covered"] for item in files)
            count = sum(item["summary"]["lines"]["count"] for item in files)
            files.append(record("src/sqlite/extra.rs",
                                int(coverage.MINIMUM_LINES * 100) - covered + delta,
                                10000 - count))
            self.assertEqual(coverage.evaluate(report)[0]["passed"], passed)

    def test_dependencies_cannot_inflate_database_coverage(self):
        report = self.report()
        report["data"][0]["files"].append(record("src/sqlite/extra.rs", 0, 10000))
        report["data"][0]["files"].append(record("../stateset-core/src/lib.rs", 100000, 100000))
        self.assertFalse(coverage.evaluate(report)[0]["passed"])

    def test_missing_and_duplicate_files_fail(self):
        for mutate in (lambda files: files.pop(), lambda files: files.append(files[0])):
            report = self.report()
            mutate(report["data"][0]["files"])
            with self.assertRaises(ValueError):
                coverage.evaluate(report)

    def test_invalid_counts_fail(self):
        for covered, count in ((101, 100), (-1, 100), (True, 100), (95, "100"), (0, 0)):
            report = self.report()
            report["data"][0]["files"][0]["summary"]["lines"] = {"covered": covered, "count": count}
            with self.assertRaises(ValueError):
                coverage.evaluate(report)


if __name__ == "__main__":
    unittest.main()
