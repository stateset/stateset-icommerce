# Vector 07 — Settlement Receipts (§9)

**Spec sections covered:** ICP-1.0 §9 (Settlement: the co-signed
SettlementReceipt), `schemas/error-codes.md` (`format.missing_field`,
`signature.invalid`, `settlement.settler_signature_invalid`).

§9 calls the SettlementReceipt "the canonical proof of payment" and requires
it to be signed jointly by the Settler and the receiving party. This family
checks that implementations agree on whether a given receipt verifies, and on
which error a bad one produces. Its source of truth is recorded in
`expected.json` under `_provenance`: §9 plus the reference
`verifySettlementReceipt` in `packages/icp-client`, with the settler signature
required.

## What a case contains

Each case supplies a `receipt` and two raw 32-byte Ed25519 public keys in hex
(`merchant_pubkey_hex`, `settler_pubkey_hex`). The receipt carries the §9
fields — `escrow_id`, `final_state`, `amount`, `currency`, `rail`, `rail_txid`,
`settled_at` — and two signature objects, `merchant_signature` and
`settler_signature`, each `{ "alg": "ed25519", "kid", "sig" }` with `sig` a
64-byte signature in hex.

Both signatures are over the same bytes: the RFC 8785 canonical JSON of the
receipt with `merchant_signature` and `settler_signature` removed. The
signatures in this vector are real Ed25519 signatures produced from fixed
seeds (merchant `0x11` × 32, settler `0x22` × 32), so the inputs are
deterministic.

## Checks and precedence

Applied in this order; the **first** failure wins:

1. `merchant_signature` or `settler_signature` absent → `format.missing_field`
2. merchant signature does not verify under `merchant_pubkey_hex` →
   `signature.invalid`
3. settler signature does not verify under `settler_pubkey_hex` →
   `settlement.settler_signature_invalid`
4. otherwise → valid

## Cases

| Case | What differs from the valid receipt | Expected |
|---|---|---|
| `r01_valid_released` | nothing; `final_state: "released"` | valid |
| `r08_valid_refunded` | `final_state: "refunded"`, signed afresh | valid |
| `r02_missing_merchant_sig` | `merchant_signature` omitted | `format.missing_field` |
| `r03_missing_settler_sig` | `settler_signature` omitted | `format.missing_field` |
| `r04_tampered_merchant_sig` | first byte of the merchant `sig` altered | `signature.invalid` |
| `r05_tampered_settler_sig` | first byte of the settler `sig` altered | `settlement.settler_signature_invalid` |
| `r06_tampered_amount` | `amount` changed from `125.00` to `1.00`, signatures left as in `r01` | `signature.invalid` |
| `r07_wrong_merchant_pubkey` | receipt as in `r01`, verified against a different merchant public key | `signature.invalid` |

`r06` is the precedence case: changing a signed field invalidates **both**
signatures, and the expected result is the merchant-signature error, not the
settler one.

## Not covered

- No case varies `alg` or `kid`; every signature object uses `ed25519` and
  the keys are supplied directly rather than resolved from `kid`.
- No case has a wrong settler public key, a malformed (wrong-length or
  non-hex) signature, or a missing §9 body field such as `rail_txid`.
- In both valid cases, including `refunded`, the receiving party's signature
  is carried in `merchant_signature`. The vector does not exercise a receipt
  counter-signed by a buyer.
- The §11.2 rule that a receipt is bound to its rail is not exercised; `rail`
  is only covered as one of the signed fields.

## Adapter contract

stdin: `inputs.json`
(`{ "cases": [{ "id", "receipt", "merchant_pubkey_hex", "settler_pubkey_hex" }, ...] }`).
stdout: `{ "verifications": { "r01_valid_released": {"valid": true},
"r02_missing_merchant_sig": {"error": "format.missing_field"}, ... } }` — one
entry per case id, either `{"valid": true}` or `{"error": "<code>"}`.
