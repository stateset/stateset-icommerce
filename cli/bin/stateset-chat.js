#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Interactive Chat Mode
 *
 * Multi-turn conversational interface for commerce operations.
 *
 * Usage:
 *   stateset-chat
 *   stateset-chat --db ./mystore.db
 *   stateset-chat --apply
 */

import { runAgentLoop, RichOutput, ICONS } from '../src/claude-harness.js';
import { createConfirmHandler } from '../src/utils/confirm.js';
import { DEFAULT_MODEL, CLI_VERSION, THINK_LEVELS } from '../src/config.js';
import { parseArgs } from 'node:util';
import * as readline from 'node:readline';

const HELP = `
StateSet iCommerce CLI - Interactive Chat

USAGE:
  stateset-chat [options]

OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --apply            Enable write operations
  --model <model>    AI model to use
  --provider <name>  AI provider: claude, openai, gemini, ollama (default: claude)
  --think <level>    Extended thinking: off, low, medium, high (default: off)
  --stream           Enable streaming output
  --budget <usd>     Maximum spend per query in USD
  --memory           Enable conversation memory (overrides settings)
  --no-memory        Disable conversation memory (overrides settings)
  --x402             Enable x402 MCP tools
  --verbose, -V      Enable verbose telemetry
  --yes, -y          Skip confirmation prompts
  --help, -h         Show this help message

IN-CHAT COMMANDS:
  /help              Show available commands
  /status            Show current settings
  /apply on|off      Toggle apply mode
  /think <level>     Set thinking level (off|low|medium|high)
  /stream            Toggle streaming mode
  /provider <name>   Switch AI provider
  /budget <usd>      Set budget per query ($)
  /memory            Toggle conversation memory (overrides settings)
  /verbose on|off    Toggle verbose mode
  /db <path>         Switch database
  /new               Start new session (clear context)
  /exit, /quit       Exit chat
`;

