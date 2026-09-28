/**
 * Explain Commands Module
 *
 * Tool-backed: dispatches to the explain MCP tool definitions (explain_order,
 * explain_cart_pricing) so the CLI surface stays in lockstep with the tool
 * surface. Run with no action (or `help`) for the generated action list;
 * parameters are key=value pairs. Both tools are read-only.
 */

import { explainTools } from '../tools/explain.js';
import { createToolBackedCommand } from '../utils/tool-backed-command.js';

export const { execute, metadata, toolActionMap } = createToolBackedCommand(
  'explain',
  explainTools,
);
