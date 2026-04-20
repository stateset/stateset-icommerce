import type { Commerce } from './index'
import type {
  CreateEmbeddedAgentToolkitOptions,
  EmbeddedAgentToolkit,
  ToolCallPayload,
  ToolDescriptor,
  ToolExecutionOptions,
} from './agent-toolkit'

export type ToolkitTarget = Commerce | EmbeddedAgentToolkit

export declare function createToolDescriptors(
  commerceOrToolkit: ToolkitTarget,
  options?: {
    filter?: Array<string> | null
    allowApply?: boolean
    toolkitOptions?: CreateEmbeddedAgentToolkitOptions
    executionOptions?: ToolExecutionOptions
  },
): Array<ToolDescriptor>

export declare function createCallableRegistry(
  commerceOrToolkit: ToolkitTarget,
  options?: {
    filter?: Array<string> | null
    allowApply?: boolean
    toolkitOptions?: CreateEmbeddedAgentToolkitOptions
    executionOptions?: ToolExecutionOptions
  },
): Record<string, (params?: Record<string, unknown>) => Promise<any>>

export declare function executeTool(
  commerceOrToolkit: ToolkitTarget,
  toolName: string,
  params?: Record<string, unknown>,
  options?: {
    allowApply?: boolean
    toolkitOptions?: CreateEmbeddedAgentToolkitOptions
    executionOptions?: ToolExecutionOptions
  },
): Promise<any>

export declare function executeToolCalls(
  commerceOrToolkit: ToolkitTarget,
  toolCalls: Array<ToolCallPayload>,
  options?: {
    allowApply?: boolean
    toolkitOptions?: CreateEmbeddedAgentToolkitOptions
    executionOptions?: ToolExecutionOptions
  },
): Promise<Array<any>>
