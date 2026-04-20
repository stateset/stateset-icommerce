import { resolveToolkit } from './toolkit-helpers.mjs';

export function createVercelAITools(
  commerceOrToolkit,
  {
    tool,
    filter = null,
    allowApply = false,
    toolkitOptions = {},
    executionOptions = {},
  } = {},
) {
  const toolkit = resolveToolkit(commerceOrToolkit, { allowApply, toolkitOptions });
  return toolkit.createVercelAITools({
    tool,
    filter,
    executionOptions,
  });
}
