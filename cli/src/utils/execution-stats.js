import { ICONS } from '../output.js';
import { formatSessionRefreshReason } from './session-refresh.js';

/**
 * Print execution stats and optional prompt-budget diagnostics.
 *
 * Returns true when anything was printed.
 */
export function printExecutionStats({
  output,
  ioConsole = console,
  result,
  includePromptReport = true,
  title = 'Execution Stats',
} = {}) {
  const hasPromptReport = includePromptReport && result?.promptReport;
  if (!result?.telemetry && !hasPromptReport) {
    return false;
  }

  const stats = result?.telemetry || {};

  ioConsole.log(`
${output.dim('─'.repeat(40))}`);
  ioConsole.log(`${ICONS.analytics} ${output.bold(title)}`);

  if (result?.traceId) {
    ioConsole.log(`   ${output.dim('Trace ID:')}    ${result.traceId}`);
  }
  if (result?.sessionRefresh) {
    ioConsole.log(
      `   ${output.dim('Session Refresh:')} ${formatSessionRefreshReason(result.sessionRefresh.reason)}`,
    );
    if (result.sessionRefresh.previousSessionId || result.sessionRefresh.sessionId) {
      const fromSession = result.sessionRefresh.previousSessionId || 'none';
      const toSession = result.sessionRefresh.sessionId || 'pending';
      ioConsole.log(`   ${output.dim('Session IDs:')} ${fromSession} -> ${toSession}`);
    }
    if (result.sessionRefresh.replayedMessages > 0) {
      ioConsole.log(
        `   ${output.dim('Replayed:')}    ${result.sessionRefresh.replayedMessages} prior messages`,
      );
    }
  }
  if (stats.duration !== undefined) {
    ioConsole.log(`   ${output.dim('Duration:')}    ${stats.duration}ms`);
  }
  if (stats.toolCalls) {
    ioConsole.log(
      `   ${output.dim('Tool Calls:')}  ${stats.toolCalls.total || 0} (${stats.toolCalls.successRate || 'N/A'} success)`,
    );
  }
  if (stats.avgToolDuration > 0) {
    ioConsole.log(`   ${output.dim('Avg Latency:')} ${stats.avgToolDuration}ms per tool`);
  }
  if (result?.provider) {
    ioConsole.log(`   ${output.dim('Provider:')}    ${result.provider}`);
  }
  if (result?.cost !== null && result?.cost !== undefined) {
    ioConsole.log(`   ${output.dim('Cost:')}        $${result.cost.toFixed(4)}`);
  }
  if (result?.budgetExceeded) {
    ioConsole.log(`   ${output.yellow('Budget exceeded')}`);
  }
  if (result?.thinkLevel && result.thinkLevel !== 'off') {
    ioConsole.log(`   ${output.dim('Thinking:')}    ${result.thinkLevel}`);
  }
  if (hasPromptReport) {
    ioConsole.log(`
${output.promptReport(result.promptReport)}`);
  }

  return true;
}
