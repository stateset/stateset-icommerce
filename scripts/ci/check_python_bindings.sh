#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
PYTHON_BINDINGS_DIR="$ROOT_DIR/bindings/python"
PYTHON_BIN="${STATESET_PYTHON_BIN:-}"

if [[ -z "$PYTHON_BIN" ]]; then
  if [[ -x "$PYTHON_BINDINGS_DIR/.venv/bin/python" ]]; then
    PYTHON_BIN="$PYTHON_BINDINGS_DIR/.venv/bin/python"
  elif command -v python3 >/dev/null 2>&1; then
    PYTHON_BIN="python3"
  elif command -v python >/dev/null 2>&1; then
    PYTHON_BIN="python"
  else
    echo "Python interpreter not found. Set STATESET_PYTHON_BIN or create bindings/python/.venv." >&2
    exit 1
  fi
fi

# A repository-local virtualenv may contain an extension compiled from an older
# checkout. Rebuild it before exercising source tests so the local release gate
# validates the current Rust binding rather than stale site-packages. CI installs
# a freshly built wheel into its system interpreter in the preceding workflow
# step, so it intentionally skips this developer-only refresh.
if [[ "$PYTHON_BIN" == "$PYTHON_BINDINGS_DIR/.venv/bin/python" ]]; then
  (
    cd "$PYTHON_BINDINGS_DIR"
    "$PYTHON_BIN" -m maturin develop --quiet
  )
fi

"$PYTHON_BIN" -m py_compile \
  "$PYTHON_BINDINGS_DIR/python/stateset_embedded/agent_toolkit.py" \
  "$PYTHON_BINDINGS_DIR/python/stateset_embedded/langchain.py" \
  "$PYTHON_BINDINGS_DIR/python/stateset_embedded/crewai.py" \
  "$PYTHON_BINDINGS_DIR/python/stateset_embedded/autogen.py" \
  "$PYTHON_BINDINGS_DIR/python/stateset_embedded/openai.py" \
  "$PYTHON_BINDINGS_DIR/python/stateset_embedded/generic.py" \
  "$ROOT_DIR/examples/python/embedded_runtime.py" \
  "$ROOT_DIR/examples/python/agent_toolkit.py" \
  "$ROOT_DIR/examples/python/openai_tools.py" \
  "$ROOT_DIR/examples/python/generic_tools.py" \
  "$ROOT_DIR/examples/python/langchain_tools.py" \
  "$ROOT_DIR/examples/python/crewai_tools.py" \
  "$ROOT_DIR/examples/python/autogen_tools.py" \
  "$ROOT_DIR/examples/python/framework_adapters.py"

STATESET_TOOLKIT_QUIET=1 "$PYTHON_BIN" "$ROOT_DIR/examples/python/agent_toolkit.py"
STATESET_TOOLKIT_QUIET=1 "$PYTHON_BIN" "$ROOT_DIR/examples/python/openai_tools.py"
STATESET_TOOLKIT_QUIET=1 "$PYTHON_BIN" "$ROOT_DIR/examples/python/generic_tools.py"
STATESET_TOOLKIT_QUIET=1 "$PYTHON_BIN" "$ROOT_DIR/examples/python/langchain_tools.py"
STATESET_TOOLKIT_QUIET=1 "$PYTHON_BIN" "$ROOT_DIR/examples/python/crewai_tools.py"
STATESET_TOOLKIT_QUIET=1 "$PYTHON_BIN" "$ROOT_DIR/examples/python/autogen_tools.py"
STATESET_TOOLKIT_QUIET=1 "$PYTHON_BIN" "$ROOT_DIR/examples/python/framework_adapters.py"

(
  cd "$PYTHON_BINDINGS_DIR"
  "$PYTHON_BIN" -m pytest tests/test_commerce.py tests/test_sync_runtime.py tests/test_agent_toolkit.py -q
)

"$PYTHON_BIN" "$ROOT_DIR/scripts/ci/check_python_binding_package.py" "$PYTHON_BINDINGS_DIR" "$PYTHON_BIN"
