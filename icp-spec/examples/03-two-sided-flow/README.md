# Two-Sided Integration Demo

**The big claim:** ICP-1.0's trust model rests on two separate entities
(merchant Backend + Settler operator) running as separate processes with
separate signing keys, where every counterparty can independently verify
every signature against keys published in `.well-known/` endpoints.

**This demo proves the claim** by literally running both servers and
verifying every signature from outside both processes.

## Run it

```sh
cd icp-spec/examples/03-two-sided-flow
node demo.mjs
```

Zero deps. Stock Node 20+. Under 5 seconds. Spawns:
- `icp-handler/src/server.mjs` (merchant Backend) on a random port
- `services/settler-stateset/src/server.mjs` (Settler operator) on a
  random port

Then walks the full ICP-1.0 commerce lifecycle and writes the transcript
to `transcript.md`.

## What it verifies

10-step demonstration:

| Step | Outcome |
|---|---|
| 1 | Both servers spawned, ports announced |
| 2 | Each entity's `.well-known/` endpoint queried; **keys confirmed independent** |
| 3 | Buyer Agent generates its own keypair + AID per spec §4.2 |
| 4 | Buyer submits signed Intent → receives signed Quote |
| 5 | **Merchant signature verified independently** + tampered Quote rejected |
| 6 | Mock chain events injected into Settler (simulating Base Sepolia ICPEscrow.sol observation) |
| 7 | **Settler signature verified independently** + tampered EscrowEvent rejected |
| 8 | Full state-machine walk: fund → fulfill → release (3 monotonic-seq events) |
| 9 | Audit narrative: regulator presented with the receipt can independently verify everything |
| 10 | Summary |

## Why this matters

For a partner reviewing ICP, the critical question is: *"Can I trust
the protocol without trusting the parties?"*

The answer is yes, and this demo executes the proof:

- The merchant's Quote signature can be checked **against the merchant
  key published in the merchant's `.well-known/icp` endpoint** — by
  anyone, not just the buyer.
- The Settler's EscrowEvent signatures can be checked **against the
  Settler key published in the Settler's `.well-known/icp-settler`
  endpoint** — by anyone, including regulators, auditors, dispute
  counterparties.
- The buyer's Intent signature can be checked against the buyer's AID
  resolution — by the merchant, by the Settler, by anyone else who has
  the buyer's public key.

No counterparty needs to trust another counterparty's representation
of what was signed. **Only trust the public keys.**

This is the same property that gives Ethereum its credibility (transactions
verify against the signer's address regardless of the node you query) and
the same property that PCI-DSS, EMV, ISO 20022 provide for traditional
commerce rails. The agentic generation of commerce needs the same
guarantee. ICP-1.0 provides it.

## Sample output

The demo's transcript.md is gitignored (random keys each run), but here
is the load-bearing assertion sequence:

```
Merchant Quote signature verified independently: PASS ✓
Tampered Quote rejected by signature check:       PASS ✓
Settler EscrowEvent signature verified:           PASS ✓
Tampered EscrowEvent rejected:                    PASS ✓
```

If any of these fails, the demo exits non-zero. CI will block the merge.

## Production parallel

In production, the "mock chain event injection" in step 6 is replaced by
the Settler daemon's chain-mode subscriber watching `ICPEscrow.sol`
events on Base L2. Everything else — the architecture, the key
separation, the independent verifiability — is identical. The demo and
production paths diverge only at the rail-event source.
