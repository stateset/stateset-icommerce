# Run the Muse connector as a service

These templates host the locally verified HTTP/OpenAPI connector. They do not
submit it to Meta or establish that Muse has approved the integration.

Install this repository's CLI and embedded binding dependencies and build the
binding, or use the matching released package once this connector is published.
Use Node 20.20 or newer. The systemd template expects the checkout at
`/opt/stateset-icommerce` and Node at `/usr/bin/node`; adjust those paths for your
host. Do not point a deployment at a package release that lacks `stateset-muse`.

Create a dedicated `stateset` service user and a credential file at
`/etc/stateset/muse-keys.txt` readable by that user. Use a cryptographically random
key with at least 16 characters. Keep this file out of source control.
systemd creates `/var/lib/stateset-muse` for the database and runtime state.

For separate operator and reviewer permissions, copy
[accounts.example.json](accounts.example.json) to `/etc/stateset/muse-accounts.json`,
create its referenced private key files, and replace `--api-key-file` in the unit
with `--accounts-file /etc/stateset/muse-accounts.json`. The paths inside that
file resolve relative to it. Each account's tools must be a subset of the
service's `--tools` allowlist (the curated default applies if omitted). Accounts
without `allowApply: true` remain preview-only even when the service applies
writes. All accounts still use the same store and trusted kernel principal.
Restart the service to apply configuration or key-file changes.

The defaults allow 60 authenticated requests per minute per account, 120 public
or invalid-key requests per minute across the process, 16 concurrent tool
requests across the process, and 4 per account. Adjust `--requests-per-minute`,
`--public-requests-per-minute`, `--max-in-flight`, and `--max-account-in-flight`
in `ExecStart` to match your workload. HTTP 429 and 503 responses include
`Retry-After`. Health probes remain available under overload. Limits reset on
restart and are not shared across replicas.

The CLI writes JSON request telemetry to stderr, which the service manager can
collect. Correlate its `requestId` with the response's `X-Request-Id`. Telemetry
contains account/tool metadata and outcomes without keys, tool arguments,
results, or internal errors. Add `--quiet-audit` if another transport telemetry
pipeline is preferred; kernel commerce receipts remain the authoritative
mutation evidence.

Replace `commerce.example.com` in both `stateset-muse.service` and `Caddyfile`
with your real hostname. Configure its DNS for the service host. The connector
binds to loopback; Caddy exposes the HTTPS endpoint and forwards the expected
Host header. Its configuration follows Caddy's
[reverse proxy](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy)
and [automatic HTTPS](https://caddyserver.com/docs/automatic-https) documentation.

Validate your edited files on the target host before installing them:

```bash
systemd-analyze verify ./stateset-muse.service
caddy validate --config ./Caddyfile --adapter caddyfile
```

Install the unit under `/etc/systemd/system/`, include the Caddy site in your
existing configuration, then reload the services using your host's normal
deployment process. The supplied unit leaves writes in preview mode.

To enable writes, add `--apply` and all three trusted kernel flags to the unit's
`ExecStart`, using operator-owned files readable by the service user. Do not
grant write authority by modifying request arguments. Applied requests must
include a stable `Idempotency-Key` header so retries use the same kernel receipt.
Keep each key tied to one tool and one set of arguments.

## Check a running service

From the repository root:

```bash
node cli/deploy/muse/smoke.mjs \
  --base-url https://commerce.example.com \
  --api-key-file /path/to/muse-keys.txt
```

The command checks health, OpenAPI/catalog agreement, mandatory authentication,
a read with no required resource identifiers, and product creation previews
when that tool is present and preview-only. It never sends an applied write.
Discovery uses the supplied account's Bearer key, so its schema and catalog
must agree even when the public schema lists more tools. Run the check once
per account using that account's key file.
Its JSON summary contains tool names and check status, not credentials or store
records. You can also use `STATESET_MUSE_API_KEYS` instead of a credential file.

Use `http://127.0.0.1:8092` for a local check; the service template allows that
Host header for probes. The public OpenAPI schema still advertises the public
HTTPS origin. Customer identity linking and OAuth remain separate work; this
service uses one operator-defined store scope for all configured keys.

See the [connector guide](../../../docs/src/guides/meta-muse-connector.md) and
[review preparation draft](submission.example.json) for the Muse setup and
directory review steps. In a deployed package, consult the repository guide
at https://github.com/stateset/stateset-icommerce/blob/master/docs/src/guides/meta-muse-connector.md.
