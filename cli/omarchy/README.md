# StateSet iCommerce for Omarchy

This is the native Omarchy shell surface bundled with `@stateset/cli`. It adds
a StateSet bar widget and popup operator panel backed by a shared, read-only
status service.

Install from a store project:

```bash
npx -y -p @stateset/cli stateset-omarchy install --db ./store.db
```

The installer adds the widget, Commerce entries in the Omarchy menu, and local
MCP configuration for Claude, Codex, and OpenCode. The widget surfaces failed
payments, low stock, pending returns, and pending orders, with optional desktop
notifications when actionable conditions increase. MCP writes remain in
preview mode. See the main iCommerce documentation for governed apply mode.

The QML plugin runs only `stateset-omarchy status --json` and explicit commands
selected by the operator. It does not read credentials, edit the commerce
database, or accept model-supplied shell commands.

The optional loopback MCP service supports explicit `status`, `start`, `stop`,
`restart`, and `remove` lifecycle actions through `stateset-omarchy service`.
Use `stateset-omarchy attention` for a sanitized, provider-free operations
report, `stateset-omarchy remediate` to open the matching preview-only
specialist, and `stateset-omarchy doctor` to verify a target desktop installation.
Shell and menu actions prefer the locally installed controller and use `npx`
only as a fallback.
