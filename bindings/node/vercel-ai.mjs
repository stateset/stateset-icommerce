import { resolveToolkit } from './toolkit-helpers.mjs';

export function createVercelAITools(
  commerceOrToolkit,
  {
    tool,
    jsonSchema = null,
    filter = null,
    allowApply = false,
    toolkitOptions = {},
    executionOptions = {},
  } = {},
) {
  const toolkit = resolveToolkit(commerceOrToolkit, { allowApply, toolkitOptions });
  return toolkit.createVercelAITools({
    tool,
    ...(jsonSchema ? { jsonSchema } : {}),
    filter,
    executionOptions,
  });
}
