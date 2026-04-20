# Agent Framework Examples

Runnable examples showing how to embed the iCommerce engine inside agent
frameworks and agent-like runtimes.

The embedded toolkit examples run from a raw repo checkout and also work after
installing the published packages. They load published packages first and fall
back to workspace modules when you are developing inside this repository.

## Embedded Runtime Examples

| File | What it demonstrates |
|------|----------------------|
| `openai-embedded-toolkit.mjs` | OpenAI-style tool definitions plus `executeOpenAIToolCall()` round-tripping |
| `custom-framework-adapter.mjs` | Generic `{ name, description, schema, execute }` descriptors for custom runtimes |
| `framework-adapters.mjs` | Minimal Vercel AI SDK and LangChain adapter patterns |

## x402 Agent Demo Flows

Runnable demo flows showing how agents in the iCommerce codebase can use x402.

## Flows

| File | What it demonstrates |
|------|----------------------|
| `x402-exact-http-flow.mjs` | Buyer agent pays a seller agent over local HTTP using exact x402 v2 helpers and unlocks a paid response |
| `x402-local-intent-flow.mjs` | Buyer and seller agent cards create, sign, and settle a local x402 payment intent through the native embedded binding |
| `x402-credit-ledger-flow.mjs` | Agent pre-funds an x402 credit ledger and consumes it across metered debits through the native embedded binding |
| `x402-demo-flows.mjs` | Runs all three demo flows sequentially |

## Run

From the repo root:

```bash
~/.nvm/versions/node/v20.20.0/bin/node examples/agents/x402-demo-flows.mjs
```

Or run each flow individually:

```bash
~/.nvm/versions/node/v20.20.0/bin/node examples/agents/openai-embedded-toolkit.mjs
~/.nvm/versions/node/v20.20.0/bin/node examples/agents/custom-framework-adapter.mjs
~/.nvm/versions/node/v20.20.0/bin/node examples/agents/framework-adapters.mjs
~/.nvm/versions/node/v20.20.0/bin/node examples/agents/x402-exact-http-flow.mjs
~/.nvm/versions/node/v20.20.0/bin/node examples/agents/x402-local-intent-flow.mjs
~/.nvm/versions/node/v20.20.0/bin/node examples/agents/x402-credit-ledger-flow.mjs
```

## Notes

- The embedded examples are the shortest path to framework adoption of the
  engine itself.
- `embedded-toolkit-runtime.mjs` keeps the examples runnable both from the
  workspace and from published packages.
- `custom-framework-adapter.mjs` demonstrates the framework-neutral descriptor
  surface exposed by `createToolDescriptors()`.
- The paid HTTP flow uses the exact x402 v2 helpers already shipped in `cli/src/x402/`.
- The local intent and credit-ledger flows call `commerce.x402` directly so the examples match the native binding API.
- The exact-flow demo uses simulated settlement so it can run locally without a live facilitator or chain.
