#!/usr/bin/env node
/**
 * Generate `tool-descriptors.json` from the napi-generated `index.d.ts`.
 *
 * The descriptors let `native-toolkit.mjs` expose every public method of
 * every module reachable from a `Commerce` getter (`commerce.orders.create`
 * → tool `orders.create`) as a framework-neutral tool without the optional
 * `@stateset/cli` peer dependency. Runs from `scripts/postbuild.mjs` after
 * every build, and stands alone: `node scripts/generate-tool-descriptors.mjs`.
 *
 * The parser is deliberately small and targets the shapes napi emits plus the
 * hand-written augment block:
 *
 *   - `export interface X [extends A, B] { field?: Type  [key: string]: T }`
 *   - `export type X = 'a' | 'b' | Other`
 *   - `export enum X { A = 'a' }`
 *   - `export declare class X { method(a: T, b?: U): Promise<R>  get x(): Y }`
 *   - `/** JSDoc *\/` blocks (single- and multi-line) attached to the next
 *     declaration or member
 *
 * Anything it does not understand is recorded under `meta.warnings` in the
 * output rather than silently dropped, so a d.ts change that outgrows the
 * parser is visible in the artifact.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const packageRoot = path.resolve(here, '..');
const SOURCE = path.join(packageRoot, 'index.d.ts');
const TARGET = path.join(packageRoot, 'tool-descriptors.json');

// ---------------------------------------------------------------------------
// Policy knobs. Everything that decides *which* methods become tools and
// whether they count as reads lives here so it can be audited in one place.
// ---------------------------------------------------------------------------

/** Method names never exposed as tools, on any module. */
const SKIPPED_METHODS = new Set(['constructor', 'open', 'close', 'ref', 'unref']);

/**
 * A method whose camelCase first word is in this set is treated as read-only
 * (executes without `allowApply`). Everything else is a write and previews
 * unless the toolkit was constructed with `allowApply: true`. Unknown verbs
 * therefore fail closed.
 */
const READ_ONLY_PREFIXES = new Set([
  'get',
  'list',
  'count',
  'find',
  'search',
  'calculate',
  'validate',
  'validation',
  'preview',
  'is',
  'has',
  'check',
  'supports',
  'estimate',
  'quote',
  'lookup',
  'describe',
  'read',
  'history',
  'latest',
  'discover',
  'distinct',
  'exportable',
  'importable',
]);

/**
 * Explicit overrides, keyed by tool name or `<module>.*`. A tool-name entry
 * wins over a module wildcard, which wins over the prefix heuristic. Add an
 * entry whenever a method's verb lies about what it does.
 */
const READ_ONLY_OVERRIDES = {
  // Every analytics method is a report, whatever its name.
  'analytics.*': true,
  // "find" — but creates the customer when missing.
  'customers.findOrCreate': false,
  // Reads phrased without a verb.
  'tax.customerIsExempt': true,
  'carts.forCustomer': true,
  // "verify" here upgrades the agent's trust level: an admin write.
  'x402.verifyAgent': false,
  // Dequeues the job for the station.
  'printStations.nextJob': false,
};

/** Tools excluded by name even though the parser could describe them. */
const SKIPPED_TOOLS = new Set([]);

// ---------------------------------------------------------------------------
// Source scanning
// ---------------------------------------------------------------------------

