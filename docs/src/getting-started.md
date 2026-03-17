# Getting Started

## Install

### Rust

```bash
cargo add stateset-embedded
```

### Node.js

```bash
npm install @stateset/embedded@0.8.0
```

### Python

```bash
pip install stateset-embedded==0.8.0
```

### CLI (global)

```bash
npm install -g @stateset/cli@0.8.0
stateset-init --quickstart
```

## Initialize

### Rust

```rust
use stateset_embedded::Commerce;

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

### Embedded Toolkit (OpenAI, Vercel AI SDK, LangChain)

```javascript
import { Commerce } from '@stateset/embedded';
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const commerce = new Commerce('./store.db');
const toolkit = createEmbeddedAgentToolkit({
    commerce,
    allowApply: false   // Read-only by default
});

// Get tools in OpenAI format
const tools = toolkit.getTools({ format: 'openai' });

// Execute a tool
const result = await toolkit.executeTool('list_customers');

// Simulate a write (preview without executing)
const preview = await toolkit.simulateMutation({
    tool: 'create_order',
    params: { customerId: 'cust-001', items: [...] }
});
```

### MCP (Claude Desktop, Cursor, Windsurf)

```bash
npx -y @stateset/cli@latest stateset-setup --yes --quickstart --db ./store.db
```

This registers the MCP server with your client. All 520+ tools appear automatically.

## Next Steps

- [Standalone Quickstart](standalone-quickstart.md) — Full 5-minute walkthrough with Shopify import and Stripe webhooks
- [AI Agent Quickstart](ai-agents.md) — Vercel AI SDK, LangChain, and OpenAI Responses API
- [Examples](examples.md) — End-to-end scenarios
- [API Reference](api/index.md) — Language-specific docs
- [What is iCommerce?](concepts/icommerce.md) — The paradigm shift
