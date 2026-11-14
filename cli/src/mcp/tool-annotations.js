// MCP tool annotations derived from a tool definition's `permission`.
//
// Clients use `readOnlyHint` / `destructiveHint` to decide what an agent may
// call without confirmation, so the mapping fails closed: only an explicit
// `read` permission is ever advertised as read-only, and anything unrecognised
// is advertised as a destructive mutation.

/** The only values a tool definition's `permission` may take. */
export const KNOWN_TOOL_PERMISSIONS = Object.freeze(['read', 'write', 'delete', 'admin']);

const HINTS_BY_PERMISSION = Object.freeze({
  read: Object.freeze({ readOnlyHint: true }),
  write: Object.freeze({ readOnlyHint: false, destructiveHint: false }),
  delete: Object.freeze({ readOnlyHint: false, destructiveHint: true }),
  // Admin tools vary too much to classify from the definition alone.
  admin: Object.freeze({ readOnlyHint: false, destructiveHint: true }),
});

const UNKNOWN_PERMISSION_HINTS = Object.freeze({ readOnlyHint: false, destructiveHint: true });

/**
 * Derive MCP `ToolAnnotations` for one tool definition.
 *
 * Annotations a definition declares itself override the derived values field
 * by field. `idempotentHint` and `openWorldHint` are never derived: they are
 * present only when the definition declares them.
 *
 * @param {{permission?: string, annotations?: object} | null | undefined} toolDef
 * @returns {{title?: string, readOnlyHint: boolean, destructiveHint?: boolean, idempotentHint?: boolean, openWorldHint?: boolean}}
 */
export function deriveToolAnnotations(toolDef) {
  const permission = toolDef?.permission;
  const annotations = {
    ...(Object.hasOwn(HINTS_BY_PERMISSION, permission)
      ? HINTS_BY_PERMISSION[permission]
      : UNKNOWN_PERMISSION_HINTS),
  };
  const declared = toolDef?.annotations;
  if (declared && typeof declared === 'object') {
    for (const [field, value] of Object.entries(declared)) {
      if (value !== undefined) annotations[field] = value;
    }
  }
  return annotations;
}
