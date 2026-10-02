/** Browser-safe descriptors, serialized from the StateSet toolkit. */
export interface WebMCPDescriptor {
  name: string;
  description?: string;
  inputSchema?: unknown;
  permission?: string;
  readOnly?: boolean;
  execute?: (
    params: Record<string, unknown>,
    options: WebMCPExecutionOptions,
  ) => unknown | Promise<unknown>;
}

export interface WebMCPExecutionOptions {
  signal?: AbortSignal;
}

export interface WebMCPTool {
  name: string;
  description: string;
  inputSchema: object;
  annotations: { readOnlyHint: boolean; consequentialHint: boolean };
  execute: (params?: Record<string, unknown>, options?: WebMCPExecutionOptions) => Promise<unknown>;
}

/** Structural types avoid depending on experimental browser global declarations. */
export interface WebMCPContext {
  registerTool(tool: WebMCPTool, options: { signal: AbortSignal }): void | Promise<void>;
  /** Compatibility with early implementations which ignore registration signals. */
  unregisterTool?(name: string): void;
}

export interface WebMCPOptions {
  /** null selects all supplied descriptors; [] exposes none. Accepts native __ or . names. */
  filter?: string[] | null;
  /** Trusted host opt-in only. The backend must independently enforce permissions and policy. */
  allowApply?: boolean;
  executeTool?: (
    name: string,
    params: Record<string, unknown>,
    options: WebMCPExecutionOptions,
  ) => unknown | Promise<unknown>;
}

export declare function getWebMCPContext(): WebMCPContext | null;
export declare function createWebMCPTools(
  descriptors: WebMCPDescriptor[],
  options?: WebMCPOptions,
): WebMCPTool[];
export declare function registerWebMCPTools(
  descriptors: WebMCPDescriptor[],
  options?: WebMCPOptions & { modelContext?: WebMCPContext | null; signal?: AbortSignal },
): Promise<{ supported: boolean; toolNames: string[]; dispose: () => void }>;
