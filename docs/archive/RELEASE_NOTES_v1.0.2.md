# StateSet iCommerce v1.0.2 Release Notes

Released: 2026-05-01

## Summary

v1.0.2 is a patch release on the stable v1 line. It tightens admin rate-limit trust boundaries, fixes Agent OS version reporting, and hardens generated runbook skill metadata without changing the v1 compatibility contract.

## Highlights

- Hardened admin rate limiting so spoofable `x-forwarded-for` and `x-real-ip` headers are ignored unless trusted proxy mode is explicitly enabled.
- Added `STATESET_ADMIN_TRUST_PROXY_HEADERS` documentation for deployments that terminate traffic behind a controlled proxy boundary.
- Synced Agent OS status output to the package version instead of a hardcoded stale version.
- Escaped generated runbook `SKILL.md` frontmatter so multiline descriptions cannot corrupt skill metadata.
- Added regression coverage for proxy-header trust behavior, Agent OS version sync, and runbook frontmatter safety.

## Install

```bash
npm install @stateset/embedded@1.0.2
npm install -g @stateset/cli@1.0.2
pip install stateset-embedded==1.0.2
gem install stateset_embedded -v 1.0.2
```

## Compatibility

This release remains within the stable `v1.x` compatibility contract established by `v1.0.0`.
