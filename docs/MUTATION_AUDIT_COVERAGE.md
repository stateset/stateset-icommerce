# Mutation evidence coverage

Required evidence must commit in the business transaction. A successful database
call that affects zero rows is insufficient: SQLite triggers can use
`RAISE(IGNORE)` and PostgreSQL triggers can return `NULL`. The outbox, governed
receipt, and receipt-chain insert helpers reject that condition, including
preview-to-apply updates of the materialized receipt.

## Customer repository contract

Both SQLite and PostgreSQL record these domain facts in `kernel_outbox` with
`tier = recorded`. Failure to persist the fact rolls back the business write.

| Entry points | Required facts | Boundaries verified |
| --- | --- | --- |
| Create, get-or-create, guest checkout's shared creation helper, atomic create batch | `customer.created` | An existing account emits no new creation fact; failure on a later batch member rolls back prior customers and facts |
| Update, atomic update batch | `customer.updated` | Supplied field names and before/after status; rejected status changes emit nothing; late batch failure rolls back earlier edits |
| Delete, atomic delete batch | `customer.deleted` | Only an actual transition emits; missing/already-deleted accounts remain no-ops; late batch failure rolls back earlier deletions |
| Anonymize | `customer.anonymized`, plus `customer.deleted` if deletion is needed | The PII scrub, address removal, deletion transition, and both facts commit together; fact includes the removed-address count |
| Add/update/delete address | `customer_address.created`, `.updated`, `.deleted` | Facts identify the address and its owning customer; default-pointer and derived-flag changes roll back with a failed fact |
| Set default address | `customer.default_address_changed` | The requested role, pointers, flags, and fact commit together |

Non-atomic batch entry points call the same single-record operations and retain
their documented partial-success behavior. Default pointers and address flags
are derived parts of their containing address operation; they do not emit extra
independent facts. Empty customer updates retain the existing behavior of
advancing update time/version and emit an update fact with no supplied fields.

Deleted customers reject address additions, edits, and default changes. Removing
existing addresses remains allowed for cleanup. SQLite's IMMEDIATE transaction
serializes these operations with anonymization. PostgreSQL locks the parent
customer before any address row, matching anonymization's lock order; concurrent
writers either complete before the scrub or fail without restoring contact data.
The shared contract tests this terminal-state rule and races all four address
mutation entry points against anonymization.

Anonymization also reads back the customer and checks that no customer addresses
remain before committing. Suppressed profile updates or address deletions must
roll back the whole scrub and its facts. The final profile update always resets
the email tombstone and email key, including already-deleted legacy accounts.
Soft deletion requires exactly one updated row before recording its transition;
a trigger that silently suppresses the update cannot produce a deletion fact.

New edit, deletion, anonymization, and address payloads contain identifiers,
field names, counts, status, address type and default flags. They omit names,
contact details, street addresses, tags, and metadata values. The existing
`customer.created` contract includes email; PostgreSQL now mirrors that contract.
Anonymization does **not** erase previously recorded or externally exported
facts. Retention and privacy reviews must account for that history.
Identifiers can also be personal data when linkable; these payloads are not
anonymous records.

These facts do not assert an operator identity: native repository calls leave
principal fields empty. Recorded facts are not the governed receipt hash chain
or a complete, immutable compliance archive. Outbox delivery/retention remains
an operator responsibility. General `audit_log` helper coverage is still opt-in.

## Acceptance evidence

`crates/stateset-db/tests/customer_audit_contract.rs` runs the same real-repository
scenarios on both backends. It covers lifecycle payloads, no-ops, address/default
invariants, all new mutation refusal paths, creation/get-or-create, and late
atomic-batch failures. Every failure matrix exercises both an explicit database
error and silent trigger suppression, then retries and verifies one new fact.
Separate cases suppress the profile scrub and address removal themselves and
verify that anonymization cannot claim success with contact data still present.
Another case suppresses the deletion transition, verifies unchanged customer,
addresses and facts, then retries and checks exactly one deletion fact.

The required-row regression in `sqlite_kernel_outbox.rs` and
`postgres_kernel_executor.rs` suppresses outbox, audit-chain, and receipt inserts
and a receipt update during preview promotion. It checks business rollback,
retained preview state, and successful retry/replay without duplicate effects.

The `AI Kernel Contract & Adversarial Evals` CI job runs the shared customer
contract with a live PostgreSQL service as well as both governed-command suites.
Local success is development evidence; release acceptance requires the CI run
and immutable artifacts for the release commit.

## Refund settlement contract

The public payment repository's refund completion and failure entry points on
SQLite and PostgreSQL require `payments.refund_completed.v1` and
`payments.refund_failed.v1` facts respectively. Each transition and its fact
share one transaction; completion includes the payment balance and derived
order payment status. Already-settled retries emit nothing, and completion and
failure serialize so only one terminal outcome can commit.

Facts carry refund/payment identifiers, exact decimal-string amount, currency,
and previous/new status. They omit processor failure text and other contact or
payment credentials. As with customer facts, native calls assert no operator
identity and do not provide an immutable compliance archive.

Both backends' refund correctness suites inject explicit and silently skipped
outbox inserts, skipped refund updates and skipped payment-balance updates.
They verify unchanged business state and absent facts after refusal, successful
retry and one fact, stable replay, rejected opposite transitions, and competing
completion/failure calls. The refund transition itself must affect exactly one
row before completion may advance the payment balance.
PostgreSQL also runs terminal retries through a one-connection pool to verify
that replay does not acquire a second connection while holding its transaction.

These entry points do not establish coverage for every internal return,
chargeback or payment mutation; those paths need their own evidence.

## Remaining coverage

The source-based inventory lives in
`crates/stateset-db/tests/outbox_emission_parity.rs`. Its backlog remains nonempty
outside customers, including cart edits, finance, inventory configuration, WMS,
and specialized domains. The source lint catches drift but does not prove
transaction boundaries or payload correctness; every resolved area needs
behavioral coverage comparable to the customer contract. PostgreSQL coverage
must be checked explicitly rather than inferred from a SQLite call site.

Do not infer complete auditing from a valid governed receipt chain, a zero
general-audit failure counter, or a passing source inventory.
