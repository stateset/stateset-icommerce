# StateSet iCommerce v1.0.3 Release Notes

Released: 2026-05-04

## Summary

v1.0.3 is a patch release on the stable v1 line. It hardens CLI outbound network paths against SSRF-style failures, tightens remote marketplace package handling, and keeps BlueBubbles authentication compatible while moving token delivery out of URLs.

## Highlights

- Blocked private and loopback DNS resolution for outbound CLI webhook, marketplace, MPP, and x402 fetch flows.
- Added redirect validation so approved outbound requests cannot be bounced to private hosts.
- Hardened remote skill marketplace installs with package size limits, checksum enforcement, and archive path preflight.
- Changed BlueBubbles authentication to prefer the `x-api-key` header with legacy query-token fallback.
- Added regression coverage for URL validation, webhook retries, marketplace package limits, and iMessage auth fallback.

## Install

```bash
npm install @stateset/embedded@1.0.3
npm install -g @stateset/cli@1.0.3
pip install stateset-embedded==1.0.3
gem install stateset_embedded -v 1.0.3
```

## Compatibility

This release remains within the stable `v1.x` compatibility contract established by `v1.0.0`.
