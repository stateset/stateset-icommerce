export interface PurchaseIdentity {
  agentId: string;
  principalId: string;
  tenantId: string;
  storeId: string;
}
export interface PurchaseRequest {
  idempotencyKey: string;
  quoteId: string;
  budgetId: string;
  maxAmount: string;
  asset: string;
}
export interface PurchaseQuote {
  id: string;
  counterpartyId: string;
  amount: string;
  asset: string;
  expiresAt: string;
  [key: string]: unknown;
}
export type PurchaseStep =
  | 'reserve_inventory'
  | 'pay'
  | 'create_order'
  | 'confirm_inventory'
  | 'release_inventory';
export interface PurchaseOperation {
  id: string;
  scope: string;
  agent: string;
  principal: string;
  policyVersion: string;
  request: PurchaseRequest;
  fingerprint: string;
  quote: PurchaseQuote;
  status:
    | 'preview'
    | 'pending'
    | 'running'
    | 'reconciling'
    | 'compensating'
    | 'needs_attention'
    | 'cancelled'
    | 'completed';
  budgetState: 'unreserved' | 'reserved' | 'spent' | 'released';
  steps: Partial<
    Record<
      PurchaseStep,
      {
        dispatched: boolean;
        status: string;
        evidence?: Record<string, unknown>;
      }
    >
  >;
  createdAt: string;
  error: string | null;
  busy?: boolean;
  skipped?: 'needs_attention';
  cancelRequested?: boolean;
  compensationRequested?: boolean;
  authorization?: { allowed: boolean; [key: string]: unknown };
  receipt?: Record<string, unknown>;
}
export type PurchaseAdapterResult =
  | { status: 'succeeded'; evidence: Record<string, unknown> }
  | { status: 'failed'; reason?: string }
  | { status: 'pending' | 'unknown' | 'not_found' };
export interface PurchaseAdapterContext {
  operation: PurchaseOperation;
  idempotencyKey: string;
}
/** Both methods must address the same persistent idempotency key.
 * not_found is an authoritative assertion that no effect exists or is in flight.
 * succeeded for pay requires transaction_id, amount, and asset in evidence.
 */
export interface PurchaseAdapter {
  execute(context: PurchaseAdapterContext): Promise<PurchaseAdapterResult>;
  lookup(context: PurchaseAdapterContext): Promise<PurchaseAdapterResult>;
}
export interface PurchaseBudget {
  id: string;
  asset: string;
  limit: string;
  expiresAt: string;
}
export interface PurchaseBudgetStatus extends PurchaseBudget {
  reserved: string;
  spent: string;
  available: string;
}
export interface PurchaseRecoveryOptions {
  limit?: number;
  after?: string | null;
}
export interface PurchaseRecoveryPage {
  operations: PurchaseOperation[];
  nextCursor: string | null;
}
export interface PurchaseRecoveryResult {
  results: Array<
    | { id: string; operation: PurchaseOperation; error?: never }
    | { id: string; error: string; operation?: never }
  >;
  nextCursor: string | null;
}
export class SqlitePurchaseStore {
  constructor(db: { prepare: Function; transaction: Function; exec: Function });
  provisionBudget(scope: string, budget: PurchaseBudget): PurchaseBudgetStatus;
  budget(scope: string, id: string): PurchaseBudgetStatus;
  get(id: string): PurchaseOperation | null;
  find(scope: string, agent: string, key: string): PurchaseOperation | null;
  pending(scope: string, agent: string, options?: PurchaseRecoveryOptions): PurchaseRecoveryPage;
  create(operation: PurchaseOperation, now: number): PurchaseOperation;
  claim(id: string, owner: string, now: number, leaseMs: number): boolean;
  save(
    operation: PurchaseOperation,
    owner: string,
    budgetAction?: 'capture' | 'release' | null,
  ): void;
  release(id: string, owner: string): void;
}
export class PurchaseRuntime {
  constructor(options: {
    store: SqlitePurchaseStore;
    identity: PurchaseIdentity;
    policyVersion: string;
    resolveQuote: (quoteId: string, identity: PurchaseIdentity) => Promise<PurchaseQuote>;
    authorize: (
      operation: PurchaseOperation,
      identity: PurchaseIdentity,
    ) => Promise<{ allowed: boolean; [key: string]: unknown }>;
    adapters: Record<PurchaseStep, PurchaseAdapter>;
    allowApply?: boolean;
    clock?: () => number;
    leaseMs?: number;
  });
  readonly scope: string;
  get(id: string): PurchaseOperation;
  pending(options?: PurchaseRecoveryOptions): PurchaseRecoveryPage;
  recover(options?: PurchaseRecoveryOptions): Promise<PurchaseRecoveryResult>;
  buy(request: PurchaseRequest): Promise<PurchaseOperation>;
  resume(id: string): Promise<PurchaseOperation>;
  cancel(id: string): Promise<PurchaseOperation>;
}

export function createKernelPurchaseAdapter(options: {
  commerce: { executeKernelCommand(command: unknown, policy: unknown): Promise<any> };
  policy: Record<string, unknown>;
  principal: Record<string, unknown>;
  storeId: string;
  commandType: string;
  buildPayload: (operation: PurchaseOperation) => unknown;
  readReceipt: (idempotencyKey: string) => Promise<any>;
  evidence: (receipt: any) => Record<string, unknown>;
}): PurchaseAdapter;
