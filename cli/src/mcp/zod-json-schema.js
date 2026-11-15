import { z } from 'zod';

/**
 * Convert a Zod schema to a self-contained JSON Schema describing the *input*
 * a caller may send (fields with defaults stay optional).
 *
 * Zod 4 ships its own converter; `zod-to-json-schema` only understands Zod 3
 * and silently returns an empty schema for Zod 4 types, which would advertise
 * every tool as taking no arguments. These adjustments keep the published tool
 * catalog in the shape the Zod 3 converters emitted:
 *
 * - plain objects are closed with `additionalProperties: false`;
 * - `z.date()` (not JSON) is advertised as an RFC 3339 `date-time` string;
 * - a nullable primitive is `type: [t, "null"]` rather than an `anyOf`;
 * - a union of primitives stays an `anyOf` (Zod 4 folds it into a type array).
 *
 * Types JSON Schema cannot express (transforms, custom refinements) become
 * `{}`; the Zod parser still enforces them at call time.
 *
 * @param {import('zod').ZodType} schema
 * @param {{ target?: 'draft-7' | 'draft-2020-12' }} [options]
 * @returns {Record<string, unknown>}
 */
export function zodToInputJsonSchema(schema, { target = 'draft-7' } = {}) {
  const jsonSchema = z.toJSONSchema(schema, {
    target,
    io: 'input',
    unrepresentable: 'any',
    override: ({ zodSchema, jsonSchema: out }) => {
      const type = zodSchema._zod?.def?.type;
      if (type === 'date') {
        out.type = 'string';
        out.format = 'date-time';
      }
      if (
        out.type === 'object' &&
        out.properties !== undefined &&
        out.additionalProperties === undefined
      ) {
        out.additionalProperties = false;
      }
      if (type === 'nullable' && Array.isArray(out.anyOf) && out.anyOf.length === 2) {
        const [inner, nullable] = out.anyOf;
        if (
          nullable?.type === 'null' &&
          typeof inner?.type === 'string' &&
          inner.anyOf === undefined
        ) {
          delete out.anyOf;
          Object.assign(out, inner, { type: [inner.type, 'null'] });
        }
      }
    },
  });
  delete jsonSchema.$schema;
  return restoreUnionAnyOf(jsonSchema);
}

/**
 * Zod 4 folds a union of primitives into a type array (`type: ["string",
 * "number"]`) in a final pass after `override` runs. Turn those back into
 * `anyOf`, leaving the two-member nullable form (`[t, "null"]`) alone.
 *
 * @param {unknown} node
 * @returns {any}
 */
function restoreUnionAnyOf(node) {
  if (Array.isArray(node)) return node.map(restoreUnionAnyOf);
  if (node === null || typeof node !== 'object') return node;
  const out = Object.fromEntries(
    Object.entries(node).map(([key, value]) => [key, restoreUnionAnyOf(value)]),
  );
  const nullable = Array.isArray(out.type) && out.type.length === 2 && out.type.includes('null');
  if (Array.isArray(out.type) && out.type.length > 1 && !nullable) {
    out.anyOf = out.type.map((member) => ({ type: member }));
    delete out.type;
  }
  return out;
}
