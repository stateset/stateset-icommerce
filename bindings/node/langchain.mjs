import { resolveToolkit } from './toolkit-helpers.mjs';

export function createLangChainTools(
  commerceOrToolkit,
  {
    DynamicStructuredTool,
    filter = null,
    allowApply = false,
    toolkitOptions = {},
    executionOptions = {},
  } = {},
) {
  const toolkit = resolveToolkit(commerceOrToolkit, { allowApply, toolkitOptions });
  return toolkit.createLangChainTools({
    DynamicStructuredTool,
    filter,
    executionOptions,
  });
}
