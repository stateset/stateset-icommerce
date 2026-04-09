# WASM Connectors

iCommerce supports a WASM-based connector system for extensibility. Third-party integrations — custom shipping calculators, tax engines, fraud rules, loyalty providers — can be packaged as WASM modules and installed from a local marketplace.

## Architecture

```
┌─────────────────────────────────────────────┐
│  Connector Marketplace (Local Catalog)       │
│                                               │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │ Freight  │  │ Tax      │  │ Loyalty  │  │
│  │ Calc v1  │  │ Engine   │  │ Bridge   │  │
│  │ (.wasm)  │  │ (.wasm)  │  │ (.wasm)  │  │
│  └──────────┘  └──────────┘  └──────────┘  │
└──────────────────────┬──────────────────────┘
                       │ install
                       ▼
┌─────────────────────────────────────────────┐
│  WASM Runtime (native-export or wasi-command)│
│                                               │
│  - Memory limits enforced                     │
│  - Timeout per action                         │
│  - Sandboxed execution                        │
└──────────────────────────────────────────────┘
```

## Runtime Kinds

| Kind | Description | Best For |
|------|-------------|----------|
| `native-export` | Call exported WASM functions directly | Pure computation (pricing, scoring) |
| `wasi-command` | Run as a WASI command-line program | I/O, file access, complex logic |

## Publishing a Connector

```javascript
await toolkit.executeTool('publish_wasm_connector', {
    connectorId: 'freight-calculator',
    version: '1.0.0',
    name: 'Custom Freight Calculator',
    description: 'Calculates freight rates based on weight, dimensions, and destination zone',
    wasmPath: './connectors/freight-calc.wasm',
    runtimeKind: 'native-export',
    tags: ['shipping', 'freight', 'rates'],
    actions: [
        {
            name: 'calculate_rate',
            description: 'Calculate shipping rate for a package',
            exportName: 'calculate_rate',
            args: ['weight_kg', 'length_cm', 'width_cm', 'height_cm', 'zone_id'],
            timeoutMs: 5000,
        },
    ],
});
```

## Browsing the Marketplace

```javascript
// List all available connectors
const catalog = await toolkit.executeTool('list_connector_marketplace', {});

// Filter by tag
const shipping = await toolkit.executeTool('list_connector_marketplace', {
    tag: 'shipping',
});

// Search by name/description
const results = await toolkit.executeTool('list_connector_marketplace', {
    query: 'tax calculation',
});
```

## Installing & Executing

```javascript
// Install a connector
await toolkit.executeTool('install_wasm_connector', {
    connectorId: 'freight-calculator',
    version: '1.0.0',
});

// Execute an action
const rate = await toolkit.executeTool('execute_connector_action', {
    connectorId: 'freight-calculator',
    action: 'calculate_rate',
    input: {
        weight_kg: 5.2,
        length_cm: 40,
        width_cm: 30,
        height_cm: 20,
        zone_id: 'US-WEST',
    },
});
// → { rate: 12.99, currency: 'USD', carrier: 'USPS', estimatedDays: 5 }
```

## Safety Assessment

Before installation, assess a connector's safety profile:

```javascript
const safety = await toolkit.executeTool('assess_connector_safety', {
    connectorId: 'freight-calculator',
});
// → {
//     runtimeKind: 'native-export',
//     memoryLimitMb: 64,
//     hasNetworkAccess: false,
//     hasFileAccess: false,
//     maxTimeoutMs: 5000,
//     actionCount: 1,
//     risk: 'low',
// }
```

## Certification & Attestation

Connectors can be signed and certified for trust:

```javascript
// Sign attestation
await toolkit.executeTool('sign_connector_attestation', {
    connectorId: 'freight-calculator',
    version: '1.0.0',
    signerKey: '0x...',
});

// Verify attestation
const verified = await toolkit.executeTool('verify_connector_attestation', {
    connectorId: 'freight-calculator',
    version: '1.0.0',
});
// → { valid: true, signer: '0x...', signedAt: '2026-03-17T...' }

// Certify (admin operation)
await toolkit.executeTool('certify_wasm_connector', {
    connectorId: 'freight-calculator',
    version: '1.0.0',
    certificationLevel: 'verified',
});
```

## Uninstalling

```javascript
await toolkit.executeTool('uninstall_wasm_connector', {
    connectorId: 'freight-calculator',
});
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `list_connector_marketplace` | Browse available connectors |
| `publish_wasm_connector` | Publish a WASM connector |
| `install_wasm_connector` | Install a connector |
| `uninstall_wasm_connector` | Remove a connector |
| `execute_connector_action` | Run a connector action |
| `assess_connector_safety` | Check safety profile |
| `sign_connector_attestation` | Sign connector for trust |
| `verify_connector_attestation` | Verify connector signature |
| `certify_wasm_connector` | Certify a connector (admin) |
| `get_installed_connector` | Get installed connector details |
| `list_installed_connectors` | List all installed connectors |
