/**
 * Cycle Counts Commands Module
 *
 * Tool-backed: dispatches to the cycle-counts MCP tool definitions so the CLI
 * surface stays in lockstep with the tool surface. Run with no action (or
 * `help`) for the generated action list; parameters are key=value pairs and
 * write operations require --apply.
 */

import { cycleCountTools } from '../tools/cycle-counts.js';
import { createToolBackedCommand } from '../utils/tool-backed-command.js';

export const { execute, metadata, toolActionMap } = createToolBackedCommand(
  'cycle-counts',
  cycleCountTools,
);
