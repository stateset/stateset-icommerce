# iCommerce / sequencer / Set integration gates

Status: integration work in progress, **not a 10/10 production certification**.

## Ownership

| System | Responsibility |
| --- | --- |
| iCommerce | Quote commitment, authority, budgets, commerce state, durable purchase/submission recovery |
| stateset-sequencer | Authenticated intent admission, stable identity, ordering, batching and settlement dispatch |
| Set | Payer authorization enforcement, token transfer, settlement events and chain state |

The inspected sequencer checkout is `/home/dom/icommerce-app/stateset-sequencer`,
not `/home/dom/icommerce/stateset-sequencer` (which was absent). The Set checkout
is `/home/dom/icommerce-app/set`. Both have unrelated ongoing work. The coordinated
identity changes below modify the sequencer only; Set contracts are unchanged.

## Coordinated identity protocol

The sequencer's `src/settlement.rs::uuid_to_bytes32` places a UUID in the first
16 bytes and zero-pads the remaining 16 bytes. Its `build_payment` uses this as
the on-chain intent identity.

Previously, `submit_payment_intent` created that UUID **after receiving the payer
authorization**, making correct pre-signing impossible. The coordinated sequencer
change accepts a non-nil `intent_id` in `SubmitX402PaymentRequest` and requires it
when `eip712_authorization` is supplied. Legacy requests without on-chain
authorization can still receive a server-generated UUID.

The x402 router now exposes `GET /capabilities`, advertising
`x402.client_intent_id.v1` and encoding `uuid-prefix-zero-pad-bytes32`.
Authenticated same-idempotency-key submissions are serialized inside the database
transaction and checked against immutable identity, agent, payment, authorization
and metadata fields. Conflicts are rejected, and lookup failures propagate instead
of being treated as absence. Expired requests still require read-only lookup, not
POST replay. Database concurrency behavior needs a PostgreSQL integration run.

iCommerce preserves `intentEncoding: 'sha256-v1'` as its default. Explicit
`intentEncoding: 'sequencer-uuid-v1'` derives a deterministic UUID v8 and applies
the sequencer's padding. This profile requires an operator-owned
`getSequencerCapabilities()` callback against the submission server, and refuses
dispatch unless both capability and encoding match. Read-only reconciliation
does not depend on capability availability. Never change profiles for unresolved
operations: it changes the signed identity.

The host's submission gateway must convert the first 16 bytes of `intentId` to
the standard hyphenated UUID for the request's `intent_id`, preserve that identity
when signing, and submit the authenticated sequencer envelope. This is not yet a
complete built-in HTTP gateway. The sequencer advertises authorization validation
as **on-chain**; admission only shape-checks the EIP-712 blob. Capability support
does not certify signature validity or settlement readiness.

Remaining gates before claiming end-to-end interoperability:

1. Verify payer authorization before admitting executable work, with explicit
   EOA/ERC-1271 handling and an operator-owned settlement domain.
2. Exercise concurrent authenticated admission and conflicting identity/nonce
   requests against PostgreSQL, including response loss.
3. Complete the operator submission gateway and test old-server refusal over HTTP.
4. Exercise submission, batch settlement, lost responses and receipt matching
   across all three real components, not only independent mocks.

The reviewed source heads were sequencer `3fe41ce5bf85a77bdc3e783c1aa001300224ce42`
and Set `c41bc18724ee4152eb7e03d8a2687e14a763dd4b`; these were dirty checkouts,
so the head identifiers alone are not an attestation of their working trees.

## Concrete transaction verification now available

Source module `cli/src/x402/set-transaction.js` supplies:

- `sequencerUuidToBytes32`: exact Rust-compatible encoding, with explicit input validation.
- `buildSetPaymentAuthorization`: the Set EIP-712 domain and typed fields,
  including `validUntil` mapped to the signed `validBefore` field.
- `buildSetBatchCalldata`: validates the EOA payer signature and encodes one
  payment in the exact Set `settleBatch` ABI tuple order.
- `createSetTransactionVerifier`: a concrete `validateSigned` callback for the
  durable journal. Decodes the signed EIP-1559 transaction, recovers the relayer,
  validates authorization/calldata, chain, recipient, zero native value, both
  nonces, exact planned gas/fees and independent operator gas ceilings. Rejects
  access lists and returns the actual transaction hash.

The module uses the CLI's existing ethers dependency; it is not a new native
binding entrypoint or a model-facing signing tool. It neither signs nor broadcasts.
See [ethers typed-data verification](https://docs.ethers.org/v6/api/hashing/).

Example source-tree wiring:

```javascript
const validateSigned = createSetTransactionVerifier({
  relayer: operatorRelayerAddress,
  maxGasLimit: '500000',
  maxFeePerGas: operatorMaxFeePerGas, // exact base-unit string, not a recommendation
  maxPriorityFeePerGas: operatorMaxPriorityFeePerGas,
});
// Supply validateSigned to createDurableSetSubmission.
// prepare() persists { batch, transaction: { nonce, gasLimit, maxFeePerGas,
// maxPriorityFeePerGas } }; sign() returns signed raw bytes without broadcasting.
```

This initial codec supports **one payment per batch, EOA payer authorization,
and type-2 transactions only**. ERC-1271 payers require chain verification. Batch
root/tenant commitments still come from trusted preparation; this codec does not
prove Merkle inclusion or attest deployed proxy implementations. Relayer nonce
allocation, authorized-sequencer deployment configuration and live RPC finality
remain operational integrations.

## Evidence and remaining gates

```bash
node --test cli/test/unit/set-transaction.test.js
```

Tests use real local EIP-712 signatures and signed EIP-1559 transactions with
public deterministic test keys, including journal recovery of identical signed
bytes. They reject substituted authorization fields, recipients, chains, nonces,
native value, calldata, access lists and gas changes. Broadcast remains a test
double. These tests do not establish deployed-contract compatibility by themselves.

Production readiness still requires the identity handshake above, a native order
lifecycle connected to real settlement, fulfillment/refund/dispute reconciliation,
multi-process failure drills, live PostgreSQL parity, backup restoration, pinned
deployment configuration and independent security review.
