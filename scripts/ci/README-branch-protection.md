# Branch protection for `master`

Branch protection is repository state, not code: nothing in this repo can set it,
and nothing in CI can detect that it drifted. This file is the source of truth
for what it should be, and the exact commands that put it there.

**Do not run these commands as part of a change.** They are a deliberate,
reviewed administrative action. Every command below needs a token with admin
rights on `stateset/stateset-icommerce` (`gh auth refresh -s admin:repo` if the
read command 403s).

## What is wrong today

```console
$ gh api repos/stateset/stateset-icommerce/branches/master/protection/required_status_checks --jq '{strict, contexts: (.contexts | length)}'
{"strict":false,"contexts":38}
```

Two gaps:

1. **`strict: false`.** A pull request can merge while its branch is behind
   `master`. Every required check passed — against a tree that no longer exists.
   Semantic conflicts (a rename here, a new caller there) land red on `master`.
2. **`CI Success` is not required.** The 38 required contexts are individual
   `ci.yml` jobs, so a job *added* to `ci.yml` is not required until someone
   remembers to add its name here. `ci-success` is the aggregate job that
   `needs` every other job in `ci.yml` (enforced by
   `scripts/ci/check_workflow_job_refs.mjs`), so requiring it makes every
   present and future job required automatically. It is also the fallback
   context that `scripts/ci/check_required_checks.sh` uses when it cannot read
   branch protection — which is always, from inside a workflow, because
   `GITHUB_TOKEN` is not an admin token.

## The fix

### 1. Read the current state and keep a copy to roll back to

```bash
gh api repos/stateset/stateset-icommerce/branches/master/protection/required_status_checks \
  > /tmp/required-status-checks.before.json
cat /tmp/required-status-checks.before.json | jq
```

### 2. Set `strict: true` and add `CI Success`

Build the payload from the live list rather than retyping 38 context names —
each entry keeps its `app_id` (15368 = GitHub Actions), which is what stops a
third-party app from satisfying a context it does not own:

```bash
jq '{
  strict: true,
  checks: ((.checks | map({context, app_id})) + [{context: "CI Success", app_id: 15368}] | unique_by(.context))
}' /tmp/required-status-checks.before.json > /tmp/required-status-checks.after.json

gh api --method PATCH \
  repos/stateset/stateset-icommerce/branches/master/protection/required_status_checks \
  --input /tmp/required-status-checks.after.json
```

### 3. Verify

```bash
gh api repos/stateset/stateset-icommerce/branches/master/protection/required_status_checks \
  --jq '{strict, has_ci_success: (.contexts | index("CI Success") != null), contexts: (.contexts | length)}'
```

Expected: `{"strict":true,"has_ci_success":true,"contexts":39}`.

### 4. Roll back, if the merge queue backs up

```bash
gh api --method PATCH \
  repos/stateset/stateset-icommerce/branches/master/protection/required_status_checks \
  --input /tmp/required-status-checks.before.json
```

## What changes for contributors

`strict: true` means GitHub will refuse to merge a pull request whose branch is
behind `master`, so an out-of-date branch has to be updated (the "Update branch"
button, or `git merge origin/master`) and its checks re-run. On a busy day that
is one extra CI round-trip per merge. That is the cost of never landing a
red `master`; if it becomes the bottleneck, enable a merge queue rather than
turning `strict` back off.

## Related gates

Branch protection is only half of the release story; the other half lives in
this repo and is enforced automatically:

| Gate | Where | What it refuses |
| --- | --- | --- |
| `npm run release:tag` | `scripts/release-tag.sh` | Tagging a dirty tree, a commit that is not `origin/master`, a failing hygiene run, or a commit whose required checks are not green. Pushes `v`, `cli-v` and `py-v` in one push. |
| Release guard | `scripts/ci/check_release_guard.sh` | Publishing from a commit that is not an ancestor of `master`, whose required checks are red, or whose three sibling tags do not all exist on it. |
| Required checks | `scripts/ci/check_required_checks.sh` | Both of the above read the required-context list from this branch protection API, falling back to `CI Success` when the token cannot read it. |
| Registry drift | `.github/workflows/release-consistency.yml` | Nothing — it reports. Daily, it compares the latest `v*` tag to npm, PyPI and crates.io and files the "Registry drift" issue. |
