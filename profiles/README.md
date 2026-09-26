# Business profile starters

These starter profiles are data-only examples. Copy one into an application's
`.stateset/business.yaml`, edit it, then run `stateset-profile doctor`.

Included vertical starters cover financial services, travel and hospitality,
healthcare, retail and consumer goods, telecommunications, technology, media,
and wholesale. Their descriptions are deliberately short; the profile is a
starting point for the operator's own policies, workflows, and terminology.

The profile layer changes business vocabulary and records intended operating
behavior. Most policy and workflow entries are declarations only. Explicit
`kernel-restriction` entries can be compiled against an operator-owned kernel
policy, but only tighten commands that policy already permits. Compilation
previews by default and requires a separate `--apply` to write a policy file;
installing a pack never changes commerce permissions or runs jobs.
