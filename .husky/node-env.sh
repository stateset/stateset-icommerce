#!/usr/bin/env sh
#
# Ensure Node >=18 for repo tooling (ESLint v9, commitlint v19).
# Git hooks run in a non-interactive shell, so nvm isn't always loaded.
#

need_major=18

node_major() {
  node -p "Number(process.versions.node.split('.')[0])" 2>/dev/null || echo 0
}

hook_dir=$(cd "$(dirname "$0")" && pwd)
repo_root=$(cd "$hook_dir/.." && pwd)

major=$(node_major)
if [ "$major" -lt "$need_major" ]; then
  # Try nvm if installed.
  export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
  if [ -s "$NVM_DIR/nvm.sh" ]; then
    # shellcheck disable=SC1090
    . "$NVM_DIR/nvm.sh"

    if command -v nvm >/dev/null 2>&1; then
      oldpwd=$PWD
      cd "$repo_root" || exit 1

      # Prefer repo pinned version, otherwise fall back to a recent LTS line.
      if [ -f "$repo_root/.nvmrc" ]; then
        nvm use --silent >/dev/null 2>&1 || true
      fi

      major=$(node_major)
      if [ "$major" -lt "$need_major" ]; then
        nvm use --silent 20 >/dev/null 2>&1 || true
      fi

      cd "$oldpwd" || exit 1
    fi
  fi
fi

major=$(node_major)
if [ "$major" -lt "$need_major" ]; then
  echo "husky - Node >=$need_major is required for hooks (current: $(node -v 2>/dev/null || echo 'none'))."
  echo "husky - Fix: install/switch Node (e.g. 'nvm use') and retry."
  exit 1
fi

