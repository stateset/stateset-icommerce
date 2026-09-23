#!/usr/bin/env bash
# Model-check the TLA+ specs, and hold each one to the code.
#
# For each spec this proves three things, and fails if any stops being true:
#   1. the implementation's configuration satisfies every invariant;
#   2. the deliberately broken configuration VIOLATES one -- a model that
#      cannot fail proves nothing, so a model that has stopped finding the
#      bug it was built to find is itself a failure;
#   3. the spec's state machine equals the golden file the Rust tests use,
#      so the proof about the spec stays a proof about the code.
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
trap 'rm -rf "$HERE/payments/states"' EXIT

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

exit $fail
