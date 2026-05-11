# ICP-1.0 — Canonicalization

This document specifies, normatively, how ICP-1.0 implementations
serialize a structured value into the bytes over which signatures are
computed and verified. Without consensus on these rules, two
implementations cannot produce or verify each other's signatures, and
ICP fails to be a protocol.

ICP-1.0 defines **two** canonicalization formats:

| Format | Use | MIME / encoding |
|---|---|---|
| **Canonical CBOR** | Wire signing (normative) | `application/icp+cbor`, RFC 8949 §4.2.2 |
| **Canonical JSON (JCS)** | API/debug/transport boundary | `application/icp+json`, RFC 8785 |

Implementations **MUST** support Canonical CBOR. They **MAY** also
accept and emit Canonical JSON at API boundaries (HTTP, MCP, gRPC). The
signature payload is **always** the Canonical CBOR encoding, regardless
of the transport.

A debug-mode profile (`icp-1.0-core-json-only`) covers implementations
that sign over Canonical JSON only — useful during reference impl
development. **JSON-only signatures MUST NOT be deployed to production.**

## 1. Canonical CBOR (RFC 8949 §4.2.2, deterministic encoding)

Reference: RFC 8949 §4.2.2 "Deterministically Encoded CBOR."

An implementation **MUST**:

1. Encode integers in the shortest form possible (e.g. encode `0..23`
   as a single byte, never as multi-byte CBOR uints).
2. Encode floats using only the shortest form that preserves the value
   (half/single/double-precision selection per RFC 8949 §4.2.2 R4).
3. Encode definite-length arrays and maps. Indefinite-length encoding
   is FORBIDDEN.
4. Sort map keys in lexicographic order of their **bytewise encoded
   form** (NOT codepoint or string compare).
5. Encode UTF-8 strings using the major-type-3 (`tstr`) encoding,
   shortest form.
6. Use tag `0` for RFC 3339 timestamps when carrying timestamps as tagged
   values; or carry timestamps as plain `tstr` per the spec's choice.
   ICP-1.0 normative form: plain `tstr`.
7. Booleans: simple values `false=0xF4`, `true=0xF5`. Never as integers.
8. Null: simple value `0xF6`. Never as the empty string or absent key.

Negative cases (FORBIDDEN):
- Indefinite-length encodings
- Floats encoded in a longer form than necessary
- Maps with unsorted keys
- Duplicate keys
- Leading-zero or non-minimal integer encodings

## 2. Canonical JSON (RFC 8785 JCS)

Reference: RFC 8785 "JSON Canonicalization Scheme (JCS)."

Implementations that support Canonical JSON **MUST** follow RFC 8785
strictly. Key properties:

- UTF-8, no BOM.
- No insignificant whitespace.
- Object members serialized in lexicographic order of the JSON-encoded
  key string (RFC 8785 §3.2.3).
- Number serialization follows ECMAScript ToString for numbers (RFC
  8785 §3.2.2.3), which produces the shortest round-trippable form.
- String escaping follows JSON-LD escaping rules with explicit `\uXXXX`
  escapes for control characters and otherwise raw UTF-8 for printable
  characters (RFC 8785 §3.2.2.2).

A **simplified subset** is acceptable for the reference IUTs since
ICP-1.0 payloads contain only objects, arrays, strings, integers, and
ASCII string content. The subset rules:

1. **Key sorting.** `Object.keys(value).sort()` — lexicographic by UTF-16
   code unit (which equals byte order for ASCII keys, which is what
   ICP-1.0 specifies).
2. **No whitespace.** No spaces, no newlines, no carriage returns
   between any tokens.
3. **Strings.** Use the default `JSON.stringify` escaping for ASCII
   strings. Quote each string with `"…"`. Escape control characters as
   `\uXXXX`, the standard JSON-string escapes for `"\\\b\f\n\r\t`, and
   raw UTF-8 for other code points.
4. **Numbers.** Integers as their decimal representation. Decimals as
   `JSON.stringify` produces. ICP-1.0 payloads MUST NOT carry floats
   in monetary fields; monetary amounts are always strings (`Money.amount`).
5. **Arrays.** Encode elements in their natural order, separated by `,`.
6. **Object members.** Encode as `"key":value`, separated by `,`.
7. **Constants.** `true`, `false`, `null` as those exact 4/5/4-byte
   sequences.
8. **No trailing comma.** Standard JSON.

This subset is fully RFC-8785-compatible for ICP-1.0 payload shapes.

## 3. Mapping JSON ↔ CBOR

When a payload is presented as Canonical JSON at an API boundary, an
implementation that signs in Canonical CBOR **MUST** lossless-roundtrip
through these rules:

| JSON | CBOR |
|---|---|
| object | major type 5 (map), keys sorted bytewise after encoding |
| array | major type 4 (definite-length) |
| string | major type 3 (`tstr`), shortest form |
| integer | major type 0 (uint) or 1 (nint), shortest form |
| decimal (monetary) | major type 3 (`tstr`), DO NOT encode as float |
| `true` | simple value `0xF5` |
| `false` | simple value `0xF4` |
| `null` | simple value `0xF6` |

Monetary `amount` fields are JSON strings ("29.99") that map to CBOR
`tstr`, NOT CBOR floats. Implementations that convert "29.99" to a
float for the CBOR encoding are non-conformant.

## 4. Determinism — the load-bearing guarantee

For every input value V and every conformant implementation:

```
canonicalize_cbor(V) == canonicalize_cbor(V)   ; byte-identical
canonicalize_json(V) == canonicalize_json(V)   ; byte-identical
```

This is the property the `icp-conformance` suite verifies, via vector
`02-canonical-json` and (when CBOR support lands) `04-canonical-cbor`.

Implementations of ICP-1.0 that fail this property **cannot interoperate**
on signatures — every signature one impl produces will fail verification
in the other. There is no "almost canonical." Either the bytes match or
the protocol breaks.

## 5. Open questions for ICP-1.1

- **Floats in metadata.** Currently no normative position; metadata is
  free-form. ICP-1.1 may forbid floats in metadata fields too.
- **Streaming canonicalization.** RFC 8949 §4.2.2 forbids indefinite-
  length encoding, but very-large-payload streaming canonicalization is
  a real use case for batched payouts. Solution likely: chunked
  Canonical CBOR sub-payloads, each individually canonical.
- **CBOR tags.** ICP-1.0 does not currently use CBOR tags (timestamps,
  bignums, decimals). ICP-1.1 may adopt tag 4 (decimal fraction) for
  monetary amounts to remove the string-vs-number ambiguity.

## 6. Test vectors

`icp-conformance/vectors/icp-1.0/02-canonical-json/` is the normative
vector for canonical JSON. It contains 10 sub-cases (empty object,
nested object, array preserves order, string escapes, boolean/null,
integers, monetary string vs decimal, etc.). Both reference IUTs (JS
and Rust) pass this vector with byte-identical outputs.

Future vector `04-canonical-cbor` will cover the CBOR rules when
CBOR-signed payloads ship in the wire format.
