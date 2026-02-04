import fs from 'node:fs/promises';

import { formatStructuredOutput } from '../output.js';

/**
 * Resolve output format from flags.
 * --format wins when explicitly provided; otherwise --json implies json.
 */
export function resolveOutputFormat({ format = 'table', json = false, argv = process.argv } = {}) {
  const normalized = typeof format === 'string' ? format.toLowerCase() : 'table';
  const hasFormatFlag = Array.isArray(argv)
    ? argv.some(arg => arg === '--format' || arg.startsWith('--format='))
    : false;

  if (hasFormatFlag) {
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
export function buildAgentOutputData({ agent, request, allowApply, result }) {
  const output = {
    agent,
    request,
    sessionId: result?.sessionId,
    traceId: result?.traceId,
    response: result?.response,
    toolResults: (result?.toolResults || []).map(tr => ({
      tool: tr.toolCall.name,
      input: tr.toolCall.input,
      result: tr.result
    }))
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

  return output;
}

/**
 * Write structured output to a file.
 */
export async function writeAgentOutputFile(outputPath, data, format = 'table') {
  const formatted = formatStructuredOutput(data, format);
  await fs.writeFile(outputPath, formatted);
}
