# stateset-a2a

[![crates.io](https://img.shields.io/crates/v/stateset-a2a.svg)](https://crates.io/crates/stateset-a2a)
[![docs.rs](https://docs.rs/stateset-a2a/badge.svg)](https://docs.rs/stateset-a2a)

Agent-to-Agent commerce primitives: multi-party split payments, recurring
subscriptions, conditional escrow, agent credit terms, signed webhooks, dispute
resolution, reputation scoring, circuit breakers, SLAs, and marketplace RFQs.

When agents transact with each other, the hard parts are the ones a human would
normally absorb: splitting a payment three ways without losing a cent to rounding,
holding funds until a condition is met, deciding whether a counterparty is
trustworthy, and cutting off a runaway spender. This crate is the business logic for
those, with no I/O of its own.

## Modules

| Module | Purpose |
|--------|---------|
| `splits` | Multi-party payment splitting with rounding-drift prevention |
| `subscriptions` | Recurring billing with trial periods and a validated state machine |
| `escrow` | Conditional fund holding with four condition types |
| `credit` | Agent credit terms (Net15/30/60/90), balance tracking, overdue detection |
| `notifications` | HMAC-SHA256 webhook signing and SSRF URL validation |
| `events` | Event-type filtering with wildcard and prefix matching |
| `disputes` | Dispute resolution with evidence hashing and deadline management |
| `reputation` | Trust scoring with dimension-based evaluation and tier promotion |
| `circuit_breaker` | Spending limits and failure-rate tracking |
| `sla` | Service level agreements with compliance checks and penalty calculation |
| `marketplace` | Multi-party RFQ with scoring and response ranking |
| `agent_cards` | Agent card validation and discovery filtering |

## Usage

Split a payment without losing money to rounding:

```rust
use rust_decimal_macros::dec;
use stateset_a2a::splits::{Recipient, calculate_percentage_split};

let recipients = vec![
    Recipient { address: "0xAlice".into(), percent: Some(dec!(50)), amount: None },
    Recipient { address: "0xBob".into(), percent: Some(dec!(30)), amount: None },
    Recipient { address: "0xCharlie".into(), percent: Some(dec!(20)), amount: None },
];

let result = calculate_percentage_split(dec!(100), dec!(2.5), &recipients).unwrap();

// Every cent is accounted for — drift is absorbed, not dropped
assert_eq!(result.total_distributed, dec!(100));
```

Sign and verify a webhook:

```rust
use stateset_a2a::notifications::{sign_webhook, verify_webhook};

let secret = b"whsec_abc123";
let payload = br#"{"event":"payment.completed"}"#;

let signature = sign_webhook(secret, payload);
assert!(verify_webhook(secret, payload, &signature));

// A tampered payload fails verification
assert!(!verify_webhook(secret, br#"{"event":"payment.refunded"}"#, &signature));
```

## Security Notes

- Webhook signatures are HMAC-SHA256 and verified in constant time.
- Outbound webhook URLs are validated against SSRF — loopback, link-local, and
  private ranges are rejected before a request is made.
- Escrow release and dispute deadlines are computed from explicit timestamps you
  pass in; this crate never reads the clock for you.

## Part of StateSet iCommerce

Consumed by [`stateset-http`](https://crates.io/crates/stateset-http) to expose the
A2A surface over REST. Part of the
[StateSet iCommerce](https://github.com/stateset/stateset-icommerce) engine.

## License

MIT OR Apache-2.0
