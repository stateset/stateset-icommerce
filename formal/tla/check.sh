#!/usr/bin/env bash
# Model-check the TLA+ specs, and hold each one to the code.
#
# Every spec checks its guarded configuration and requires a counterexample
# from a deliberately broken one. PaymentRefunds additionally shares a
# transition golden file with Rust; the newer protocols are tied to focused
# repository regressions for the modelled races.
#
# Usage: formal/tla/check.sh            (downloads tla2tools.jar if needed)
#        TLA2TOOLS=/path/to/jar formal/tla/check.sh
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
# v1.8.0 is a rolling prerelease: upstream replaces its jar at the same URL.
# Pin the stable v1.7.4 release and its exact bytes instead.
TLA_VERSION="1.7.4"
TLA_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
JAR="${TLA2TOOLS:-${XDG_CACHE_HOME:-$HOME/.cache}/tla2tools-${TLA_VERSION}.jar}"

if [[ ! -f "$JAR" ]]; then
  mkdir -p "$(dirname "$JAR")"
  curl -fsSL -o "$JAR" \
    "https://github.com/tlaplus/tlaplus/releases/download/v${TLA_VERSION}/tla2tools.jar"
fi
echo "${TLA_SHA256}  ${JAR}" | sha256sum -c --quiet - \
  || { echo "tla2tools.jar checksum mismatch" >&2; exit 1; }

tlc() {
  # -cleanup removes TLC's state directory. v1.7.4 predates -noGenerateSpecTE.
  java -XX:+UseParallelGC -cp "$JAR" tlc2.TLC -workers auto -cleanup "$@"
}

fail=0
cd "$HERE/payments"
# A run that ends on a counterexample leaves its state directory behind.
trap 'rm -rf "$HERE/payments/states" "$HERE/inventory/states" "$HERE/x402/states" "$HERE/returns/states" "$HERE/finance/states" "$HERE/subscriptions/states" "$HERE/warehouse/states"' EXIT

echo "== PaymentRefunds: the implementation (Locked = TRUE) must satisfy every property"
if tlc -config PaymentRefunds_locked.cfg PaymentRefunds.tla > locked.log 2>&1; then
  grep -E 'distinct states found|No error has been found' locked.log || true
else
  echo "FAIL: the locked model violated a property:" >&2
  grep -E 'Error:|is violated' locked.log >&2 || tail -20 locked.log >&2
  fail=1
fi

echo "== PaymentRefunds: without the lock (Locked = FALSE) TLC must find an over-refund"
set +e
tlc -config PaymentRefunds_unlocked.cfg PaymentRefunds.tla > unlocked.log 2>&1
code=$?
set -e
if [[ $code -eq 12 ]] && grep -q 'Invariant NoOverRefund is violated' unlocked.log; then
  echo "ok: NoOverRefund is violated without the lock, as it must be"
else
  echo "FAIL: expected TLC to report 'Invariant NoOverRefund is violated' (exit 12), got exit $code." >&2
  echo "      The model no longer demonstrates the race the lock prevents." >&2
  fail=1
fi
rm -rf states

echo "== PaymentRefunds: the spec's state machine must equal can_transition.golden"
cp PaymentRefunds_locked.cfg PrintTransitions.cfg
tlc -config PrintTransitions.cfg PrintTransitions.tla 2>/dev/null \
  | grep -E '^"[a-z_]+ -> [a-z_]+"$' | tr -d '"' | sort > spec_transitions.txt
rm -f PrintTransitions.cfg
if diff -u can_transition.golden spec_transitions.txt; then
  echo "ok: $(wc -l < spec_transitions.txt) transitions, identical to the golden file"
else
  echo "FAIL: the spec's CanTransition drifted from can_transition.golden (diff above)." >&2
  fail=1
fi
rm -f spec_transitions.txt locked.log unlocked.log

check_pair() {
  local dir="$1" module="$2" good="$3" bad="$4" invariant="$5"
  local code
  cd "$HERE/$dir"
  echo "== $module: guarded configuration"
  if tlc -config "$good" "$module.tla" > guarded.log 2>&1; then
    grep -E 'distinct states found|No error has been found' guarded.log || true
  else
    echo "FAIL: $module violated a property:" >&2
    grep -E 'Error:|is violated' guarded.log >&2 || tail -20 guarded.log >&2
    fail=1
  fi
  echo "== $module: $bad must violate $invariant"
  set +e
  tlc -config "$bad" "$module.tla" > broken.log 2>&1
  code=$?
  set -e
  if [[ $code -eq 12 ]] && grep -q "Invariant $invariant is violated" broken.log; then
    echo "ok: $invariant counterexample found"
  else
    echo "FAIL: expected $invariant counterexample (exit 12), got exit $code" >&2
    fail=1
  fi
  rm -f guarded.log broken.log
  rm -rf states
}

check_pair inventory InventoryReservations InventoryReservations_guarded.cfg InventoryReservations_unguarded.cfg AllocatedMatchesOpen
check_pair x402 X402Claims X402Claims_atomic.cfg X402Claims_split.cfg NoDuplicateClaim
check_pair x402 EscrowSettlement EscrowSettlement_guarded.cfg EscrowSettlement_split.cfg NoDoubleSettlement

cd "$HERE/x402"
echo "== EscrowSettlement: unbalanced dispute split must violate settlement conservation"
set +e
tlc -config EscrowSettlement_unbalanced.cfg EscrowSettlement.tla > unbalanced.log 2>&1
code=$?
set -e
if [[ $code -eq 12 ]] && grep -q 'Invariant NoDoubleSettlement is violated' unbalanced.log; then
  echo "ok: NoDoubleSettlement counterexample found for an unbalanced split"
else
  echo "FAIL: expected unbalanced split counterexample (exit 12), got exit $code" >&2
  fail=1
fi
rm -f unbalanced.log
rm -rf states

check_pair returns ShippedReturns ShippedReturns_guarded.cfg ShippedReturns_split.cfg NoOverReturn

cd "$HERE/returns"
echo "== ShippedReturns: allowing rejection after disposition must violate claim persistence"
set +e
tlc -config ShippedReturns_unprotected.cfg ShippedReturns.tla > unprotected.log 2>&1
code=$?
set -e
if [[ $code -eq 12 ]] && grep -q 'Invariant DispositionClaimsPersist is violated' unprotected.log; then
  echo "ok: DispositionClaimsPersist counterexample found"
else
  echo "FAIL: expected DispositionClaimsPersist counterexample (exit 12), got exit $code" >&2
  fail=1
fi
rm -f unprotected.log
rm -rf states

check_pair finance PeriodClose PeriodClose_atomic.cfg PeriodClose_split.cfg ClosedBalanceFrozen
check_pair subscriptions BillingClaim BillingClaim_atomic.cfg BillingClaim_split.cfg OneCyclePerKey
check_pair subscriptions ChargeRetry ChargeRetry_atomic.cfg ChargeRetry_split.cfg AtMostOneLivePayment
check_pair warehouse LocationMove LocationMove_atomic.cfg LocationMove_split.cfg StockConserved
check_pair warehouse LotGenealogy LotGenealogy_complete.cfg LotGenealogy_missing.cfg TraceComplete

exit $fail
