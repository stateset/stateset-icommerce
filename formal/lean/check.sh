#!/usr/bin/env bash
# Check the Lean proofs, and hold them to the code.
#
#   1. Every module must compile, and every kernel-safe declaration must rest
#      on Lean's standard axioms only -- a `sorry` depends on sorryAx;
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

# Discover every source module, including generators and future additions.
# Compile in dependency order using Lean itself, rather than parsing imports or
# maintaining an order by hand. A stuck pass reports every remaining error.
lean -o "$build/Audit.olean" Audit.lean
modules=()
for source in *.lean; do
  [[ "$source" == Audit.lean ]] && continue
  modules+=("${source%.lean}")
done
pending=("${modules[@]}")
while ((${#pending[@]})); do
  remaining=()
  for module in "${pending[@]}"; do
    if lean -DwarningAsError=true -o "$build/$module.olean" "$module.lean" > "$build/$module.log" 2>&1; then
      echo "ok: $module.lean compiled"
    else
      remaining+=("$module")
    fi
  done
  if ((${#remaining[@]} == ${#pending[@]})); then
    for module in "${remaining[@]}"; do cat "$build/$module.log" >&2; done
    echo "FAIL: proof compilation failed or imports form a cycle" >&2
    exit 1
  fi
  pending=("${remaining[@]}")
done

echo "== every compiled declaration must use only standard axioms"
for module in "${modules[@]}"; do
  printf 'import Audit\nimport %s\naudit_module %s\n' "$module" "$module" | lean --stdin
done
bash "$HERE/test-audit.sh" "$build"

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

echo "== reservation holds must match the TLC/Rust golden table"
lean --run InventoryGolden.lean > "$build/holds_stock.golden"
if diff -u ../tla/inventory/holds_stock.golden "$build/holds_stock.golden"; then
  echo "ok: 7 reservation statuses, identical"
else
  echo "FAIL: Lean reservation holds drifted from the TLC/Rust table" >&2
  fail=1
fi

echo "== SQLite transition fixtures must equal the proved executable model"
lean --run InventoryTransitionsGolden.lean > "$build/inventory_transitions.jsonl"
if diff -u inventory_transitions.golden.jsonl "$build/inventory_transitions.jsonl" > "$build/transitions.diff"; then
  echo "ok: $(wc -l < inventory_transitions.golden.jsonl) transition cases, identical"
else
  head -40 "$build/transitions.diff" >&2
  echo "FAIL: reservation transition fixtures drifted from the Lean model" >&2
  fail=1
fi

echo "== reservation preview predictions must equal the database fixture table"
lean --run InventoryPreviewGolden.lean > "$build/inventory_preview.golden"
if diff -u inventory_preview.golden "$build/inventory_preview.golden"; then
  echo "ok: 42 reservation preview cases, identical"
else
  echo "FAIL: reservation preview fixtures drifted from the Lean model" >&2
  fail=1
fi

echo "== receipt replay decisions must equal the Rust fixture table"
lean --run ReceiptPromotionGolden.lean > "$build/receipt_promotion.golden"
if diff -u receipt_promotion.golden "$build/receipt_promotion.golden"; then
  echo "ok: 32 receipt replay decisions, identical"
else
  echo "FAIL: receipt replay decisions drifted from the Lean model" >&2
  fail=1
fi

echo "== line-owned reservation selection must match the Rust planner fixtures"
lean --run ReservationOwnershipGolden.lean > "$build/reservation_ownership.jsonl"
if diff -u reservation_ownership.golden.jsonl "$build/reservation_ownership.jsonl" > "$build/ownership.diff"; then
  echo "ok: $(wc -l < reservation_ownership.golden.jsonl) ownership cases, identical"
else
  head -40 "$build/ownership.diff" >&2
  echo "FAIL: reservation ownership fixtures drifted from the Lean model" >&2
  fail=1
fi

exit $fail
