import { filterByToolName, resolveToolkit } from './toolkit-helpers.mjs';

export function createOpenAITools(
  commerceOrToolkit,
  { filter = null, allowApply = false, toolkitOptions = {} } = {},
) {
  const toolkit = resolveToolkit(commerceOrToolkit, { allowApply, toolkitOptions });
  return filterByToolName(toolkit.getTools({ format: 'openai' }), filter, (tool) => tool?.function?.name);
}

export async function executeOpenAIToolCall(
  commerceOrToolkit,
  toolCall,
  { allowApply = false, toolkitOptions = {}, executionOptions = {} } = {},
) {
  const toolkit = resolveToolkit(commerceOrToolkit, { allowApply, toolkitOptions });
  return toolkit.executeOpenAIToolCall(toolCall, executionOptions);
}

export async function executeOpenAIToolCalls(
  commerceOrToolkit,
  toolCalls,
  { allowApply = false, toolkitOptions = {}, executionOptions = {} } = {},
) {
  const toolkit = resolveToolkit(commerceOrToolkit, { allowApply, toolkitOptions });
  return toolkit.executeToolCalls(toolCalls, executionOptions);
}
