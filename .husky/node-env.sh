#!/usr/bin/env sh
#
# Ensure Git hooks run on the repo's pinned Node version.
# Git hooks run in a non-interactive shell, so nvm isn't always loaded.
#

fallback_required_version=20.20.0

node_version() {
  node -p "process.versions.node" 2>/dev/null || echo 0.0.0
}

version_ge() {
  awk -v current="$1" -v required="$2" '
    BEGIN {
      split(current, c, ".");
      split(required, r, ".");
      for (i = 1; i <= 3; i++) {
        cv = (c[i] == "" ? 0 : c[i]) + 0;
        rv = (r[i] == "" ? 0 : r[i]) + 0;
        if (cv > rv) exit 0;
        if (cv < rv) exit 1;
      }
      exit 0;
    }
  '
}

hook_dir=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$hook_dir/.." && pwd)

required_version=""
if [ -f "$repo_root/.node-version" ]; then
  required_version=$(tr -d '[:space:]' < "$repo_root/.node-version")
elif [ -f "$repo_root/.nvmrc" ]; then
  required_version=$(tr -d '[:space:]' < "$repo_root/.nvmrc")
fi

if [ -z "$required_version" ]; then
  required_version=$fallback_required_version
fi

current_version=$(node_version)
if ! version_ge "$current_version" "$required_version"; then
  # Try nvm if installed.
  export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
  if [ -s "$NVM_DIR/nvm.sh" ]; then
    # shellcheck disable=SC1090
    . "$NVM_DIR/nvm.sh"

    if command -v nvm >/dev/null 2>&1; then
      oldpwd=$PWD
      cd "$repo_root" || exit 1

      if [ -f "$repo_root/.nvmrc" ]; then
        nvm use --silent >/dev/null 2>&1 || true
      fi

      current_version=$(node_version)
      if ! version_ge "$current_version" "$required_version"; then
        required_major=${required_version%%.*}
        nvm use --silent "$required_major" >/dev/null 2>&1 || true
      fi

      cd "$oldpwd" || exit 1
    fi
  fi
fi

current_version=$(node_version)
if ! version_ge "$current_version" "$required_version"; then
  echo "husky - Node >=$required_version is required for hooks (current: $(node -v 2>/dev/null || echo 'none'))."
  echo "husky - Fix: install/switch the repo version (for example: 'nvm install $required_version && nvm use')."
  exit 1
fi
