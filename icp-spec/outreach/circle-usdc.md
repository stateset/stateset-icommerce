**To:** Circle — Programmable Wallets / Developer Platform
**Subject:** USDC on Base named as first ICP reference Settler — would Circle operate it?

Hi [name],

We just published ICP-1.0-DRAFT (Intelligent Commerce Protocol), an open
spec for the multi-step lifecycle of agent-driven commerce — quote,
escrow, fulfillment, dispute, settlement. CC-BY-4.0 spec, Apache-2.0
schemas, royalty-free patent grant.

In the spec, value moves through entities called **Settlers**. ICP names
USDC on Base as the **first reference Settler** (`settler:circle.usdc.base`)
because it has the right characteristics: regulated MTL operator,
sub-cent fees, ~2-second blocktime, agent-readable, with a clean fiat
on/off-ramp story.

The reference binding is fully specified at
`icp-spec/settlers/usdc-base.md` — including the on-chain ICPEscrow
contract, lifecycle hooks, SettlementReceipt format, proof-of-reserves
attestation expectations, SLAs, and failure modes.

Until Circle operates this Settler, we'll bootstrap from a StateSet-
operated **testnet-only** Settler at `settler:stateset.usdc.base-sepolia`
that's clearly marked non-production. Real-dollar volume can't move
through ICP until a regulated operator runs the mainnet binding.

**One ask:** could the right Circle team review `settlers/usdc-base.md`
and tell us:

1. Is the contract design (proxy + 5-of-9 Safe + 48h timelock + Chainlink
   POR) close to what Circle would actually deploy, or are there material
   things we got wrong from the operator side?
2. Is Circle willing to be the named operator of `settler:circle.usdc.base`
   when the spec ratifies (target Q4 2026)? If yes, what's needed from
   us — IP grant signature, integration support, security review?
3. Are there other Circle-operated rails we should write reference
   bindings for in the same release? (`circle.usdc.ethereum`,
   `circle.eurc.base`, etc.)

Spec link, contract draft, and a working AID/sign demo (~200 lines of
zero-dep Node.js, runs in 5 seconds) attached.

The agentic-commerce stack is being decided right now. If USDC is the
default value rail for agent transactions — and the demo unit economics
say it should be — then the Settler interface IS that decision in code.
We'd rather spec it with you than around you.

— Dom Steil
StateSet, Inc.
dom@stateset.com
