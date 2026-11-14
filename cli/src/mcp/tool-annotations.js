// MCP tool annotations derived from a tool definition's `permission`.
//
// Clients use `readOnlyHint` / `destructiveHint` to decide what an agent may
// call without confirmation, so the mapping fails closed:
//
//   - only `read` is ever advertised as read-only, and not even every `read`
//     tool: READ_TOOLS_THAT_WRITE lists the ones known to change persisted
//     state;
//   - `write` omits `destructiveHint`, so the spec default (`true`) applies. A
//     `write` tool can delete, void, refund or overwrite, and the permission
//     alone cannot tell which ones only add;
//   - `delete`, `admin` and anything unrecognised are advertised destructive;
//   - `idempotentHint` and `openWorldHint` are never derived, so their spec
//     defaults (not idempotent, open world) apply unless a tool declares them.
//
// A definition's own `annotations` can only make the result more conservative
// where it matters: it cannot claim read-only for a tool that is not derived
// read-only, nor non-destructive for a `delete`/`admin` tool. See
// `deriveToolAnnotations` for the exact rules.

/** The only values a tool definition's `permission` may take. */
export const KNOWN_TOOL_PERMISSIONS = Object.freeze(['read', 'write', 'delete', 'admin']);

/**
 * Tools declared `read` that are known to change persisted rows. They are
 * advertised as mutations (`readOnlyHint: false`, destructiveness left to the
 * spec default) until their permission is fixed.
 *
 * Must equal the `kind: "write"` entries of
 * test/fixtures/read-tool-mutation-baseline.json (a test enforces it); the
 * `kind: "init"` entries only create empty structure on first use and stay
 * read-only.
 */
export const READ_TOOLS_THAT_WRITE = Object.freeze(['resolve_payment_link', 'x402_credit_balance']);

const READ_TOOLS_THAT_WRITE_SET = new Set(READ_TOOLS_THAT_WRITE);

/**
 * The annotation class of a tool definition.
 *
 * @param {{name?: string, permission?: string} | null | undefined} toolDef
 * @returns {'read' | 'mutation' | 'destructive'}
 */
function annotationClassOf(toolDef) {
  switch (toolDef?.permission) {
    case 'read':
      return READ_TOOLS_THAT_WRITE_SET.has(toolDef?.name) ? 'mutation' : 'read';
    case 'write':
      return 'mutation';
    default:
      // delete, admin, and any unknown or missing permission.
      return 'destructive';
  }
}

const HINTS_BY_CLASS = Object.freeze({
  read: Object.freeze({ readOnlyHint: true }),
  mutation: Object.freeze({ readOnlyHint: false }),
  destructive: Object.freeze({ readOnlyHint: false, destructiveHint: true }),
});

/**
 * Derive MCP `ToolAnnotations` for one tool definition.
 *
 * A definition may declare `annotations` of its own. They are applied field
 * by field, under these rules:
 *
 *   - `title` passes through.
 *   - `readOnlyHint: false` is always honoured; `readOnlyHint: true` only on a
 *     tool already derived read-only (so it changes nothing).
 *   - `destructiveHint: true` is always honoured; `destructiveHint: false`
 *     only on a `permission: 'write'` tool. That is a per-tool, code-reviewed
 *     claim that the tool only adds (e.g. `create_customer`), which the spec
 *     defines the hint for and the permission cannot express. It is ignored on
 *     `delete`/`admin`/unknown tools and on READ_TOOLS_THAT_WRITE.
 *   - `idempotentHint` and `openWorldHint` pass through: they too are
 *     per-tool claims the permission cannot express.
 *   - A value of the wrong type, `undefined`, or any other field is ignored.
 *
 * @param {{name?: string, permission?: string, annotations?: object} | null | undefined} toolDef
 * @returns {{title?: string, readOnlyHint: boolean, destructiveHint?: boolean, idempotentHint?: boolean, openWorldHint?: boolean}}
 */
export function deriveToolAnnotations(toolDef) {
  const annotationClass = annotationClassOf(toolDef);
  const annotations = { ...HINTS_BY_CLASS[annotationClass] };

  const declared = toolDef?.annotations;
  if (!declared || typeof declared !== 'object') return annotations;

  if (typeof declared.title === 'string') annotations.title = declared.title;

  if (declared.readOnlyHint === false) {
    annotations.readOnlyHint = false;
  }

  if (declared.destructiveHint === true) {
    annotations.destructiveHint = true;
  } else if (
    declared.destructiveHint === false &&
    toolDef.permission === 'write' &&
    annotationClass === 'mutation'
  ) {
    annotations.destructiveHint = false;
  }

  for (const field of ['idempotentHint', 'openWorldHint']) {
    if (typeof declared[field] === 'boolean') annotations[field] = declared[field];
  }
  return annotations;
}
