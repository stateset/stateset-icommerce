# Vector 08 — Replay Timing (§5.3)

**Spec sections covered:** ICP-1.0 §5.3 (replay protection: `iat`/`exp`
window), `schemas/error-codes.md` (`replay.*` namespace).

The temporal half of replay protection. §5.3 requires `exp ≤ iat + 600s` for
Intents and rejection of expired messages; a verifier that gets the window
arithmetic wrong either accepts a stale intent an attacker replayed or rejects
a fresh one. This is the first family with **clock-relative** logic — each
case supplies a reference `now`.

## Checks and precedence

Applied in this order; the **first** failure wins:

1. `iat` or `exp` (or `now`) not a strict `YYYY-MM-DDTHH:MM:SSZ` timestamp with
   in-range fields → `replay.timestamp_malformed`
2. `exp − iat > 600s` → `replay.window_too_long` (§5.3 intent ceiling;
   clock-independent, so checked before the clock-relative rule)
3. `exp < now` → `replay.expired`
4. otherwise → valid

Boundaries are pinned by cases: `exp − iat = 600` exactly is **valid** (`t04`),
`exp = now` exactly is **not** expired (`t05`), and a case that is both
over-window and expired resolves to `window_too_long` by precedence (`t08`).
`t09` crosses a UTC day boundary to exercise the date arithmetic.

## Portability

Timestamps are parsed with a **strict fixed-format parser** (regex +
range-checked fields) and converted to epoch seconds with the `days_from_civil`
civil-calendar algorithm — **not** each language's built-in date parser, which
disagree on lenient inputs (e.g. JS `Date.parse` accepts `"2026-07-14"` as
midnight where a strict parser rejects it). All four IUTs implement the same
parser and the same epoch algorithm, so they agree byte-for-byte.

## Deferred

`replay.iat_in_future` is **not** covered here: §5.3 says `iat` more than "the
allowed clock skew" in the future is rejected but does not specify the skew.
Pinning a value in conformance would prescribe behavior the spec leaves open —
this is a spec-clarification item (candidate ICPIP), not a conformance gap to
paper over.

## Adapter contract

stdin: `inputs.json` (`{ "cases": [{ "id", "iat", "exp", "now" }, ...] }`).
stdout: `{ "validations": { "t01_valid": {"valid": true},
`t02_expired": {"error": "replay.expired"}, ... } }`.
