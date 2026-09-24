# Business profiles

A business profile is the part of iCommerce that belongs to an operator. The
commerce kernel remains responsible for exact money, inventory conservation,
authorization, audit history, and database migrations. A profile describes the
business name, defaults, terminology, enabled modules, policies, workflows,
views, automations, and integrations around that kernel.

Profiles are plain YAML at `.stateset/business.yaml`. They are safe to commit,
review, fork, and share. They contain data only; loading one never executes
JavaScript or SQL.

```bash
stateset-profile init
stateset-profile doctor
stateset-profile show
stateset-profile export --output ./profiles/acme.json
stateset-profile diff --against ./profiles/wholesale.yaml
stateset-profile apply --file ./profiles/acme.yaml --force
```

`apply` installs the declaration after validation and reports preview mode. It
does not mutate commerce records. Database mutations still require the normal
governed write path with an explicit operator policy and principal.

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
