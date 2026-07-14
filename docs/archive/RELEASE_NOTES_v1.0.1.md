# StateSet iCommerce v1.0.1 Release Notes

Released: 2026-04-30

## Summary

v1.0.1 is a patch release on the stable v1 line. It adds the local Agent OS surface, refreshes generated release inventories, and tightens dependency-policy hygiene without changing the v1 compatibility contract.

## Highlights

- Added `stateset-agent` plus `stateset agent` commands for setup, status, context, skills, sessions, memory, and runbook creation.
- Regenerated workspace inventory for the expanded CLI surface.
- Cleaned cargo-deny policy warnings by removing stale OpenSSL/license exceptions and pinning known duplicate dependency skips.
- Documented the temporary `RUSTSEC-2026-0097` audit ignore in CI with runtime rationale.

## Install

```bash
npm install @stateset/embedded@1.0.1
npm install -g @stateset/cli@1.0.1
pip install stateset-embedded==1.0.1
gem install stateset_embedded -v 1.0.1
```

## Compatibility

This release remains within the stable `v1.x` compatibility contract established by `v1.0.0`.
