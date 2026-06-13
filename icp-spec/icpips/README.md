# ICPIPs — Intelligent Commerce Protocol Improvement Proposals

ICPIPs are the only mechanism for normative changes to the protocol after
ICP-1.0 ratification. See `governance/ICPIP-process.md` for the lifecycle
spec.

## Index

| # | Title | Track | Status |
|---|---|---|---|
| [0001](./icpip-0001-process.md) | ICPIP-0001 Process | Meta | Draft |
| [0002](./icpip-0002-hybrid-pqc-mandate.md) | Hybrid Ed25519 + ML-DSA-65 signature mandate for high-value Intents | Standards (Core) | Draft |
| [0003](./icpip-0003-quote-request.md) | `quote.request` verb specification (B2B wholesale RFQ) | Standards (Core) | Draft |
| [0004](./icpip-0004-payout-request.md) | `payout.request` verb specification (marketplace payouts) | Standards (Core) | Draft |
| [0005](./icpip-0005-push-channels.md) | Push channels — webhooks + Server-Sent Events for merchant→Agent push delivery | Standards (Core) | Draft |
| [0006](./icpip-0006-idempotency-pagination.md) | Idempotency keys (`purchase.create`) and cursor pagination (`inventory.query`) | Standards (Core) | Draft |
| 0007 | Hybrid X25519 + ML-KEM-768 mandate for confidential PrincipalBinding transport | Standards (Core) | Solicited (companion to 0002) |

## Template

A new ICPIP starts as a copy of `icpip-template.md` (forthcoming) named
`icpip-<NNNN>-<short-name>.md` where NNNN is the next sequence number.
Drafts can be opened by anyone — see `ICPIP-process.md` §"Lifecycle"
for the path from Draft → Final.

## Mailing list

Discussion happens publicly on `https://github.com/stateset/icp-spec/discussions`
(forthcoming) and on the ICP Foundation mailing list. Editors will quote
substantive feedback in the ICPIP comments section.
