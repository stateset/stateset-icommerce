# StateSet iCommerce for Omarchy

The native Omarchy surface for
[StateSet iCommerce](https://github.com/stateset/stateset-icommerce). It adds a
commerce health widget and operator panel backed by a local, read-only status
service.

## Install with Omarchy

Install the version-matched controller explicitly, then add the plugin from its
public repository:

```bash
npm install --global @stateset/cli@1.28.4
omarchy plugin add https://github.com/stateset/stateset-omarchy-plugin.git --enable
```

The plugin contains no install hooks and does not request elevated privileges.
Omarchy clones and validates the QML before enabling it. Routine plugin use
never downloads or executes code: install the StateSet controller separately
before enabling the shell surface.

To configure a store, run this from its project directory:

```bash
stateset-omarchy install --db ./store.db
```

The configurator recognizes the Git-managed plugin and leaves its checkout
intact, so `omarchy plugin update com.stateset.icommerce` remains the owner of
shell upgrades.

Requirements are Omarchy Quattro with shell plugin support and a separately
installed StateSet CLI on Node.js 20.20 or newer. If the controller is missing,
the widget fails closed and displays a bounded installation error.

Upgrade atomically with `--force`. If the updated plugin cannot be enabled, the
installer restores the previous plugin. Remove the shell integration with
`stateset-omarchy uninstall`; store data, StateSet configuration, and agent
configuration are retained.

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
Shell and menu actions require the locally installed controller and never fetch
packages at runtime.

## Remove

Remove a native Git installation through Omarchy, then clean up the optional
StateSet menu and user service:

```bash
omarchy plugin remove com.stateset.icommerce
stateset-omarchy uninstall --no-disable
```

Store data, StateSet configuration, and project agent configuration are
retained.

## Security

Commerce writes remain preview-only unless an operator explicitly configures
governed apply mode with kernel policy, principal, and store identity files.
The plugin never receives those identities as model arguments. Review
[the trust model](https://github.com/stateset/stateset-icommerce/blob/master/TRUST_FOUNDATION.md)
before enabling mutations.

## Development

The source of truth is
[`cli/omarchy`](https://github.com/stateset/stateset-icommerce/tree/master/cli/omarchy).
Release automation opens synchronization PRs in this standalone repository so
that plugin updates remain reviewable. Validate a checkout with:

```bash
omarchy plugin validate .
```
