#!/usr/bin/env python3
"""Enforce SQLite database coverage without diluting it with dependency code."""

import argparse
from decimal import Decimal
import json
from pathlib import Path
import sys

DB_ROOT = Path(__file__).resolve().parents[2] / "crates/stateset-db"
# Ratchet measured floors upward; never lower them to admit a regression.
MINIMUM_LINES = Decimal("78")
CRITICAL_FILES = {
    "src/maintenance.rs": Decimal("94"),
    "src/backup.rs": Decimal("96"),
}
REQUIRED_FILES = {*CRITICAL_FILES, "src/sqlite/inventory.rs", "src/sqlite/orders.rs",
                  "src/sqlite/payments.rs", "src/sqlite/kernel_executor.rs"}


def evaluate(report, root=DB_ROOT):
    files = {}
    for data in report["data"]:
        for record in data["files"]:
            try:
                name = Path(record["filename"]).resolve().relative_to(root.resolve()).as_posix()
            except ValueError:
                continue
            if not name.startswith("src/") or not name.endswith(".rs"):
                continue
            if name in files:
                raise ValueError(f"duplicate coverage record: {name}")
            lines = record["summary"]["lines"]
            covered, count = lines["covered"], lines["count"]
            if type(covered) is not int or type(count) is not int or not 0 <= covered <= count:
                raise ValueError(f"invalid line counts: {name}")
            files[name] = (covered, count)
    missing = REQUIRED_FILES - files.keys()
    if missing:
        raise ValueError(f"missing database coverage: {', '.join(sorted(missing))}")
    checks = [("stateset-db (SQLite)", sum(c for c, _ in files.values()),
               sum(n for _, n in files.values()), MINIMUM_LINES)]
    checks.extend((name, *files[name], floor) for name, floor in CRITICAL_FILES.items())
    results = []
    for name, covered, count, floor in checks:
        if count == 0:
            raise ValueError(f"empty coverage: {name}")
        percent = Decimal(covered) * 100 / Decimal(count)
        results.append({"name": name, "covered": covered, "count": count,
                        "percent": percent, "floor": floor, "passed": percent >= floor})
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path, help="cargo llvm-cov JSON export")
    args = parser.parse_args()
    try:
        results = evaluate(json.loads(args.report.read_text()))
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"Database coverage unavailable: {error}", file=sys.stderr)
        return 1
    for result in results:
        print(f"{'PASS' if result['passed'] else 'FAIL'} {result['name']}: "
              f"{result['percent']:.2f}% ({result['covered']}/{result['count']} lines; "
              f"floor {result['floor']}%)")
    return 0 if all(result["passed"] for result in results) else 1


if __name__ == "__main__":
    sys.exit(main())
