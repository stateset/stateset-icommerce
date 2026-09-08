# Releasing StateSet Artifacts

There is one release procedure. It cuts the three tags that carry the product —
crates.io, npm, and PyPI — from a single green commit on `master`, in one
command.

```bash
npm run release:tag 1.34.0
```

Everything below explains what that command refuses to do, what it triggers,
and what to do when a lane is genuinely broken.

---

## Before you tag

1. Bump versions across the workspace and bindings (`scripts/release-bump.sh`).
2. Update `CHANGELOG.md`.
3. Refresh anything in `docs/src/` the release changes.
4. Run the local preflight: `npm run check:release`.
5. Merge to `master` and wait for CI to go green on that exact commit.

`npm run check:release` is the authoritative *local* gate. It extends
`npm run check` with doc-tool validation, the mdBook build, and the generated
inventory checks. It is not a substitute for CI: only CI runs the full binding,
admin, and Postgres matrices.

---

## Cutting the release

```bash
npm run release:tag 1.34.0 -- --dry-run   # print what would be pushed
npm run release:tag 1.34.0                # push v, cli-v and py-v together
```

`scripts/release-tag.sh` refuses to tag unless **HEAD is a commit on
`origin/master` whose CI run is green**. That single precondition removes the
release failure this repo has hit repeatedly: three tags cut by hand, one of
them forgotten, and two registries left serving the previous version for weeks.

When it accepts, it pushes all three tags atomically:

| Tag           | Publishes                            | Workflow                    |
| ------------- | ------------------------------------ | --------------------------- |
| `vX.Y.Z`      | Rust crates to crates.io             | `publish-rust-crates.yml`   |
| `cli-vX.Y.Z`  | `@stateset/cli` + `@stateset/embedded` to npm | `publish-cli.yml`  |
| `py-vX.Y.Z`   | `stateset-embedded` to PyPI          | `publish-python.yml`        |

Do not push these tags by hand. A tag pushed outside the script is the one case
the guards below exist to catch.

### The publish guard

Every publish workflow runs a **publish-guard** job first, and the publishing
jobs `needs:` it. The guard refuses a tag that is

- not reachable from `master`, or
- pointing at a commit whose CI is not green.

A guard failure means the tag is wrong, not that the guard is wrong. Delete the
tag, fix the commit, and cut again.

For a genuine emergency — a registry that rejected an otherwise-good publish, a
re-run after an infrastructure outage — each publish workflow's
`workflow_dispatch` accepts a **`skip_guards`** input. Using it is a deliberate,
logged decision by a maintainer; it is never the way to get a normal release
out.

### The daily consistency check

`.github/workflows/release-consistency.yml` runs daily. It compares the
workspace version against what crates.io, npm, and PyPI actually serve, and
fails when they disagree. This is the backstop that catches a publish that
silently did not happen — the failure mode that left npm and PyPI on 1.30.0
through two releases.

If it fires, re-run the failing lane's publish workflow via
`workflow_dispatch` with the version input; do not cut a new version to paper
over a missing publish.

---

## Branch protection

`master` is branch-protected. The required checks, who may bypass them, and how
to re-apply the protection after changing the CI job graph are documented in
[`scripts/ci/README-branch-protection.md`](./scripts/ci/README-branch-protection.md).

The remote release gate is CI on `master`; local git hooks are convenience
checks only and do not gate anything.

---

## Release lanes and their support level

| Lane | Covers | Gate |
| ---- | ------ | ---- |
| Tier 1 | Default-workspace Rust crates, Node binding, admin app, CLI | `npm run check` locally; required CI jobs on `master` |
| Tier 2 | Python, Go, .NET, Java/Kotlin, Swift and WASM bindings | Dedicated CI jobs, required before tagging |
| Tier 3 | Ruby native gem and PHP extension distribution | Representative CI jobs plus release-workflow validation |

Ruby and PHP are kept in-repo but excluded from default workspace membership:
they need host runtimes or headers that are frequently missing in local dev.
They are still exercised in dedicated CI lanes.

**Currently published:** crates.io, npm, and PyPI serve the workspace version.
The Ruby gem (`stateset_embedded`) is stale on RubyGems and the Java artifacts
have never been published to Maven Central; their tag prefixes below are wired
but not part of the routine release.

### Tag prefixes

