/**
 * Price Levels Commands Module
 *
 * Tool-backed: dispatches to the price-levels MCP tool definitions so the CLI
 * surface stays in lockstep with the tool surface. Run with no action (or
 * `help`) for the generated action list; parameters are key=value pairs and
 * write operations require --apply.
 */

import { priceLevelTools } from '../tools/price-levels.js';
import { createToolBackedCommand } from '../utils/tool-backed-command.js';

export const { execute, metadata, toolActionMap } = createToolBackedCommand(
  'price-levels',
  priceLevelTools,
);
