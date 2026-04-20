import type { Commerce } from './index'
import type {
  CreateEmbeddedAgentToolkitOptions,
  EmbeddedAgentToolkit,
  ToolExecutionOptions,
} from './agent-toolkit'

export type ToolkitTarget = Commerce | EmbeddedAgentToolkit

export declare function createLangChainTools(
  commerceOrToolkit: ToolkitTarget,
  options: {
    DynamicStructuredTool: new (config: Record<string, unknown>) => any
    filter?: Array<string> | null
    allowApply?: boolean
    toolkitOptions?: CreateEmbeddedAgentToolkitOptions
    executionOptions?: ToolExecutionOptions
  },
): Array<any>
