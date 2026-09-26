# Business profiles

A business profile is the part of iCommerce that belongs to an operator. The
commerce kernel remains responsible for exact money, inventory conservation,
authorization, audit history, and database migrations. A profile describes the
business name, defaults, terminology, module preferences, and declarations for
policies, workflows, views, automations, and integrations around that kernel.

Profiles are plain YAML at `.stateset/business.yaml`. They are safe to commit,
review, fork, and share. They contain data only; loading one never executes
JavaScript or SQL.

```bash
stateset-profile init
stateset-profile doctor
stateset-profile context
stateset-profile show
stateset-profile export --output ./profiles/acme.json
stateset-profile diff --against ./profiles/wholesale.yaml
stateset-profile apply --file ./profiles/acme.yaml --force
stateset-profile apply --file ./profiles/acme.yaml --apply --force
stateset-profile pack list
stateset-profile pack inspect --file ./profiles/wholesale.yaml
stateset-profile pack install --file ./profiles/wholesale.yaml
stateset-profile pack install --file ./profiles/wholesale.yaml --apply --force
stateset-profile pack create --output ./packs/acme --name acme
```

`apply` previews the profile changes without writing by default. Add `--apply`
to install the declaration. Profile installation does not mutate commerce
records; those mutations still require the governed write path with an
operator policy and principal.

`context` emits a compact, deterministic operating brief for agents and custom
adapters. It includes the business vocabulary, enabled modules, declared
workflows, and safety boundary without exposing credentials or database rows.
In this first version, those declarations shape the agent brief and support
review and sharing. They do not install executable policies or workflows or
change tool authorization. Keep enforcement in the existing governed kernel
until a declaration has an explicit compiler and validation path.

A pack is a profile plus optional `pack.yaml` metadata. Packs are local by
design in this first version, so a business can review a Git checkout before
installing it. `pack install` previews the leaf changes and writes a lock file
under `.stateset/packs/` only when `--apply` is supplied. Network fetching and
code-bearing plugins stay outside this path.

Pack installation merges named declarations into the current profile by
default and preserves the business name, currency, and timezone. Use
`--replace` when a pack should become the complete profile, including those
identity fields.
`pack create` snapshots the current profile into a directory that can be
committed, edited, and shared.

The repository includes starters for financial services, travel and
hospitality, healthcare, retail and consumer goods, telecommunications,
technology, media, and wholesale. They are deliberately conservative defaults;
each business is expected to fork and edit them.

The profile is intentionally small and composable:

```yaml
schemaVersion: 1
business:
  name: Acme Bicycle Co.
  currency: CAD
  timezone: America/Vancouver
modules:
  orders: true
  inventory: true
  payments: true
  returns: true
terminology:
  order: work order
  customer: rider
policies:
  - name: manager-approval-over-500
    kind: approval
    resource: refund
    threshold: '500.00'
workflows:
  - name: service-intake
    resource: order
    states: [requested, diagnosed, approved, complete]
```

Keep profiles in source control beside the application that owns the business.
Use `doctor` in CI, review changes with `diff`, and pin pack versions before
applying them to production. The profile layer is additive: it cannot weaken
the invariants documented in [Commerce Invariants](../advanced/invariants.md).
