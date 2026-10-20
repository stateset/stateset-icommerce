#!/usr/bin/env bash
# Exercise the audit's failure paths, not just its acceptance of good proofs.
# Invoked by check.sh with its compiled Audit.olean directory.
set -euo pipefail
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
export LEAN_PATH="$scratch:$1:${LEAN_PATH:-}"

cat > "$scratch/AuditGood.lean" <<'LEAN'
theorem good (n : Nat) : n = n := rfl
LEAN
cat > "$scratch/AuditPrivateHole.lean" <<'LEAN'
private theorem unused_private : False := by sorry
theorem good : True := True.intro
LEAN
cat > "$scratch/AuditExtraAxiom.lean" <<'LEAN'
axiom unreferenced : False
theorem good : True := True.intro
LEAN
cat > "$scratch/AuditDefinitionHole.lean" <<'LEAN'
def untrusted : Nat := by sorry
theorem good : True := True.intro
LEAN

for module in AuditGood AuditPrivateHole AuditExtraAxiom AuditDefinitionHole; do
  if ! lean --root="$scratch" -o "$scratch/$module.olean" "$scratch/$module.lean" > "$scratch/compile.log" 2>&1; then
    cat "$scratch/compile.log" >&2
    exit 1
  fi
  printf 'import Audit\nimport %s\naudit_module %s\n' "$module" "$module" > "$scratch/Run.lean"
  if lean --root="$scratch" "$scratch/Run.lean" > "$scratch/result.log" 2>&1; then
    if [[ "$module" != AuditGood ]]; then
      echo "FAIL: declaration audit accepted $module" >&2
      exit 1
    fi
  else
    if [[ "$module" == AuditGood ]] || ! grep -q 'forbidden axiom' "$scratch/result.log"; then
      cat "$scratch/result.log" >&2
      echo "FAIL: unexpected audit result for $module" >&2
      exit 1
    fi
  fi
done
echo "ok: declaration audit rejects private proof holes, unused axioms, and definition holes"
