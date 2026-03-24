import { parseArgs } from 'node:util';

import { runAgentLoop } from '../claude-harness.js';
import { DEFAULT_MODEL, CLI_VERSION } from '../config.js';
import { createOutput, ICONS } from '../output.js';
import { createConfirmHandler } from './confirm.js';
import { buildAgentOutputData, resolveOutputFormat, writeAgentOutputFile } from './agent-output.js';
import { resolveAgentRuntimeOptions, createStreamingHandler } from './agent-runtime.js';
import { printExecutionStats } from './execution-stats.js';

/**
 * Build the standard options object used by commerce agent CLIs.
 */
export function buildAgentCliOptions({ allowApply = true } = {}) {
  return {
    db: { type: 'string', default: './store.db' },
    ...(allowApply ? { apply: { type: 'boolean', default: false } } : {}),
    model: { type: 'string', default: DEFAULT_MODEL },
    provider: { type: 'string' },
    think: { type: 'string', default: 'off' },
    stream: { type: 'boolean', default: false },
    budget: { type: 'string' },
    memory: { type: 'boolean', default: false },
    'no-memory': { type: 'boolean', default: false },
    x402: { type: 'boolean', default: false },
    resume: { type: 'string' },
    json: { type: 'boolean', default: false },
    format: { type: 'string', default: 'table' },
    output: { type: 'string' },
    stats: { type: 'boolean', default: false },
    ...(allowApply ? { yes: { type: 'boolean', short: 'y', default: false } } : {}),
    help: { type: 'boolean', short: 'h', default: false },
    version: { type: 'boolean', short: 'v', default: false },
  };
}

function resolveModeLabel(config, values, allowApply) {
  if (typeof config.modeLabel === 'function') {
    return config.modeLabel(values, allowApply);
  }
  if (typeof config.modeLabel === 'string') {
    return config.modeLabel;
  }
  if (config.allowApply === false) {
    return '👁️  Read-only';
  }
  return allowApply ? '✏️  Write enabled' : '👁️  Preview only';
}

/**
 * Run a standard commerce agent CLI invocation.
 *
 * Returns the exit code instead of exiting, which keeps it testable.
 */
