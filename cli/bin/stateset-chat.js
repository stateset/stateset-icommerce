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
import { parseArgs } from 'node:util';
import * as readline from 'node:readline';

const HELP = `
StateSet iCommerce CLI - Interactive Chat

USAGE:
  stateset-chat [options]

OPTIONS:
  --db <path>        Path to SQLite database (default: ./store.db)
  --apply            Enable write operations
  --model <model>    Claude model to use
  --verbose, -V      Enable verbose telemetry
  --help, -h         Show this help message

IN-CHAT COMMANDS:
  /help              Show available commands
  /status            Show current settings
  /apply on|off      Toggle apply mode
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
      model: { type: 'string', default: 'claude-sonnet-4-20250514' },
      verbose: { type: 'boolean', short: 'V', default: false },
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

  // Create readline interface
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
  });

  const prompt = () => {
    const modeIndicator = allowApply ? '✏️ ' : '👁️ ';
    rl.question(`${modeIndicator}stateset> `, handleInput);
  };

  const showStatus = () => {
    console.log(`\n${ICONS.analytics} ${output.bold('Current Settings:')}`);
    console.log(`   ${output.dim('Database:')} ${dbPath}`);
    console.log(`   ${output.dim('Mode:')}     ${allowApply ? output.green('Write enabled') : output.yellow('Preview only')}`);
    console.log(`   ${output.dim('Verbose:')}  ${verbose ? output.cyan('On') : 'Off'}`);
    console.log(`   ${output.dim('Model:')}    ${model}`);
    console.log(`   ${output.dim('Session:')}  ${sessionId || output.dim('(none)')}`);
    console.log();
  };

  const showHelp = () => {
    console.log(`
${output.bold('Available Commands:')}
   /help              Show this help
   /status            Show current settings
   /apply on|off      Toggle write mode
   /verbose on|off    Toggle verbose mode
   /db <path>         Switch database
   /new               Start new session
   /exit, /quit       Exit chat

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
        onToolCall: (toolCall) => {
          if (!verbose) {
            // Standard tool call display (verbose mode handles its own output)
            console.log(output.toolCall(toolCall.name, toolCall.input));
          }
        }
      });

      // Update session ID
      if (result.sessionId) {
        sessionId = result.sessionId;
      }

      console.log('\n' + result.response);

      // Show stats in verbose mode
      if (verbose && result.telemetry) {
        const stats = result.telemetry;
        console.log(`\n${output.dim('─'.repeat(40))}`);
        console.log(`${output.dim('Stats:')} ${stats.toolCalls?.total || 0} tools, ${stats.duration}ms`);
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