async function main() {
  const { values } = parseArgs({
    options: {
      db: { type: 'string', default: './store.db' },
      apply: { type: 'boolean', default: false },
      model: { type: 'string', default: DEFAULT_MODEL },
      provider: { type: 'string', default: 'claude' },
      think: { type: 'string', default: 'off' },
      stream: { type: 'boolean', default: false },
      budget: { type: 'string' },
      memory: { type: 'boolean', default: false },
      noMemory: { type: 'boolean', default: false },
      x402: { type: 'boolean', default: false },
      verbose: { type: 'boolean', short: 'V', default: false },
      yes: { type: 'boolean', short: 'y', default: false },
      help: { type: 'boolean', short: 'h', default: false }
    },
    allowPositionals: true
  });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  // Initialize output formatter
  const output = new RichOutput({ color: true });

  // State
  let dbPath = values.db;
  let allowApply = values.apply;
  let model = values.model;
  let verbose = values.verbose;
  let sessionId = null;
  let thinkLevel = values.think || 'off';
  let streaming = values.stream || false;
  let provider = values.provider || 'claude';
  let budget = values.budget || null;
  let memoryEnabled = values.noMemory ? false : (values.memory ? true : null);
  let x402Enabled = values.x402 || false;

  // Create readline interface
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
  });

  const confirmPrompt = (message) => new Promise((resolve) => {
    rl.question(`${message} [y/N] `, (answer) => {
      resolve(answer.toLowerCase() === 'y' || answer.toLowerCase() === 'yes');
    });
  });

  const onConfirmRequired = createConfirmHandler({
    output,
    assumeYes: values.yes,
    nonInteractive: false,
    confirmPrompt
  });

  const prompt = () => {
    const modeIndicator = allowApply ? '✏️ ' : '👁️ ';
    rl.question(`${modeIndicator}stateset> `, handleInput);
  };

  const showStatus = () => {
    console.log(`\n${ICONS.analytics} ${output.bold('Current Settings:')}`);
    console.log(`   ${output.dim('Database:')}  ${dbPath}`);
    console.log(`   ${output.dim('Mode:')}      ${allowApply ? output.green('Write enabled') : output.yellow('Preview only')}`);
    console.log(`   ${output.dim('Provider:')}  ${provider}`);
    console.log(`   ${output.dim('Model:')}     ${model}`);
    console.log(`   ${output.dim('Thinking:')}  ${thinkLevel === 'off' ? 'Off' : output.cyan(thinkLevel)}`);
    console.log(`   ${output.dim('Streaming:')} ${streaming ? output.cyan('On') : 'Off'}`);
    if (budget) {
      console.log(`   ${output.dim('Budget:')}    ${output.cyan('$' + budget)}/query`);
    }
    const memoryLabel = memoryEnabled === null
      ? output.dim('Default')
      : (memoryEnabled ? output.cyan('On') : 'Off');
    console.log(`   ${output.dim('Memory:')}    ${memoryLabel}`);
    console.log(`   ${output.dim('x402:')}      ${x402Enabled ? output.cyan('On') : 'Off'}`);
    console.log(`   ${output.dim('Verbose:')}   ${verbose ? output.cyan('On') : 'Off'}`);
    console.log(`   ${output.dim('Session:')}   ${sessionId || output.dim('(none)')}`);
    console.log();
  };

  const showHelp = () => {
    console.log(`
${output.bold('Available Commands:')}
   /help                    Show this help
   /status                  Show current settings
   /apply on|off            Toggle write mode
   /think off|low|med|high  Set extended thinking level
   /stream                  Toggle streaming output
   /provider <name>         Switch provider (claude|openai|gemini|ollama)
   /budget <usd>            Set max spend per query (e.g., /budget 1.00)
   /memory                  Toggle conversation memory
   /verbose on|off          Toggle verbose mode
   /db <path>               Switch database
   /new                     Start new session
   /exit, /quit             Exit chat

${output.bold('Example Queries:')}
   "show me all customers"
   "how much stock do we have of WIDGET-001?"
   "create a customer named Bob with email bob@example.com"
   "list recent orders"
`);
  };

  const handleInput = async (input) => {
    const trimmed = input.trim();

    // Handle empty input
    if (!trimmed) {
      prompt();
      return;
    }

    // Handle commands
    if (trimmed.startsWith('/')) {
      const parts = trimmed.slice(1).split(/\s+/);
      const cmd = parts[0].toLowerCase();
      const args = parts.slice(1);

      switch (cmd) {
        case 'help':
          showHelp();
          break;

        case 'status':
          showStatus();
          break;

        case 'apply':
          if (args[0] === 'on') {
            allowApply = true;
            console.log(output.status('success', 'Write mode enabled'));
          } else if (args[0] === 'off') {
            allowApply = false;
            console.log(output.status('info', 'Preview mode enabled'));
          } else {
            console.log(`Apply mode: ${allowApply ? 'on' : 'off'}`);
            console.log('Use /apply on or /apply off to change');
          }
          break;

        case 'verbose':
          if (args[0] === 'on') {
            verbose = true;
            console.log(output.status('success', 'Verbose mode enabled'));
          } else if (args[0] === 'off') {
            verbose = false;
            console.log(output.status('info', 'Verbose mode disabled'));
          } else {
            console.log(`Verbose mode: ${verbose ? 'on' : 'off'}`);
            console.log('Use /verbose on or /verbose off to change');
          }
          break;

        case 'think': {
          const level = (args[0] || '').toLowerCase();
          if (['off', 'low', 'medium', 'med', 'high'].includes(level)) {
            thinkLevel = level === 'med' ? 'medium' : level;
            console.log(output.status('success', `Extended thinking: ${thinkLevel}${thinkLevel !== 'off' ? ` (${THINK_LEVELS[thinkLevel]?.toLocaleString()} tokens)` : ''}`));
          } else {
            console.log(`Thinking: ${thinkLevel}`);
            console.log('Use /think off|low|medium|high');
          }
          break;
        }

        case 'stream':
          streaming = !streaming;
          console.log(output.status('info', `Streaming: ${streaming ? 'on' : 'off'}`));
          break;

        case 'provider':
          if (args[0]) {
            const p = args[0].toLowerCase();
            if (['claude', 'openai', 'gemini', 'ollama'].includes(p)) {
              provider = p;
              console.log(output.status('success', `Provider: ${provider}`));
              if (provider !== 'claude') {
                console.log(output.dim('   Note: Non-Claude providers run in chat-only mode (no MCP tools)'));
              }
            } else {
              console.log(`Unknown provider: ${p}. Available: claude, openai, gemini, ollama`);
            }
          } else {
            console.log(`Current provider: ${provider}`);
          }
          break;

        case 'budget':
          if (args[0]) {
            const val = parseFloat(args[0]);
            if (!isNaN(val) && val > 0) {
              budget = args[0];
              console.log(output.status('success', `Budget: $${budget}/query`));
            } else {
              console.log('Usage: /budget <amount> (e.g., /budget 1.00)');
            }
          } else {
            console.log(`Budget: ${budget ? '$' + budget + '/query' : 'unlimited'}`);
          }
          break;

        case 'memory':
          if (memoryEnabled === null) {
            memoryEnabled = true;
          } else {
            memoryEnabled = !memoryEnabled;
          }
          console.log(output.status('info', `Memory: ${memoryEnabled ? 'on' : 'off'}`));
          break;

        case 'db':
          if (args[0]) {
            dbPath = args[0];
            sessionId = null; // Reset session when switching DB
            console.log(`📂 Switched to database: ${dbPath}`);
          } else {
            console.log(`Current database: ${dbPath}`);
          }
          break;

        case 'new':
          sessionId = null;
          console.log('🆕 Started new session');
          break;

        case 'exit':
        case 'quit':
          console.log('\n👋 Goodbye!');
          rl.close();
          process.exit(0);
          break;

        default:
          console.log(`Unknown command: /${cmd}`);
          console.log('Use /help to see available commands');
      }

      prompt();
      return;
    }

    // Run agent query
    try {
      console.log(); // Blank line before response

      const result = await runAgentLoop({
        request: trimmed,
        dbPath,
        model,
        allowApply,
        verbose,
        resumeSessionId: sessionId,
        onConfirmRequired,
        thinkLevel,
        streaming,
        maxBudgetUsd: budget,
        provider,
        enableMemory: memoryEnabled === null ? null : memoryEnabled,
        enableX402: x402Enabled,
        onPartialMessage: streaming ? (event) => {
          if (event?.content) {
            process.stdout.write(event.content);
          } else if (event?.delta?.text) {
            process.stdout.write(event.delta.text);
          } else if (typeof event?.text === 'string') {
            process.stdout.write(event.text);
          }
        } : null,
        onThinkingBlock: thinkLevel !== 'off' ? (block) => {
          if (verbose) {
            const preview = (block.thinking || block.text || '').slice(0, 200);
            console.log(output.dim(`\n[Thinking] ${preview}${preview.length >= 200 ? '...' : ''}\n`));
          }
        } : null,
        onToolCall: (toolCall) => {
          if (!verbose) {
            console.log(output.toolCall(toolCall.name, toolCall.input));
          }
        }
      });

      // Update session ID
      if (result.sessionId) {
        sessionId = result.sessionId;
      }

      // If streaming was used, just add a newline; otherwise print full response
      if (streaming && result.response) {
        console.log(); // newline after streamed output
      } else {
        console.log('\n' + result.response);
      }

      // Show stats in verbose mode
      if (verbose && result.telemetry) {
        const stats = result.telemetry;
        console.log(`\n${output.dim('─'.repeat(40))}`);
        console.log(`${output.dim('Stats:')} ${stats.toolCalls?.total || 0} tools, ${stats.duration}ms${result.cost != null ? `, $${result.cost.toFixed(4)}` : ''}`);
        if (result.budgetExceeded) {
          console.log(output.yellow('Budget exceeded'));
        }
      }

      console.log();

    } catch (error) {
      console.error(`\n${output.status('error', error.message)}\n`);
    }

    prompt();
  };

  // Welcome message
  console.log(`
╔════════════════════════════════════════════════════════════╗
║        ${ICONS.order} ${output.bold('StateSet iCommerce - Interactive Chat')}          ║
╠════════════════════════════════════════════════════════════╣
║  Type your request in natural language.                    ║
║  Use ${output.cyan('/help')} for commands, ${output.cyan('/exit')} to quit.                    ║
╚════════════════════════════════════════════════════════════╝
`);
  showStatus();

  // Start prompting
  prompt();
}

main();
