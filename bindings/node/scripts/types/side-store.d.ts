// The JavaScript-side surface index.js layers over `SideStore` and
// `SideStoreStatement` (see `Commerce.openSideStore()`): the better-sqlite3
// methods whose argument handling lives in JavaScript.

/**
 * A value a side-store statement binds, as better-sqlite3 binds it: a number
 * as REAL, a bigint as INTEGER, a string as TEXT, a Buffer or other
 * ArrayBufferView as BLOB, and `null`/`undefined` as NULL. Booleans and other
 * objects are refused.
 */
export type SideStoreValue = number | bigint | string | Buffer | ArrayBufferView | null | undefined

/**
 * One argument to `run`/`get`/`all`: a positional value, an array of
 * positional values, or one plain object of named values (`:name`, `@name`
 * and `$name` are looked up without the prefix).
 */
export type SideStoreBindParameters = SideStoreValue | ReadonlyArray<SideStoreValue> | Record<string, SideStoreValue>

/** The body of a side-store transaction. It must not return a promise. */
export type SideStoreTransactionBody = (...args: any[]) => any

/**
 * A function that runs its body in a transaction: `BEGIN` (or a savepoint
 * when one is already open), `COMMIT` on return, rollback on throw. The
 * `deferred`, `immediate` and `exclusive` variants choose the `BEGIN` mode.
 */
export type SideStoreTransaction<F extends SideStoreTransactionBody> = F & {
  readonly default: SideStoreTransaction<F>
  readonly deferred: SideStoreTransaction<F>
  readonly immediate: SideStoreTransaction<F>
  readonly exclusive: SideStoreTransaction<F>
  readonly database: SideStore
}

export interface SideStore {
  /** Run a script of zero or more statements. Returns this side store. */
  exec(sql: string): this
  /**
   * Run `PRAGMA <source>`: every row, or with `simple` the first column of the
   * first row.
   */
  pragma(source: string, options?: { simple?: boolean }): unknown
  /**
   * Wrap `fn` so each call runs in a transaction on this side store's one
   * connection. Nested calls use savepoints.
   */
  transaction<F extends SideStoreTransactionBody>(fn: F): SideStoreTransaction<F>
}

export interface SideStoreStatement {
  /** Execute once. Reports the rows changed and the last inserted rowid. */
  run(...params: SideStoreBindParameters[]): SideStoreRunResult
  /** The first row as an object keyed by column name, or `undefined`. */
  get(...params: SideStoreBindParameters[]): Record<string, unknown> | undefined
  /** Every row as an object keyed by column name. */
  all(...params: SideStoreBindParameters[]): Array<Record<string, unknown>>
}

export interface Commerce {
  /**
   * This engine's shared side store: one `openSideStore()` handle per engine,
   * opened on first use and reopened if it was closed. Every caller gets the
   * same connection, so stores that share it can never wait on each other's
   * write locks. `commerce.close()` closes it; callers should not.
   */
  sharedSideStore(): SideStore
}
