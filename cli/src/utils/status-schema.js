/**
 * Status parameters that tell the truth about what the engine accepts.
 *
 * - `statusEnum` offers only values the engine can accept. A value it
 *   deliberately leaves out (because the engine always refuses it, or
 *   another tool performs that move) is refused by the schema with a message
 *   naming what to do instead -- "call ship_order" -- rather than a bare
 *   "invalid option".
 * - `statusFilter` offers the values the store actually holds, plus older
 *   spellings the tool used to accept as aliases, so a call that worked keeps
 *   working; `resolveStatusAlias` maps an alias to the stored value in the
 *   handler.
 */

import { z } from 'zod';

const key = (value) => (typeof value === 'string' ? value.trim().toLowerCase() : undefined);

/**
 * @param {readonly [string, ...string[]]} values - the accepted values
 * @param {Record<string, string>} [redirects] - left-out value -> what to do instead
 */
export function statusEnum(values, redirects = {}) {
  return z.enum(values, {
    error: (issue) => {
      const redirect = redirects[key(issue.input)];
      return redirect ? `"${issue.input}" is not accepted here: ${redirect}` : undefined;
    },
  });
}

/**
 * An optional status filter over the values the store holds.
 *
 * @param {readonly [string, ...string[]]} stored - values the store holds
 * @param {{ aliases?: Record<string, string>, redirects?: Record<string, string>,
 *           subject?: string, derived?: string }} [options]
 *   `aliases`: older spelling -> stored value (still accepted);
 *   `redirects`: value no longer offered -> why (refused with that message);
 *   `subject`: what is filtered ("intent", "settlement", ...);
 *   `derived`: a sentence for a value that is computed rather than stored.
 */
export function statusFilter(stored, { aliases = {}, redirects = {}, subject, derived } = {}) {
  const aliasText = Object.entries(aliases)
    .map(([alias, value]) => `${alias} = ${value}`)
    .join(', ');
  const description = [
    `Filter by ${subject ? `${subject} ` : ''}status: ${stored.join(', ')}.`,
    derived,
    aliasText ? `Also accepted for compatibility: ${aliasText}.` : null,
  ]
    .filter(Boolean)
    .join(' ');
  return statusEnum([...stored, ...Object.keys(aliases)], redirects)
    .optional()
    .describe(description);
}

/** The stored value for `value` (an alias maps to its stored value). */
export function resolveStatusAlias(value, aliases = {}) {
  if (value === undefined || value === null) return value;
  return aliases[value] ?? value;
}
