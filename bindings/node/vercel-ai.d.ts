import type { Commerce } from './index'
import type {
  CreateEmbeddedAgentToolkitOptions,
  EmbeddedAgentToolkit,
  ToolExecutionOptions,
} from './agent-toolkit'

export type ToolkitTarget = Commerce | EmbeddedAgentToolkit

export declare function createVercelAITools(
  commerceOrToolkit: ToolkitTarget,
  options: {
    tool: (definition: unknown) => any
    filter?: Array<string> | null
    allowApply?: boolean
    toolkitOptions?: CreateEmbeddedAgentToolkitOptions
    executionOptions?: ToolExecutionOptions
  },
): Record<string, any>
