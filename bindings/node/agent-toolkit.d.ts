import type { Commerce } from './index'

export interface ToolExecutionOptions {
  payment?: Record<string, unknown>
  http?: Record<string, unknown>
  [key: string]: unknown
}

export interface CreateEmbeddedAgentToolkitOptions {
  dbPath?: string
  allowApply?: boolean
  /** Required when allowApply is true. Supports exact tool names, read:*, domain.*, and kernel capabilities. */
  capabilities?: Array<string>
  /** Trusted host configuration for governed high-risk mutations. */
  kernel?: {
    policy: Record<string, unknown>
    principal: Record<string, unknown>
    storeId: string
    approval?: Record<string, unknown> | ((command: Record<string, unknown>) => unknown)
    authorize?: (command: Record<string, unknown>) => unknown
  }
  commerce?: Commerce
  autonomousEngine?: unknown | null
  policyStorePath?: string
  treasury?: Record<string, unknown>
  mpp?: Record<string, unknown>
  agentId?: string | null
  [key: string]: unknown
}

export interface OpenAIToolDefinition {
  type?: string
  function: {
    name: string
    description?: string
    parameters?: unknown
    [key: string]: unknown
  }
  [key: string]: unknown
}

export interface RawToolDefinition {
  name: string
  description?: string
  inputSchema?: unknown
  permission?: string
  policyDomain?: string
  runtime?: string
  [key: string]: unknown
}

export interface PrepareToolPaymentOptions {
  params?: Record<string, unknown>
  requestId?: string | null
  sessionId?: string | null
  includeSchema?: boolean
}

export interface ToolDescriptor {
  name: string
  description?: string
  schema?: unknown
  inputSchema?: unknown
  permission?: string
  policyDomain?: string
  runtime?: string
  preparePayment?: (options?: PrepareToolPaymentOptions) => Promise<unknown>
  execute: (params?: Record<string, unknown>) => Promise<any>
  executeWithPayment?: (
    params?: Record<string, unknown>,
    paymentOptions?: Record<string, unknown>,
  ) => Promise<any>
  [key: string]: unknown
}

export interface ToolCallPayload {
  callId?: string | null
  id?: string | null
  name?: string
  tool?: string
  arguments?: Record<string, unknown> | string | null
  params?: Record<string, unknown> | string | null
  function?: {
    name?: string
    arguments?: Record<string, unknown> | string | null
  }
  [key: string]: unknown
}

export interface OpenAIToolExecution {
  callId: string | null
  name: string
  arguments: Record<string, unknown>
  result: any
  outputMessage: {
    type: 'function_call_output'
    call_id: string
    output: string
  } | null
}

export interface EmbeddedAgentToolkit {
  engine: string
  dbPath: string
  commerce: Commerce
  server: unknown
  getTools(options?: { format?: string }): Array<unknown>
  listTools(options?: { format?: string }): Array<unknown>
  getRawTools(): Array<RawToolDefinition>
  getToolCatalog(options?: Record<string, unknown>): Promise<unknown>
  getPayableToolCatalog(options?: Record<string, unknown>): Promise<unknown>
  getPaymentDiscovery(options?: Record<string, unknown>): Promise<unknown>
  discoverPayableTools(options?: Record<string, unknown>): Promise<unknown>
  prepareToolPayment(options?: Record<string, unknown>): Promise<unknown>
  createHttpPaymentAgent(options?: Record<string, unknown>): {
    fetch: (url: string, requestOptions?: Record<string, unknown>) => Promise<any>
  }
  discoverRemotePaymentService(baseUrl: string, options?: Record<string, unknown>): Promise<unknown>
  discoverRemotePayableRoutes(baseUrl: string, options?: Record<string, unknown>): Promise<unknown>
  executeRemoteHttpRoute(
    baseUrl: string,
    route: Record<string, unknown>,
    request?: Record<string, unknown>,
    executionOptions?: ToolExecutionOptions,
  ): Promise<any>
  createRemoteHttpToolDescriptors(
    baseUrl: string,
    options?: Record<string, unknown>,
  ): Promise<Array<unknown>>
  getTool(toolName: string, options?: { format?: string }): unknown
  getRawTool(toolName: string): RawToolDefinition | undefined
  executeTool(
    toolName: string,
    params?: Record<string, unknown>,
    executionOptions?: ToolExecutionOptions,
  ): Promise<any>
  executeToolWithPayment(
    toolName: string,
    params?: Record<string, unknown>,
    executionOptions?: ToolExecutionOptions,
  ): Promise<any>
  executePaidTool(
    toolName: string,
    params?: Record<string, unknown>,
    executionOptions?: ToolExecutionOptions,
  ): Promise<any>
  executeToolCalls(
    toolCalls?: Array<ToolCallPayload>,
    executionOptions?: ToolExecutionOptions,
  ): Promise<Array<any>>
  executePlan(options?: Record<string, unknown>): Promise<unknown>
  simulatePlan(options?: Record<string, unknown>): Promise<unknown>
  getRuntimeContract(options?: Record<string, unknown>): Promise<unknown>
  simulateMutation(options?: Record<string, unknown>): Promise<unknown>
  replayMutation(options?: Record<string, unknown>): Promise<unknown>
  getReplayLog(options?: Record<string, unknown>): Promise<unknown>
  executeOpenAIToolCall(
    toolCall: ToolCallPayload,
    executionOptions?: ToolExecutionOptions,
  ): Promise<OpenAIToolExecution>
  executePaidOpenAIToolCall(
    toolCall: ToolCallPayload,
    executionOptions?: ToolExecutionOptions,
  ): Promise<OpenAIToolExecution>
  createVercelAITools(options: {
    tool: (definition: unknown) => any
    filter?: Array<string> | null
    executionOptions?: ToolExecutionOptions
  }): Record<string, any>
  createLangChainTools(options: {
    DynamicStructuredTool: new (config: Record<string, unknown>) => any
    filter?: Array<string> | null
    executionOptions?: ToolExecutionOptions
  }): Array<any>
  createToolDescriptors(options?: {
    filter?: Array<string> | null
    executionOptions?: ToolExecutionOptions
  }): Array<ToolDescriptor>
  close(): boolean
}

export declare function createEmbeddedAgentToolkit(
  options?: CreateEmbeddedAgentToolkitOptions,
): EmbeddedAgentToolkit

export declare const createEmbeddedAgentKit: typeof createEmbeddedAgentToolkit

export default createEmbeddedAgentToolkit
