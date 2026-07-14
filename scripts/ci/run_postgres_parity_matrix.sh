#!/usr/bin/env bash
set -euo pipefail

MODE="${POSTGRES_PARITY_MODE:-all}"
DRY_RUN="${DRY_RUN:-0}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

if [[ -z "${POSTGRES_URL:-}" && -z "${DATABASE_URL:-}" ]]; then
  echo "error: set POSTGRES_URL or DATABASE_URL for postgres parity tests" >&2
  exit 1
fi

if [[ -z "${POSTGRES_URL:-}" ]]; then
  export POSTGRES_URL="${DATABASE_URL}"
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
  export DATABASE_URL="${POSTGRES_URL}"
fi

run_cmd() {
  echo "+ $*"
  if [[ "${DRY_RUN}" == "1" ]]; then
    return 0
  fi
  "$@"
}

# Auto-discover postgres_*.rs integration tests so a new test file can never
# silently escape the parity matrix. Saga-feature tests follow the
# postgres_saga* naming convention and run in their own lane.
discover_tests() {
  local dir="$1"
  local include_pattern="$2"
  local exclude_pattern="${3:-}"

  local names
  names="$(find "${dir}" -maxdepth 1 -name "${include_pattern}.rs" -exec basename {} .rs \; | sort)"
  if [[ -n "${exclude_pattern}" ]]; then
    names="$(printf '%s\n' "${names}" | grep -v "^${exclude_pattern}$" || true)"
  fi

  if [[ -z "${names}" ]]; then
    echo "error: no tests matching '${include_pattern}' found in ${dir}" >&2
    exit 1
  fi
  printf '%s\n' "${names}"
}

run_db_postgres() {
  local tests
  mapfile -t tests < <(discover_tests "${REPO_ROOT}/crates/stateset-db/tests" 'postgres_*' 'postgres_saga.*')

  for test_name in "${tests[@]}"; do
    run_cmd cargo test --locked -p stateset-db --no-default-features --features postgres --test "${test_name}"
  done
}

run_db_postgres_saga() {
  # postgres_migrations carries saga-gated migration coverage, so it runs in
  # this lane too; every postgres_saga* test is discovered automatically.
  local tests=(postgres_migrations)
  mapfile -t saga_tests < <(find "${REPO_ROOT}/crates/stateset-db/tests" -maxdepth 1 -name 'postgres_saga*.rs' -exec basename {} .rs \; | sort)
  if [[ ${#saga_tests[@]} -eq 0 ]]; then
    echo "error: no postgres_saga* tests found in crates/stateset-db/tests" >&2
    exit 1
  fi
  tests+=("${saga_tests[@]}")

  for test_name in "${tests[@]}"; do
    run_cmd cargo test --locked -p stateset-db --no-default-features --features postgres,saga --test "${test_name}"
  done
}

run_embedded_postgres() {
  local tests
  mapfile -t tests < <(discover_tests "${REPO_ROOT}/crates/stateset-embedded/tests" 'postgres_*')

  for test_name in "${tests[@]}"; do
    run_cmd cargo test --locked -p stateset-embedded --features postgres --test "${test_name}"
  done
}

case "${MODE}" in
  db-postgres)
    run_db_postgres
    ;;
  db-postgres-saga)
    run_db_postgres_saga
    ;;
  embedded-postgres)
    run_embedded_postgres
    ;;
  all)
    run_db_postgres
    run_db_postgres_saga
    run_embedded_postgres
    ;;
  *)
    echo "error: unsupported POSTGRES_PARITY_MODE '${MODE}'" >&2
    echo "valid values: db-postgres, db-postgres-saga, embedded-postgres, all" >&2
    exit 1
    ;;
esac