export async function runAgentCli(config, deps = {}) {
  const parseArgsFn = deps.parseArgsFn || parseArgs;
  const argv = deps.argv || process.argv;
  const stdin = deps.stdin || process.stdin;
  const ioConsole = deps.console || console;
  const runAgentLoopFn = deps.runAgentLoopFn || runAgentLoop;
  const createConfirmHandlerFn = deps.createConfirmHandlerFn || createConfirmHandler;
  const buildAgentOutputDataFn = deps.buildAgentOutputDataFn || buildAgentOutputData;
  const resolveOutputFormatFn = deps.resolveOutputFormatFn || resolveOutputFormat;
  const writeAgentOutputFileFn = deps.writeAgentOutputFileFn || writeAgentOutputFile;
  const resolveAgentRuntimeOptionsFn =
    deps.resolveAgentRuntimeOptionsFn || resolveAgentRuntimeOptions;
  const createStreamingHandlerFn = deps.createStreamingHandlerFn || createStreamingHandler;
  const createOutputFn = deps.createOutputFn || createOutput;

  const allowApplyOption = config.allowApply !== false;
  const { values, positionals } = parseArgsFn({
    options: buildAgentCliOptions({ allowApply: allowApplyOption }),
    allowPositionals: true,
    args: argv.slice(2),
  });

  if (values.help) {
    ioConsole.log(config.help);
    return 0;
  }

  if (values.version) {
    ioConsole.log(
      `@stateset/cli ${config.versionLabel || `${config.agent}-agent`} v${CLI_VERSION}`,
    );
    return 0;
  }

  const request = positionals.join(' ').trim();
  if (!request) {
    ioConsole.error('Error: No request provided');
    ioConsole.error(`Usage: ${config.commandName} "<your request>"`);
    ioConsole.error(`Run ${config.commandName} --help for more information`);
    return 1;
  }

  const outputFormat = resolveOutputFormatFn({
    format: values.format,
    json: values.json,
    argv,
  });
  const isJsonOutput = outputFormat === 'json';
  const output = createOutputFn({ format: isJsonOutput ? 'json' : 'pretty' });

  if (values.stream && isJsonOutput) {
    ioConsole.error(
      'Error: --stream cannot be used with JSON output. Remove --stream or use a non-JSON format.',
    );
    return 1;
  }

  let runtimeOptions;
  try {
    runtimeOptions = resolveAgentRuntimeOptionsFn(values);
  } catch (error) {
    if (isJsonOutput) {
      ioConsole.log(JSON.stringify({ error: error.message }));
    } else {
      ioConsole.error(`
❌ Error: ${error.message}`);
    }
    return 1;
  }

  const { thinkLevel, providerName, streaming, maxBudgetUsd, memoryOverride, enableX402 } =
    runtimeOptions;
  const allowApply = allowApplyOption ? values.apply : false;

  if (!isJsonOutput) {
    ioConsole.log(`
${config.icon} ${config.title}`);
    ioConsole.log(`   Database: ${values.db}`);
    ioConsole.log(`   Mode: ${resolveModeLabel(config, values, allowApply)}`);
    if (values.resume) {
      ioConsole.log(`   Session: ${values.resume}`);
    }
    ioConsole.log();
  }

  try {
    const nonInteractive = !stdin.isTTY || isJsonOutput;
    const onConfirmRequired = allowApplyOption
      ? createConfirmHandlerFn({
          output: null,
          assumeYes: values.yes,
          nonInteractive,
        })
      : undefined;

    const result = await runAgentLoopFn({
      request,
      dbPath: values.db,
      model: values.model,
      allowApply,
      resumeSessionId: values.resume,
      agent: config.agent,
      onConfirmRequired,
      thinkLevel,
      streaming,
      maxBudgetUsd,
      provider: providerName,
      enableMemory: memoryOverride === null ? null : memoryOverride,
      enableX402,
      onPartialMessage: createStreamingHandlerFn(streaming),
      onToolCall: (toolCall) => {
        if (!isJsonOutput) {
          ioConsole.log(output.toolCall(toolCall.name, toolCall.input));
        }
      },
    });

    const outputData = buildAgentOutputDataFn({
      agent: config.agent,
      request,
      allowApply,
      result,
      includeTelemetry: values.stats,
      includePromptReport: values.stats,
    });

    if (values.output) {
      await writeAgentOutputFileFn(values.output, outputData, outputFormat);
      if (!isJsonOutput) {
        ioConsole.log(`Output written to ${values.output}`);
      }
    } else if (isJsonOutput) {
      ioConsole.log(JSON.stringify(outputData, null, 2));
    } else {
      if (streaming && result.response) {
        ioConsole.log();
      } else {
        ioConsole.log(`
${result.response}`);
      }

      if (values.stats) {
        printExecutionStats({ output, ioConsole, result, includePromptReport: true });
      }

      if (result.sessionId) {
        ioConsole.log(`
${ICONS.session} Session ID: ${result.sessionId}`);
        ioConsole.log(
          `   Use --resume ${result.sessionId} to continue this ${config.resumeTarget || 'conversation'}`,
        );
      }
    }

    return 0;
  } catch (error) {
    if (isJsonOutput) {
      ioConsole.log(JSON.stringify({ error: error.message }));
    } else {
      ioConsole.error(`
❌ Error: ${error.message}`);
    }
    return 1;
  }
}

/**
 * Create a main() function for use with runMain.
 */
export function createAgentCliMain(config, deps = {}) {
  return async function main() {
    const exitCode = await runAgentCli(config, deps);
    process.exit(exitCode);
  };
}
