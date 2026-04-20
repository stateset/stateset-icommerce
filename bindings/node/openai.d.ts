import type { Commerce } from './index'
import type {
  CreateEmbeddedAgentToolkitOptions,
  EmbeddedAgentToolkit,
  OpenAIToolDefinition,
  OpenAIToolExecution,
  ToolCallPayload,
  ToolExecutionOptions,
} from './agent-toolkit'

export type ToolkitTarget = Commerce | EmbeddedAgentToolkit

export declare function createOpenAITools(
  commerceOrToolkit: ToolkitTarget,
  options?: {
    filter?: Array<string> | null
    allowApply?: boolean
    toolkitOptions?: CreateEmbeddedAgentToolkitOptions
  },
): Array<OpenAIToolDefinition>

export declare function executeOpenAIToolCall(
  commerceOrToolkit: ToolkitTarget,
  toolCall: ToolCallPayload,
  options?: {
    allowApply?: boolean
    toolkitOptions?: CreateEmbeddedAgentToolkitOptions
    executionOptions?: ToolExecutionOptions
  },
): Promise<OpenAIToolExecution>

export declare function executeOpenAIToolCalls(
  commerceOrToolkit: ToolkitTarget,
  toolCalls: Array<ToolCallPayload>,
  options?: {
    allowApply?: boolean
    toolkitOptions?: CreateEmbeddedAgentToolkitOptions
    executionOptions?: ToolExecutionOptions
  },
): Promise<Array<any>>
