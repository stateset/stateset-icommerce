# Releasing StateSet Artifacts

This guide covers publishing Rust crates and language bindings. It also captures the repo-wide checklist for docs and changelog updates.

## Release Checklist (All)

1. Update version numbers across the workspace and bindings.
2. Update `README.md` highlights and `CHANGELOG.md`.
3. Refresh mdBook docs in `docs/src/`.
4. Run the local release preflight: `npm run check:release`.
   This rebuilds the latest `docs/book/` output and verifies it does not drift from the live engine docs.
   It also verifies that the versioned docs snapshot flow builds a standalone
   mdBook with snapshot-specific versioning text.
   It also validates that GitHub workflow `needs:` edges only reference real jobs.
5. Push the release commit and require a green `CI Success` job from `.github/workflows/ci.yml` on that exact commit.
   `master` is branch-protected (since 2026-08-24): every non-Admin job in `ci.yml` is a required
   status check, admins included, and force-pushes/deletions are refused. The two `Admin *` lanes
   and the `CI Success` aggregate are deliberately NOT required until `@stateset/design` resolves in CI;
   re-add them with `gh api -X PUT repos/stateset/stateset-icommerce/branches/master/protection` when it does.
6. Create a versioned docs snapshot: `./docs/scripts/snapshot-version.sh vX.Y.Z`.
7. (Optional) Generate API docs into `docs/api/` with `./docs/scripts/generate-api.sh`.
8. Create annotated tags and push them.

## Authoritative Gates

- Local preflight: `npm run check:release`
- Remote release gate: the `CI Success` aggregate job in `.github/workflows/ci.yml` (branch protection currently enforces its constituent jobs individually; see step 5)
- Git hooks are convenience checks only; they do not replace the local preflight or the protected CI aggregate

## Tag Prefixes

- `vX.Y.Z`: publish Rust crates
- `cli-vX.Y.Z`: publish the npm CLI and embedded Node package
- `py-vX.Y.Z`: publish the Python package
- `java-vX.Y.Z`: publish the Java artifacts
- `ruby-vX.Y.Z`: publish Ruby gems
- `php-vX.Y.Z`: publish the PHP release artifacts

## Prerequisites

### Secrets Required

Add these secrets to your GitHub repository settings:

| Secret | Description |
|--------|-------------|
| `CARGO_REGISTRY_TOKEN` | crates.io API token with publish scope |
| `RUBYGEMS_API_KEY` | API key from https://rubygems.org/profile/api_keys |
| `PACKAGIST_USERNAME` | Your Packagist.org username |
| `PACKAGIST_API_TOKEN` | API token from https://packagist.org/profile |

### One-Time Setup

#### RubyGems

1. Create account at https://rubygems.org
2. Go to Profile → API Keys → Create New Key
3. Add the key as `RUBYGEMS_API_KEY` secret in GitHub

#### Packagist

1. Create account at https://packagist.org
2. Submit package at https://packagist.org/packages/submit
3. Enter: `https://github.com/stateset/stateset-icommerce`
4. Go to Profile → API Tokens → Generate Token
5. Add username and token as GitHub secrets

## Release Process

### Rust Crates Release (crates.io)

```bash
# 1) Run the authoritative local release preflight.
npm run check:release

# 2) Push the release commit and wait for the green CI Success job.

# 3) Validate publishability for all publishable crates.
bash ./scripts/publish-rust-crates.sh --dry-run

# 4) Publish all crates in dependency order.
export CARGO_REGISTRY_TOKEN=...
bash ./scripts/publish-rust-crates.sh --publish

# 5) Create and push the annotated release tag.
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

GitHub Actions workflow:

- `.github/workflows/publish-rust-crates.yml`
- Tag trigger: `v*`
- Manual trigger: `workflow_dispatch` with `version=X.Y.Z` plus `publish=true` to publish, or `publish=false` for dry-run only
- Script used by workflow: `scripts/publish-rust-crates.sh`
- Release preflight: `scripts/ci/check_release_hygiene.sh`
- Release preflight regression test: `scripts/ci/check_release_hygiene.test.sh`

### Ruby Release

```bash
# 1. Update version in these files:
#    - bindings/ruby/lib/stateset_embedded.rb (VERSION constant)
#    - bindings/ruby/stateset_embedded.gemspec (s.version)
#    - CHANGELOG.md (top released entry must match the workspace version)

# 2. Commit the changes
git add -A
git commit -m "chore: bump ruby gem to vX.Y.Z"

# 3. Create and push tag
git tag ruby-vX.Y.Z
git push origin ruby-vX.Y.Z

# GitHub Actions will automatically:
# - Build native gems for all platforms
# - Publish to RubyGems
# - Create GitHub Release with gem files
```

### PHP Release

```bash
# 1. Update version in these files:
#    - bindings/php/composer.json (version field)
#    - bindings/php/scripts/install-extension.php (VERSION constant)
#    - CHANGELOG.md (top released entry must match the workspace version)

