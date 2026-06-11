# ICPIP-XXXX: <Title>

```
ICPIP:        xxxx                  (editors assign at Review)
Title:        <Concise title, capitalized>
Author:       Your Name <your@email>
Discussions:  <URL to discussion thread>
Status:       Draft
Type:         Standards Track | Meta | Informational
Category:     Core | Networking      (Standards Track only)
Created:      YYYY-MM-DD
Requires:     <ICPIP numbers this depends on, or — >
Supersedes:   <ICPIP numbers this replaces, or — >
```

## Abstract

Two-paragraph summary of what this proposal changes and why. Should
be understandable by a reader who knows ICP-1.0 but not the specific
problem domain.

## Motivation

Why does ICP need this change? What use case is unaddressed? What
goes wrong if we don't make this change?

Use real examples. Cite real situations. Avoid hand-wavy claims.

## Specification

The normative change. Be precise. Use RFC 2119 keywords (MUST, MUST
NOT, SHOULD, SHOULD NOT, MAY) for normative requirements — these
are interpreted per BCP 14.

For Standards Track:
- Describe the wire format change (canonical JSON; reserved CBOR profile if applicable).
- Describe how existing implementations must migrate.
- Provide a JSON Schema if applicable.
- Define new error codes if applicable.
- Specify backward compatibility behavior for unknown / missing
  fields.

For Meta / Informational:
- Describe the process / recommendation precisely enough to be
  unambiguously implemented.

## Rationale

Why this design and not alternatives? Document the design space.
What other approaches were considered and why were they rejected?
What tradeoffs does this design make?

Be honest about weaknesses. ICPIPs that gloss over tradeoffs are
weaker than ICPIPs that name them and explain why the chosen tradeoff
is acceptable.

## Backwards Compatibility

How does this change interact with existing implementations? What
breaks if an older implementation receives a new-shape message?
What's the migration path for existing wire traffic?

If this ICPIP requires a major version bump (ICP-2.0), say so and
explain why.

## Security Considerations

What new attack surface does this introduce? What existing
mitigations are weakened? Has this been reviewed by anyone with
security expertise?

This section is REQUIRED for Standards Track ICPIPs. Editors will
reject ICPIPs that omit it.

## Test Vectors

For Standards Track / Core ICPIPs that affect the wire format:
provide concrete test vectors (input → expected output) that
conformance implementations can verify against. Vectors will be
added to `icp-conformance/vectors/icp-N.M/<ICPIP-number>/` upon Final.

Two prototype implementations passing these vectors is a hard
gate for Final promotion.

## Reference Implementation

Link to a public branch or PR demonstrating the proposed change in
at least one ICP implementation. Required for Standards Track Final.

## References

- Prior ICPIPs this builds on
- External standards (RFCs, FIPS, ISO)
- Discussion threads
- Related implementations

## Copyright

This ICPIP is licensed under CC-BY-4.0.
