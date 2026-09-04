# Security Architecture

Security is a first-class concern across all layers of iCommerce. This chapter covers the security measures applied to the Rust core, CLI, adapters, and protocol implementations.

## Input Validation

### SQL Injection Prevention

All database queries use parameterized prepared statements. No string concatenation is used for SQL construction.

### SSRF Protection

All webhook URLs and external endpoints are validated against a blocklist:

- Localhost and loopback addresses (`127.0.0.1`, `::1`)
- Private IP ranges (`10.x.x.x`, `172.16.x.x`, `192.168.x.x`)
- Internal TLDs (`.local`, `.internal`)
- Non-HTTP(S) schemes

### Command Injection Prevention

Shell commands (used in sync and adapter operations) use parameterized execution — arguments are passed as array elements, never interpolated into command strings.

### Prototype Pollution

All user-supplied JSON objects are validated through Zod schemas before processing. Object property access uses safe patterns that prevent `__proto__` and `constructor` manipulation.

### ReDoS Prevention

All regular expressions in the codebase have been audited for catastrophic backtracking. Complex patterns use atomic groups or are replaced with non-regex alternatives.

## Authentication

### API Key Authentication

The HTTP gateway supports multi-level API key authentication:

| Level | Value | Access |
|-------|-------|--------|
| `none` | 0 | Public routes only (`/health`) |
| `read` | 1 | Read-only endpoints |
| `write` | 3 | Create and update |
| `delete` | 4 | Destructive operations |
| `admin` | 5 | Full access |

See [Permissions & Auth](../guides/permissions.md) for configuration details.

### Webhook Signature Verification

All webhook payloads are signed with HMAC-SHA256:

- **Stripe**: v1 signature scheme with timestamp tolerance
- **WooCommerce**: HMAC-SHA256 with configured secret
- **A2A**: HMAC-SHA256 with per-endpoint secrets

Signature verification uses constant-time comparison to prevent timing attacks.

## Cryptographic Safety

### Key Management

- Ed25519 signing keys are zeroized from memory after use (via `zeroize` crate)
- No key material is logged or included in error messages
- Key rotation is supported via `stateset-sync keys:rotate`

### Random Number Generation

All cryptographic operations use `crypto.randomBytes()` (Node.js) or `OsRng` (Rust) — never `Math.random()`.

### Constant-Time Operations

Signature verification and HMAC comparison use constant-time algorithms (via the `subtle` crate in Rust) to prevent timing side-channels.

## Permission Model

### CLI Safety Model

- All write operations require `--apply` (read-only by default)
- MCP tools are tagged with permission levels
- 20 high-risk tools require explicit approval in `permissions.js`
- Audit log records all tool invocations with timestamps

### Policy Engine

The [policy engine](../policy/engine.md) provides declarative safety guardrails:

- Deny-override semantics (deny always wins over allow)
- Per-condition explainability
- Transform audit trails
- Hot-reload for policy file changes

## Sandbox Mode

The HTTP gateway supports sandbox mode that blocks dangerous routes regardless of permission level:

- **Browser sandbox**: Blocks `/browser/evaluate`, `/browser/navigate`, `/browser/click`, `/browser/type`
- **Shell sandbox**: Blocks `/daemon` management

## Network Security

### TLS

iCommerce does not terminate TLS itself. In production:

- Bind to `127.0.0.1` and use a reverse proxy (nginx, Caddy) for TLS
- Or use a cloud load balancer with TLS termination

### CORS

Configure allowed origins explicitly; avoid `*` in production:

```json
{
    "httpGateway": {
        "cors": {
            "allowedOrigins": ["https://admin.example.com"]
        }
    }
}
```

## Dependency Security

### Rust

- `cargo-deny` enforces license compliance and vulnerability scanning
- Workspace-level clippy lints catch common security issues
- `#[deny(unsafe_code)]` on all crates except FFI boundaries

### Node.js

- Zod schemas validate all tool inputs (120+ validation constraints)
- ESLint with security rules
- No `eval()` or dynamic code execution

## Runtime Protection

### Rate Limiting

The MCP rate limiter prevents compromised or misconfigured agents from overwhelming the system:

```javascript
// Per-agent, per-tool sliding window (default: 60 requests/minute)
// Tool-specific overrides for high-risk operations:
// a2a_pay: 10/minute, delete_customer: 5/minute
```

See [A2A Infrastructure — Rate Limiter](../a2a/infrastructure.md) for configuration.

### Circuit Breaker

Agent circuit breakers halt all transactions when:
- Daily spending limit exceeded
- Monthly spending limit exceeded
- Per-transaction limit exceeded
- Manual trip by an operator
- Global kill switch activated

See [Compliance & Audit — Circuit Breaker](../advanced/compliance.md#circuit-breaker--kill-switch).

### Audit Logging for Intrusion Detection

Every tool invocation is logged with timestamp, agent ID, tool name, and result. Monitor for:

- Unusual volume of `delete_*` operations
- Repeated policy denials from a single agent (probing)
- Tool calls outside normal business hours
- Access to customer PII by non-customer-service agents

```javascript
// Alert on suspicious patterns
const denials = await toolkit.executeTool('audit_query', {
    result: 'denied',
    startDate: new Date(Date.now() - 3600000).toISOString(),
    limit: 100
});
if (denials.length > 50) {
    // Trigger alert: possible policy probing
}
```

### Secret Rotation

| Secret | Rotation Frequency | How |
|--------|-------------------|-----|
| Ed25519 signing keys | Every 90 days | `stateset-sync keys:rotate --all --register` |
| API gateway keys | On team member departure or suspected compromise | Update `apiKeys` in gateway config |
| Stripe webhook secret | On key rotation or suspected compromise | Regenerate in Stripe dashboard, update `--stripe-secret` |
| WooCommerce API keys | On employee departure | Regenerate in WooCommerce settings |

Rotate immediately on any suspected compromise. The VES key rotation protocol signs the rotation event with the old key, proving continuity.
