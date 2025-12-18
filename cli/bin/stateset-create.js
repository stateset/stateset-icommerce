#!/usr/bin/env node

/**
 * StateSet Create - AI-powered storefront scaffolding
 *
 * Creates complete e-commerce storefronts using StateSet iCommerce engine.
 *
 * Usage:
 *   stateset-create "create a store called Urban Thread"
 *   stateset-create --apply "create a nextjs storefront for my coffee shop"
 *   stateset-create --dir ./projects "scaffold an online bookstore"
 */

import { query } from '@anthropic-ai/claude-agent-sdk';
import { createScaffoldMcpServer, SCAFFOLD_TOOL_NAMES } from '../src/scaffold-server.js';
import { AgentTelemetry, noOpTelemetry } from '../src/telemetry.js';
import { RichOutput, ICONS } from '../src/output.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../src/config.js';
import { parseArgs } from 'node:util';
import { AGENTS } from '../src/claude-harness.js';
import path from 'node:path';

const HELP = `
StateSet Create - AI-powered storefront scaffolding

USAGE:
  stateset-create [options] "<request>"

OPTIONS:
  --dir <path>       Directory for new projects (default: current directory)
  --apply            Enable write operations (create files, run commands)
  --model <model>    Claude model to use (default: see config.js)
  --verbose, -V      Enable verbose output with telemetry
  --stats            Show execution statistics
  --json             Output as JSON
  --help, -h         Show this help message

TEMPLATES:
  nextjs             Full-stack Next.js 14 with App Router, SSR, Tailwind (recommended)
  nextjs-minimal     Minimal Next.js setup for learning
  vite-react         Client-side SPA using WASM bindings
  astro              Static-first with Islands architecture

EXAMPLES:
  # Preview what would be created (safe, no files written)
  stateset-create "create a clothing store called Urban Thread"

  # Actually create the project
  stateset-create --apply "create a nextjs storefront for my coffee shop"

  # Create in a specific directory
  stateset-create --apply --dir ~/projects "build an online bookstore"

  # Add features to existing project
  stateset-create --apply --dir ./my-store "add a wishlist page"

WORKFLOW:
  1. Describe your store (name, type, features)
  2. Review the preview of what will be created
  3. Run with --apply to create files
  4. Follow the next steps to start your store

SAFETY:
  By default, all write operations are blocked (preview mode).
  Use --apply to enable file creation and npm commands.
`;

