# AI Agent Quickstart

StateSet supports two onboarding modes for agents:

- Embedded toolkit for OpenAI-style/server-side runtimes
- MCP setup for Claude Desktop, Cursor, Windsurf, and other MCP-native clients

## Install

```bash
npm install @stateset/cli@0.7.27 @stateset/embedded@0.7.27
```

## Embedded Toolkit

```javascript
import { Commerce } from '@stateset/embedded';
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const commerce = new Commerce('./store.db');
const toolkit = createEmbeddedAgentToolkit({
  commerce,
  allowApply: false,
});

const tools = toolkit.getTools({ format: 'openai' });
const result = await toolkit.executeTool('list_customers');
```

`getTools({ format: 'openai' })` returns JSON-schema function tools.
`executeTool()` runs those tools against the same policy, permission, replay, and audit runtime used by the MCP server.

## Vercel AI SDK

```javascript
import { tool } from 'ai';

const tools = toolkit.createVercelAITools({
  tool,
  filter: ['list_customers', 'get_order'],
});
```

This returns the object shape expected by `streamText()` / `generateText()`.

## LangChain / LangGraph

```javascript
import { DynamicStructuredTool } from '@langchain/core/tools';

const tools = toolkit.createLangChainTools({
  DynamicStructuredTool,
  filter: ['list_customers', 'get_order'],
});
```

These tools use the original Zod schemas and call back into the same embedded execution runtime.

## OpenAI Responses Loop

```javascript
const tools = toolkit.getTools({ format: 'openai' });

const response = await client.responses.create({
  model: 'gpt-4.1',
  input: 'List the most recent customers.',
  tools,
});

const toolCall = response.output.find((item) => item.type === 'function_call');
const execution = await toolkit.executeOpenAIToolCall(toolCall);

const finalResponse = await client.responses.create({
  model: 'gpt-4.1',
  previous_response_id: response.id,
  input: [execution.outputMessage],
  tools,
});
```

## Safe Writes

Start with `allowApply: false`.

- Use `simulateMutation({ tool, params })` for write tools.
- Use `executePlan({ dryRun: true, steps })` to preview multi-step flows.
- Use `executeToolCalls([...])` to batch multiple tool invocations in a single agent loop.
- Enable `allowApply: true` only for agents that should mutate commerce state.

## MCP-Native Clients

For Claude Desktop, Cursor, Windsurf, or other MCP-native clients, use the CLI onboarding flow instead:

```bash
npx -y @stateset/cli@latest stateset-setup --yes --quickstart --db ./store.db
```
