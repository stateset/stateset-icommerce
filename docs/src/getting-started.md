# Getting Started

## Install

### Rust

```bash
cargo add stateset-sdk --features full
```

Use `stateset-sdk` for the recommended Rust entry point. Reach for
`stateset-embedded` directly when you specifically want the lower-level core
crate without the facade re-exports.

### Node.js

```bash
npm install @stateset/embedded@1.23.5
```

### Python

```bash
pip install stateset-embedded==1.23.5
# or install optional framework adapters as well
pip install "stateset-embedded[agents]==1.23.5"
```

### CLI (global)

```bash
npm install -g @stateset/cli@1.23.5
stateset-init --quickstart
```

## Initialize

### Rust

```rust
use stateset_sdk::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let commerce = Commerce::new("./store.db")?;

    // Create a customer
    let customer = commerce.customers().create(CreateCustomer {
        email: "alice@example.com".into(),
        first_name: "Alice".into(),
        last_name: "Smith".into(),
        ..Default::default()
    })?;

    println!("Created: {}", customer.email);
    Ok(())
}
```

### Node.js

```javascript
import { Commerce } from '@stateset/embedded';

const commerce = new Commerce('./store.db');

// Create a customer
const customer = commerce.customers.create({
    email: 'alice@example.com',
    firstName: 'Alice',
    lastName: 'Smith'
});

console.log(`Created: ${customer.email}`);
```

### Python

```python
from stateset_embedded import Commerce

commerce = Commerce("./store.db")

customer = commerce.customers.create(
    email="alice@example.com",
    first_name="Alice",
    last_name="Smith"
)

print(f"Created: {customer.email}")
```

## Use the CLI

Tip: `ss` is a shorthand alias for `stateset`.

Read-only by default:

```bash
stateset "show me pending orders"
stateset "what products are low on stock?"
stateset "what is my revenue this month?"
```

Apply writes explicitly:

```bash
stateset --apply "create a customer named Alice with email alice@example.com"
stateset --apply "ship order #12345 with tracking FEDEX123"
```

Optional: enable hybrid vector search (semantic + BM25):

```bash
export OPENAI_API_KEY=sk-...
stateset "find products similar to wireless earbuds"
```

## AI Agents

### Embedded Toolkit (OpenAI, Vercel AI SDK, LangChain, Python runtimes)

```bash
npm install @stateset/cli@1.23.5 @stateset/embedded@1.23.5
```

```javascript
import { Commerce } from '@stateset/embedded';
import { createOpenAITools, executeOpenAIToolCall } from '@stateset/embedded/openai';
import { createToolDescriptors } from '@stateset/embedded/generic';

const commerce = new Commerce('./store.db');
const tools = createOpenAITools(commerce, {
    filter: ['list_customers']
});
const execution = await executeOpenAIToolCall(commerce, {
    call_id: 'demo_call_1',
    function: {
        name: 'list_customers',
        arguments: '{}'
    }
});
const descriptors = createToolDescriptors(commerce, {
    filter: ['list_customers', 'list_orders', 'get_sales_summary']
});
```

Use `@stateset/embedded/agent-toolkit` when you need the full advanced runtime:
preview-mode write simulation, priced-tool helpers, delegation through
`autonomousEngine`, or planning/replay APIs.

For Python agent runtimes:

```python
from stateset_embedded import Commerce, create_embedded_agent_toolkit

commerce = Commerce(":memory:")
toolkit = create_embedded_agent_toolkit(commerce, allow_apply=False)
tools = toolkit.get_tools(format="openai")
```

For framework-first Python hosts:

```python
from stateset_embedded.generic import create_tool_descriptors, create_callable_registry
from stateset_embedded.openai import create_openai_tools, execute_openai_tool_call
from stateset_embedded.langchain import create_langchain_tools
from stateset_embedded.crewai import create_crewai_tools
from stateset_embedded.autogen import create_autogen_tools
```

Runnable examples for those paths live under `examples/python/`:
`openai_tools.py`, `generic_tools.py`, `langchain_tools.py`, `crewai_tools.py`, and
`autogen_tools.py`.

If your runtime needs agent-to-agent delegation, pass `autonomousEngine` and turn on `allowApply: true`; `delegate_to_agent` remains preview-only until writes are enabled.

### MCP (Claude Desktop, Cursor, Windsurf)

```bash
npx -y @stateset/cli@latest stateset-setup --yes --quickstart --db ./store.db
```

This registers the MCP server with your client. The full registry-generated tool inventory appears automatically.

## Next Steps

- [Standalone Quickstart](standalone-quickstart.md) — Full 5-minute walkthrough with Shopify import and Stripe webhooks
- [AI Agent Quickstart](ai-agents.md) — Vercel AI SDK, LangChain, and OpenAI Responses API
- [Examples](examples.md) — End-to-end scenarios
- [API Reference](api/index.md) — Language-specific docs
- [What is iCommerce?](concepts/icommerce.md) — The paradigm shift
