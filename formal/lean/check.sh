#!/usr/bin/env bash
# Check the Lean proofs, and hold them to the code.
#
#   1. Every proof file must compile, and its public theorems must rest on
#      Lean's standard axioms only -- a `sorry` would show up as sorryAx;
#   2. the golden vectors the proved model computes must equal the committed
#      allocate_rounded.golden.json, which the Rust test
#      `allocate_rounded_matches_the_lean_model` holds the engine to.
#
# Usage: formal/lean/check.sh   (needs the `lean` named in lean-toolchain on
#                                PATH; with elan that is automatic)
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"
want="$(sed 's/.*:v//' lean-toolchain)"
have="$(lean --version | sed -E 's/^Lean \(version ([0-9.]+).*/\1/')"
if [[ "$have" != "$want" ]]; then
  echo "FAIL: lean-toolchain pins $want but lean on PATH is $have" >&2
  exit 1
fi

build="$(mktemp -d)"
trap 'rm -rf "$build"' EXIT
LEAN_PATH="$build:$(lean --print-prefix)/lib/lean"
export LEAN_PATH
fail=0

echo "== Allocation.lean: the proofs must check"
lean -o "$build/Allocation.olean" Allocation.lean
echo "== MerklePath.lean: the proofs must check"
lean -o "$build/MerklePath.olean" MerklePath.lean
echo "== RevenueSchedule.lean: the proofs must check"
lean -o "$build/RevenueSchedule.olean" RevenueSchedule.lean
echo "== LedgerRevaluation.lean: the proofs must check"
lean -o "$build/LedgerRevaluation.olean" LedgerRevaluation.lean

echo "== the theorems must use no axiom beyond Lean's standard three"
theorems=(round_within residue_le_length select_marks nudge_sum allocate_sum allocate_near)
axioms="$(printf 'import Allocation\nimport MerklePath\nimport RevenueSchedule\nimport LedgerRevaluation\n' ; for t in "${theorems[@]}"; do printf '#print axioms Allocation.%s\n' "$t"; done; printf '#print axioms MerklePath.leaf_binding\n#print axioms MerklePath.index_exhausted\n#print axioms RevenueSchedule.ratable_sum\n#print axioms RevenueSchedule.recognized_add_deferred\n#print axioms LedgerRevaluation.journal_balanced\n#print axioms LedgerRevaluation.journal_lines_valid\n#print axioms LedgerRevaluation.reversal_balanced\n')"
report="$(lean --stdin <<< "$axioms")"
echo "$report"
if grep -Eo '\b[A-Za-z.]+\b' <<< "$(grep -o '\[.*\]' <<< "$report")" \
    | grep -vxE 'propext|Classical\.choice|Quot\.sound' | grep -q .; then
  echo "FAIL: a theorem depends on a non-standard axiom (sorryAx means an unfinished proof)" >&2
  fail=1
fi

echo "== the model's golden vectors must equal allocate_rounded.golden.json"
lean --run Golden.lean > "$build/golden.json"
if diff -u allocate_rounded.golden.json "$build/golden.json" > "$build/golden.diff"; then
  echo "ok: $(grep -c '"strategy"' allocate_rounded.golden.json) vectors, identical"
else
  head -40 "$build/golden.diff" >&2
  echo "FAIL: regenerate with 'lean --run Golden.lean > allocate_rounded.golden.json'" >&2
  echo "      (LEAN_PATH as in this script) and rerun the Rust test." >&2
  fail=1
fi

exit $fail
