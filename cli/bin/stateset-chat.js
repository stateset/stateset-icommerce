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

import { RichOutput, ICONS } from '../src/claude-harness.js';
import { createChatTransport } from '../src/utils/chat-transport.js';
import { createConfirmHandler } from '../src/utils/confirm.js';
import { printExecutionStats } from '../src/utils/execution-stats.js';
import {
  appendSessionRefresh,
  formatSessionRefreshReason,
  formatSessionRefreshTimestamp,
} from '../src/utils/session-refresh.js';
import { DEFAULT_MODEL, THINK_LEVELS } from '../src/config.js';
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
  --stats            Show prompt budget and execution stats
  --memory           Enable conversation memory (overrides settings)
  --no-memory        Disable conversation memory (overrides settings)
  --x402             Enable x402 MCP tools
  --treasury         Enable treasury billing (stablecoins)
  --treasury-chain <id>    Treasury chain id (e.g., base, solana)
  --treasury-token <sym>   Treasury token symbol (e.g., USDC)
  --treasury-agent <id>    Treasury agent id (default: default)
  --treasury-db <path>     Treasury DB path
  --treasury-erc8004-registry <uri>  ERC-8004 registry URI
  --treasury-erc8004-db <path>       ERC-8004 db path (defaults to --db)
  --verbose, -V      Enable verbose telemetry
  --yes, -y          Skip confirmation prompts
  --help, -h         Show this help message

