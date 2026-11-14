#!/usr/bin/env node
/**
 * StateSet Agent Report
 *
 * Summarise what each agent session did, from the MCP server's replay log
 * (`agentic-tool-calls.jsonl`). Read-only: it never writes to the log or to
 * a store.
 *
 * Usage:
 *   stateset-agent-report --log .stateset/agentic-tool-calls.jsonl
 *   stateset-agent-report --log <path> --session <id>
 *   stateset-agent-report --log <path> --json
 */

import { parseArgs } from 'node:util';
import { CLI_VERSION } from '../src/config.js';
import { runMain } from '../src/graceful-shutdown.js';
import { buildSessionReportFromFile, formatSessionReport } from '../src/mcp/session-report.js';

const HELP = `
StateSet Agent Report - what each agent session did

Reads the MCP server's replay log and reports, per session: calls and plan
steps by status and tool, committed writes apart from previews and dry runs,
refusals grouped by reason code, errors by code, retries that replayed an
earlier kernel receipt, charges, and latency (p50 / p95 / max). Read-only.

Usage:
  stateset-agent-report --log <path> [--session <id>] [--json]

Options:
  --log <path>       Replay log to read (agentic-tool-calls.jsonl; the MCP
                     server writes it under ./.stateset by default)
  --session <id>     Report one session only ("(unsessioned)" selects events
                     logged without a session id, or whose session id is only
                     their own request id)
  --json             Print the report object as JSON (sessions as an array,
                     in the order they first appear in the log)
  -h, --help         Show this help
  -v, --version      Show version

Exit codes:
  0  report printed
  1  the log could not be read, or --session matched no events
  2  bad usage (including an empty --session)

Examples:
  stateset-agent-report --log .stateset/agentic-tool-calls.jsonl
  stateset-agent-report --log .stateset/agentic-tool-calls.jsonl --session sess-42 --json
`;

async function main() {
  let values;
  try {
    ({ values } = parseArgs({
      options: {
        log: { type: 'string' },
        session: { type: 'string' },
        json: { type: 'boolean', default: false },
        help: { type: 'boolean', short: 'h', default: false },
        version: { type: 'boolean', short: 'v', default: false },
      },
      allowPositionals: false,
    }));
  } catch (error) {
    console.error(`stateset-agent-report: ${error.message}`);
    console.error('Run stateset-agent-report --help for usage.');
    process.exitCode = 2;
    return;
  }

  if (values.help) {
    console.log(HELP);
    return;
  }
  if (values.version) {
    console.log(`stateset-agent-report v${CLI_VERSION}`);
    return;
  }
  if (!values.log) {
    console.error('stateset-agent-report: --log <path> is required.');
    console.error('Run stateset-agent-report --help for usage.');
    process.exitCode = 2;
    return;
  }
  if (values.session !== undefined && !values.session.trim()) {
    console.error('stateset-agent-report: --session needs a session id.');
    console.error('Run stateset-agent-report --help for usage.');
    process.exitCode = 2;
    return;
  }

  let report;
  try {
    report = await buildSessionReportFromFile(values.log, { sessionId: values.session || null });
  } catch (error) {
    console.error(`stateset-agent-report: cannot read ${values.log}: ${error.message}`);
    process.exitCode = 1;
    return;
  }

  if (values.json) {
    console.log(JSON.stringify({ logPath: values.log, ...report }, null, 2));
  } else {
    console.log(formatSessionReport(report, { logPath: values.log }));
  }

  if (values.session && report.sessionCount === 0) {
    console.error(`stateset-agent-report: no events for session ${values.session}`);
    process.exitCode = 1;
  }
}

runMain('stateset-agent-report', main);
