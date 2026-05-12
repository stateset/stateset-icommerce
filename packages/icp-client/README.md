# @stateset/icp-client

TypeScript-ergonomic client SDK for the Intelligent Commerce Protocol (ICP-1.0).

**Zero runtime dependencies.** Uses only `node:crypto` and `fetch` (Node 20+).

## Install

```sh
npm install @stateset/icp-client
```

## Use

```js
import { ICPClient } from '@stateset/icp-client';

// 1. Create a client (fresh identity, persistable for production)
const client = await ICPClient.create({
  handlerUrl: 'https://merchant.example/icp',
  principal: 'did:web:my-store.example',
});

// 2. Discover the merchant's capabilities + cache their public key
const caps = await client.capabilities();

// 3. Browse inventory (signed snapshot, verified against merchant pubkey)
const stock = await client.inventory({
  merchant: caps.merchant_aid,
  settler: 'settler:circle.usdc.base',
  skus: [{ sku: 'WIDGET-001', quantity: 1 }],
  filters: { in_stock_only: true },
});

// 4. Make a purchase (returns a signed Quote)
const order = await client.purchase({
  merchant: caps.merchant_aid,
  settler: 'settler:circle.usdc.base',
  items: [{ sku: 'WIDGET-001', quantity: 1, unit_price: { amount: '29.99', currency: 'USDC' } }],
  max_total: { amount: '35.00', currency: 'USDC' },
});

// 5. Accept the Quote → get on-chain funding instructions
const funding = await client.accept(order.quote.quote_id);
// → buyer wallet signs + broadcasts funding.funding.args to the chain

// 6. Observe escrow state via SSE
for await (const event of client.observe(funding.funding.escrow_id)) {
  console.log(event.from_state, '→', event.to_state);
}

// 7. Audit replay: fetch the SettlementReceipt by id
const receipt = await client.settlement('icp_set_01HXYZ...');
```

## API

### `ICPClient.create(options): Promise<ICPClient>`

| Option | Type | Description |
|---|---|---|
| `handlerUrl` | string | Base URL of the ICP HTTP handler |
| `principal` | string | Principal identifier (DID, LEI, etc.) |
| `identity` | Identity? | Pre-existing identity. Default: generate fresh. |
| `verbs` | string[]? | PrincipalBinding authority.verbs. Default: all 4 ICP-1.0 verbs. |
| `maxPerIntent` | Money? | Authority cap. Default: $10,000 USDC. |
| `revocationUrl` | string? | Where revocation can be checked. |

### Methods

| Method | Returns |
|---|---|
| `client.capabilities()` | merchant's `.well-known/icp` doc |
| `client.inventory({merchant, settler, skus?, filters?, max_results?})` | signed InventorySnapshot |
| `client.purchase({merchant, settler, items, max_total, ship_to?})` | signed Quote |
| `client.accept(quote_id)` | EscrowFunding instructions |
| `client.subscribe({merchant, settler, service_id, cadence, max_total_per_period, ...})` | signed SubscriptionAuthorization |
| `client.return_({merchant, settler, original_settlement_id, items, desired_outcome, max_refund?})` | signed ReturnAuthorization |
| `client.observe(escrowId)` | async iterator over EscrowEvents (SSE) |
| `client.settlement(settlementId)` | SettlementReceipt |

Every merchant response is **independently verified** against the public
key from the merchant's `.well-known/icp` discovery document. A
verification failure throws `ICPError('signature.invalid', ...)` —
do NOT rely on responses where verification was skipped.

### Errors

The SDK throws typed `ICPError` instances. Branch on `.code`:

```js
try {
  await client.purchase({ ... });
} catch (err) {
  if (err.code === 'policy.settler.not_allowed') {
    // Re-route through a different Settler
  } else if (err.code === 'policy.quote.exceeds_max_total') {
    // Renegotiate or reject
  } else if (err.code === 'signature.invalid') {
    // Merchant signature didn't verify — protocol fraud or misconfig
  } else {
    throw err;
  }
}
```

Full error code enumeration: `icp-spec/schemas/error-codes.md`.

### Identity persistence

For production deployments, persist the identity rather than generating
fresh on each run:

```js
import { generateIdentity, identityFromSeeds } from '@stateset/icp-client';
import { readFileSync, writeFileSync } from 'node:fs';

// First run: generate + persist
const id = generateIdentity();
writeFileSync('keypair.json', JSON.stringify({
  ed25519_seed: id.ed25519_seed.toString('hex'),
  x25519_seed: id.x25519_seed.toString('hex'),
  aid: id.aid,
}));

// Subsequent runs: restore
const saved = JSON.parse(readFileSync('keypair.json', 'utf8'));
const id = identityFromSeeds(
  Buffer.from(saved.ed25519_seed, 'hex'),
  Buffer.from(saved.x25519_seed, 'hex'),
);
const client = await ICPClient.create({ ..., identity: id });
```

In production, seeds should be held in a KMS or HSM, not on disk.

## Test

```sh
node --test test/client.test.mjs
```

The test spawns a live `icp-handler` server, exercises every public
method, asserts merchant signatures verify, and runs a full
purchase → accept → fulfill → settlement lifecycle. **11/11 PASS.**

## License

MIT OR Apache-2.0 (your choice).
