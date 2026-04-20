import { resolveToolkit } from './toolkit-helpers.mjs';

export function createToolDescriptors(
  commerceOrToolkit,
  { filter = null, allowApply = false, toolkitOptions = {}, executionOptions = {} } = {},
) {
  const toolkit = resolveToolkit(commerceOrToolkit, { allowApply, toolkitOptions });
  return toolkit.createToolDescriptors({ filter, executionOptions });
}

export function createCallableRegistry(
  commerceOrToolkit,
  { filter = null, allowApply = false, toolkitOptions = {}, executionOptions = {} } = {},
) {
  const descriptors = createToolDescriptors(commerceOrToolkit, {
    filter,
    allowApply,
    toolkitOptions,
    executionOptions,
  });

  return Object.fromEntries(
    descriptors.map((descriptor) => [
      descriptor.name,
      (params = {}) => descriptor.execute(params),
    ]),
  );
}

export async function executeTool(
  commerceOrToolkit,
  toolName,
  params = {},
  { allowApply = false, toolkitOptions = {}, executionOptions = {} } = {},
) {
  const toolkit = resolveToolkit(commerceOrToolkit, { allowApply, toolkitOptions });
  return toolkit.executeTool(toolName, params, executionOptions);
}

export async function executeToolCalls(
  commerceOrToolkit,
  toolCalls,
  { allowApply = false, toolkitOptions = {}, executionOptions = {} } = {},
) {
  const toolkit = resolveToolkit(commerceOrToolkit, { allowApply, toolkitOptions });
  return toolkit.executeToolCalls(toolCalls, executionOptions);
}
