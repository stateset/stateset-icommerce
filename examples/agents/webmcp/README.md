# StateSet WebMCP browser demo

Start the demo with Node 20.20 or newer from the repository root:

```bash
node examples/agents/webmcp/server.mjs
```

Open `http://127.0.0.1:8091`. The demo seeds a fresh in-memory product catalog
and exposes a curated checkout surface: `products.list`/`products.get` for
browsing, plus the cart tools needed to build a cart (`carts.create`,
`carts.addItem`, `carts.setShipping`, `carts.setPayment`, `carts.recalculate`)
and complete the order (`carts.beginCheckout`, `carts.complete`). Set `PORT`
to change the demo port.

The buttons walk the whole flow without WebMCP. **Build demo cart** assembles
a cart with shipping and payment; **Review cart** shows it; **Checkout** calls
`carts.complete` through the WebMCP wrapper, which pauses for buyer
confirmation first: the page fetches the order summary and asks the buyer to
approve. Only an explicit approval lets the order complete; declining leaves
the cart untouched. The same `requireConfirmation: ['carts.complete']` gate
is registered on the browser's `modelContext`, so a real agent driving these
tools hits the same approval step.

Cart writes apply because this is a trusted loopback demo (`allowApply: true`
on the server toolkit). Nothing here charges money; completing the order only
records it in the demo's in-memory store.

For local native WebMCP testing, enable
`chrome://flags/#enable-webmcp-testing` and relaunch Chrome. See
[Chrome's WebMCP setup guide](https://developer.chrome.com/docs/ai/webmcp#get_started).
The connector uses `document.modelContext`; older implementations can use
`navigator.modelContext` with explicit `unregisterTool` cleanup.

## Repeatable browser checks

Install the optional browser test dependency in this directory:

```bash
cd examples/agents/webmcp
npm install --no-save --package-lock=false puppeteer-core
```

With the demo still running, launch a separate Chromium instance with native
WebMCP and debugging enabled. Substitute your local Chromium/Chrome binary
if needed:

```bash
chromium --headless --disable-gpu \
  --enable-features=WebMCPTesting,DevToolsWebMCPSupport \
  --remote-debugging-address=127.0.0.1 --remote-debugging-port=9227 \
  --user-data-dir=/tmp/stateset-webmcp-chromium about:blank
```

Run the check from the repository root:

```bash
node examples/agents/webmcp/browser-smoke.mjs
```

Set `BROWSER_URL` or `BASE_URL` to use a different debugging endpoint or demo
address. The script connects to the existing browser, opens a temporary tab,
and closes that tab when finished. It checks native discovery, product list
and detail calls, visible results, write previews without execution, rollback
after a registration conflict, and cleanup that preserves unrelated tools.

To check the ordinary-browser fallback, launch a second Chromium instance
without the testing flag (use `--disable-features=WebMCPTesting`), with a
different profile and debugging port, then run:

```bash
EXPECT_WEBMCP=0 BROWSER_URL=http://127.0.0.1:9228 \
  node examples/agents/webmcp/browser-smoke.mjs
```

This checks that the page reports WebMCP as unavailable and its buttons still
drive the engine directly.

These checks passed against Chromium 153.0.8010.47. They invoke native WebMCP
programmatically; natural-language tool selection by a browser agent is a
separate integration check. For that check, Chrome documents its Model
Context Tool Inspector extension in the setup guide.

The demo is a local checkout example. A production application should
use its authenticated commerce routes and enforce permissions, argument
validation, record scope, and write policy on the backend. See the
[connector guide](../../../docs/src/ai-agents.md#webmcp-browser-connector).
