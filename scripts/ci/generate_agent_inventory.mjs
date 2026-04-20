#!/usr/bin/env node

import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { AGENTS } from '../../cli/src/agent-definitions.js';
import { TOOL_NAMES, getStaticMcpToolDefinitions } from '../../cli/src/mcp-server.js';
import { getAllStaticMcpToolNames, getStaticMcpServerDefinitions } from '../../cli/src/mcp-server-registry.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '../..');
const jsonOutputPath = path.join(rootDir, 'artifacts/compatibility/agent-inventory.json');
const markdownOutputPath = path.join(rootDir, 'docs/src/appendix/agent-inventory.md');
const checkMode = process.argv.includes('--check');

function compareStrings(left, right) {
  return left.replace(/\r\n/g, '\n') === right.replace(/\r\n/g, '\n');
}

function renderMarkdownTable(headers, rows) {
  const headerRow = `| ${headers.join(' | ')} |`;
  const dividerRow = `| ${headers.map(() => '---').join(' | ')} |`;
  const bodyRows = rows.map((row) => `| ${row.join(' | ')} |`);
  return [headerRow, dividerRow, ...bodyRows].join('\n');
}

function renderToolAccess(agent, inventory) {
  if (agent.usesAllCommerceTools) {
    return `All ${inventory.totalCommerceTools} commerce MCP tools`;
  }
  if (agent.serverCounts['stateset-scaffold'] && agent.serverNamespaces.length === 1) {
    return `${agent.toolCount} scaffold tools`;
  }
  return `${agent.toolCount} named tools`;
}

function buildInventory() {
  const commerceTools = getStaticMcpToolDefinitions();
  const commerceToolNames = new Set(TOOL_NAMES);
  const allToolNames = getAllStaticMcpToolNames();
  const supportedToolNames = new Set(allToolNames);
  const supportedServers = getStaticMcpServerDefinitions()
    .map((server) => ({
      name: server.name,
      toolCount: server.tools.length,
      source: server.source,
    }))
    .sort((left, right) => left.name.localeCompare(right.name));
  const agents = Object.entries(AGENTS)
    .map(([id, config]) => {
      const tools = Array.isArray(config.tools) ? [...config.tools] : [];
      const unknownTools = tools.filter((toolName) => !supportedToolNames.has(toolName));
      const serverCounts = {};
      for (const toolName of tools) {
        const match = toolName.match(/^mcp__([^_]+(?:-[^_]+)*)__.+$/);
        const serverName = match?.[1] ?? 'unknown';
        serverCounts[serverName] = (serverCounts[serverName] ?? 0) + 1;
      }
      const serverNamespaces = Object.keys(serverCounts).sort((left, right) => left.localeCompare(right));
      const usesAllCommerceTools =
        tools.length === TOOL_NAMES.length && tools.every((toolName) => commerceToolNames.has(toolName));

      return {
        id,
        name: config.name ?? id,
        description: config.description ?? '',
        toolCount: tools.length,
        usesAllCommerceTools,
        serverCounts,
        serverNamespaces,
        unknownTools,
      };
    })
    .sort((left, right) => left.id.localeCompare(right.id));

  const invalidAgents = agents.filter((agent) => agent.unknownTools.length > 0);
  if (invalidAgents.length > 0) {
    const details = invalidAgents
      .map((agent) => `${agent.id}: ${agent.unknownTools.join(', ')}`)
      .join('; ');
    throw new Error(`Agent inventory found unknown MCP tool references (${details}).`);
  }

  const fullAccessAgents = agents.filter((agent) => agent.usesAllCommerceTools);
  const scopedAgents = agents.filter((agent) => !agent.usesAllCommerceTools);
  const totalScopedToolReferences = scopedAgents.reduce((sum, agent) => sum + agent.toolCount, 0);

  return {
    source: {
      agents: 'cli/src/agent-definitions.js',
      registry: 'cli/src/mcp-server-registry.js',
    },
    totalAgents: agents.length,
    totalCommerceTools: commerceTools.length,
    totalScaffoldTools: supportedServers.find((server) => server.name === 'stateset-scaffold')?.toolCount ?? 0,
    totalX402Tools: supportedServers.find((server) => server.name === 'stateset-x402')?.toolCount ?? 0,
    fullAccessAgentCount: fullAccessAgents.length,
    scopedAgentCount: scopedAgents.length,
    totalScopedToolReferences,
    supportedServers,
    agents,
  };
}