IN-CHAT COMMANDS:
  /help              Show available commands
  /status            Show current settings
  /apply on|off      Toggle apply mode
  /think <level>     Set thinking level (off|low|medium|high)
  /stream            Toggle streaming mode
  /stats             Toggle live prompt budget and execution stats
  /prompt            Show the latest prompt budget report
  /refreshes         Show session refresh history
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
      stats: { type: 'boolean', default: false },
      memory: { type: 'boolean', default: false },
      'no-memory': { type: 'boolean', default: false },
      x402: { type: 'boolean', default: false },
      treasury: { type: 'boolean', default: false },
      treasuryChain: { type: 'string' },
      treasuryToken: { type: 'string' },
      treasuryAgent: { type: 'string' },
      treasuryDb: { type: 'string' },
      treasuryErc8004Registry: { type: 'string' },
      treasuryErc8004Db: { type: 'string' },
      verbose: { type: 'boolean', short: 'V', default: false },
      yes: { type: 'boolean', short: 'y', default: false },
      help: { type: 'boolean', short: 'h', default: false },
    },
    allowPositionals: true,
  });

  if (values.help) {
    console.log(HELP);
    process.exit(0);
  }

  const output = new RichOutput({ color: true });

  let dbPath = values.db;
  let allowApply = values.apply;
  const model = values.model;
  let verbose = values.verbose;
  let sessionId = null;
  let thinkLevel = values.think || 'off';
  let streaming = values.stream || false;
  let provider = values.provider || 'claude';
  let budget = values.budget || null;
  let statsEnabled = values.stats || false;
  const noMemoryFlag = values['no-memory'] ?? values.noMemory ?? false;
  let memoryEnabled = noMemoryFlag ? false : values.memory ? true : null;
  const x402Enabled = values.x402 || false;
  let lastPromptReport = null;
  let lastSessionRefresh = null;
  let sessionRefreshHistory = [];
  const treasuryEnabled = Boolean(
    values.treasury ||
    values.treasuryChain ||
    values.treasuryToken ||
    values.treasuryAgent ||
    values.treasuryDb ||
    values.treasuryErc8004Registry ||
    values.treasuryErc8004Db,
  );
  const treasuryConfig = treasuryEnabled
    ? {
        enabled: true,
        chainId: values.treasuryChain,
        tokenSymbol: values.treasuryToken,
        agentId: values.treasuryAgent,
        dbPath: values.treasuryDb,
        erc8004Registry: values.treasuryErc8004Registry,
        erc8004DbPath: values.treasuryErc8004Db,
      }
    : null;

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  const confirmPrompt = (message) =>
    new Promise((resolve) => {
      rl.question(`${message} [y/N] `, (answer) => {
        resolve(answer.toLowerCase() === 'y' || answer.toLowerCase() === 'yes');
      });
    });

  const onConfirmRequired = createConfirmHandler({
    output,
    assumeYes: values.yes,
    nonInteractive: false,
    confirmPrompt,
  });
  const chatTransport = createChatTransport();
  const MAX_SESSION_REFRESH_HISTORY = 10;

  const resetChatSession = (reason = 'reset') => {
    chatTransport.reset(reason);
    sessionId = null;
    lastPromptReport = null;
    lastSessionRefresh = null;
    sessionRefreshHistory = [];
  };

  const resetPersistentSessionIfActive = (reason) => {
    if (chatTransport.isPersistentActive()) {
      resetChatSession(reason);
    }
  };

  const prompt = () => {
    const modeIndicator = allowApply ? '✏️ ' : '👁️ ';
    rl.question(`${modeIndicator}stateset> `, handleInput);
  };

  const recordSessionRefresh = (refresh) => {
    sessionRefreshHistory = appendSessionRefresh(sessionRefreshHistory, refresh, {
      maxEntries: MAX_SESSION_REFRESH_HISTORY,
    });
    lastSessionRefresh = sessionRefreshHistory[sessionRefreshHistory.length - 1] || null;
    return lastSessionRefresh;
  };

  const printSessionRefreshNotice = (refresh) => {
    if (!refresh) return;
    console.log(
      output.status(
        'info',
        `Started a fresh Claude session for ${formatSessionRefreshReason(refresh.reason)}`,
      ),
    );
    if (refresh.previousSessionId || refresh.sessionId) {
      console.log(
        output.dim(
          `   Session IDs: ${refresh.previousSessionId || 'none'} -> ${refresh.sessionId || 'pending'}`,
        ),
      );
    }
    if (refresh.replayedMessages > 0) {
      console.log(
        output.dim(`   Replayed ${refresh.replayedMessages} prior messages into the new session.`),
      );
    }
  };

  const printSessionRefreshHistory = () => {
    if (sessionRefreshHistory.length === 0) {
      console.log(output.dim('No session refreshes recorded in this chat.'));
      return;
    }

    const rows = sessionRefreshHistory.map((entry) => ({
      sequence: `#${entry.sequence}`,
      reason: formatSessionRefreshReason(entry.reason),
      sessions: `${entry.previousSessionId || 'none'} -> ${entry.sessionId || 'pending'}`,
      replayed: entry.replayedMessages || 0,
      recordedAt: formatSessionRefreshTimestamp(entry.recordedAt),
    }));

    console.log(`
${ICONS.session} ${output.bold('Session Refresh History:')}`);
    console.log(
      output.table(rows, [
        { key: 'sequence', header: '#' },
        { key: 'reason', header: 'Reason', width: 24 },
        { key: 'sessions', header: 'Sessions', width: 28 },
        { key: 'replayed', header: 'Replayed', align: 'right' },
        { key: 'recordedAt', header: 'At', width: 20 },
      ]),
    );
    console.log();
  };

  const showStatus = () => {
    console.log(`
${ICONS.analytics} ${output.bold('Current Settings:')}`);
    console.log(`   ${output.dim('Database:')}  ${dbPath}`);
    console.log(
      `   ${output.dim('Mode:')}      ${allowApply ? output.green('Write enabled') : output.yellow('Preview only')}`,
    );
    console.log(`   ${output.dim('Provider:')}  ${provider}`);
    console.log(`   ${output.dim('Model:')}     ${model}`);
    console.log(
      `   ${output.dim('Thinking:')}  ${thinkLevel === 'off' ? 'Off' : output.cyan(thinkLevel)}`,
    );
    console.log(`   ${output.dim('Streaming:')} ${streaming ? output.cyan('On') : 'Off'}`);
    if (budget) {
      console.log(`   ${output.dim('Budget:')}    ${output.cyan('$' + budget)}/query`);
    }
    console.log(`   ${output.dim('Stats:')}     ${statsEnabled ? output.cyan('On') : 'Off'}`);
    const memoryLabel =
      memoryEnabled === null ? output.dim('Default') : memoryEnabled ? output.cyan('On') : 'Off';
    console.log(`   ${output.dim('Memory:')}    ${memoryLabel}`);
    console.log(`   ${output.dim('x402:')}      ${x402Enabled ? output.cyan('On') : 'Off'}`);
    console.log(`   ${output.dim('Treasury:')}  ${treasuryConfig ? output.cyan('On') : 'Off'}`);
    console.log(`   ${output.dim('Verbose:')}   ${verbose ? output.cyan('On') : 'Off'}`);
    console.log(`   ${output.dim('Session:')}   ${sessionId || output.dim('(none)')}`);
    console.log(`   ${output.dim('Refreshes:')} ${sessionRefreshHistory.length}`);
    if (lastSessionRefresh) {
      console.log(
        `   ${output.dim('Last Refresh:')} ${formatSessionRefreshReason(lastSessionRefresh.reason)} (${lastSessionRefresh.previousSessionId || 'none'} -> ${lastSessionRefresh.sessionId || 'pending'}, ${formatSessionRefreshTimestamp(lastSessionRefresh.recordedAt)})`,
      );
    }
    if (lastPromptReport) {
      console.log(
        `   ${output.dim('Last Prompt:')} ~${lastPromptReport.totalInputTokens || 0} tokens from ${lastPromptReport.historySource || 'none'}`,
      );
    }
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
   /stats                   Toggle live prompt diagnostics
   /prompt                  Show the latest prompt budget
   /refreshes               Show session refresh history
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

  const printLatestPromptReport = () => {
    if (!lastPromptReport) {
      console.log(output.dim('No prompt report available yet.'));
      return;
    }
    console.log(`
${output.promptReport(lastPromptReport)}
`);
  };

  const handleInput = async (input) => {
    const trimmed = input.trim();

    if (!trimmed) {
      prompt();
      return;
    }

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
            resetPersistentSessionIfActive('apply mode changed');
            console.log(output.status('success', 'Write mode enabled'));
          } else if (args[0] === 'off') {
            allowApply = false;
            resetPersistentSessionIfActive('apply mode changed');
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
            resetPersistentSessionIfActive('thinking level changed');
            console.log(
              output.status(
                'success',
                `Extended thinking: ${thinkLevel}${thinkLevel !== 'off' ? ` (${THINK_LEVELS[thinkLevel]?.toLocaleString()} tokens)` : ''}`,
              ),
            );
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

        case 'stats':
          if (args[0] === 'on') {
            statsEnabled = true;
          } else if (args[0] === 'off') {
            statsEnabled = false;
          } else {
            statsEnabled = !statsEnabled;
          }
          console.log(output.status('info', `Stats: ${statsEnabled ? 'on' : 'off'}`));
          break;

        case 'prompt':
          printLatestPromptReport();
          break;

        case 'refreshes':
          printSessionRefreshHistory();
          break;

        case 'provider':
          if (args[0]) {
            const p = args[0].toLowerCase();
            if (['claude', 'openai', 'gemini', 'ollama'].includes(p)) {
              const providerChanged = provider !== p;
              provider = p;
              if (providerChanged) {
                resetChatSession('provider changed');
              }
              console.log(output.status('success', `Provider: ${provider}`));
              if (providerChanged) {
                console.log(output.dim('   Started a new session for the provider change.'));
              }
              if (provider !== 'claude') {
                console.log(
                  output.dim('   Note: Non-Claude providers run in chat-only mode (no MCP tools)'),
                );
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
            if (!Number.isNaN(val) && val > 0) {
              budget = args[0];
              resetPersistentSessionIfActive('budget changed');
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
          resetPersistentSessionIfActive('memory changed');
          console.log(output.status('info', `Memory: ${memoryEnabled ? 'on' : 'off'}`));
          break;

        case 'db':
          if (args[0]) {
            dbPath = args[0];
            resetChatSession('database changed');
            console.log(`📂 Switched to database: ${dbPath}`);
          } else {
            console.log(`Current database: ${dbPath}`);
          }
          break;

        case 'new':
          resetChatSession('new chat requested');
          console.log('🆕 Started new session');
          break;

        case 'exit':
        case 'quit':
          resetChatSession('chat exit');
          console.log(`\n👋 Goodbye!`);
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

    try {
      console.log();

      let livePromptReportPrinted = false;
      const result = await chatTransport.query({
        request: trimmed,
        dbPath,
        model,
        allowApply,
        verbose,
        resumeSessionId: sessionId,
        onConfirmRequired,
        treasury: treasuryConfig,
        thinkLevel,
        streaming,
        maxBudgetUsd: budget,
        provider,
        enableMemory: memoryEnabled === null ? null : memoryEnabled,
        enableX402: x402Enabled,
        onEvent: (event) => {
          if (event?.type === 'prompt_report' && event.report) {
            lastPromptReport = event.report;
            if (statsEnabled) {
              console.log(`
${output.promptReport(event.report)}
`);
              livePromptReportPrinted = true;
            }
          }
        },
        onPartialMessage: streaming
          ? (event) => {
              if (event?.content) {
                process.stdout.write(event.content);
              } else if (event?.delta?.text) {
                process.stdout.write(event.delta.text);
              } else if (typeof event?.text === 'string') {
                process.stdout.write(event.text);
              }
            }
          : null,
        onThinkingBlock:
          thinkLevel !== 'off'
            ? (block) => {
                if (verbose) {
                  const preview = (block.thinking || block.text || '').slice(0, 200);
                  console.log(
                    output.dim(`
[Thinking] ${preview}${preview.length >= 200 ? '...' : ''}
`),
                  );
                }
              }
            : null,
        onToolCall: (toolCall) => {
          if (!verbose) {
            console.log(output.toolCall(toolCall.name, toolCall.input));
          }
        },
      });

      lastPromptReport = result.promptReport || lastPromptReport;

      if (result.sessionId || chatTransport.getSessionId()) {
        sessionId = result.sessionId || chatTransport.getSessionId();
      }
      const recordedSessionRefresh = result.sessionRefresh
        ? recordSessionRefresh(result.sessionRefresh)
        : null;

      if (streaming && result.response) {
        console.log();
      } else {
        console.log(`\n${result.response}`);
      }

      if (recordedSessionRefresh && !(statsEnabled || verbose)) {
        printSessionRefreshNotice(recordedSessionRefresh);
      }

      if (statsEnabled || verbose) {
        printExecutionStats({
          output,
          ioConsole: console,
          result,
          includePromptReport: statsEnabled && !livePromptReportPrinted,
        });
      }

      console.log();
    } catch (error) {
      console.error(`
${output.status('error', error.message)}
`);
    }

    prompt();
  };

  console.log(`
╔════════════════════════════════════════════════════════════╗
║        ${ICONS.order} ${output.bold('StateSet iCommerce - Interactive Chat')}          ║
╠════════════════════════════════════════════════════════════╣
║  Type your request in natural language.                    ║
║  Use ${output.cyan('/help')} for commands, ${output.cyan('/exit')} to quit.                    ║
╚════════════════════════════════════════════════════════════╝
`);
  showStatus();
  prompt();
}

import { runMain } from '../src/graceful-shutdown.js';
runMain('stateset-chat', main);
