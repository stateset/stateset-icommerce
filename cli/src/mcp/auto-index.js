// Background vector auto-indexing for newly-created entities.
//
// When a tool creates a product, customer, or order, the vector index
// (when enabled) should pick up the new entity asynchronously so semantic
// search stays fresh without blocking the tool response. This helper
// fans out to the appropriate `indexX` method on the runtime's
// `vectorAutoIndex` and swallows failures with a console log — indexing
// is a best-effort enrichment, not a critical path.
//
// Extracted from mcp-server.js. Takes the indexer as an explicit
// argument (rather than closing over `getSharedRuntime()`) so unit tests
// can inject a stub.

/**
 * @typedef {Object} VectorAutoIndex
 * @property {(id: string) => Promise<unknown>} indexProduct
 * @property {(id: string) => Promise<unknown>} indexCustomer
 * @property {(id: string) => Promise<unknown>} indexOrder
 */

/**
 * Best-effort background indexing of a newly-created entity.
 *
 * No-ops cleanly when:
 *   - `vectorAutoIndex` is null/undefined (vector indexing disabled)
 *   - `entity` is null/undefined or missing `.id`
 *   - `entityType` is not one of the supported categories
 *
 * The promise from the underlying `indexX` call is intentionally not
 * awaited — caller continues immediately. Failures are logged.
 *
 * @param {VectorAutoIndex|null|undefined} vectorAutoIndex
 * @param {'product'|'customer'|'order'} entityType
 * @param {{id: string|number}|null|undefined} entity
 * @returns {void}
 */
export function autoIndexEntity(vectorAutoIndex, entityType, entity) {
  if (!vectorAutoIndex || !entity?.id) return;
  const indexFn = {
    product: () => vectorAutoIndex.indexProduct(entity.id.toString()),
    customer: () => vectorAutoIndex.indexCustomer(entity.id.toString()),
    order: () => vectorAutoIndex.indexOrder(entity.id.toString()),
  }[entityType];
  if (indexFn) {
    indexFn().catch((err) =>
      console.error(`[AutoIndex] Failed to index ${entityType} ${entity.id}: ${err.message}`),
    );
  }
}
