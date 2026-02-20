/**
 * Standard --apply guard response for write operations.
 *
 * @param {string} operation - e.g., 'Create supplier', 'Approve return'
 * @param {object} [preview] - Data that would be affected (shown as preview)
 * @returns {{ error: string, hint: string, wouldDo?: object }}
 */
export function applyRequired(operation, preview = null) {
  const response = {
    error: `${operation} requires --apply flag.`,
    hint: 'Run with --apply to enable write operations.',
  };
  if (preview) {
    response.wouldDo = preview;
  }
  return response;
}
