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
    /** The AI SDK's `jsonSchema()` helper. Used by the native toolkit to wrap the JSON Schema it emits. */
    jsonSchema?: (schema: Record<string, unknown>) => unknown
    filter?: Array<string> | null
    allowApply?: boolean
    toolkitOptions?: CreateEmbeddedAgentToolkitOptions
    executionOptions?: ToolExecutionOptions
  },
): Record<string, any>
