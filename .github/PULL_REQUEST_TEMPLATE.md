<!--
  Thanks for contributing to StateSet iCommerce!
  Please fill out the sections below. Delete any that don't apply.
  See CONTRIBUTING.md for the full development and review workflow.
-->

## Summary

<!-- What does this PR do, and why? Link the motivating issue/discussion. -->

Closes #

## Affected surface

<!-- Tick everything this PR touches. -->

- [ ] Rust crates (`crates/`)
- [ ] Language bindings (`bindings/`)
- [ ] CLI / MCP server (`cli/`)
- [ ] Admin dashboard (`admin/`)
- [ ] Scaffolding / templates (`packages/`)
- [ ] ICP protocol (`crates/stateset-icp-*`, `icp-spec/`)
- [ ] Documentation only
- [ ] Build / CI / tooling

## Type of change

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that changes existing behavior or public API)
- [ ] Documentation
- [ ] Refactor / chore (no functional change)

## Checklist

<!-- All items should be checked before requesting review. -->

- [ ] My commits follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`) — enforced by commitlint.
- [ ] I added or updated **tests** that cover my change (success and error cases).
- [ ] Rust checks pass locally: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test`.
- [ ] Node/CLI checks pass locally where relevant (`npm --prefix cli run check`) and admin tests pass (`npm run check:admin`).
- [ ] I updated the `## [Unreleased]` section of [`CHANGELOG.md`](../CHANGELOG.md) (skip only for docs/chore-only PRs that the maintainers exempt).
- [ ] If I changed MCP tools, agents, bindings, the HTTP gateway, or workspace surface, I regenerated the affected inventories (`npm run generate:*`) and the `--check` variants pass (`npm run check:mcp-inventory`, `check:agent-inventory`, `check:binding-api-inventory`, `check:http-gateway-inventory`, `check:rust-openapi-inventory`, `check:workspace-inventory`).
- [ ] I updated relevant **documentation** (`README.md`, `docs/`, crate/binding READMEs).
- [ ] For SQL migrations: numbered sequentially, tested against SQLite and PostgreSQL where applicable.
- [ ] This change does **not** weaken a security control or lower a coverage/lint gate without explicit justification below.

## How was this tested?

<!-- Commands run, manual verification steps, platforms/backends covered. -->

## Notes for reviewers

<!-- Anything that warrants extra attention: trade-offs, follow-ups, out-of-scope items. -->
