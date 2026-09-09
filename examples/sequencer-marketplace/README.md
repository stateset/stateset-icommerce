# Multi-Agent Marketplace over StateSet Sequencer

This demo uses the StateSet Sequencer as a shared, globally ordered message
board for autonomous economic agents. A buyer opens an RFQ, three merchants
bid, the buyer counters, one merchant accepts, and payment and commerce agents
record reservation, settlement, order, and receipt events.

The messages are generic JSON inside the sequencer's existing event envelope.
Their `entity_type` is `marketplace.negotiation`, their `entity_id` is the
auction ID, and their namespaced event types form the protocol:

```text
marketplace.rfq.opened
marketplace.bid.submitted
marketplace.counteroffer.created
marketplace.counteroffer.accepted
marketplace.award.created
commerce.inventory.reserved
commerce.settlement.authorized
commerce.order.created
economic.receipt.issued
```

This makes the board interoperable without treating free-form prose as an
authorization. Every message includes a protocol version, sender, recipients,
conversation ID, reply link, structured commercial terms, and an Ed25519
signature. Amounts are exact decimal strings and bid comparison uses integer
minor units rather than floating-point arithmetic. Fiat order totals and
non-fiat settlement assets remain distinct contracts.

## Execute awards through the kernel

`@stateset/cli/marketplace` exports `KernelMarketplaceBridge`. A buyer-side
worker turns its signed award into `a2a.escrow.create`; a merchant-side worker
turns the same award into `inventory.reserve`. Each side uses its own trusted
identity and deny-by-default policy:

```js
import { Commerce } from '@stateset/embedded';
import {
  KernelMarketplaceBridge,
  SqliteBridgeStore,
  createAwardCommandPlanner,
} from '@stateset/cli/marketplace';

const bridge = new KernelMarketplaceBridge({
  id: 'acme-buyer-worker',
  sequencer,
  commerce: new Commerce('./store.db'),
  store: new SqliteBridgeStore(stateDatabase),
  identity: operatorOwnedIdentity,
  policy: operatorOwnedPolicy,
  registry: operatorOwnedAgentRegistry,
  planner: createAwardCommandPlanner({ side: 'buyer' }),
  publishReceipt,
});

await bridge.pollOnce();
```

The durable inbox advances only after receipt publication. If execution
succeeds but publication fails, the worker retries the identical command ID and
idempotency key; the kernel returns the sealed receipt without committing the
transaction twice. Receipt publication also receives a deterministic event ID,
so the sequencer can deduplicate a publish-then-crash retry. Invalid signatures,
unknown agents, cross-tenant/store events, planner scope escalation, and
modified event replays fail closed.

## Run against the real sequencer

Start `/home/dom/icommerce-app/stateset-sequencer` on port 8080, then run:

```bash
STATESET_SEQUENCER_URL=http://localhost:8080 \
STATESET_SEQUENCER_API_KEY=dev_admin_key \
node examples/sequencer-marketplace/demo.mjs
```

The defaults match the sequencer's local Docker Compose configuration. Override
`STATESET_TENANT_ID` and `STATESET_STORE_ID` for another scoped board.
Add `--json` to emit the complete machine-readable transcript.

For a deterministic protocol/state-machine check without a server:

```bash
node examples/sequencer-marketplace/demo.mjs --self-test
```

After building the workspace Node binding, add `--kernel` to execute the award
against two independent embedded databases. The buyer kernel creates real A2A
escrow, the merchant kernel reserves real inventory, and both publish their
sealed execution receipts back to the board:

```bash
(cd bindings/node && npm run build)
node examples/sequencer-marketplace/demo.mjs --self-test --kernel
```

Production agents should additionally use signed VES envelopes and registered
transport keys. The demo signs the marketplace message itself, so consumers
can authenticate commercial terms even when using the legacy ingestion
endpoint. That endpoint keeps the demo dependency-light while retaining
sequencing, idempotency, tenant/store isolation, payload hashing, persistence,
and authenticated access.
