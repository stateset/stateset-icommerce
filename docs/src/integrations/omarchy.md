# Omarchy

StateSet iCommerce can run as a native part of Omarchy: a local commerce engine,
an MCP tool server for Omarchy's agents, and a shell widget for daily operations.
The store stays in one SQLite file and requires no hosted service.

## Install

From the directory containing your store:

```bash
npx -y -p @stateset/cli stateset-omarchy install --db ./store.db
```

This command:

- installs and enables the `com.stateset.icommerce` Omarchy shell plugin;
- adds Commerce actions to the Super-key Omarchy menu;
- records the absolute store location for the long-running shell process; and
- configures project-local MCP access for Claude, Codex, and OpenCode.

The bar widget shows store health, the active write mode, entity counts, and
operational attention signals for failed payments, low stock, pending returns,
and pending orders. Its panel opens the commerce agent, creates a consistent
backup, or launches the terminal dashboard. Right-click the widget to refresh
it. Desktop notifications are sent only when failed payments, low-stock items,
or pending returns increase after the initial status check; notifications can
be disabled in the widget settings.

Re-run the installer with `--force` to update the bundled plugin:

```bash
npx -y -p @stateset/cli stateset-omarchy install --db ./store.db --force
```

## Safety model

Agent writes are preview-only by default. The Omarchy shell plugin only reads
sanitized status JSON and launches fixed controller commands; it never opens or
mutates the database itself.

Governed apply mode requires all operator-owned identity and policy inputs:

```bash
npx -y -p @stateset/cli stateset-omarchy install \
  --db ./store.db --force --apply \
  --kernel-policy ./kernel-policy.json \
  --kernel-principal ./kernel-principal.json \
  --kernel-store-id store:production
```

The installer refuses partial apply configuration. These values are taken from
the local operator configuration, never from model arguments.

## Commands

```bash
stateset-omarchy status --json
stateset-omarchy dashboard
stateset-omarchy attention
stateset-omarchy remediate
stateset-omarchy doctor
stateset-omarchy agent
stateset-omarchy backup
stateset-omarchy configure --agent all
stateset-omarchy service install --port 8090
stateset-omarchy service status
stateset-omarchy service restart
stateset-omarchy service stop
stateset-omarchy service start
stateset-omarchy service remove
stateset-omarchy db-path
```

The plugin falls back to `npx -p @stateset/cli` when `stateset-omarchy` is not
globally installed. When it is installed, shell and menu actions use that local
binary first, so routine operation has no network dependency.

Review sanitized operational details without an API key or model provider:

```bash
stateset-omarchy attention
stateset-omarchy attention --kind failed-payments
stateset-omarchy attention --json
```

The panel's **Review** action opens the same local report. It shows at most five
sanitized examples per category and never includes customer contact or payment
amount data. **Resolve** opens the highest-severity alert in the matching
Payments, Inventory, Returns, or Orders specialist. The specialist is launched
in preview mode; writes still require a separate explicit `--apply` action.

```bash
stateset-omarchy remediate
stateset-omarchy remediate --kind low-stock
```

## Optional background MCP service

Install a loopback-only systemd user service when local applications need a
persistent Streamable HTTP endpoint:

```bash
stateset-omarchy service install --port 8090
```

The preview configuration adds `--read-only` explicitly. Governed apply mode
uses the same operator policy and identity recorded by the installer. To write
the unit without enabling it, add `--no-start`; `stateset-omarchy install
--service` also installs and starts it during initial setup.

The complete service lifecycle is operator-controlled through `service
status|start|stop|restart|remove`. Status also supports `--json` for scripts and
health checks.

Validate a target desktop after installation:

```bash
stateset-omarchy doctor
stateset-omarchy doctor --json
```

The doctor checks the database, saved configuration, installed manifest,
Omarchy CLI, notification support, and optional MCP user service. Required
failures produce a non-zero exit code, making the command suitable for machine
provisioning and acceptance checks.

## Build a standalone plugin

Maintainers can export the bundled shell files as an independently installable
Omarchy plugin directory:

```bash
npm run package:omarchy-plugin -- --output /tmp/stateset-omarchy-plugin
omarchy plugin add /tmp/stateset-omarchy-plugin
```

The exporter verifies that the plugin and CLI versions match and includes the
project licenses. `npm run check:omarchy` validates the manifest, entry points,
shell safety invariants, command allowlist, alert model, large-snapshot refresh
budget, diagnostics, and service controls. CI additionally validates the
artifact against a pinned upstream Omarchy revision and uploads an installable
plugin directory. CLI release builds produce an attested, checksummed standalone
plugin archive alongside the npm package.
