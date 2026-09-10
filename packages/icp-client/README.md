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
| `principalIdentity` | PrincipalIdentity? | The principal's Ed25519 key. The client signs a real PrincipalBinding on every Intent. |
| `principalBinding` | PrincipalBinding? | A binding signed elsewhere (offline/KMS). Must delegate this client's AID. |

### Delegation — `principal_binding`

An Intent says "this Agent acts for that principal". A handler is entitled
to check it, and one running in enforcing trust mode does: it verifies the
binding against the principal key its **operator** registered
(`ICP_PRINCIPAL_KEYS_JSON`), never against key material from the request.

So the client needs the principal's signature. Two ways:

```js
import { generatePrincipalIdentity, signPrincipalBinding } from '@stateset/icp-client';

// (a) the client signs each binding — fine for tests and single-tenant agents
const principalIdentity = generatePrincipalIdentity();   // persist ed25519_seed
const client = await ICPClient.create({ handlerUrl, principal, principalIdentity });

// give the merchant: ICP_PRINCIPAL_KEYS_JSON={"did:web:my-store.example":"<hex>"}
principalIdentity.ed25519_pubkey.toString('hex');

// (b) sign offline / in a KMS and hand the Agent only the finished binding —
//     the production shape, since the Agent never holds the principal's key
const principalBinding = signPrincipalBinding(
  { principal, agent: identity.aid, verbs: ['purchase.create'], expiresAt: Date.now() + 86_400_000 },
  principalIdentity,
);
const agent = await ICPClient.create({ handlerUrl, principal, identity, principalBinding });
```

With **neither**, Intents carry no `principal_binding` at all and the
handler decides: an enforcing one answers `delegation.required`. The client
never fabricates a self-signed binding — a delegation an Agent could mint
for itself proves nothing.

The signing input is `canonicalJson(binding)` with the `signature` field
removed, so every other field — including `expiry` and `authority` — is
covered. Mutating one after signing yields `delegation.signature_invalid`.

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
