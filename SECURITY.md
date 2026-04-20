# Security Policy

## Supported Versions

We release patches for security vulnerabilities in the following versions:

| Version | Supported                          |
| ------- | ---------------------------------- |
| 1.x     | :white_check_mark: (current)       |
| 0.9.x   | :white_check_mark: (security only) |
| < 0.9.0 | :x:                                |

This table tracks the currently maintained release line for this repository and should be updated in the same change that advances the supported release series.

## Reporting a Vulnerability

We take the security of StateSet iCommerce seriously. If you discover a security vulnerability, please report it responsibly.

### How to Report

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please send an email to: **security@stateset.io**

Include the following information in your report:

1. **Description** - A clear description of the vulnerability
2. **Impact** - What an attacker could achieve by exploiting it
3. **Steps to Reproduce** - Detailed steps to reproduce the issue
4. **Affected Components** - Which crates, bindings, or features are affected
5. **Suggested Fix** - If you have ideas on how to fix it (optional)

### What to Expect

- **Acknowledgment** - We will acknowledge receipt within 48 hours
- **Initial Assessment** - We will provide an initial assessment within 7 days
- **Updates** - We will keep you informed of our progress
- **Resolution** - We aim to resolve critical issues within 30 days
- **Credit** - We will credit you in our security advisories (unless you prefer anonymity)

### Disclosure Policy

- Please give us reasonable time to address the issue before public disclosure
- We follow coordinated disclosure practices
- We will work with you to understand and resolve the issue quickly

For the current audit, formal-verification, and trust-assumption status of the repo, see `TRUST_FOUNDATION.md`.

## Security Considerations

### Database Security

- **SQLite**: Database files should be protected with appropriate file system permissions
- **PostgreSQL**: Use strong credentials and secure connection strings
- **MySQL is not supported.** The `sqlx` dependency excludes MySQL at the workspace level (`default-features = false`, features list does not include `mysql`). Do not enable the MySQL driver in downstream crates — see the Vulnerability History entry for RUSTSEC-2023-0071.
- Never commit database files or credentials to version control

### API Keys & Credentials

When using the CLI with AI capabilities:

```bash
# Use environment variables for API keys
export ANTHROPIC_API_KEY=sk-ant-...

# Never hardcode keys in scripts or commit them
```

### Safe Mode Architecture

The CLI implements a safety architecture for write operations:

```bash
# Read-only operations (safe) - no flag needed
stateset "list orders"
stateset "show inventory for SKU-001"

# Write operations require explicit --apply flag
stateset --apply "create order for customer@example.com"
stateset --apply "adjust inventory SKU-001 by -10"
```

This prevents accidental data modifications when using AI-powered commands.

### Input Validation

All input to the commerce engine is validated:

- Email addresses are validated before customer creation
- SKUs are checked against inventory before order creation
- Quantities must be positive numbers
- Currency codes must be valid ISO 4217 codes

### Event Auditing

All operations emit events that can be used for security auditing:

```rust
pub enum CommerceEvent {
    OrderCreated(Order),
    OrderStatusChanged { id, from, to },
    InventoryAdjusted { sku, delta, reason },
    PaymentProcessed(Payment),
    // All state changes are logged
}
```

## Security Best Practices

### For Production Deployments

1. **Use PostgreSQL** for production workloads with proper access controls
2. **Enable TLS** for all database connections
3. **Rotate credentials** regularly
4. **Monitor events** for suspicious activity
5. **Keep dependencies updated** - run `cargo update` regularly

### For Development

1. **Use in-memory SQLite** for testing: `Commerce::new(":memory:")`
2. **Never use production data** in development environments
3. **Review dependencies** before adding them

## Dependencies

We minimize dependencies and audit them regularly. Key dependencies:

| Dependency | Purpose | Security Notes |
|------------|---------|----------------|
| `rusqlite` | SQLite bindings | Uses bundled SQLite |
| `sqlx` | PostgreSQL async | Prepared statements prevent SQL injection; MySQL driver is disabled at the workspace level |
| `serde` | Serialization | No unsafe code |
| `uuid` | ID generation | Cryptographically random UUIDs |

## Known Advisories

### RUSTSEC-2023-0071 — Marvin Attack (rsa crate)

- **Affected crate**: `rsa` (transitive dependency of `sqlx-mysql`)
- **Severity**: Medium (CVSS 5.9) — timing sidechannel in RSA PKCS#1 v1.5 decryption
- **Upstream fix**: None available at the time of v1.0.0
- **Impact on this workspace**: **None at default settings.** `sqlx` is declared with `default-features = false` and the `mysql` feature is not enabled (`Cargo.toml` workspace dependency). The vulnerable code is compiled out.
- **Mitigation**: MySQL support is not a supported backend for StateSet iCommerce. Downstream crates and bindings must not enable `sqlx`'s `mysql` feature. CI enforces this via `cargo audit --ignore RUSTSEC-2023-0071` (`.github/workflows/ci.yml`) with the ignore justified by non-use rather than acceptance.
- **Recommended action**: Use SQLite (default) or PostgreSQL. If you need MySQL, evaluate the advisory independently before enabling the feature.

## Vulnerability History

No security vulnerabilities affecting the supported surface (SQLite/PostgreSQL, default features) have been reported to date. See "Known Advisories" above for non-impacting transitive findings.

---

Thank you for helping keep StateSet iCommerce and its users safe!
