# ADR-0002: Embedded SQLite as the Default Backend

- Status: Accepted
- Date: 2026-02-05

## Context

The product target includes edge devices, single-tenant deployments, and developers who want a "drop-in" commerce engine without external infrastructure. Defaulting to a server database would slow onboarding and complicate local development.

## Decision

Make SQLite the default backend with zero configuration. PostgreSQL remains available via a feature flag and builder configuration for production scale and multi-instance deployments.

## Consequences

- Fast local setup and predictable developer experience.
- A single binary can run end-to-end without external services.
- Some advanced deployments require switching to PostgreSQL and configuring pooling.