function renderMarkdownInventory(inventory) {
  const summaryRows = [
    ['Total agents', String(inventory.totalAgents)],
    ['Commerce MCP tools', String(inventory.totalCommerceTools)],
    ['Scaffold MCP tools', String(inventory.totalScaffoldTools)],
    ['x402 MCP tools', String(inventory.totalX402Tools)],
    ['Agents with full commerce access', String(inventory.fullAccessAgentCount)],
    ['Agents with scoped tool sets', String(inventory.scopedAgentCount)],
    ['Scoped tool references', String(inventory.totalScopedToolReferences)],
  ];

  const serverRows = inventory.supportedServers.map((server) => [
    server.name,
    String(server.toolCount),
    `\`${server.source}\``,
  ]);
  const agentRows = inventory.agents.map((agent) => [
    `\`${agent.id}\``,
    agent.name,
    renderToolAccess(agent, inventory),
    agent.serverNamespaces.map((server) => `\`${server}\``).join(', '),
    agent.description || '—',
  ]);

  return `# Agent Inventory

This page is generated from the live agent definitions in \`cli/src/agent-definitions.js\`
and validated against the MCP server registry in \`cli/src/mcp-server-registry.js\`.
Do not edit it by hand. Regenerate it with:

\`\`\`bash
node ./scripts/ci/generate_agent_inventory.mjs
\`\`\`

Machine-readable output lives at \`artifacts/compatibility/agent-inventory.json\`.

## Summary

${renderMarkdownTable(['Metric', 'Value'], summaryRows)}

## Supported MCP Servers

${renderMarkdownTable(['MCP server', 'Tools', 'Source'], serverRows)}

## Agent Registry

${renderMarkdownTable(['Agent', 'Display name', 'Tool access', 'MCP servers', 'Description'], agentRows)}
`;
}

async function verifyOutput(filePath, expectedContent) {
  const relativePath = path.relative(rootDir, filePath);

  try {
    const actualContent = await readFile(filePath, 'utf8');
    if (!compareStrings(actualContent, expectedContent)) {
      console.error(
        `::error file=${relativePath}::Generated agent inventory is out of date. Run 'node ./scripts/ci/generate_agent_inventory.mjs'.`,
      );
      return false;
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.error(
      `::error file=${relativePath}::Unable to read generated agent inventory output (${message}). Run 'node ./scripts/ci/generate_agent_inventory.mjs'.`,
    );
    return false;
  }

  return true;
}

async function main() {
  const inventory = buildInventory();
  const jsonContent = `${JSON.stringify(inventory, null, 2)}\n`;
  const markdownContent = renderMarkdownInventory(inventory);

  if (checkMode) {
    const ok = await Promise.all([
      verifyOutput(jsonOutputPath, jsonContent),
      verifyOutput(markdownOutputPath, markdownContent),
    ]);

    if (!ok.every(Boolean)) {
      process.exit(1);
    }

    console.log(
      `Agent inventory is up to date (${inventory.totalAgents} agents, ${inventory.totalCommerceTools + inventory.totalScaffoldTools + inventory.totalX402Tools} MCP tools across supported servers).`,
    );
    return;
  }

  await mkdir(path.dirname(jsonOutputPath), { recursive: true });
  await mkdir(path.dirname(markdownOutputPath), { recursive: true });
  await writeFile(jsonOutputPath, jsonContent, 'utf8');
  await writeFile(markdownOutputPath, markdownContent, 'utf8');

  console.log(
    `Generated agent inventory (${inventory.totalAgents} agents, ${inventory.totalCommerceTools + inventory.totalScaffoldTools + inventory.totalX402Tools} MCP tools across supported servers).`,
  );
}

await main();
