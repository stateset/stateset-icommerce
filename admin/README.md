# StateSet Admin Dashboard

Local admin dashboard for StateSet iCommerce operations.

## Environment

- `NEXT_PUBLIC_STATESET_API_URL`: browser-visible API base URL used by client fetches and CSP.
- `STATESET_API_URL`: server-side API base URL used by Next route handlers.
- `STATESET_API_TOKEN`: optional server-side token used for admin session and agent-session API access.
- `STATESET_ADMIN_DISABLE_AUTH`: dev-only bypass that disables the login gate for local admin use. It is ignored in production.
- `STATESET_ADMIN_TRUST_PROXY_HEADERS`: trusts `x-forwarded-for` / `x-real-ip` for rate limiting. Enable only behind a trusted proxy that strips client-supplied forwarded headers.
- `STATESET_ADMIN_ALLOW_MOCK_DATA`: explicit mock embedded-engine fallback for demos and tests. It is rejected in production.

Server route precedence is:

1. `STATESET_API_URL`
2. `NEXT_PUBLIC_STATESET_API_URL`
3. `https://api.sandbox.stateset.app`

Set both API URL vars to the same value in deployments to avoid browser/server drift.

## Local Development

```bash
npm run dev
```

The local workspace currently enables login bypass through `.env.local` for fast local admin access. Keep `STATESET_ADMIN_DISABLE_AUTH=false` in shared environments. Production ignores the bypass flag and requires a real session.
