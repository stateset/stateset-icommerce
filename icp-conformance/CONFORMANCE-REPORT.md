# Conformance report format

`node runner/run.mjs ... --report <path>` writes one JSON document describing
the run. It is the artefact to keep, publish or attach when you state that an
implementation passes a profile. Writing it changes neither the runner's
stdout nor its exit code.

```sh
node runner/run.mjs --profile icp-1.0-core --iut reference-demo \
  --fail-on-skip --report results/reference-demo-core.json
```

`<path>` is resolved against the current directory; missing parent
directories are created and an existing file is overwritten.

## Example

```json
{
  "report_format": "icp-conformance-report/1",
  "suite": { "name": "@stateset/icp-conformance", "version": "1.0.0" },
  "spec_version": "icp-1.0",
  "profile": "icp-1.0-core",
  "complete_profile": true,
  "iut": {
    "name": "my-impl",
    "implementation": "my-impl 0.3.1 (commit abc1234)",
    "language": "zig",
    "version": "0.1.0"
  },
  "timestamp": "2026-11-14T09:30:12.345Z",
  "fail_on_skip": true,
  "vectors": [
    { "name": "01-aid-derivation", "result": "pass", "checks_passed": 6 },
    { "name": "02-canonical-json", "result": "skip", "reason": "no handler" },
    {
      "name": "03-signature-verification",
      "result": "fail",
      "reason": "output divergence",
      "failures": [
        { "key": "verifications", "expected": { "...": "..." }, "actual": { "...": "..." } }
      ]
    }
  ],
  "totals": { "pass": 1, "fail": 1, "skip": 1, "total": 3 },
  "exit_code": 1
}
```

## Fields

| Field | Meaning |
|---|---|
| `report_format` | `icp-conformance-report/1`. The number changes only if an existing field is removed or changes meaning; new fields may be added without changing it. |
| `suite.name`, `suite.version` | The suite's `package.json` name and version — the same version `--version` prints. |
| `spec_version` | The profile's `spec_version` (also the directory under `vectors/`). |
| `profile` | The `--profile` that was run. |
| `complete_profile` | `false` when `--vector` narrowed the run to one vector. Such a report says nothing about the profile as a whole. |
| `iut.name` | The `--iut` value, as given. |
| `iut.implementation`, `iut.language`, `iut.version` | Copied from the IUT's registry entry (`null` if absent). |
| `timestamp` | When the report was written, UTC, ISO 8601. |
| `fail_on_skip` | Whether skips were fatal (`--fail-on-skip` or `ICP_CONFORMANCE_FAIL_ON_SKIP=1`). |
| `vectors` | One entry per vector, in the order run (profile order). |
| `totals` | Counts of `pass`, `fail` and `skip`, and `total` vectors run. Equal to the `Result:` line on stdout. |
| `exit_code` | The runner's exit code for this run: `0` or `1`. |

### Vector entries

Every entry has `name` (the vector directory name) and `result`.

| `result` | Other fields |
|---|---|
| `pass` | `checks_passed` — how many top-level `expected.json` keys matched. |
| `skip` | `reason` — taken from the adapter's stderr JSON (`"unsupported"` if it gave none). The adapter exited with code 2. |
| `fail` | `reason`, plus whichever of these apply: `failures` — a list of `{ key, expected, actual }` for each top-level key that diverged (`actual` is omitted when the adapter did not produce the key); `stderr` — the adapter's stderr, when it exited non-zero; `stdout` — the adapter's stdout, when it was not JSON. |

Values are not truncated, unlike the console output.

## What a report does and does not show

- A profile is passed when `complete_profile` is `true`, `fail_on_skip` is
  `true` and `exit_code` is `0` — equivalently, `totals.pass` equals
  `totals.total`.
- The `iut` block is **self-declared**. The runner copies it from the registry
  entry and cannot check that the adapter really exercises the implementation
  it names. `timestamp` is likewise the local clock. A report is a record of a
  run, not an attestation; anyone can reproduce it by running the same suite
  version against the same adapter.
- No report is written when the runner stops with exit code `2` before running
  any vector (unknown IUT or profile, unreadable registry). If the report
  itself cannot be written, the runner exits `2`.
