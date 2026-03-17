# Embedded Agent Toolkit

The embedded agent toolkit enables programmatic integration of iCommerce with any AI framework — OpenAI, Vercel AI SDK, LangChain, or custom agent runtimes. Instead of going through the CLI, your application embeds the full 520+ tool surface directly.

## Quick Start

```javascript
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const toolkit = createEmbeddedAgentToolkit({
    dbPath: './store.db',
    allowApply: true,
});

// Get all tools in OpenAI format
const tools = toolkit.getTools({ format: 'openai' });

// Execute a tool directly
const result = await toolkit.executeTool('list_orders', { status: 'pending' });
```

## Supported Formats

The toolkit produces tool definitions in multiple formats for different AI frameworks:

| Format | Framework | Output Shape |
|--------|-----------|-------------|
| `openai` | OpenAI API, GPT-4 | `{ type: "function", function: { name, description, parameters } }` |
| `vercel` | Vercel AI SDK | `{ toolName, description, inputSchema }` |
| `langchain` | LangChain | `{ name, description, schema }` with Zod schemas |
| `generic` | Custom runtimes | `{ name, description, inputSchema }` |

```javascript
// OpenAI format
const openaiTools = toolkit.getTools({ format: 'openai' });

// Vercel AI SDK format
const vercelTools = toolkit.getTools({ format: 'vercel' });

// LangChain format
const langchainTools = toolkit.getTools({ format: 'langchain' });
```

## OpenAI Integration

```javascript
import OpenAI from 'openai';
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const openai = new OpenAI();
const toolkit = createEmbeddedAgentToolkit({ dbPath: './store.db' });
const tools = toolkit.getTools({ format: 'openai' });

// Send tools to OpenAI
const response = await openai.chat.completions.create({
    model: 'gpt-4o',
    messages: [{ role: 'user', content: 'Show me pending orders' }],
    tools,
});

// Execute tool calls from the response
for (const toolCall of response.choices[0].message.tool_calls || []) {
    const result = await toolkit.executeOpenAIToolCall(toolCall);
    console.log(result);
}
```

## Vercel AI SDK Integration

```javascript
import { generateText } from 'ai';
import { openai } from '@ai-sdk/openai';
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const toolkit = createEmbeddedAgentToolkit({ dbPath: './store.db' });
const tools = toolkit.getTools({ format: 'vercel' });

const result = await generateText({
    model: openai('gpt-4o'),
    tools,
    prompt: 'What products are low on stock?',
});
```

## LangChain Integration

```javascript
import { ChatOpenAI } from '@langchain/openai';
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const toolkit = createEmbeddedAgentToolkit({ dbPath: './store.db' });
const tools = toolkit.getTools({ format: 'langchain' });

const model = new ChatOpenAI({ model: 'gpt-4o' });
const modelWithTools = model.bindTools(tools);
const response = await modelWithTools.invoke('List all customers');
```

## MCP Server Integration

For Claude Desktop, Cursor, or any MCP-compatible client:

```javascript
import { createStatesetMcpServer } from '@stateset/cli/mcp-server';

const server = createStatesetMcpServer({
    dbPath: './store.db',
    allowApply: false,  // read-only by default
});

// The server exposes 520+ tools via the MCP protocol
await server.start();
```

See the [AI Agent Quickstart](../ai-agents.md) for full MCP configuration.

## Direct Tool Execution

Execute tools without an AI framework:

```javascript
const toolkit = createEmbeddedAgentToolkit({
    dbPath: './store.db',
    allowApply: true,
});

// List orders
const orders = await toolkit.executeTool('list_orders', { status: 'pending' });

// Create a customer (requires allowApply)
const customer = await toolkit.executeTool('create_customer', {
    name: 'Alice',
    email: 'alice@example.com',
});

// Run analytics
const revenue = await toolkit.executeTool('sales_summary', { period: 'month' });
```

## Configuration Options

```javascript
const toolkit = createEmbeddedAgentToolkit({
    dbPath: './store.db',       // SQLite database path
    allowApply: false,           // Enable write operations (default: false)
    commerce: existingInstance,   // Re-use an existing Commerce instance
    policiesDir: './policies/',   // Policy YAML directory
});
```

| Option | Default | Description |
|--------|---------|-------------|
| `dbPath` | `./store.db` | Path to SQLite database |
| `allowApply` | `false` | Enable write operations |
| `commerce` | — | Existing Commerce instance (skip DB init) |
| `policiesDir` | `./policies/` | Policy YAML directory |

## Tool Filtering

Get a specific tool or subset:

```javascript
// Get a single tool definition
const tool = toolkit.getTool('create_order', { format: 'openai' });

// Get raw tool definitions (before format conversion)
const rawTools = toolkit.getRawTools();
```

## Error Handling

Tool execution returns structured errors:

```javascript
const result = await toolkit.executeTool('create_order', { customerId: 'invalid' });
// → {
//     success: false,
//     error: 'Customer not found',
//     details: { customerId: 'invalid' }
// }
```

When `allowApply` is `false`, write operations return a preview:

```javascript
const result = await toolkit.executeTool('create_customer', { name: 'Alice', email: 'alice@example.com' });
// → {
//     success: false,
//     error: 'Creating customer requires --apply flag.',
//     wouldCreate: { name: 'Alice', email: 'alice@example.com' }
// }
```

## Python Integration

Using the Rust bindings via PyO3:

```python
from stateset import Commerce

commerce = Commerce("./store.db")
orders = commerce.orders.list(status="pending")
```

For AI frameworks like LangChain Python, use the Node.js toolkit as an MCP server and connect via the MCP protocol. See the [Python binding docs](../api/python.md).