/** Code-unit ordering: identical on every platform, unlike localeCompare. */
function byCodeUnit(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

const warnings = [];
function warn(message) {
  if (!warnings.includes(message)) warnings.push(message);
}

function cleanDoc(lines) {
  const text = lines
    .map((line) =>
      line
        .replace(/^\s*\/\*\*\s?/, '')
        .replace(/\s*\*\/\s*$/, '')
        .replace(/^\s*\*\s?/, ''),
    )
    .join('\n')
    .trim();
  if (!text) return '';
  // Paragraph breaks survive; soft line wraps collapse to spaces.
  return text
    .split(/\n\s*\n/)
    .map((paragraph) => paragraph.replace(/\s*\n\s*/g, ' ').trim())
    .join('\n\n');
}

/**
 * Split `source` on a separator that appears at bracket depth zero, honouring
 * quotes. Used for union members, generic arguments and parameter lists.
 */
function splitTopLevel(source, separator) {
  const parts = [];
  let depth = 0;
  let quote = null;
  let current = '';
  for (let i = 0; i < source.length; i += 1) {
    const ch = source[i];
    if (quote) {
      current += ch;
      if (ch === quote && source[i - 1] !== '\\') quote = null;
      continue;
    }
    if (ch === "'" || ch === '"' || ch === '`') {
      quote = ch;
      current += ch;
      continue;
    }
    if (ch === '<' || ch === '(' || ch === '[' || ch === '{') depth += 1;
    if (ch === '>' || ch === ')' || ch === ']' || ch === '}') depth -= 1;
    if (ch === separator && depth === 0) {
      parts.push(current);
      current = '';
      continue;
    }
    current += ch;
  }
  parts.push(current);
  return parts.map((part) => part.trim()).filter((part) => part.length > 0);
}

/** Find the index of the `)` that closes the `(` at `openIndex`. */
function matchingParen(source, openIndex) {
  let depth = 0;
  let quote = null;
  for (let i = openIndex; i < source.length; i += 1) {
    const ch = source[i];
    if (quote) {
      if (ch === quote && source[i - 1] !== '\\') quote = null;
      continue;
    }
    if (ch === "'" || ch === '"' || ch === '`') {
      quote = ch;
      continue;
    }
    if (ch === '(') depth += 1;
    if (ch === ')') {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  return -1;
}

function bracketBalance(text) {
  let balance = 0;
  let quote = null;
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (quote) {
      if (ch === quote && text[i - 1] !== '\\') quote = null;
      continue;
    }
    if (ch === "'" || ch === '"' || ch === '`') {
      quote = ch;
      continue;
    }
    if ('<([{'.includes(ch)) balance += 1;
    if ('>)]}'.includes(ch)) balance -= 1;
  }
  return balance;
}

/**
 * First pass: turn the file into top-level declarations, each with the JSDoc
 * that immediately precedes it and its raw body lines.
 */
function scanDeclarations(source) {
  const lines = source.split('\n');
  const declarations = [];
  let pendingDoc = [];
  let i = 0;

  const takeDoc = () => {
    const doc = cleanDoc(pendingDoc);
    pendingDoc = [];
    return doc;
  };

  while (i < lines.length) {
    const line = lines[i];
    const trimmed = line.trim();

    if (trimmed.startsWith('/**')) {
      const block = [line];
      while (!lines[i].includes('*/') && i + 1 < lines.length) {
        i += 1;
        block.push(lines[i]);
      }
      pendingDoc = block;
      i += 1;
      continue;
    }
    if (trimmed === '' || trimmed.startsWith('//') || trimmed.startsWith('/*')) {
      // A stray line comment or blank line between a doc block and its owner
      // keeps the doc; a doc followed by nothing is dropped below.
      i += 1;
      continue;
    }

    let match;
    if ((match = /^export interface (\w+)(?:<[^>]*>)?(?: extends (.+?))? \{$/.exec(trimmed))) {
      const body = [];
      i += 1;
      while (i < lines.length && lines[i].trim() !== '}') {
        body.push(lines[i]);
        i += 1;
      }
      declarations.push({
        kind: 'interface',
        name: match[1],
        extends: match[2] ? splitTopLevel(match[2], ',') : [],
        doc: takeDoc(),
        body,
      });
      i += 1;
      continue;
    }
    if ((match = /^export (?:declare )?class (\w+)(?:<[^>]*>)?(?: extends (\w+))?(?: implements .+?)? \{$/.exec(trimmed))) {
      const body = [];
      i += 1;
      while (i < lines.length && lines[i].trim() !== '}') {
        body.push(lines[i]);
        i += 1;
      }
      declarations.push({
        kind: 'class',
        name: match[1],
        extends: match[2] || null,
        doc: takeDoc(),
        body,
      });
      i += 1;
      continue;
    }
    if ((match = /^export (?:declare )?type (\w+)(?:<[^>]*>)?\s*=\s*(.*)$/.exec(trimmed))) {
      let text = match[2];
      // Multi-line aliases: keep reading while brackets are open or the union
      // continues on the next line.
      while (
        i + 1 < lines.length &&
        (bracketBalance(text) > 0 ||
          /[|=&,]$/.test(text.trim()) ||
          /^\s*[|&]/.test(lines[i + 1]))
      ) {
        i += 1;
        text += ` ${lines[i].trim()}`;
      }
      declarations.push({
        kind: 'alias',
        name: match[1],
        doc: takeDoc(),
        type: text.trim().replace(/;$/, ''),
      });
      i += 1;
      continue;
    }
    if ((match = /^export (?:declare )?(?:const )?enum (\w+) \{(.*)$/.exec(trimmed))) {
      const body = [];
      if (match[2].trim().endsWith('}')) {
        // One-line enum: `export enum X { A = 'a', B = 'b' }`
        body.push(...splitTopLevel(match[2].trim().slice(0, -1), ','));
      } else {
        i += 1;
        while (i < lines.length && lines[i].trim() !== '}') {
          body.push(lines[i]);
          i += 1;
        }
      }
      declarations.push({ kind: 'enum', name: match[1], doc: takeDoc(), body });
      i += 1;
      continue;
    }
    if (/^export (?:declare )?(?:function|const|let|var) /.test(trimmed)) {
      takeDoc();
      i += 1;
      continue;
    }
    if (/^(?:export|declare|import) /.test(trimmed)) {
      warn(`unhandled top-level declaration at index.d.ts:${i + 1}: ${trimmed.slice(0, 80)}`);
    }
    takeDoc();
    i += 1;
  }

  return declarations;
}

// ---------------------------------------------------------------------------
// Declaration models
// ---------------------------------------------------------------------------

function parseMembers(body, ownerName) {
  const members = [];
  let pendingDoc = [];
  for (let i = 0; i < body.length; i += 1) {
    const raw = body[i];
    const trimmed = raw.trim();
    if (trimmed.startsWith('/**')) {
      const block = [raw];
      while (!body[i].includes('*/') && i + 1 < body.length) {
        i += 1;
        block.push(body[i]);
      }
      pendingDoc = block;
      continue;
    }
    if (trimmed === '' || trimmed.startsWith('//')) continue;
    const doc = cleanDoc(pendingDoc);
    pendingDoc = [];

    let match;
    if ((match = /^\[(?:key|k|name|index)\s*:\s*\w+\]\s*:\s*(.+)$/.exec(trimmed))) {
      members.push({ kind: 'index', type: match[1].replace(/;$/, ''), doc });
      continue;
    }
    if (trimmed.startsWith('[')) {
      // Computed members such as [Symbol.asyncIterator]() carry no tool value.
      continue;
    }
    if ((match = /^(static\s+)?get (\w+)\(\)\s*:\s*(.+)$/.exec(trimmed))) {
      members.push({
        kind: 'getter',
        name: match[2],
        static: Boolean(match[1]),
        type: match[3].replace(/;$/, ''),
        doc,
      });
      continue;
    }
    if (/^(static\s+)?set \w+\(/.test(trimmed)) continue;
    if ((match = /^(static\s+)?(?:async\s+)?(\w+)(\??)\s*(?:<[^>]*>)?\(/.exec(trimmed))) {
      const open = trimmed.indexOf('(');
      const close = matchingParen(trimmed, open);
      if (close === -1) {
        warn(`${ownerName}.${match[2]}: could not find the end of the parameter list`);
        continue;
      }
      const paramsText = trimmed.slice(open + 1, close);
      const rest = trimmed.slice(close + 1).trim();
      const returnType = rest.startsWith(':') ? rest.slice(1).trim().replace(/;$/, '') : 'void';
      members.push({
        kind: 'method',
        name: match[2],
        static: Boolean(match[1]),
        optional: match[3] === '?',
        params: parseParams(paramsText, `${ownerName}.${match[2]}`),
        returnType,
        signature: trimmed.replace(/;$/, ''),
        doc,
      });
      continue;
    }
    if ((match = /^(readonly\s+)?(\w+)(\?)?\s*:\s*(.+)$/.exec(trimmed))) {
      members.push({
        kind: 'property',
        name: match[2],
        optional: match[3] === '?',
        readonly: Boolean(match[1]),
        type: match[4].replace(/;$/, ''),
        doc,
      });
      continue;
    }
    warn(`${ownerName}: unhandled member: ${trimmed.slice(0, 80)}`);
  }
  return members;
}

function parseParams(text, owner) {
  if (!text.trim()) return [];
  return splitTopLevel(text, ',').map((part) => {
    const match = /^(\.\.\.)?(\w+)(\?)?\s*:\s*(.+)$/.exec(part.trim());
    if (!match) {
      warn(`${owner}: unhandled parameter: ${part}`);
      return { name: part.trim(), optional: true, rest: false, type: 'any' };
    }
    return {
      name: match[2],
      optional: match[3] === '?',
      rest: Boolean(match[1]),
      type: match[4].trim(),
    };
  });
}

function parseEnumMembers(body, name) {
  const values = [];
  for (const raw of body) {
    const trimmed = raw.trim().replace(/,$/, '');
    if (!trimmed || trimmed.startsWith('/') || trimmed.startsWith('*')) continue;
    const match = /^(\w+)(?:\s*=\s*(.+))?$/.exec(trimmed);
    if (!match) {
      warn(`enum ${name}: unhandled member: ${trimmed}`);
      continue;
    }
    if (match[2] === undefined) {
      values.push(values.length);
    } else if (/^['"`]/.test(match[2])) {
      values.push(match[2].slice(1, -1));
    } else if (/^-?\d+(\.\d+)?$/.test(match[2])) {
      values.push(Number(match[2]));
    } else {
      warn(`enum ${name}: computed member ${match[1]} = ${match[2]}`);
    }
  }
  return values;
}

// ---------------------------------------------------------------------------
// Type expressions → JSON Schema
// ---------------------------------------------------------------------------

function parseType(text) {
  let source = text.trim();
  if (source.endsWith(';')) source = source.slice(0, -1).trim();

  const unionParts = splitTopLevel(source, '|');
  if (unionParts.length > 1) {
    return { kind: 'union', members: unionParts.map(parseType) };
  }
  const intersectionParts = splitTopLevel(source, '&');
  if (intersectionParts.length > 1) {
    return { kind: 'intersection', members: intersectionParts.map(parseType) };
  }

  // Function types: `(a: T) => R`
  if (source.startsWith('(')) {
    const close = matchingParen(source, 0);
    const after = source.slice(close + 1).trim();
    if (after.startsWith('=>')) return { kind: 'function', text: source };
    if (after === '') return parseType(source.slice(1, close));
    if (after.startsWith('[]')) {
      return { kind: 'array', items: parseType(source.slice(1, close)) };
    }
  }

  if (source.endsWith('[]')) {
    return { kind: 'array', items: parseType(source.slice(0, -2)) };
  }
  if (source.startsWith('[') && source.endsWith(']')) {
    return { kind: 'tuple', members: splitTopLevel(source.slice(1, -1), ',').map(parseType) };
  }
  if (source.startsWith('{') && source.endsWith('}')) {
    return { kind: 'object-literal', text: source };
  }
  if (/^['"`]/.test(source)) {
    return { kind: 'literal', value: source.slice(1, -1) };
  }
  if (/^-?\d+(\.\d+)?$/.test(source)) {
    return { kind: 'literal', value: Number(source) };
  }
  if (source === 'true' || source === 'false') {
    return { kind: 'literal', value: source === 'true' };
  }

  const generic = /^([\w.]+)\s*<(.+)>$/.exec(source);
  if (generic) {
    return {
      kind: 'generic',
      name: generic[1],
      args: splitTopLevel(generic[2], ',').map(parseType),
    };
  }
  if (/^[\w.]+$/.test(source)) {
    return { kind: 'ref', name: source };
  }
  return { kind: 'unknown', text: source };
}

const PRIMITIVE_SCHEMAS = {
  string: () => ({ type: 'string' }),
  number: () => ({ type: 'number' }),
  boolean: () => ({ type: 'boolean' }),
  bigint: () => ({ type: 'string', description: 'bigint, as a decimal string' }),
  null: () => ({ type: 'null' }),
  any: () => ({}),
  unknown: () => ({}),
  void: () => ({}),
  never: () => ({}),
  object: () => ({ type: 'object' }),
  Buffer: () => ({ type: 'string', contentEncoding: 'base64', description: 'binary, base64-encoded' }),
  Uint8Array: () => ({ type: 'string', contentEncoding: 'base64', description: 'binary, base64-encoded' }),
  Date: () => ({ type: 'string', format: 'date-time' }),
};

function makeNullable(schema) {
  if (schema.enum) {
    return {
      ...schema,
      type: Array.isArray(schema.type) ? schema.type : [schema.type, 'null'].filter(Boolean),
      enum: schema.enum.includes(null) ? schema.enum : [...schema.enum, null],
    };
  }
  if (typeof schema.type === 'string') {
    return { ...schema, type: [schema.type, 'null'] };
  }
  if (Array.isArray(schema.type)) {
    return schema.type.includes('null') ? schema : { ...schema, type: [...schema.type, 'null'] };
  }
  if (Object.keys(schema).length === 0) return schema; // `any | null` is still any
  return { anyOf: [schema, { type: 'null' }] };
}

class SchemaBuilder {
  constructor(registry) {
    this.registry = registry;
    this.stack = [];
    /** Interface name → schema; emitted once under `definitions`. */
    this.definitions = new Map();
  }

  fromText(text, owner) {
    return this.fromAst(parseType(text), owner);
  }

  /** Resolve a `$ref` to its definition (used where a property list is needed). */
  deref(schema) {
    if (schema && schema.$ref) {
      const name = schema.$ref.replace('#/definitions/', '');
      return this.definitions.get(name) || schema;
    }
    return schema;
  }

  fromAst(ast, owner) {
    switch (ast.kind) {
      case 'literal':
        return { type: typeof ast.value, enum: [ast.value] };
      case 'array':
        return { type: 'array', items: this.fromAst(ast.items, owner) };
      case 'tuple':
        return {
          type: 'array',
          prefixItems: ast.members.map((member) => this.fromAst(member, owner)),
          minItems: ast.members.length,
          maxItems: ast.members.length,
        };
      case 'function':
        warn(`${owner}: function-typed value cannot be expressed as JSON (${ast.text})`);
        return { description: `unsupported: ${ast.text}` };
      case 'object-literal':
        warn(`${owner}: inline object type is not expanded (${ast.text.slice(0, 60)})`);
        return { type: 'object' };
      case 'unknown':
        warn(`${owner}: unhandled type expression: ${ast.text}`);
        return {};
      case 'intersection': {
        const members = ast.members.map((member) => this.deref(this.fromAst(member, owner)));
        if (members.every((member) => member.type === 'object' && member.properties)) {
          return members.reduce(
            (merged, member) => ({
              type: 'object',
              properties: { ...merged.properties, ...member.properties },
              required: [...new Set([...(merged.required || []), ...(member.required || [])])],
              additionalProperties: merged.additionalProperties && member.additionalProperties,
            }),
            { type: 'object', properties: {}, required: [], additionalProperties: false },
          );
        }
        return { allOf: members };
      }
      case 'union':
        return this.fromUnion(ast, owner);
      case 'generic':
        return this.fromGeneric(ast, owner);
      case 'ref':
        return this.fromRef(ast.name, owner);
      default:
        warn(`${owner}: unhandled AST node ${ast.kind}`);
        return {};
    }
  }

  fromUnion(ast, owner) {
    // Flatten nested unions (an alias member may itself be a union).
    const members = [];
    const aliasDocs = [];
    for (const member of ast.members) {
      if (member.kind === 'ref' && member.name === 'undefined') continue;
      const alias = member.kind === 'ref' ? this.registry.aliases.get(member.name) : null;
      const resolved = alias ? parseType(alias.type) : member;
      if (alias?.doc) aliasDocs.push(alias.doc);
      if (resolved.kind === 'union') members.push(...resolved.members);
      else members.push(resolved);
    }
    const nullable = members.some((member) => member.kind === 'ref' && member.name === 'null');
    const concrete = members.filter((member) => !(member.kind === 'ref' && member.name === 'null'));

    if (concrete.length === 0) return { type: 'null' };

    let schema;
    if (concrete.every((member) => member.kind === 'literal')) {
      const values = concrete.map((member) => member.value);
      const types = [...new Set(values.map((value) => typeof value))];
      schema = { type: types.length === 1 ? types[0] : types, enum: values };
    } else if (concrete.length === 1) {
      schema = this.fromAst(concrete[0], owner);
    } else {
      const schemas = concrete.map((member) => this.fromAst(member, owner));
      // Collapse `'a' | 'b' | string`-style unions to the wider primitive.
      const allPrimitive = schemas.every(
        (member) => typeof member.type === 'string' && Object.keys(member).length <= 2,
      );
      if (allPrimitive && schemas.every((member) => !member.enum)) {
        schema = { type: [...new Set(schemas.map((member) => member.type))] };
      } else {
        schema = { anyOf: schemas };
      }
    }
    // A flattened alias keeps its JSDoc when it is the only documented member.
    if (!schema.description && aliasDocs.length === 1) schema = { ...schema, description: aliasDocs[0] };
    return nullable ? makeNullable(schema) : schema;
  }

  fromGeneric(ast, owner) {
    const [first, second] = ast.args;
    switch (ast.name) {
      case 'Array':
      case 'ReadonlyArray':
      case 'Iterable':
      case 'Set':
        return { type: 'array', items: this.fromAst(first, owner) };
      case 'Promise':
      case 'PromiseLike':
      case 'Readonly':
      case 'Required':
      case 'NonNullable':
        return this.fromAst(first, owner);
      case 'Partial': {
        const inner = this.deref(this.fromAst(first, owner));
        if (inner.type === 'object' && inner.properties) {
          const { required: _dropped, ...rest } = inner;
          return rest;
        }
        return inner;
      }
      case 'Record':
      case 'Map':
        return { type: 'object', additionalProperties: this.fromAst(second, owner) };
      default:
        warn(`${owner}: unhandled generic ${ast.name}<…>`);
        return {};
    }
  }

  fromRef(name, owner) {
    if (PRIMITIVE_SCHEMAS[name]) return PRIMITIVE_SCHEMAS[name]();
    if (name === 'undefined') return {};
    const { interfaces, aliases, enums, classes } = this.registry;

    if (aliases.has(name)) {
      const alias = aliases.get(name);
      const schema = this.fromText(alias.type, `${owner} → ${name}`);
      if (alias.doc && !schema.description) return { ...schema, description: alias.doc };
      return schema;
    }
    if (enums.has(name)) {
      const values = enums.get(name).values;
      const types = [...new Set(values.map((value) => typeof value))];
      return { type: types.length === 1 ? types[0] : types, enum: values };
    }
    if (interfaces.has(name)) {
      // Interfaces are emitted once under `definitions` and referenced with
      // `$ref`; native-toolkit.mjs inlines them at load time so consumers see
      // self-contained schemas while the shipped file stays small.
      if (!this.definitions.has(name) && !this.stack.includes(name)) {
        this.stack.push(name);
        try {
          this.definitions.set(name, this.fromInterface(interfaces.get(name), owner));
        } finally {
          this.stack.pop();
        }
      }
      return { $ref: `#/definitions/${name}` };
    }
    if (classes.has(name)) {
      return { type: 'object', description: `${name} handle (not JSON-representable)` };
    }
    warn(`${owner}: unresolved type reference ${name}`);
    return { description: name };
  }

  fromInterface(declaration, owner) {
    const properties = {};
    const required = [];
    let additionalProperties = false;

    for (const base of declaration.extends) {
      const baseAst = parseType(base);
      if (baseAst.kind === 'generic' && ['AsyncIterable', 'Iterable', 'AsyncIterator'].includes(baseAst.name)) {
        // Runtime protocol, no JSON shape.
        continue;
      }
      const baseSchema = this.deref(this.fromAst(baseAst, `${owner} → ${declaration.name} extends ${base}`));
      if (baseSchema.type === 'object' && baseSchema.properties) {
        Object.assign(properties, baseSchema.properties);
        for (const key of baseSchema.required || []) if (!required.includes(key)) required.push(key);
        if (baseSchema.additionalProperties) additionalProperties = baseSchema.additionalProperties;
      } else {
        warn(`${declaration.name} extends ${base}: base is not an object type`);
      }
    }

    const fieldNames = new Set(declaration.members.filter((m) => m.kind === 'property').map((m) => m.name));

    for (const member of declaration.members) {
      if (member.kind === 'index') {
        additionalProperties = this.fromText(member.type, `${declaration.name}[key]`);
        if (Object.keys(additionalProperties).length === 0) additionalProperties = true;
        continue;
      }
      if (member.kind !== 'property') continue;

      let schema = this.fromText(member.type, `${declaration.name}.${member.name}`);
      const notes = [];
      if (member.doc) notes.push(member.doc);
      const exactTwin = `${member.name}Exact`;
      if (isNumberSchema(schema) && fieldNames.has(exactTwin)) {
        notes.push(`Float money field; prefer the \`${exactTwin}\` string for exact amounts.`);
        schema = { ...schema, 'x-money': 'float' };
      } else if (member.name.endsWith('Exact') && schema.type === 'string') {
        schema = { ...schema, 'x-money': 'exact' };
      }
      if (notes.length > 0) {
        schema = { ...schema, description: notes.join(' ') };
      }
      properties[member.name] = schema;

      const optional = member.optional || /\bundefined\b/.test(member.type);
      const index = required.indexOf(member.name);
      if (optional && index !== -1) required.splice(index, 1);
      if (!optional && index === -1) required.push(member.name);
    }

    const schema = { type: 'object', properties };
    if (required.length > 0) schema.required = required;
    schema.additionalProperties = additionalProperties;
    if (declaration.doc) schema.description = declaration.doc;
    return schema;
  }
}

function isNumberSchema(schema) {
  return schema.type === 'number' || (Array.isArray(schema.type) && schema.type.includes('number'));
}

// ---------------------------------------------------------------------------
// Descriptor assembly
// ---------------------------------------------------------------------------

function camelWords(name) {
  return name
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/([A-Z]+)([A-Z][a-z])/g, '$1 $2')
    .split(/[\s_]+/)
    .filter(Boolean);
}

function humanize(name) {
  const words = camelWords(name).map((word) => word.toLowerCase());
  if (words.length === 0) return name;
  return words[0][0].toUpperCase() + words[0].slice(1) + (words.length > 1 ? ` ${words.slice(1).join(' ')}` : '');
}

function unwrapPromise(returnType) {
  const match = /^Promise<(.+)>$/.exec(returnType.trim());
  return match ? match[1] : returnType;
}

function isReadOnly(toolName, methodName) {
  if (Object.hasOwn(READ_ONLY_OVERRIDES, toolName)) return READ_ONLY_OVERRIDES[toolName];
  const wildcard = `${toolName.split('.')[0]}.*`;
  if (Object.hasOwn(READ_ONLY_OVERRIDES, wildcard)) return READ_ONLY_OVERRIDES[wildcard];
  const [first] = camelWords(methodName).map((word) => word.toLowerCase());
  return READ_ONLY_PREFIXES.has(first);
}

function buildRegistry(declarations) {
  const interfaces = new Map();
  const aliases = new Map();
  const enums = new Map();
  const classes = new Map();

  for (const declaration of declarations) {
    if (declaration.kind === 'interface') {
      const members = parseMembers(declaration.body, declaration.name);
      const existing = interfaces.get(declaration.name);
      if (existing) {
        // Declaration merging (the augment block re-opens generated types).
        existing.members.push(...members);
        existing.extends.push(...declaration.extends);
        if (!existing.doc) existing.doc = declaration.doc;
      } else {
        interfaces.set(declaration.name, { ...declaration, members });
      }
    } else if (declaration.kind === 'alias') {
      aliases.set(declaration.name, declaration);
    } else if (declaration.kind === 'enum') {
      enums.set(declaration.name, { ...declaration, values: parseEnumMembers(declaration.body, declaration.name) });
    } else if (declaration.kind === 'class') {
      classes.set(declaration.name, { ...declaration, members: parseMembers(declaration.body, declaration.name) });
    }
  }
  return { interfaces, aliases, enums, classes };
}

function returnsHandle(returnType, registry) {
  const ast = parseType(returnType);
  const inner = ast.kind === 'generic' && ast.name === 'Promise' ? ast.args[0] : ast;
  const candidates = inner.kind === 'union' ? inner.members : [inner];
  return candidates.some((member) => member.kind === 'ref' && registry.classes.has(member.name));
}

function collectClassMethods(className, registry, seen = new Set()) {
  const declaration = registry.classes.get(className);
  if (!declaration || seen.has(className)) return [];
  seen.add(className);
  const inherited = declaration.extends ? collectClassMethods(declaration.extends, registry, seen) : [];
  const own = declaration.members.filter((member) => member.kind === 'method');
  const byName = new Map(inherited.map((member) => [member.name, member]));
  for (const member of own) byName.set(member.name, member);
  // Interface merging can add JS-side methods to a class of the same name.
  const augment = registry.interfaces.get(className);
  if (augment) {
    for (const member of augment.members) {
      if (member.kind === 'method' && !byName.has(member.name)) byName.set(member.name, member);
    }
  }
  return [...byName.values()];
}

function generate(source) {
  const declarations = scanDeclarations(source);
  const registry = buildRegistry(declarations);
  const builder = new SchemaBuilder(registry);

  const commerce = registry.classes.get('Commerce');
  if (!commerce) throw new Error('index.d.ts does not declare `export declare class Commerce`');

  const modules = [];
  for (const member of commerce.members) {
    if (member.kind !== 'getter' || member.static) continue;
    const ast = parseType(member.type);
    if (ast.kind !== 'ref' || !registry.classes.has(ast.name)) continue;
    modules.push({ getter: member.name, className: ast.name, doc: member.doc });
  }
  modules.sort((a, b) => byCodeUnit(a.getter, b.getter));

  const tools = [];
  for (const module of modules) {
    const methods = collectClassMethods(module.className, registry);
    for (const method of methods) {
      if (method.static || method.name.startsWith('__') || SKIPPED_METHODS.has(method.name)) continue;
      const toolName = `${module.getter}.${method.name}`;
      if (SKIPPED_TOOLS.has(toolName)) continue;
      if (returnsHandle(method.returnType, registry)) {
        warn(`${toolName}: skipped, returns a live handle (${method.returnType})`);
        continue;
      }

      const properties = {};
      const required = [];
      const positional = [];
      for (const param of method.params) {
        let schema = builder.fromText(param.type, `${toolName}(${param.name})`);
        if (param.rest) schema = { type: 'array', items: schema };
        properties[param.name] = schema;
        positional.push(param.name);
        const optional = param.optional || param.rest || /\bundefined\b/.test(param.type);
        if (!optional) required.push(param.name);
      }
      const parameters = { type: 'object', properties };
      if (required.length > 0) parameters.required = required;
      parameters.additionalProperties = false;

      const readOnly = isReadOnly(toolName, method.name);
      const description =
        method.doc ||
        `${humanize(method.name)} (${module.className}.${method.name}). Returns ${unwrapPromise(method.returnType)}.`;

      tools.push({
        name: toolName,
        module: module.getter,
        className: module.className,
        method: method.name,
        description,
        signature: method.signature,
        returnType: method.returnType,
        readOnly,
        permission: readOnly ? 'read' : 'write',
        positional,
        parameters,
      });
    }
  }
  tools.sort((a, b) => byCodeUnit(a.name, b.name));

  return {
    $schema: 'https://stateset.com/schemas/embedded-tool-descriptors/v1.json',
    meta: {
      generator: 'scripts/generate-tool-descriptors.mjs',
      source: 'index.d.ts',
      modules: modules.map((module) => ({ getter: module.getter, className: module.className })),
      toolCount: tools.length,
      readOnlyCount: tools.filter((tool) => tool.readOnly).length,
      readOnlyPrefixes: [...READ_ONLY_PREFIXES].sort(byCodeUnit),
      readOnlyOverrides: Object.fromEntries(
        Object.entries(READ_ONLY_OVERRIDES).sort(([a], [b]) => byCodeUnit(a, b)),
      ),
      warnings: [...warnings].sort(byCodeUnit),
    },
    definitions: Object.fromEntries(
      [...builder.definitions.entries()].sort(([a], [b]) => byCodeUnit(a, b)),
    ),
    tools,
  };
}

export function generateToolDescriptors(source = readFileSync(SOURCE, 'utf8')) {
  warnings.length = 0;
  return generate(source);
}

const invokedDirectly = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  const descriptors = generateToolDescriptors();
  writeFileSync(TARGET, `${JSON.stringify(descriptors, null, 2)}\n`);
  const { toolCount, readOnlyCount, modules, warnings: emitted } = descriptors.meta;
  console.log(
    `tool-descriptors.json: ${toolCount} tools (${readOnlyCount} read-only) across ${modules.length} modules, ${Object.keys(descriptors.definitions).length} definitions`,
  );
  for (const warning of emitted) console.warn(`  warning: ${warning}`);
}
