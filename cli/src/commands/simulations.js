import { simulationTools } from '../tools/simulations.js';
import { createToolBackedCommand } from '../utils/tool-backed-command.js';

export const { execute, metadata, toolActionMap } = createToolBackedCommand(
  'simulations',
  simulationTools,
);
