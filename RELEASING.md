# Releasing StateSet Artifacts

This guide covers publishing Rust crates and language bindings. It also captures the repo-wide checklist for docs and changelog updates.

## Release Checklist (All)

1. Update version numbers across the workspace and bindings.
2. Update `README.md` highlights and `CHANGELOG.md`.
3. Refresh mdBook docs in `docs/src/`.
4. Create a versioned docs snapshot: `./docs/scripts/snapshot-version.sh vX.Y.Z`.
5. (Optional) Generate API docs into `docs/api/` with `./docs/scripts/generate-api.sh`.
6. Tag the release and push tags.

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
# 1) Ensure workspace version, changelog, and README are in sync.
bash ./scripts/ci/check_version_sync.sh

# 2) Validate publishability for all publishable crates.
bash ./scripts/publish-rust-crates.sh --dry-run

# 3) Publish all crates in dependency order.
export CARGO_REGISTRY_TOKEN=...
bash ./scripts/publish-rust-crates.sh --publish

# 4) Create and push release tag.
git tag vX.Y.Z
git push origin vX.Y.Z
```

GitHub Actions workflow:

- `.github/workflows/publish-rust-crates.yml`
- Tag trigger: `v*`
- Manual trigger: `workflow_dispatch` with `publish=true` to publish, or `publish=false` for dry-run only
- Script used by workflow: `scripts/publish-rust-crates.sh`

### Ruby Release

```bash
# 1. Update version in these files:
#    - bindings/ruby/lib/stateset_embedded.rb (VERSION constant)
#    - bindings/ruby/stateset_embedded.gemspec (s.version)

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
# Update all version numbers
git add -A
git commit -m "chore: release vX.Y.Z"

# Tag both
git tag ruby-vX.Y.Z
git tag php-vX.Y.Z
git push origin ruby-vX.Y.Z php-vX.Y.Z
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
