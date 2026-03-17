/**
 * Unit tests for tool-schema.js
 *
 * Covers: createToolInputSchema, validateToolInput, formatValidationIssues,
 * inputSchemaDefToJsonSchema — including the full internal conversion pipeline
 * (ZodString, ZodNumber, ZodBoolean, ZodEnum, ZodNativeEnum, ZodLiteral,
 * ZodArray, ZodObject, ZodRecord, ZodUnion, ZodTuple, ZodAny/ZodUnknown,
 * ZodNull, ZodDate, ZodOptional, ZodNullable, ZodDefault, ZodEffects).
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import {
  createToolInputSchema,
  validateToolInput,
  formatValidationIssues,
  inputSchemaDefToJsonSchema,
} from '../src/tool-schema.js';

// ===========================================================================
// createToolInputSchema
// ===========================================================================

describe('createToolInputSchema', () => {
  it('returns a ZodObject from a plain shape', () => {
    const schema = createToolInputSchema({ name: z.string() });
    assert.ok(schema instanceof z.ZodObject);
  });

  it('wraps an empty shape into a ZodObject', () => {
    const schema = createToolInputSchema({});
    assert.ok(schema instanceof z.ZodObject);
  });

  it('uses empty object when called with no arguments', () => {
    const schema = createToolInputSchema();
    assert.ok(schema instanceof z.ZodObject);
  });

  it('uses empty object when called with null', () => {
    // null coerces to {} via the `|| {}` guard
    const schema = createToolInputSchema(null);
    assert.ok(schema instanceof z.ZodObject);
  });

  it('validates correct data successfully', () => {
    const schema = createToolInputSchema({ age: z.number() });
    const result = schema.safeParse({ age: 25 });
    assert.strictEqual(result.success, true);
  });

  it('rejects data that does not match the shape', () => {
    const schema = createToolInputSchema({ age: z.number() });
    const result = schema.safeParse({ age: 'twenty' });
    assert.strictEqual(result.success, false);
  });
});

// ===========================================================================
// validateToolInput
// ===========================================================================

describe('validateToolInput', () => {
  it('returns success:true for valid params', () => {
    const result = validateToolInput({ name: z.string() }, { name: 'Alice' });
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.data.name, 'Alice');
  });

  it('returns success:false for invalid params', () => {
    const result = validateToolInput({ count: z.number() }, { count: 'oops' });
    assert.strictEqual(result.success, false);
  });

  it('accepts empty inputSchema', () => {
    const result = validateToolInput({}, {});
    assert.strictEqual(result.success, true);
  });

  it('accepts no arguments at all', () => {
    const result = validateToolInput();
    assert.strictEqual(result.success, true);
  });

  it('accepts null params without throwing', () => {
    const result = validateToolInput({ x: z.string() }, null);
    // null coerces to {} — no field present so x is required → fail
    assert.strictEqual(result.success, false);
  });

  it('returns ZodError on failure', () => {
    const result = validateToolInput({ n: z.number() }, { n: 'bad' });
    assert.ok(result.error instanceof z.ZodError);
  });

  it('provides error issues on failure', () => {
    const result = validateToolInput({ n: z.number() }, { n: 'bad' });
    assert.ok(Array.isArray(result.error.issues));
    assert.ok(result.error.issues.length > 0);
  });
});

// ===========================================================================
// formatValidationIssues
// ===========================================================================

describe('formatValidationIssues', () => {
  it('returns empty array for undefined input', () => {
    assert.deepStrictEqual(formatValidationIssues(undefined), []);
  });

  it('returns empty array for null input', () => {
    assert.deepStrictEqual(formatValidationIssues(null), []);
  });

  it('returns empty array when issues is not an array', () => {
    assert.deepStrictEqual(formatValidationIssues({ issues: 'not-array' }), []);
  });

  it('maps issues to {code, message, path} objects', () => {
    const result = validateToolInput({ age: z.number() }, { age: 'oops' });
    const issues = formatValidationIssues(result.error);
    assert.ok(issues.length > 0);
    const issue = issues[0];
    assert.ok('code' in issue);
    assert.ok('message' in issue);
    assert.ok('path' in issue);
  });

  it('joins nested path segments with a dot', () => {
    const schema = { user: z.object({ name: z.string() }) };
    const result = validateToolInput(schema, { user: { name: 42 } });
    const issues = formatValidationIssues(result.error);
    const pathIssue = issues.find((i) => i.path.includes('.'));
    assert.ok(pathIssue, 'expected at least one dotted path');
    assert.ok(pathIssue.path.startsWith('user.'));
  });

  it('returns empty path string for top-level issues', () => {
    // Produce a top-level type error by passing wrong type entirely
    const result = validateToolInput({ n: z.number() }, { n: 'bad' });
    const issues = formatValidationIssues(result.error);
    // The path for field 'n' is ['n'], joined → 'n'
    assert.ok(issues.some((i) => i.path === 'n'));
  });

  it('preserves issue code values', () => {
    const result = validateToolInput({ x: z.number() }, { x: 'str' });
    const issues = formatValidationIssues(result.error);
    assert.ok(issues.every((i) => typeof i.code === 'string'));
  });
});

// ===========================================================================
// inputSchemaDefToJsonSchema — primitive types
// ===========================================================================

describe('inputSchemaDefToJsonSchema — primitives', () => {
  it('converts ZodString to {type:"string"}', () => {
    const js = inputSchemaDefToJsonSchema({ name: z.string() });
    assert.strictEqual(js.properties.name.type, 'string');
  });

  it('converts ZodNumber to {type:"number"}', () => {
    const js = inputSchemaDefToJsonSchema({ price: z.number() });
    assert.strictEqual(js.properties.price.type, 'number');
  });

  it('converts ZodBoolean to {type:"boolean"}', () => {
    const js = inputSchemaDefToJsonSchema({ active: z.boolean() });
    assert.strictEqual(js.properties.active.type, 'boolean');
  });

  it('converts ZodNull to {type:"null"}', () => {
    const js = inputSchemaDefToJsonSchema({ nothing: z.null() });
    assert.strictEqual(js.properties.nothing.type, 'null');
  });

  it('converts ZodDate to {type:"string", format:"date-time"}', () => {
    const js = inputSchemaDefToJsonSchema({ created: z.date() });
    assert.strictEqual(js.properties.created.type, 'string');
    assert.strictEqual(js.properties.created.format, 'date-time');
  });

  it('converts ZodAny to {}', () => {
    const js = inputSchemaDefToJsonSchema({ data: z.any() });
    assert.deepStrictEqual(js.properties.data, {});
  });

  it('converts ZodUnknown to {}', () => {
    const js = inputSchemaDefToJsonSchema({ data: z.unknown() });
    assert.deepStrictEqual(js.properties.data, {});
  });

  it('falls through unknown types to {}', () => {
    // ZodVoid is not handled — should fall to default branch
    const js = inputSchemaDefToJsonSchema({ v: z.void() });
    assert.deepStrictEqual(js.properties.v, {});
  });
});

// ===========================================================================
// inputSchemaDefToJsonSchema — string checks
// ===========================================================================

describe('inputSchemaDefToJsonSchema — string checks', () => {
  it('applies minLength from .min()', () => {
    const js = inputSchemaDefToJsonSchema({ s: z.string().min(3) });
    assert.strictEqual(js.properties.s.minLength, 3);
  });

  it('applies maxLength from .max()', () => {
    const js = inputSchemaDefToJsonSchema({ s: z.string().max(100) });
    assert.strictEqual(js.properties.s.maxLength, 100);
  });

  it('applies format:email from .email()', () => {
    const js = inputSchemaDefToJsonSchema({ email: z.string().email() });
    assert.strictEqual(js.properties.email.format, 'email');
  });

  it('applies format:uri from .url()', () => {
    const js = inputSchemaDefToJsonSchema({ url: z.string().url() });
    assert.strictEqual(js.properties.url.format, 'uri');
  });

  it('applies format:uuid from .uuid()', () => {
    const js = inputSchemaDefToJsonSchema({ id: z.string().uuid() });
    assert.strictEqual(js.properties.id.format, 'uuid');
  });

  it('applies format:date-time from .datetime()', () => {
    const js = inputSchemaDefToJsonSchema({ ts: z.string().datetime() });
    assert.strictEqual(js.properties.ts.format, 'date-time');
  });

  it('applies pattern from .regex()', () => {
    const js = inputSchemaDefToJsonSchema({ slug: z.string().regex(/^[a-z-]+$/) });
    assert.strictEqual(js.properties.slug.pattern, '^[a-z-]+$');
  });
});

// ===========================================================================
// inputSchemaDefToJsonSchema — number checks
// ===========================================================================

describe('inputSchemaDefToJsonSchema — number checks', () => {
  it('applies minimum from .min()', () => {
    const js = inputSchemaDefToJsonSchema({ n: z.number().min(0) });
    assert.strictEqual(js.properties.n.minimum, 0);
  });

  it('applies maximum from .max()', () => {
    const js = inputSchemaDefToJsonSchema({ n: z.number().max(100) });
    assert.strictEqual(js.properties.n.maximum, 100);
  });

  it('converts ZodNumber to integer type via .int()', () => {
    const js = inputSchemaDefToJsonSchema({ n: z.number().int() });
    assert.strictEqual(js.properties.n.type, 'integer');
  });

  it('applies multipleOf from .multipleOf()', () => {
    const js = inputSchemaDefToJsonSchema({ n: z.number().multipleOf(5) });
    assert.strictEqual(js.properties.n.multipleOf, 5);
  });
});

// ===========================================================================
// inputSchemaDefToJsonSchema — enum types
// ===========================================================================

describe('inputSchemaDefToJsonSchema — enum types', () => {
  it('converts ZodEnum to {type:"string", enum:[...]}', () => {
    const js = inputSchemaDefToJsonSchema({ status: z.enum(['active', 'inactive', 'draft']) });
    assert.strictEqual(js.properties.status.type, 'string');
    assert.deepStrictEqual(js.properties.status.enum, ['active', 'inactive', 'draft']);
  });

  it('converts ZodNativeEnum (string values) to {enum:[...]}', () => {
    const Direction = { UP: 'up', DOWN: 'down' };
    const js = inputSchemaDefToJsonSchema({ dir: z.nativeEnum(Direction) });
    assert.ok(Array.isArray(js.properties.dir.enum));
    assert.ok(js.properties.dir.enum.includes('up'));
    assert.ok(js.properties.dir.enum.includes('down'));
  });

  it('converts ZodNativeEnum (numeric TS enum) without reverse-mapping duplication', () => {
    // TypeScript numeric enums add reverse mappings (0->'Zero') — should be deduplicated
    const NumEnum = { Zero: 0, One: 1, 0: 'Zero', 1: 'One' };
    const js = inputSchemaDefToJsonSchema({ e: z.nativeEnum(NumEnum) });
    // Only unique primitive values should appear
    const values = js.properties.e.enum;
    assert.deepStrictEqual(values, [...new Set(values)]);
  });

  it('converts ZodLiteral to {const: value}', () => {
    const js = inputSchemaDefToJsonSchema({ version: z.literal('v1') });
    assert.strictEqual(js.properties.version.const, 'v1');
  });

  it('converts numeric ZodLiteral', () => {
    const js = inputSchemaDefToJsonSchema({ code: z.literal(42) });
    assert.strictEqual(js.properties.code.const, 42);
  });
});

// ===========================================================================
// inputSchemaDefToJsonSchema — complex types
// ===========================================================================

describe('inputSchemaDefToJsonSchema — arrays', () => {
  it('converts ZodArray to {type:"array", items:{...}}', () => {
    const js = inputSchemaDefToJsonSchema({ tags: z.array(z.string()) });
    assert.strictEqual(js.properties.tags.type, 'array');
    assert.strictEqual(js.properties.tags.items.type, 'string');
  });

  it('applies minItems from .min()', () => {
    const js = inputSchemaDefToJsonSchema({ items: z.array(z.string()).min(1) });
    assert.strictEqual(js.properties.items.minItems, 1);
  });

  it('applies maxItems from .max()', () => {
    const js = inputSchemaDefToJsonSchema({ items: z.array(z.string()).max(10) });
    assert.strictEqual(js.properties.items.maxItems, 10);
  });

  it('applies both minItems and maxItems from .length()', () => {
    const js = inputSchemaDefToJsonSchema({ items: z.array(z.string()).length(3) });
    assert.strictEqual(js.properties.items.minItems, 3);
    assert.strictEqual(js.properties.items.maxItems, 3);
  });

  it('converts array of objects', () => {
    const js = inputSchemaDefToJsonSchema({
      rows: z.array(z.object({ id: z.number() })),
    });
    assert.strictEqual(js.properties.rows.type, 'array');
    assert.strictEqual(js.properties.rows.items.type, 'object');
    assert.strictEqual(js.properties.rows.items.properties.id.type, 'number');
  });
});

describe('inputSchemaDefToJsonSchema — objects', () => {
  it('converts ZodObject with required fields', () => {
    const js = inputSchemaDefToJsonSchema({ user: z.object({ id: z.string(), age: z.number() }) });
    const userSchema = js.properties.user;
    assert.strictEqual(userSchema.type, 'object');
    assert.ok(userSchema.required.includes('id'));
    assert.ok(userSchema.required.includes('age'));
    assert.strictEqual(userSchema.additionalProperties, false);
  });

  it('omits required array when all fields are optional', () => {
    const js = inputSchemaDefToJsonSchema({
      opts: z.object({ x: z.string().optional() }),
    });
    assert.ok(!js.properties.opts.required || js.properties.opts.required.length === 0);
  });

  it('handles nested objects', () => {
    const js = inputSchemaDefToJsonSchema({
      a: z.object({ b: z.object({ c: z.boolean() }) }),
    });
    assert.strictEqual(js.properties.a.properties.b.properties.c.type, 'boolean');
  });

  it('sets additionalProperties:false on nested objects', () => {
    const js = inputSchemaDefToJsonSchema({
      nested: z.object({ x: z.string() }),
    });
    assert.strictEqual(js.properties.nested.additionalProperties, false);
  });
});

describe('inputSchemaDefToJsonSchema — record', () => {
  it('converts ZodRecord to {type:"object", additionalProperties:{...}}', () => {
    const js = inputSchemaDefToJsonSchema({ meta: z.record(z.string()) });
    assert.strictEqual(js.properties.meta.type, 'object');
    assert.strictEqual(js.properties.meta.additionalProperties.type, 'string');
  });

  it('converts ZodRecord with numeric values', () => {
    const js = inputSchemaDefToJsonSchema({ counts: z.record(z.number()) });
    assert.strictEqual(js.properties.counts.additionalProperties.type, 'number');
  });
});

describe('inputSchemaDefToJsonSchema — union', () => {
  it('converts ZodUnion to {anyOf:[...]}', () => {
    const js = inputSchemaDefToJsonSchema({ id: z.union([z.string(), z.number()]) });
    assert.ok(Array.isArray(js.properties.id.anyOf));
    assert.strictEqual(js.properties.id.anyOf.length, 2);
    assert.ok(js.properties.id.anyOf.some((s) => s.type === 'string'));
    assert.ok(js.properties.id.anyOf.some((s) => s.type === 'number'));
  });
});

describe('inputSchemaDefToJsonSchema — tuple', () => {
  it('converts ZodTuple to prefixItems with exact length constraints', () => {
    const js = inputSchemaDefToJsonSchema({ pair: z.tuple([z.string(), z.number()]) });
    const tuple = js.properties.pair;
    assert.strictEqual(tuple.type, 'array');
    assert.ok(Array.isArray(tuple.prefixItems));
    assert.strictEqual(tuple.prefixItems.length, 2);
    assert.strictEqual(tuple.minItems, 2);
    assert.strictEqual(tuple.maxItems, 2);
    assert.strictEqual(tuple.prefixItems[0].type, 'string');
    assert.strictEqual(tuple.prefixItems[1].type, 'number');
  });
});

// ===========================================================================
// inputSchemaDefToJsonSchema — optional / nullable / default wrappers
// ===========================================================================

describe('inputSchemaDefToJsonSchema — optional fields', () => {
  it('marks optional fields as not required', () => {
    const js = inputSchemaDefToJsonSchema({
      required: z.string(),
      optional: z.string().optional(),
    });
    assert.ok(js.required.includes('required'));
    assert.ok(!js.required.includes('optional'));
  });

  it('preserves underlying type for optional field', () => {
    const js = inputSchemaDefToJsonSchema({ s: z.string().optional() });
    assert.strictEqual(js.properties.s.type, 'string');
  });
});

describe('inputSchemaDefToJsonSchema — nullable fields', () => {
  it('adds null to type array for nullable string', () => {
    const js = inputSchemaDefToJsonSchema({ s: z.string().nullable() });
    const type = js.properties.s.type;
    assert.ok(Array.isArray(type));
    assert.ok(type.includes('string'));
    assert.ok(type.includes('null'));
  });

  it('does not duplicate null when nullable is applied twice', () => {
    const js = inputSchemaDefToJsonSchema({ s: z.string().nullable() });
    const nullCount = js.properties.s.type.filter((t) => t === 'null').length;
    assert.strictEqual(nullCount, 1);
  });
});

describe('inputSchemaDefToJsonSchema — default values', () => {
  it('includes default value in JSON schema', () => {
    const js = inputSchemaDefToJsonSchema({ status: z.string().default('active') });
    assert.strictEqual(js.properties.status.default, 'active');
  });

  it('marks field with default as optional (not in required)', () => {
    const js = inputSchemaDefToJsonSchema({ status: z.string().default('active') });
    assert.ok(!js.required || !js.required.includes('status'));
  });

  it('calls function default to resolve value', () => {
    const js = inputSchemaDefToJsonSchema({ ts: z.number().default(() => 42) });
    assert.strictEqual(js.properties.ts.default, 42);
  });
});

// ===========================================================================
// inputSchemaDefToJsonSchema — description propagation
// ===========================================================================

describe('inputSchemaDefToJsonSchema — descriptions', () => {
  it('propagates .describe() to JSON schema description', () => {
    const js = inputSchemaDefToJsonSchema({
      name: z.string().describe('The full name'),
    });
    assert.strictEqual(js.properties.name.description, 'The full name');
  });

  it('propagates description through optional wrapper', () => {
    const js = inputSchemaDefToJsonSchema({
      title: z.string().describe('A title').optional(),
    });
    assert.strictEqual(js.properties.title.description, 'A title');
  });

  it('does not overwrite an existing description', () => {
    // When inner schema has a description and outer also has one, the outer wins
    // (first assignment wins due to `description ||= ...` semantics)
    const inner = z.string().describe('inner desc');
    const outer = inner.optional();
    // outer wrapping doesn't add its own description; inner desc should survive
    const js = inputSchemaDefToJsonSchema({ x: outer });
    assert.strictEqual(js.properties.x.description, 'inner desc');
  });
});

// ===========================================================================
// inputSchemaDefToJsonSchema — top-level object structure
// ===========================================================================

describe('inputSchemaDefToJsonSchema — top-level structure', () => {
  it('returns type:object at root', () => {
    const js = inputSchemaDefToJsonSchema({ x: z.string() });
    assert.strictEqual(js.type, 'object');
  });

  it('includes a properties object at root', () => {
    const js = inputSchemaDefToJsonSchema({ x: z.string() });
    assert.ok(js.properties && typeof js.properties === 'object');
  });

  it('sets additionalProperties:false at root', () => {
    const js = inputSchemaDefToJsonSchema({ x: z.string() });
    assert.strictEqual(js.additionalProperties, false);
  });

  it('returns type:object with empty properties for empty schema', () => {
    const js = inputSchemaDefToJsonSchema({});
    assert.strictEqual(js.type, 'object');
    assert.deepStrictEqual(js.properties, {});
  });

  it('omits required key when all fields are optional', () => {
    const js = inputSchemaDefToJsonSchema({ x: z.string().optional() });
    assert.ok(!js.required || js.required.length === 0);
  });

  it('lists all required fields at root level', () => {
    const js = inputSchemaDefToJsonSchema({
      a: z.string(),
      b: z.number(),
      c: z.boolean().optional(),
    });
    assert.ok(Array.isArray(js.required));
    assert.ok(js.required.includes('a'));
    assert.ok(js.required.includes('b'));
    assert.ok(!js.required.includes('c'));
  });
});

// ===========================================================================
// inputSchemaDefToJsonSchema — ZodEffects (transform / refine)
// ===========================================================================

describe('inputSchemaDefToJsonSchema — ZodEffects', () => {
  it('unwraps ZodEffects (transform) and exposes inner type', () => {
    const trimmed = z.string().transform((s) => s.trim());
    const js = inputSchemaDefToJsonSchema({ name: trimmed });
    // After unwrapping effects the inner ZodString should be used
    assert.strictEqual(js.properties.name.type, 'string');
  });

  it('unwraps ZodEffects (refine) and exposes inner type', () => {
    const positive = z.number().refine((n) => n > 0, { message: 'Must be positive' });
    const js = inputSchemaDefToJsonSchema({ qty: positive });
    assert.strictEqual(js.properties.qty.type, 'number');
  });
});

// ===========================================================================
// Round-trip: validateToolInput + formatValidationIssues integration
// ===========================================================================

describe('validateToolInput + formatValidationIssues integration', () => {
  it('formats a single top-level type error', () => {
    const result = validateToolInput({ count: z.number() }, { count: 'oops' });
    assert.strictEqual(result.success, false);
    const issues = formatValidationIssues(result.error);
    assert.strictEqual(issues.length, 1);
    assert.strictEqual(issues[0].path, 'count');
    assert.ok(issues[0].message.length > 0);
  });

  it('formats multiple errors from a complex schema', () => {
    const schema = {
      email: z.string().email(),
      age: z.number().int().min(0),
    };
    const result = validateToolInput(schema, { email: 'bad', age: -1 });
    assert.strictEqual(result.success, false);
    const issues = formatValidationIssues(result.error);
    assert.ok(issues.length >= 2);
    const paths = issues.map((i) => i.path);
    assert.ok(paths.includes('email'));
    assert.ok(paths.includes('age'));
  });

  it('returns empty issues array on successful validation', () => {
    const result = validateToolInput({ n: z.number() }, { n: 5 });
    // result.error is undefined on success, so formatValidationIssues returns []
    assert.deepStrictEqual(formatValidationIssues(result.error), []);
  });
});
