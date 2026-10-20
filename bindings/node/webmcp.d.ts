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

/** Approval request handed to the host's `onConfirm` callback. */
export interface WebMCPConfirmationRequest {
  /** The tool's registered name. */
  tool: string;
  /** The validated arguments the tool would execute with. */
  params: Record<string, unknown>;
  /** The execution options passed to `tool.execute`, including `signal`. */
  options: WebMCPExecutionOptions;
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
  /**
   * Tools that need explicit buyer confirmation before they may run — typically
   * the money move (`carts.complete`). Array entries accept canonical
   * (`carts.complete`) or wire (`carts__complete`) names. A predicate receives
   * `(toolName, params)` and gates on any truthy return; if it throws, the
   * call fails without executing.
   *
   * Gated tools report `consequentialHint: true`. Without `onConfirm` they fail
   * closed with `{ preview: true, confirmationRequired: true }`.
   */
  requireConfirmation?:
    string[] | ((toolName: string, params: Record<string, unknown>) => boolean) | null;
  /**
   * Host-provided approval UI, called as `onConfirm({ tool, params, options })`
   * for gated tools. Render the order summary and ask the buyer; only a return
   * value of exactly `true` approves — anything else denies and nothing
   * executes. Confirmation composes with `allowApply`: a confirmed write still
   * previews unless the host enabled `allowApply`.
   */
  onConfirm?: (request: WebMCPConfirmationRequest) => boolean | Promise<boolean>;
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
