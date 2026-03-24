import fs from 'node:fs/promises';

import { formatStructuredOutput } from '../output.js';

/**
 * Resolve output format from flags.
 * --format wins when explicitly provided; otherwise --json implies json.
 */
export function resolveOutputFormat({ format = 'table', json = false, argv = process.argv } = {}) {
  const args = Array.isArray(argv) ? argv : [];
  let formatFromArgv = null;
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === '--format' && args[i + 1] && !args[i + 1].startsWith('-')) {
      formatFromArgv = args[i + 1];
      break;
    }
    if (arg.startsWith('--format=')) {
      formatFromArgv = arg.split('=').slice(1).join('=');
      break;
    }
  }

  const normalized =
    typeof (formatFromArgv || format) === 'string'
      ? (formatFromArgv || format).toLowerCase()
      : 'table';

  if (formatFromArgv) {
    return normalized;
  }

  if (json) {
    return 'json';
  }

  return normalized || 'table';
}

/**
 * Build standard agent output payload.
 */
export function buildAgentOutputData({
  agent,
  request,
  allowApply,
  result,
  includeTelemetry = false,
  includePromptReport = false,
}) {
  const output = {
    agent,
    request,
    sessionId: result?.sessionId,
    traceId: result?.traceId,
    response: result?.response,
    toolResults: (result?.toolResults || []).map((tr) => ({
      tool: tr.toolCall.name,
      input: tr.toolCall.input,
      result: tr.result,
    })),
  };

  if (allowApply !== undefined) {
    output.allowApply = allowApply;
  }

  if (result?.provider) {
    output.provider = result.provider;
  }

  if (result?.usedModel || result?.model) {
    output.model = result.usedModel || result.model;
  }

  if (result?.cost !== undefined) {
    output.cost = result.cost;
  }

  if (result?.budgetExceeded !== undefined) {
    output.budgetExceeded = result.budgetExceeded;
  }

  if (result?.routing) {
    output.routing = result.routing;
  }

  if (includeTelemetry && result?.telemetry) {
    output.telemetry = result.telemetry;
  }

  if (includePromptReport && result?.promptReport) {
    output.promptReport = result.promptReport;
  }

  return output;
}

/**
 * Write structured output to a file.
 */
export async function writeAgentOutputFile(outputPath, data, format = 'table') {
  const formatted = formatStructuredOutput(data, format);
  await fs.writeFile(outputPath, formatted);
}
