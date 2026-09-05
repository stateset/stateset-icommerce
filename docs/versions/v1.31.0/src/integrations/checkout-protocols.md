# Checkout protocol integration status

StateSet iCommerce exposes protocol-neutral cart, checkout, order, inventory,
payment, and audit primitives. Those APIs are suitable adapter targets, but
their presence does not by itself establish ACP or UCP conformance.

| Surface | Status | Production guidance |
| --- | --- | --- |
| Embedded cart/checkout API | Implemented and tested | Use directly inside trusted applications |
| ACP wire adapter | Not released or evidenced by this repository | Treat as unavailable until a versioned artifact and official conformance results are published |
| UCP wire adapter | Not implemented in this repository | Treat as a planned integration; do not advertise conformance |
| External settlement | Engine primitive available | Verify rail-specific receipts and enforce replay protection in the adapter |

As of 2026-08-27, the separately named `stateset-acp-handler` repository has no
committed implementation or released artifact, so it is not evidence of ACP
support. An adapter is release-ready only when it has version-pinned schemas, request
authentication, exact-money mappings, idempotency and replay tests, negative
conformance fixtures, and end-to-end evidence against the target protocol's
official test suite. Marketing and generated docs should name the adapter and
tested protocol version rather than calling the embedded cart model “ACP.”