| Prefix         | Publishes                 | In the routine release? |
| -------------- | ------------------------- | ----------------------- |
| `vX.Y.Z`       | Rust crates               | yes                     |
| `cli-vX.Y.Z`   | npm CLI + embedded binding| yes                     |
| `py-vX.Y.Z`    | Python package            | yes                     |
| `ruby-vX.Y.Z`  | Ruby gems                 | no — see below          |
| `php-vX.Y.Z`   | PHP release artifacts     | no — see below          |
| `java-vX.Y.Z`  | Java artifacts            | no — see below          |

---

## Secondary channels

These lanes exist and their workflows work, but they are not cut on every
release. Publishing one is a deliberate act: bump the binding's own version
files, get a green `master`, then push its tag.

### Secrets

| Secret | Used by |
|--------|---------|
| `CARGO_REGISTRY_TOKEN` | crates.io publish |
| `RUBYGEMS_API_KEY` | Ruby gem publish |
| `PACKAGIST_USERNAME` / `PACKAGIST_API_TOKEN` | PHP / Packagist |

### Ruby

Version lives in `bindings/ruby/lib/stateset_embedded.rb` (VERSION) and
`bindings/ruby/stateset_embedded.gemspec`. Tag `ruby-vX.Y.Z`; the workflow
builds native gems per platform and pushes to RubyGems.

Manual fallback:

```bash
cd bindings/ruby
gem build stateset_embedded.gemspec
bundle exec rake native gem          # native gem; needs platform setup
gem push stateset_embedded-X.Y.Z.gem
```

Cross-compiling with `rb-sys-dock`:

```bash
gem install rb_sys
rb-sys-dock --platform x86_64-linux -r 3.2 -- bundle exec rake native gem
rb-sys-dock --platform aarch64-linux -r 3.2 -- bundle exec rake native gem
rb-sys-dock --platform arm64-darwin -r 3.2 -- bundle exec rake native gem
```

| Platform | Ruby Versions | Gem Suffix |
|----------|---------------|------------|
| Linux x86_64 | 3.0, 3.1, 3.2 | x86_64-linux |
| Linux arm64 | 3.2 | aarch64-linux |
| macOS x86_64 | 3.2 | x86_64-darwin |
| macOS arm64 | 3.2 | arm64-darwin |
| Windows | 3.2 | x64-mingw-ucrt |

### PHP

Version lives in `bindings/php/composer.json` and
`bindings/php/scripts/install-extension.php`. Tag `php-vX.Y.Z`. Packagist hosts
only the stub package; the extension binaries are attached to the GitHub
Release. PHP extensions need the target platform's PHP headers, so build them
in the CI matrix rather than cross-compiling locally.

| Platform | PHP Versions | File Suffix |
|----------|--------------|-------------|
| Linux x86_64 | 8.1, 8.2, 8.3 | linux-x86_64-phpXX |
| macOS x86_64 | 8.2 | darwin-x86_64-php82 |
| macOS arm64 | 8.2, 8.3 | darwin-arm64-phpXX |
| Windows | 8.2, 8.3 | windows-x86_64-phpXX |

---

## Version numbering

Semantic versioning. Keep these in sync — `scripts/ci/check_version_sync.sh`
(run by `npm run check:versions`) enforces most of it:

- `Cargo.toml` workspace version
- `CHANGELOG.md` top released entry
- `scripts/publish-rust-crates.sh` and `.github/workflows/publish-rust-crates.yml`
- Ruby: `stateset_embedded.gemspec`, `lib/stateset_embedded.rb`
- PHP: `composer.json`, `scripts/install-extension.php`
- The pinned install snippets in `README.md` and `docs/src/`

---

## Troubleshooting

**A publish workflow's guard failed.** The tag is not on `master`, or its
commit is not green. Delete the tag and cut again from a good commit.

**`release-consistency` says a registry is behind.** Re-run that lane's publish
workflow with `workflow_dispatch` and the version input. Check the original
run's logs first — a registry rejection and a workflow that never started look
the same from the outside.

**`npm ci` fails on every branch right after a release.** `package.json` names
the new version while the lockfiles still pin the old platform packages. Merge
the `automation/sync-locks-vX.Y.Z` pull request first, then merge `master` into
your branch. Its workflow runs sit in `action_required` because it is a bot
branch and need approving by run id.

**RubyGems push fails.** `gem signin`, then verify the gem builds and installs
locally: `gem build stateset_embedded.gemspec && gem install stateset_embedded-*.gem --local`.

**Packagist not updating.** Check the GitHub webhook, trigger an update
manually at packagist.org, and validate with `composer validate`.

**A binding does not compile.** `cargo check -p stateset-ruby`,
`cargo check -p stateset-php`, and `rustfmt --check src/lib.rs` in the binding
directory.
