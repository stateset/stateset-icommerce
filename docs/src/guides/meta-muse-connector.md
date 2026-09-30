# Meta Muse connector

StateSet's `stateset-muse` command exposes a store-scoped HTTP service for the
Meta Muse personal assistant. It provides an OpenAPI 3.1 catalog and authenticated
tool endpoints backed by the embedded agent toolkit. It does not call the Muse
Spark model API or require a model API key.

Meta describes connector submission, review, and directory approval on the
[Muse Connector Platform](https://muse.ai/platform). This implementation supplies
a standard HTTP/OpenAPI integration surface; it is not a Meta-approved connector
or a claim about an undocumented Muse runtime contract. Testing in Muse and
directory submission remain separate from local service tests.

## Run locally

From this repository, using Node 20.20 or newer:

```bash
# Create a private credential file; this prints no key.
node -e "require('fs').writeFileSync('/tmp/stateset-muse-keys.txt', require('crypto').randomBytes(32).toString('hex') + '\n', { mode: 0o600 })"
node cli/bin/stateset-muse.js --db ./store.db \
  --api-key-file /tmp/stateset-muse-keys.txt
```

The service listens on `127.0.0.1:8092`. The CLI package exports the same command
as `stateset-muse` once this change is released. You can supply comma-separated
keys through `STATESET_MUSE_API_KEYS` instead of a file. Keys must contain at
least 16 characters. Do not put keys in URLs or connector prompts.

| Endpoint                | Authentication | Purpose                                         |
| ----------------------- | -------------- | ----------------------------------------------- |
| `GET /health`           | Public         | Minimal health probe                            |
| `GET /openapi.json`     | Public         | Allowlisted operation schemas; no store records |
| `GET /v1/tools`         | Bearer token   | Catalog with permissions and preview flags      |
| `POST /v1/tools/<name>` | Bearer token   | Validate and execute one allowlisted tool       |

POST a tool's argument object directly as `application/json`; there is no
`params` envelope. The schema comes from StateSet's existing tool registry.
For example, `POST /v1/tools/list_products` accepts `{}` with
`Authorization: Bearer <your key>`.

The default tools are `list_products`, `get_product`, `get_stock`, `list_orders`,
`get_order`, `get_sales_summary`, `create_product`, and `update_order_status`.
Use `--tools a,b` to narrow or replace the exact allowlist. Unknown names fail
startup; tools outside the allowlist cannot be called even if they exist in
the underlying engine.

This is a merchant/operator connector. Each service uses one store and one
operator-owned kernel principal. Shared `--api-key-file` keys use the full service
tool allowlist. For distinct access, use named accounts:

```bash
node cli/bin/stateset-muse.js --db ./store.db --accounts-file /etc/stateset/muse-accounts.json
```

Copy [accounts.example.json](../../../cli/deploy/muse/accounts.example.json)
and create the referenced private key files. Each account has a stable `id`, an
`apiKeyFile`, an exact `tools` subset of the service allowlist, and optional
`allowApply` (default false). Key paths resolve relative to the account file.
Do not combine this option with `--api-key-file` or `STATESET_MUSE_API_KEYS`.
Duplicate keys, duplicate account IDs, unknown fields in the file, and tools
outside the service allowlist fail startup.

Authenticated `/openapi.json` and `/v1/tools` show only the account's tools and
write mode. Public OpenAPI discovery shows the service catalog. Requests to
tools outside the account's scope return 404 before reading the body or running
the engine. Writes apply only when both the service's `--apply` and the account's
`allowApply: true` are enabled; the kernel still checks the trusted principal.
To rotate credentials, add a new key to the same account's key file, restart,
switch clients, then remove the old key and restart. Never reassign an account
ID to another operator.

These accounts do not derive a customer identity from Muse, enforce
customer-specific order ownership, or route requests between merchants. Private
customer access still needs an authenticated identity and ownership checks;
separate merchants need separate stores and trusted principals.

## Write behavior

Without `--apply`, every non-read tool returns a validated preview and performs
no mutation. Request arguments cannot enable writes or replace the principal,
policy, database path, or execution options. Exact decimal strings pass through
unchanged.

For governed writes, provide all operator-owned kernel files:

```bash
node cli/bin/stateset-muse.js --db ./store.db \
  --api-key-file /path/to/muse-keys.txt --apply \
  --kernel-policy /path/to/kernel-policy.json \
  --kernel-principal /path/to/kernel-principal.json \
  --kernel-store-id store:production
```

Configure the policy and delegated principal for the selected writes, such as
`products.create` or `orders.transition`. The connector requires kernel
configuration for apply mode and exposes no legacy-write bypass. The toolkit
capability list is also restricted to the connector's exact tool names. See
[kernel execution](../kernel-execution.md) for the authority model.

Responses preserve the toolkit result and audit evidence. HTTP 200 and the
outer `success` flag indicate that a tool call ran, not that a commerce command
succeeded. For governed operations, inspect `result.success`,
`result.receipt.status`, and `result.receipt.error_code`. Do not retry a write
blindly after a transport failure. Applied write requests require an
`Idempotency-Key` header of 8–128 letters, digits, underscores, dots, colons, or
hyphens. Reuse it for retries of the same operation and arguments. The validated
key is passed to the kernel; named accounts hash it together with the stable
account ID so account retries cannot replay another account's receipt. Key
rotation within an account preserves retries. Switching between shared-key and
named-account modes changes the retry namespace; finish pending writes before
switching. Repeated product creation returns the original
receipt without creating a second product. Previews and reads do not require
this header. Authenticated OpenAPI discovery advertises it for accounts that
can apply writes.

## Request controls and telemetry

The service has sliding request budgets and concurrent tool limits:

| CLI option                     | Default | Scope                                                                          |
| ------------------------------ | ------- | ------------------------------------------------------------------------------ |
| `--requests-per-minute`        | 60      | All authenticated requests per account, including discovery and rejected calls |
| `--public-requests-per-minute` | 120     | One shared budget for public discovery and missing/invalid credentials         |
| `--max-in-flight`              | 16      | Tool requests across the service                                               |
| `--max-account-in-flight`      | 4       | Tool requests within one account                                               |
| `--max-body-bytes`             | 65536   | Bytes in a tool request body                                                   |

All values must be positive integers. Key rotation shares the account's budget.
Public traffic uses one bounded bucket; forwarded IP headers do not change its
scope. Health probes bypass budgets. Budget exhaustion returns HTTP 429;
concurrent capacity exhaustion returns HTTP 503. Both include `Retry-After` in
seconds. These controls are in memory and local to the process; restarts reset
them. Coordinate fleet-wide limits and connection limits at your ingress if
running multiple replicas.

Concurrency permits cover body reading, validation, and engine execution. A
disconnected client does not release capacity while its commerce command is
still running. For an applied write, retry with the original `Idempotency-Key`
and identical arguments after a transport failure.

Each response carries a service-generated `X-Request-Id`; caller-provided IDs
are ignored. By default, the CLI writes one JSON request telemetry event to
stderr. Events contain `type`, `timestamp`, `requestId`, `accountId`, `tool`,
`mode`, `outcome`, `statusCode`, `durationMs`, and `disconnected`. They exclude
credentials, request URLs, arguments, tool results, and exception messages.
Tool names are recorded only for an authorized tool. A disconnected request
may have a null status code because no response could be delivered.

Use `--quiet-audit` to disable this transport telemetry. It supplements the
kernel's commerce receipts and is best effort, rather than a durable commerce
audit trail. Programmatic users can supply `onAudit(event)`; a throwing or
rejecting sink does not change an executed operation or replay it. Call the
connector's `close()` during shutdown to stop limiter cleanup timers; the CLI
handles this automatically.

## Host for Muse

Muse's remote environment needs a reachable HTTPS service. Set its origin in
`--public-url`; run TLS at your existing reverse proxy:

```bash
node cli/bin/stateset-muse.js --host 0.0.0.0 --port 8092 \
  --public-url https://commerce.example.com --db /data/store.db \
  --api-key-file /run/secrets/muse-keys.txt
```

Replace the example origin with your own hostname. Preserve that Host header
through the proxy, or add the proxy's internal hostname with `--allowed-host`.
The service rejects unexpected hosts and browser origins. Server-to-server
requests need no Origin header. This version supports Bearer API keys; it does
not advertise OAuth discovery or provide an account-linking OAuth server.

For a Muse custom connector, provide the HTTPS OpenAPI URL and the supported
Bearer authentication method through Muse's current credential workflow.
A suggested task is:

> Create a StateSet iCommerce connector using
> https://commerce.example.com/openapi.json. Use its documented Bearer
> authentication. Start by listing products and checking stock. Treat write
> previews as proposals and inspect kernel receipts for commercial outcomes.

This is a proposed integration prompt, not a verified Muse setup procedure.
Do not use the placeholder URL or paste a credential into this prompt.

For the reviewed directory, prepare the actual endpoint, product description,
support and legal URLs, reviewer access, and sample workflows, then follow
Meta's current submission form. No submission is sent by this command.
An [internal review draft](../../../cli/deploy/muse/submission.example.json)
contains the product description, authentication scope, sample prompts, and
explicitly unfilled hosting, reviewer-access, and legal fields. It is not a
Meta-defined manifest or submission format.
Suggested examples are “Show my product catalog,” “Check stock for SKU X,”
“Summarize sales for the last week,” and “Preview creating a product.”

The existing `stateset-mcp-http` endpoint remains available for integrations
that explicitly accept remote MCP. Confirm the transport accepted by the
specific Muse integration before selecting it.

The [service deployment templates](../../../cli/deploy/muse/README.md) provide
a systemd unit, Caddy reverse-proxy configuration, and a smoke command for an
already-running connector. The smoke command does not apply writes.

## Verify

```bash
node --test cli/test/unit/muse-connector.test.js \
  cli/test/integration/muse-connector.test.js
```

Tests cover authentication, host/origin validation, exact allowlists, strict
argument schemas, request size limits, previews, real store reads, and governed
writes. A real engine test preserves `9007199254740993.25` exactly, rejects a
numeric price in strict apply mode, and prevents a write when delegated kernel
capabilities are absent.