# 2. Commit the changes
git add -A
git commit -m "chore: bump php package to vX.Y.Z"

# 3. Create and push tag
git tag php-vX.Y.Z
git push origin php-vX.Y.Z

# GitHub Actions will automatically:
# - Build native extensions for all platforms
# - Create GitHub Release with binary downloads
# - Notify Packagist to update
```

### Combined Release

For releasing both at once:

```bash
# Run the authoritative local release preflight
npm run check:release

# Push the release commit and wait for green CI Success

# Update all version numbers
git add -A
git commit -m "chore: release vX.Y.Z"

# Tag the canonical release plus binding aliases
git tag -a vX.Y.Z -m "vX.Y.Z"
git tag -a ruby-vX.Y.Z -m "ruby-vX.Y.Z"
git tag -a php-vX.Y.Z -m "php-vX.Y.Z"
git push origin vX.Y.Z ruby-vX.Y.Z php-vX.Y.Z
```

## Manual Publishing

### Ruby (Manual)

```bash
cd bindings/ruby

# Build source gem
gem build stateset_embedded.gemspec

# Build native gem (requires platform-specific setup)
bundle install
bundle exec rake native gem

# Publish
gem push stateset_embedded-X.Y.Z.gem

# Or publish pre-built native gems
gem push pkg/stateset_embedded-X.Y.Z-x86_64-linux.gem
gem push pkg/stateset_embedded-X.Y.Z-arm64-darwin.gem
```

### PHP (Manual)

PHP extensions are distributed as binaries via GitHub Releases. Packagist only hosts the stub package.

```bash
cd bindings/php

# Build extension
cargo build --release

# The composer package is auto-updated via GitHub webhook
# Users download binaries from GitHub Releases
```

## Cross-Compilation

### Ruby with rb-sys-dock

```bash
cd bindings/ruby

# Install rb-sys-dock
gem install rb_sys

# Build for specific platform
rb-sys-dock --platform x86_64-linux -r 3.2 -- bundle exec rake native gem
rb-sys-dock --platform aarch64-linux -r 3.2 -- bundle exec rake native gem
rb-sys-dock --platform x86_64-darwin -r 3.2 -- bundle exec rake native gem
rb-sys-dock --platform arm64-darwin -r 3.2 -- bundle exec rake native gem
```

### PHP Cross-Compilation

PHP extensions require the target platform's PHP headers, making cross-compilation complex. Use GitHub Actions matrix builds instead.

## Version Numbering

We use semantic versioning (SemVer):

- **MAJOR**: Breaking API changes
- **MINOR**: New features, backward compatible
- **PATCH**: Bug fixes, backward compatible

Keep versions in sync across:
- `Cargo.toml` (workspace version)
- `CHANGELOG.md` (entry for the release)
- `scripts/ci/check_release_hygiene.sh` (shared release preflight)
- Rust release automation: `scripts/publish-rust-crates.sh` and `.github/workflows/publish-rust-crates.yml`
- Ruby: `stateset_embedded.gemspec`, `lib/stateset_embedded.rb`
- PHP: `composer.json`, `scripts/install-extension.php`
 - Docs: `docs/versions/vX.Y.Z/` snapshot

## Troubleshooting

### RubyGems Push Fails

```bash
# Check credentials
gem signin

# Verify gem is valid
gem build stateset_embedded.gemspec
gem install stateset_embedded-*.gem --local
```

### Packagist Not Updating

1. Check webhook is configured in GitHub repo settings
2. Manually trigger update at https://packagist.org/packages/stateset/embedded
3. Verify composer.json is valid: `composer validate`

### Build Failures

```bash
# Check Rust compiles
cargo check -p stateset-ruby
cargo check -p stateset-php

# Verify syntax
cd bindings/ruby && rustfmt --check src/lib.rs
cd bindings/php && rustfmt --check src/lib.rs
```

## Platform Matrix

### Ruby

| Platform | Ruby Versions | Gem Suffix |
|----------|---------------|------------|
| Linux x86_64 | 3.0, 3.1, 3.2 | x86_64-linux |
| Linux arm64 | 3.2 | aarch64-linux |
| macOS x86_64 | 3.2 | x86_64-darwin |
| macOS arm64 | 3.2 | arm64-darwin |
| Windows | 3.2 | x64-mingw-ucrt |

### PHP

| Platform | PHP Versions | File Suffix |
|----------|--------------|-------------|
| Linux x86_64 | 8.1, 8.2, 8.3 | linux-x86_64-phpXX |
| macOS x86_64 | 8.2 | darwin-x86_64-php82 |
| macOS arm64 | 8.2, 8.3 | darwin-arm64-phpXX |
| Windows | 8.2, 8.3 | windows-x86_64-phpXX |
