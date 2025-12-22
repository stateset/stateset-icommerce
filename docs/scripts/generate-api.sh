#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/docs/api"

log() {
  printf "[docs] %s\n" "$*"
}

have() {
  command -v "$1" >/dev/null 2>&1
}

clean_dir() {
  local dir="$1"
  if [ -d "$dir" ]; then
    rm -rf "$dir"
  fi
  mkdir -p "$dir"
}

mkdir -p "$OUT"

# Rust (rustdoc)
if have cargo; then
  log "Rust: generating rustdoc"
  if (cd "$ROOT" && cargo doc -p stateset-embedded --no-deps); then
    clean_dir "$OUT/rust"
    cp -R "$ROOT/target/doc" "$OUT/rust"
  else
    log "Rust: failed to generate rustdoc"
  fi
else
  log "Rust: skipped (cargo not found)"
fi

# Node.js (TypeScript definitions)
if have typedoc; then
  log "Node.js: generating typedoc"
  clean_dir "$OUT/node"
  if ! typedoc --entryPoints "$ROOT/bindings/node/index.d.ts" --out "$OUT/node"; then
    log "Node.js: failed to generate typedoc"
  fi
else
  log "Node.js: skipped (typedoc not found)"
fi

# Python (pdoc)
PYTHON_BIN=""
if have python3; then
  PYTHON_BIN="python3"
elif have python; then
  PYTHON_BIN="python"
fi

if [ -n "$PYTHON_BIN" ]; then
  if "$PYTHON_BIN" - <<'PY'
import importlib.util
import sys
sys.exit(0 if importlib.util.find_spec("pdoc") else 1)
PY
  then
    log "Python: generating pdoc"
    clean_dir "$OUT/python"
    if ! "$PYTHON_BIN" -m pdoc -o "$OUT/python" "$ROOT/bindings/python/python/stateset_embedded"; then
      log "Python: failed to generate pdoc"
    fi
  else
    log "Python: skipped (pdoc not installed)"
  fi
else
  log "Python: skipped (python not found)"
fi

# Ruby (yard)
if have yard; then
  log "Ruby: generating yard docs"
  clean_dir "$OUT/ruby"
  if ! yard doc -o "$OUT/ruby" "$ROOT/bindings/ruby/lib"; then
    log "Ruby: failed to generate yard docs"
  fi
else
  log "Ruby: skipped (yard not found)"
fi

# PHP (phpDocumentor)
if have phpdoc; then
  log "PHP: generating phpDocumentor"
  clean_dir "$OUT/php"
  if ! phpdoc -d "$ROOT/bindings/php/stubs" -t "$OUT/php"; then
    log "PHP: failed to generate phpDocumentor"
  fi
else
  log "PHP: skipped (phpdoc not found)"
fi

# Java (javadoc)
if have javadoc; then
  log "Java: generating javadoc"
  clean_dir "$OUT/java"
  if ! javadoc -d "$OUT/java" $(find "$ROOT/bindings/java/java/src/main/java" -name '*.java'); then
    log "Java: failed to generate javadoc"
  fi
else
  log "Java: skipped (javadoc not found)"
fi

# Kotlin (Dokka)
if have dokka; then
  log "Kotlin: generating Dokka docs"
  clean_dir "$OUT/kotlin"
  if ! dokka -outputDir "$OUT/kotlin"; then
    log "Kotlin: failed to generate Dokka docs"
  fi
else
  log "Kotlin: skipped (dokka not found)"
fi

# Swift (Jazzy)
if have jazzy; then
  log "Swift: generating Jazzy docs"
  clean_dir "$OUT/swift"
  if ! (cd "$ROOT/bindings/swift" && jazzy --output "$OUT/swift"); then
    log "Swift: failed to generate Jazzy docs"
  fi
else
  log "Swift: skipped (jazzy not found)"
fi

# .NET (DocFX)
if have docfx; then
  log ".NET: generating DocFX docs"
  clean_dir "$OUT/dotnet"
  if ! (cd "$ROOT/bindings/dotnet/dotnet" && docfx); then
    log ".NET: failed to generate DocFX docs"
  fi
else
  log ".NET: skipped (docfx not found)"
fi

# Go (pkgsite)
if have pkgsite; then
  log "Go: generating pkgsite docs"
  clean_dir "$OUT/go"
  if ! (cd "$ROOT/bindings/go/stateset" && pkgsite -dir "$OUT/go"); then
    log "Go: failed to generate pkgsite docs"
  fi
else
  log "Go: skipped (pkgsite not found)"
fi

# WASM (TypeScript definitions)
if have typedoc; then
  log "WASM: generating typedoc"
  clean_dir "$OUT/wasm"
  if ! typedoc --entryPoints "$ROOT/bindings/wasm/pkg/stateset_embedded.d.ts" --out "$OUT/wasm"; then
    log "WASM: failed to generate typedoc"
  fi
else
  log "WASM: skipped (typedoc not found)"
fi

log "API generation complete"
