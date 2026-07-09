#!/usr/bin/env bash
set -euo pipefail

MODE="${POSTGRES_PARITY_MODE:-all}"
DRY_RUN="${DRY_RUN:-0}"

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

run_db_postgres() {
  local tests=(
    postgres_migrations
    postgres_validations
    postgres_order_transitions
    postgres_order_versioning
    postgres_crud
    postgres_agent_cards
    postgres_guard
    postgres_inventory_oversell
    postgres_refund_correctness
    postgres_money_guards
    postgres_x402_credits
    postgres_x402_payment_intents
  )

  for test_name in "${tests[@]}"; do
    run_cmd cargo test --locked -p stateset-db --no-default-features --features postgres --test "${test_name}"
  done
}

run_db_postgres_saga() {
  local tests=(
    postgres_migrations
    postgres_saga
  )

  for test_name in "${tests[@]}"; do
    run_cmd cargo test --locked -p stateset-db --no-default-features --features postgres,saga --test "${test_name}"
  done
}

run_embedded_postgres() {
  local tests=(
    postgres_async_smoke
    postgres_cart_checkout_smoke
    postgres_x402_smoke
  )

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
