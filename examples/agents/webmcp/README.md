# StateSet WebMCP browser demo

Start the demo with Node 20.20 or newer from the repository root:

```bash
node examples/agents/webmcp/server.mjs
```

Open `http://127.0.0.1:8091`. The demo seeds a fresh in-memory product catalog
and exposes only `products.list` and `products.get`. The catalog button works
without WebMCP. Set `PORT` to change the demo port.

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

This checks that the page reports WebMCP as unavailable and its catalog button
still retrieves products from the engine.

These checks passed against Chromium 153.0.8010.47. They invoke native WebMCP
programmatically; natural-language tool selection by a browser agent is a
separate integration check. For that check, Chrome documents its Model
Context Tool Inspector extension in the setup guide.

The demo is a local public-catalog example. A production application should
use its authenticated commerce routes and enforce permissions, argument
validation, record scope, and write policy on the backend. See the
[connector guide](../../../docs/src/ai-agents.md#webmcp-browser-connector).
