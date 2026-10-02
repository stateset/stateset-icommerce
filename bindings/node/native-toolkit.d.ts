import type { Commerce } from './index'
import type {
  OpenAIToolDefinition,
  OpenAIToolExecution,
  ToolCallPayload,
  ToolExecutionOptions,
} from './agent-toolkit'

/** A JSON Schema fragment as emitted in `tool-descriptors.json`. */
export type JsonSchema = Record<string, unknown>

/** One entry of `tool-descriptors.json`, generated from `index.d.ts`. */
export interface NativeToolDescriptor {
  /** Canonical name, `<getter>.<method>` (e.g. `orders.create`). */
  name: string
  /** The `Commerce` getter the method lives on (e.g. `orders`). */
  module: string
  /** The declared class behind the getter (e.g. `Orders`). */
  className: string
  method: string
  description: string
  /** The method signature as declared in `index.d.ts`. */
  signature: string
  returnType: string
  /** Read tools execute without `allowApply`; writes preview instead. */
  readOnly: boolean
  permission: 'read' | 'write'
  /** Parameter names in call order; `parameters.properties` uses the same keys. */
  positional: Array<string>
  /** JSON Schema for the named parameters, interfaces inlined. */
  parameters: JsonSchema & {
    type: 'object'
    properties: Record<string, JsonSchema>
    required?: Array<string>
    additionalProperties: false
  }
}

export interface NativeToolCatalogMeta {
  generator: string
  source: string
  modules: Array<{ getter: string; className: string }>
  toolCount: number
  readOnlyCount: number
  readOnlyPrefixes: Array<string>
  readOnlyOverrides: Record<string, boolean>
  warnings: Array<string>
}

export type NativeToolFilter =
  | Array<string>
  | ((tool: NativeToolDescriptor) => boolean)
  | null
  | undefined

export interface NativeRawTool {
  name: string
  /** Name with `.` replaced by `__`, safe for OpenAI/Anthropic/MCP wire formats. */
  wireName: string
  description: string
  inputSchema: JsonSchema
  permission: 'read' | 'write'
  readOnly: boolean
  module: string
  method: string
  positional: Array<string>
  returnType: string
  signature: string
  runtime: 'native'
}

export interface AnthropicToolDefinition {
  name: string
  description: string
  input_schema: JsonSchema
}

export interface McpToolDefinition {
  name: string
  title: string
  description: string
  inputSchema: JsonSchema
  annotations: { title: string; readOnlyHint: boolean; openWorldHint: boolean }
}

export type NativeToolFormat =
  | 'generic'
  | 'raw'
  | 'openai'
  | 'openai-responses'
  | 'chat-completions'
  | 'anthropic'
  | 'anthropic-sdk'
  | 'anthropic-messages'
  | 'claude'
  | 'mcp'

/** Returned instead of executing a write tool when `allowApply` is false. */
export interface NativeToolPreview {
  preview: true
  tool: string
  params: Record<string, unknown>
  note: string
}

/** Returned when the engine (or argument mapping) rejects a call. */
export interface NativeToolError {
  error: {
    /** `err.code` from the binding (`NOT_FOUND`, `VALIDATION`, …) or `TOOL_NOT_FOUND` / `INVALID_ARGUMENT` / `UNSUPPORTED`. */
    code: string
    message: string
    details: unknown
    tool: string
  }
}

export type NativeToolResult = NativeToolPreview | NativeToolError | { ok: true } | unknown

export interface NativeToolDescriptorHandle {
  name: string
  wireName: string
  description: string
  schema: JsonSchema
  inputSchema: JsonSchema
  permission: 'read' | 'write'
  readOnly: boolean
  runtime: 'native'
  execute: (params?: Record<string, unknown> | string) => Promise<NativeToolResult>
}

export interface NativeToolkit {
  engine: 'stateset-icommerce'
  backend: 'native'
  runtime: 'native'
  allowApply: boolean
  commerce: Commerce
  getTools(options: { format: 'openai' | 'openai-responses' | 'chat-completions' }): Array<OpenAIToolDefinition>
  getTools(options: { format: 'anthropic' | 'anthropic-sdk' | 'anthropic-messages' | 'claude' }): Array<AnthropicToolDefinition>
  getTools(options: { format: 'mcp' }): Array<McpToolDefinition>
  getTools(options?: { format?: 'generic' | 'raw' }): Array<NativeRawTool>
  listTools(options?: { format?: NativeToolFormat }): Array<unknown>
  getRawTools(): Array<NativeRawTool>
  getTool(toolName: string, options?: { format?: NativeToolFormat }): unknown
  getRawTool(toolName: string): NativeRawTool | undefined
  executeTool(
    toolName: string,
    params?: Record<string, unknown> | string,
    executionOptions?: ToolExecutionOptions,
  ): Promise<NativeToolResult>
  executeToolCalls(
    toolCalls?: Array<ToolCallPayload>,
    executionOptions?: ToolExecutionOptions,
  ): Promise<Array<{ callId: string | null; name: string; arguments: Record<string, unknown>; result: NativeToolResult } | OpenAIToolExecution>>
  executeOpenAIToolCall(
    toolCall: ToolCallPayload,
    executionOptions?: ToolExecutionOptions,
  ): Promise<OpenAIToolExecution>
  createToolDescriptors(options?: {
    filter?: NativeToolFilter
    executionOptions?: ToolExecutionOptions
  }): Array<NativeToolDescriptorHandle>
  createVercelAITools(options: {
    tool: (definition: unknown) => any
    filter?: NativeToolFilter
    executionOptions?: ToolExecutionOptions
    /** The AI SDK's `jsonSchema()` helper; when given, schemas are wrapped with it. */
    jsonSchema?: (schema: JsonSchema) => unknown
  }): Record<string, any>
  createLangChainTools(options: {
    DynamicStructuredTool: new (config: Record<string, unknown>) => any
    filter?: NativeToolFilter
    executionOptions?: ToolExecutionOptions
  }): Array<any>
  /** Always `false`: the toolkit never owns the Commerce instance. */
  close(): boolean
}

export interface CreateNativeToolkitOptions {
  /** Execute write tools. Defaults to false: writes return a preview object. */
  allowApply?: boolean
  /** Tool names (`orders.create`, `orders__create`, `orders.*`) or a predicate. */
  filter?: NativeToolFilter
}

export declare function createNativeToolkit(
  commerce: Commerce,
  options?: CreateNativeToolkitOptions,
): NativeToolkit

/** `orders.create` → `orders__create`. */
export declare function toWireName(name: string): string
/** `orders__create` (or an `mcp__server__` prefixed form) → `orders.create`. */
export declare function toCanonicalName(name: string): string
/** The static catalog with interface schemas inlined. */
export declare function loadToolDescriptors(): {
  meta: NativeToolCatalogMeta
  tools: Array<NativeToolDescriptor>
}

export default createNativeToolkit