async function main() {
  const { values, positionals } = parseArgs({
    options: {
      dir: { type: 'string', default: process.cwd() },
      apply: { type: 'boolean', default: false },
      model: { type: 'string', default: DEFAULT_MODEL },
      verbose: { type: 'boolean', short: 'V', default: false },
      stats: { type: 'boolean', default: false },
      json: { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false },
      version: { type: 'boolean', short: 'v', default: false }
    },
    allowPositionals: true
  });

  // Initialize output formatter
  const output = new RichOutput({ color: !values.json });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  if (values.version) {
    console.log(`@stateset/cli create v${CLI_VERSION}`);
    process.exit(0);
  }

  const request = positionals.join(' ').trim();
  if (!request) {
    console.error('Error: No request provided');
    console.error('Usage: stateset-create "<your request>"');
    console.error('Run stateset-create --help for more information');
    process.exit(1);
  }

  // Resolve work directory
  const workDir = path.resolve(values.dir);

  // Show mode indicator
  if (!values.json) {
    console.log(`\n${ICONS.order} StateSet Create - Storefront Generator`);
    console.log(`   ${output.dim('Directory:')} ${workDir}`);
    console.log(`   ${output.dim('Mode:')}      ${values.apply ? output.green('Write enabled') : output.yellow('Preview only')}`);
    if (values.verbose) {
      console.log(`   ${output.dim('Verbose:')}   ${output.cyan('Enabled')}`);
    }
    console.log();
  }

  // Initialize telemetry
  const telemetry = values.verbose ? new AgentTelemetry({ verbose: true }) : noOpTelemetry;
  const mainSpan = telemetry.startSpan('storefront_create', { request: request.slice(0, 100) });

  // Create scaffold MCP server
  const mcpServer = createScaffoldMcpServer({
    workDir,
    allowWrite: values.apply
  });

  // Get agent config
  const agentConfig = AGENTS['storefront'];

  try {
    // Track tool calls
    const toolResults = [];

    // Run the agent
    const mcpTools = SCAFFOLD_TOOL_NAMES.map(t => `mcp__stateset-scaffold__${t}`);
    for await (const message of query({
      prompt: request,
      options: {
        model: values.model,
        systemPrompt: agentConfig.systemPrompt,
        mcpServers: {
          'stateset-scaffold': mcpServer
        },
        // Restrict to only MCP scaffold tools
        tools: mcpTools,
        // Auto-allow all MCP tools without permission prompts
        allowedTools: mcpTools,
        maxTurns: 15
      }
    })) {
      // Debug: log all message types
      if (values.verbose) {
        console.log(`[DEBUG] Message type: ${message.type}`);
        if (message.type === 'system') {
          console.log(`[DEBUG] System message:`, JSON.stringify(message).slice(0, 500));
        }
        if (message.type === 'assistant') {
          console.log(`[DEBUG] Assistant content:`, JSON.stringify(message.content || message).slice(0, 300));
        }
      }
      if (message.type === 'assistant') {
        // Handle nested message structure from SDK
        const content = message.message?.content || message.content;
        if (content) {
          for (const block of content) {
            if (block.type === 'tool_use') {
              const toolCall = {
                name: block.name,
                input: block.input,
                startTime: Date.now()
              };
              toolResults.push({ toolCall, result: null });

              if (!values.json) {
                const shortName = block.name.replace('mcp__stateset-scaffold__', '');
                console.log(output.toolCall(shortName, block.input));
              }
            } else if (block.type === 'text' && block.text) {
              if (!values.json) {
                console.log('\n' + block.text);
              }
            }
          }
        }
      } else if (message.type === 'result') {
        const pending = toolResults.find(tr => tr.result === null);
        if (pending) {
          pending.result = message.content;
          pending.duration = Date.now() - pending.toolCall.startTime;
          telemetry.logToolCall(
            pending.toolCall.name,
            pending.toolCall.input,
            pending.result,
            pending.duration
          );
        }
      }
    }

    telemetry.endSpanRef(mainSpan, 'ok');

    // JSON output
    if (values.json) {
      console.log(JSON.stringify({
        request,
        workDir,
        allowWrite: values.apply,
        toolResults: toolResults.map(tr => ({
          tool: tr.toolCall.name,
          input: tr.toolCall.input,
          result: tr.result,
          duration: tr.duration
        })),
        telemetry: values.stats ? telemetry.getSummary() : undefined
      }, null, 2));
    } else {
      // Show stats if requested
      if ((values.stats || values.verbose) && telemetry.getSummary) {
        const stats = telemetry.getSummary();
        console.log(`\n${output.dim('─'.repeat(40))}`);
        console.log(`${ICONS.analytics} ${output.bold('Execution Stats')}`);
        console.log(`   ${output.dim('Duration:')}    ${stats.duration}ms`);
        console.log(`   ${output.dim('Tool Calls:')}  ${stats.toolCalls?.total || 0}`);
      }

      // Show next steps if not in apply mode
      if (!values.apply) {
        console.log(`\n${output.yellow('Preview Mode:')} No files were created.`);
        console.log(`Run with ${output.cyan('--apply')} to create the files.`);
      }
    }

    process.exit(0);
  } catch (error) {
    telemetry.logError(error);
    telemetry.endSpanRef(mainSpan, 'error');

    if (values.json) {
      console.log(JSON.stringify({ error: error.message }));
    } else {
      console.error(`\n${output.status('error', error.message)}`);
    }
    process.exit(1);
  }
}

main();
