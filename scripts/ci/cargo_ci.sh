#!/usr/bin/env bash
set -euo pipefail

# Keep CI-style Rust checks lean so the monorepo quality gate does not exhaust
# disk on debug symbols or incremental caches.
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0

exec cargo "$@"
