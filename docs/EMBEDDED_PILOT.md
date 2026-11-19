# Embedded operations pilot

A developer-led team can keep its current commerce platform and evaluate StateSet for a governed order cancellation or customer refund. Enroll one independent merchant team, record its shortlist before it decides, and record its decision even if it chooses another platform or stops evaluating. Obtain explicit permission for a production evidence review. A sandbox demonstration can complete the technical evaluation. Activation requires a real customer operation completed on the existing platform after enrollment.

From the standalone pilot shop, run `npm run verify:embedded-refund` to try a governed refund with the installed SDK. It creates a disposable SQLite store and verifies the receipt across two processes. The fixture demonstrates the integration before the merchant chooses a production workflow. [Developer pilot](./DEVELOPER_PILOT.md)

## Enroll the team privately

Use the private evaluation ledger described in the [pilot guide](./DEVELOPER_PILOT.md). Give this team a stable anonymous `id`, `track` of `embedded-operations`, and `workflow` of `order-cancellation` or `customer-refund`. Record the opt-in time as `enrolledAt` in UTC ISO format including milliseconds so same-day completions can be checked against the start of the evaluation. Keep the original `enrolledOn`, `source`, `alternatives` and optional `decision` fields. Enter organizations working through the same independent team only once. Keep this primary track even if the team also experiments with the storefront.

A merchant who wants activation reviewed creates an Ed25519 key on its own machine. The maintainer keeps a separate signing key. Each person can use the following command with their own unused private path. It prints the public key and creates the secret file with owner-only access.

```bash
node scripts/pilot-scorecard-operations.mjs keygen --key /private/pilot/reviewer.pem --apply
```

Confirm the merchant’s public key with its known representative through an established independent channel and pin it as `merchantPublicKey` in the evaluation row before evidence arrives. The pilot operator separately verifies the maintainer’s identity and pins the maintainer public key in a private authority file. A key included in a submitted report cannot establish either identity. Keep private keys with their individual owners.

Once per private pilot ledger, the operator creates a random 32-byte HMAC matching key and keeps it stable across teams and reporting periods. Its SHA-256 hex digest is `matchingKeyId`. The private authority JSON has exactly the two configuration values `maintainerPublicKey` and `matchingKeyId`. The matching key lets the operator detect reuse of a receipt or external operation without retaining transaction identifiers in the ledger.

```bash
umask 077
(set -C; openssl rand 32 > /private/pilot/matching-key.bin)
openssl dgst -sha256 /private/pilot/matching-key.bin
```

## Review one production completion

1. The merchant shows the completed operation in its live commerce system. The maintainer inspects the corresponding successful StateSet receipt from the merchant’s production store and the external system’s completed operation through authorized access or a live walkthrough. Both people verify that the action, merchant account and underlying order or reimbursement match. A queued refund, preview, demo or sandbox record cannot establish activation.
2. The operator prepares the redacted summary locally. Pass a private JSON object through standard input, directly from the authorized records or a private temporary file. `receipt` is the unmodified governed execution receipt. Copy the row’s original `enrolledAt` into the private input. Set `productionStoreId` to the independently established live store identity. Set `receiptEnvironment` to `production` and `provenance` to `live-merchant-operation` only after inspecting the original sources. `external` supplies `environment` of `production`, a stable `system` label, the stable `account` and `operationId` from that system, the timezone-qualified `completedAt` timestamp, and an `outcome` of `order-cancelled` or `refund-completed`.
3. Both people inspect the same redacted summary alongside the records. The merchant signs to confirm its own live operation. The maintainer signs independently to confirm matching the StateSet receipt to the external completion. Each runs the signing command with their own role and key. The signature covers the role, review date, pilot, team, workflow, dates and both opaque evidence tags.
4. The operator keeps the two signed JSON files outside the repository. Set the team's `operationReview.merchant` and `operationReview.maintainer` to their file paths relative to the ledger. Retain the original files for historical reporting. Each signature counts only on or after its review date. If one party declines or has yet to review, omit that file path.

```bash
node scripts/pilot-scorecard-operations.mjs prepare --team "$TEAM" --workflow "$WORKFLOW" \
  --matching-key /private/pilot/matching-key.bin < /private/operator/source.json > /private/operator/summary.json
node scripts/pilot-scorecard-operations.mjs sign --role merchant --key /private/merchant/reviewer.pem \
  < /private/operator/summary.json > /private/merchant/confirmation.json
node scripts/pilot-scorecard-operations.mjs sign --role maintainer --key /private/maintainer/reviewer.pem \
  < /private/operator/summary.json > /private/maintainer/review.json
node scripts/pilot-scorecard.mjs --ledger /private/pilot/evaluations.json \
  --embedded-authority /private/pilot/authority.json --from "$FROM" --through "$THROUGH" --as-of "$AS_OF"
```

The receipt must contain an audit hash, at least one committed event and the expected result for the governed command. An order cancellation uses `orders.transition` with a cancelled order. A refund uses `payments.create_refund` and requires separately observed completion in the merchant’s external payment system. Receipt and external completion times must both fall on or after the exact opt-in timestamp. Only the completion dates and the result of the timestamp comparison enter the redacted summary. The human review binds the underlying operation across systems. The scorecard checks its provenance, integrity, signatures and dates locally. It does not call a provider or independently certify the source records. Raw receipt contents, transaction identifiers, customer details, amounts and account identifiers stay out of the retained signed files and scorecard output.

Inspect `embeddedOperations.evidenceAmongWins` for missing confirmation, missing review, pre-enrollment evidence and completed human-reviewed activations. Each team can contribute one activation to its primary track. A receipt or external operation reused for a different team invalidates the submission. Teams with undecided outcomes stay in the enrollment denominator. A shortlist breakdown includes every team in that track considering that platform, including teams selecting another finalist. Use these rates to assess the enrolled pilot population.
