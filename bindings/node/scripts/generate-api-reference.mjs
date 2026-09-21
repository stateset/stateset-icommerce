#!/usr/bin/env node
/**
 * Generate `docs/src/api/node-reference.md` from `index.d.ts`.
 *
 * The declarations file is the single source of truth for the public surface
 * of `@stateset/embedded`: napi-rs writes it from the Rust binding and
 * `postbuild-types.mjs` appends the hand-written JavaScript-side additions.
 * This script turns it into one Markdown page for the mdBook docs site:
 *
 *   1. the `Commerce` entry point,
 *   2. one section per sub-API class reachable from a `Commerce` getter
 *      (headed by the access path, e.g. `commerce.orders`), plus classes
 *      reached through those (e.g. `commerce.events.subscribe()`),
 *   3. the free functions (crypto / VES / x402),
 *   4. every `interface`, `type` alias and `enum`, alphabetically.
 *
 * No dependencies: the parser is a depth-aware statement scanner tuned to the
 * shape napi-rs emits, with a verbatim fallback for anything it does not
 * recognise (reported on stderr, never fatal). Output is deterministic for a
 * given input; `test/api-reference.js` fails when the committed page drifts.
 *
 * Usage:
 *   node scripts/generate-api-reference.mjs            # write the page
 *   node scripts/generate-api-reference.mjs --check    # exit 1 if stale
 *   node scripts/generate-api-reference.mjs --stdout   # print instead
 */
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
export const DECLARATIONS_PATH = path.join(here, '..', 'index.d.ts');
export const OUTPUT_PATH = path.join(here, '..', '..', '..', 'docs', 'src', 'api', 'node-reference.md');

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/** Turn a `/** ... *\/` block into { text, deprecated }. */
function parseDoc(lines) {
  const body = [];
  for (let line of lines) {
    line = line.trim();
    if (line.startsWith('/**')) line = line.slice(3).trimStart();
    if (line.endsWith('*/')) line = line.slice(0, -2);
    line = line.replace(/^\s*\*\s?/, '').replace(/\s+$/, '');
    body.push(line);
  }
  while (body.length && body[0] === '') body.shift();
  while (body.length && body[body.length - 1] === '') body.pop();
  let deprecated = null;
  const kept = [];
  for (let i = 0; i < body.length; i += 1) {
    const m = /^@deprecated\b\s*(.*)$/.exec(body[i]);
    if (!m) {
      kept.push(body[i]);
      continue;
    }
    const parts = [m[1]];
    while (i + 1 < body.length && body[i + 1] !== '' && !body[i + 1].startsWith('@')) {
      i += 1;
      parts.push(body[i].trim());
    }
    deprecated = parts.join(' ').trim();
  }
  return { text: kept.join('\n').trim(), deprecated };
}

/** Bracket depth of `text` after stripping arrow tokens and string literals. */
function depthDelta(text) {
  const stripped = text.replace(/=>/g, '').replace(/'(?:[^'\\]|\\.)*'|"(?:[^"\\]|\\.)*"|`(?:[^`\\]|\\.)*`/g, '');
  let depth = 0;
  for (const ch of stripped) {
    if (ch === '(' || ch === '[' || ch === '{' || ch === '<') depth += 1;
    else if (ch === ')' || ch === ']' || ch === '}' || ch === '>') depth -= 1;
  }
  return depth;
}

/** Split `text` on top-level commas (ignores commas nested in brackets or strings). */
function splitTopLevel(text, separator = ',') {
  const parts = [];
  let depth = 0;
  let current = '';
  let quote = null;
  for (let i = 0; i < text.length; i += 1) {
    const ch = text[i];
    if (quote) {
      current += ch;
      if (ch === '\\') {
        current += text[i + 1] ?? '';
        i += 1;
      } else if (ch === quote) quote = null;
      continue;
    }
    if (ch === "'" || ch === '"' || ch === '`') {
      quote = ch;
      current += ch;
      continue;
    }
    if (ch === '=' && text[i + 1] === '>') {
      current += '=>';
      i += 1;
      continue;
    }
    if ('([{<'.includes(ch)) depth += 1;
    else if (')]}>'.includes(ch)) depth -= 1;
    if (ch === separator && depth === 0) {
      parts.push(current);
      current = '';
    } else current += ch;
  }
  if (current.trim() !== '') parts.push(current);
  return parts.map((p) => p.trim()).filter(Boolean);
}

const IDENT = String.raw`[A-Za-z_$][\w$]*`;
const MODIFIERS = String.raw`(?:(?:public|protected|private|static|readonly|abstract|override|declare)\s+)*`;

/** Parse one whitespace-normalised member statement of a class/interface body. */
function parseMember(raw) {
  const text = raw.replace(/\s+/g, ' ').replace(/[;,]$/, '').trim();
  let m;
  if ((m = new RegExp(`^(readonly )?\\[(${IDENT}): ([^\\]]+)\\]\\??: (.*)$`).exec(text))) {
    return { kind: 'index', name: `[${m[2]}: ${m[3]}]`, type: m[4], readonly: Boolean(m[1]), signature: text };
  }
  if ((m = new RegExp(`^\\[(${IDENT}(?:\\.${IDENT})*)\\](<[^)]*>)?\\((.*)\\)(?:: (.*))?$`).exec(text))) {
    return { kind: 'method', name: `[${m[1]}]`, params: m[3], returnType: m[4] ?? 'void', signature: text };
  }
  if ((m = /^constructor\((.*)\)$/.exec(text))) {
    return { kind: 'constructor', name: 'constructor', params: m[1], signature: text };
  }
  if ((m = new RegExp(`^(${MODIFIERS})(get |set )(${IDENT})\\((.*)\\)(?:: (.*))?$`).exec(text))) {
    return {
      kind: m[2].trim() === 'get' ? 'getter' : 'setter',
      name: m[3],
      modifiers: m[1].trim(),
      params: m[4],
      returnType: m[5] ?? 'void',
      static: /\bstatic\b/.test(m[1]),
      signature: text,
    };
  }
  if ((m = new RegExp(`^(${MODIFIERS})(${IDENT})(\\?)?(<[^(]*>)?\\((.*)\\)(?:: (.*))?$`).exec(text))) {
    return {
      kind: 'method',
      name: m[2],
      modifiers: m[1].trim(),
      optional: Boolean(m[3]),
      typeParams: m[4] ?? '',
      params: m[5],
      returnType: m[6] ?? 'void',
      static: /\bstatic\b/.test(m[1]),
      signature: text,
    };
  }
  if ((m = new RegExp(`^(${MODIFIERS})(${IDENT})(\\?)?: (.*)$`).exec(text))) {
    return {
      kind: 'property',
      name: m[2],
      modifiers: m[1].trim(),
      optional: Boolean(m[3]),
      type: m[4],
      readonly: /\breadonly\b/.test(m[1]),
      static: /\bstatic\b/.test(m[1]),
      signature: text,
    };
  }
  if ((m = new RegExp(`^(${IDENT})(?: = (.*))?$`).exec(text))) {
    // enum member
    return { kind: 'enum-member', name: m[1], value: m[2] ?? null, signature: text };
  }
  return { kind: 'unknown', name: text, signature: text };
}

/**
 * Parse a `.d.ts` source into declarations.
 *
 * @returns {{ declarations: Array<object>, warnings: Array<string> }}
 */
export function parseDeclarations(source) {
  const lines = source.split(/\r?\n/);
  const declarations = [];
  const warnings = [];
  let pendingDoc = null;
  let i = 0;

  const takeDoc = () => {
    const doc = pendingDoc ?? { text: '', deprecated: null };
    pendingDoc = null;
    return doc;
  };

  /** Read a statement starting at line `i` until brackets balance; returns joined text. */
  const readStatement = (stopAtBrace) => {
    const parts = [];
    let depth = 0;
    while (i < lines.length) {
      const line = lines[i];
      i += 1;
      parts.push(line.trim());
      depth += depthDelta(line);
      if (stopAtBrace && depth > 0) break; // opening `{` of a body: caller reads members
      if (depth <= 0) {
        const next = lines[i]?.trim() ?? '';
        if (!/^[|&]/.test(next)) break; // multi-line unions continue with a leading `|`
      }
    }
    return parts.join(' ').trim();
  };

  /** Parse members written inline (`{ a: string; b?: number }`), with any `/** *\/` docs. */
  const parseInlineMembers = (inner) => {
    const docs = [];
    const placeholders = inner.replace(/\/\*\*[\s\S]*?\*\//g, (m) => {
      docs.push(m);
      return `\u0000${docs.length - 1}\u0000`;
    });
    const members = [];
    for (const piece of splitTopLevel(placeholders, ';').flatMap((p) => splitTopLevel(p, ','))) {
      let memberDoc = { text: '', deprecated: null };
      const text = piece
        .replace(/\u0000(\d+)\u0000/g, (_, n) => {
          memberDoc = parseDoc([docs[Number(n)]]);
          return '';
        })
        .trim();
      if (!text) continue;
      const member = parseMember(text);
      member.doc = memberDoc;
      if (member.kind === 'unknown') warnings.push(`unrecognised member: ${member.signature}`);
      members.push(member);
    }
    return members;
  };

  /**
   * Consume a declaration header line. Returns the members: the whole body when
   * it sits on the header line, otherwise anything after `{` plus the following
   * lines up to the closing `}`.
   */
  const readDeclarationBody = (headerLine) => {
    i += 1;
    const open = headerLine.indexOf('{');
    const rest = headerLine.slice(open + 1);
    if (depthDelta(headerLine) <= 0) {
      return parseInlineMembers(rest.slice(0, rest.lastIndexOf('}')));
    }
    const leading = rest.trim() ? parseInlineMembers(rest) : [];
    return [...leading, ...readBody()];
  };

  /** Read the members of a class/interface/enum body until the closing `}` at depth 0. */
  const readBody = () => {
    const members = [];
    let memberDoc = null;
    let docLines = null;
    let statement = null;
    let depth = 0;
    while (i < lines.length) {
      const line = lines[i];
      i += 1;
      const trimmed = line.trim();
      if (docLines) {
        docLines.push(trimmed);
        if (trimmed.endsWith('*/')) {
          memberDoc = parseDoc(docLines);
          docLines = null;
        }
        continue;
      }
      if (statement === null) {
        if (trimmed === '') continue;
        if (trimmed === '}') return members;
        if (trimmed.startsWith('/**')) {
          if (trimmed.endsWith('*/') && trimmed.length > 4) memberDoc = parseDoc([trimmed]);
          else docLines = [trimmed];
          continue;
        }
        if (trimmed.startsWith('//') || trimmed.startsWith('/*')) continue;
        statement = [];
        depth = 0;
      }
      statement.push(trimmed);
      depth += depthDelta(trimmed);
      const closesBody = depth < 0 && trimmed.endsWith('}');
      if (closesBody) statement[statement.length - 1] = trimmed.slice(0, -1).trim();
      if (depth <= 0) {
        const next = lines[i]?.trim() ?? '';
        if (!closesBody && /^[|&]/.test(next)) continue;
        for (const member of parseInlineMembers(statement.join(' '))) {
          member.doc = memberDoc ?? member.doc;
          members.push(member);
        }
        statement = null;
        memberDoc = null;
        if (closesBody) return members;
      }
    }
    warnings.push('unterminated body');
    return members;
  };

  while (i < lines.length) {
    const trimmed = lines[i].trim();
    if (trimmed === '') {
      i += 1;
      continue;
    }
    if (trimmed.startsWith('/**')) {
      const docLines = [trimmed];
      i += 1;
      if (!(trimmed.endsWith('*/') && trimmed.length > 4)) {
        while (i < lines.length) {
          const l = lines[i].trim();
          docLines.push(l);
          i += 1;
          if (l.endsWith('*/')) break;
        }
      }
      pendingDoc = parseDoc(docLines);
      continue;
    }
    if (trimmed.startsWith('//') || trimmed.startsWith('/*')) {
      // A plain comment separates a doc block from the next declaration.
      pendingDoc = null;
      if (trimmed.startsWith('/*') && !trimmed.includes('*/')) {
        while (i < lines.length && !lines[i].includes('*/')) i += 1;
      }
      i += 1;
      continue;
    }

    let m;
    const head = trimmed.replace(/^export\s+/, '').replace(/^declare\s+/, '');
    if ((m = new RegExp(`^(abstract\\s+)?class\\s+(${IDENT})(<[^{]*?>)?(?:\\s+extends\\s+([^{]+?))?(?:\\s+implements\\s+([^{]+?))?\\s*\\{`).exec(head))) {
      const members = readDeclarationBody(trimmed);
      declarations.push({
        kind: 'class',
        name: m[2],
        typeParams: m[3] ?? '',
        extends: m[4]?.trim() ?? null,
        implements: m[5]?.trim() ?? null,
        abstract: Boolean(m[1]),
        doc: takeDoc(),
        members,
      });
      continue;
    }
    if ((m = new RegExp(`^interface\\s+(${IDENT})(<[^{]*?>)?(?:\\s+extends\\s+([^{]+?))?\\s*\\{`).exec(head))) {
      const members = readDeclarationBody(trimmed);
      declarations.push({
        kind: 'interface',
        name: m[1],
        typeParams: m[2] ?? '',
        extends: m[3] ? splitTopLevel(m[3]) : [],
        doc: takeDoc(),
        members,
      });
      continue;
    }
    if ((m = new RegExp(`^(const\\s+)?enum\\s+(${IDENT})\\s*\\{`).exec(head))) {
      const members = readDeclarationBody(trimmed);
      declarations.push({ kind: 'enum', name: m[2], const: Boolean(m[1]), doc: takeDoc(), members });
      continue;
    }
    if ((m = new RegExp(`^type\\s+(${IDENT})(<[^=]*?>)?\\s*=`).exec(head))) {
      const statement = readStatement(false);
      const eq = statement.indexOf('=');
      declarations.push({
        kind: 'type',
        name: m[1],
        typeParams: m[2] ?? '',
        definition: statement.slice(eq + 1).replace(/;$/, '').trim().replace(/^\|\s*/, ''),
        doc: takeDoc(),
      });
      continue;
    }
    if ((m = new RegExp(`^function\\s+(${IDENT})`).exec(head))) {
      const statement = readStatement(false).replace(/^export\s+/, '').replace(/^declare\s+/, '').replace(/;$/, '');
      const sig = new RegExp(`^function\\s+(${IDENT})(<[^(]*>)?\\((.*)\\)(?:: (.*))?$`).exec(statement.replace(/\s+/g, ' '));
      declarations.push({
        kind: 'function',
        name: m[1],
        typeParams: sig?.[2] ?? '',
        params: sig?.[3] ?? '',
        returnType: sig?.[4] ?? 'void',
        signature: statement.replace(/^function\s+/, '').replace(/\s+/g, ' '),
        doc: takeDoc(),
      });
      if (!sig) warnings.push(`unrecognised function shape: ${statement}`);
      continue;
    }
    if ((m = new RegExp(`^(?:const|let|var)\\s+(${IDENT})`).exec(head))) {
      const statement = readStatement(false).replace(/^export\s+/, '').replace(/^declare\s+/, '').replace(/;$/, '');
      declarations.push({ kind: 'const', name: m[1], signature: statement.replace(/^(?:const|let|var)\s+/, '').replace(/\s+/g, ' '), doc: takeDoc() });
      continue;
    }
    if (/^(import|export\s*\{|export\s+\*|export\s+default)/.test(trimmed)) {
      readStatement(false);
      pendingDoc = null;
      continue;
    }
    // Unknown top-level construct (namespace, module, ...): skip its statement or block.
    warnings.push(`skipped top-level statement: ${trimmed}`);
    const statement = readStatement(true);
    if (depthDelta(statement) > 0) {
      let depth = depthDelta(statement);
      while (i < lines.length && depth > 0) {
        depth += depthDelta(lines[i]);
        i += 1;
      }
    }
    pendingDoc = null;
  }
  return { declarations, warnings };
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

const ENTRY_CLASS = 'Commerce';
const ENTRY_VAR = 'commerce';

/** mdBook / GitHub style heading id. */
function slugify(text) {
  let out = '';
  for (const ch of text) {
    if (/[\p{L}\p{N}_-]/u.test(ch)) out += ch.toLowerCase();
    else if (/\s/.test(ch)) out += '-';
  }
  return out;
}

class Anchors {
  constructor() {
    this.counts = new Map();
    this.byKey = new Map();
  }
  /** Register a heading; returns its id (mirrors mdBook's duplicate handling). */
  register(key, headingText) {
    const base = slugify(headingText);
    const n = this.counts.get(base) ?? 0;
    this.counts.set(base, n + 1);
    const id = n === 0 ? base : `${base}-${n}`;
    this.byKey.set(key, id);
    return id;
  }
  get(key) {
    return this.byKey.get(key) ?? null;
  }
}

function escapeCell(text) {
  // Backslashes first: escaping `|` into `\|` after a literal backslash would
  // produce `\\|`, which Markdown reads as an escaped backslash followed by an
  // unescaped cell separator, splitting the row.
  return text
    .replace(/\\/g, '\\\\')
    .replace(/\|/g, '\\|')
    .replace(/\r?\n\s*\r?\n/g, ' ')
    .replace(/\r?\n/g, ' ')
    .trim();
}

function identifiers(typeText) {
  return typeText.match(/[A-Za-z_$][\w$]*/g) ?? [];
}

/** Names of declared types referenced by a type/params string, in first-seen order. */
function referencedTypes(text, known, exclude) {
  const seen = new Set();
  for (const id of identifiers(text)) {
    if (known.has(id) && id !== exclude && !seen.has(id)) seen.add(id);
  }
  return [...seen];
}

function moneyNote(name) {
  return name.endsWith('Exact') ? ' Exact money: a base-10 decimal string; prefer it over any float twin.' : '';
}

function renderDocBlock(doc, out) {
  if (doc.deprecated !== null) out.push(`**Deprecated.** ${doc.deprecated || 'This member will be removed.'}`, '');
  if (doc.text) out.push(doc.text, '');
}

/**
 * Member documentation. `table` mode flattens to one line for a table cell;
 * otherwise paragraphs are kept and returned as an array of lines.
 */
function renderMemberDoc(member, { table }) {
  const parts = [];
  if (member.doc.deprecated !== null) parts.push(`**Deprecated.** ${member.doc.deprecated || 'This member will be removed.'}`);
  if (member.doc.text) parts.push(table ? escapeCell(member.doc.text) : member.doc.text);
  if (member.kind === 'property' || member.kind === 'index') {
    const note = moneyNote(member.name).trim();
    if (note) parts.push(`_${note}_`);
  }
  return table ? parts.join(' ') : parts;
}

function typeLinks(names, anchors) {
  return names
    .map((n) => {
      const id = anchors.get(`type:${n}`) ?? anchors.get(`class:${n}`);
      return id ? `[\`${n}\`](#${id})` : `\`${n}\``;
    })
    .join(', ');
}

/** Render one class's callable members. */
function renderClassMembers(cls, extraMembers, known, anchors, out) {
  const members = [...cls.members, ...extraMembers];
  const order = { constructor: 0, getter: 1, setter: 1, property: 1, index: 1, method: 2, 'enum-member': 3, unknown: 4 };
  const sorted = members
    .map((member, index) => ({ member, index }))
    .sort((a, b) => order[a.member.kind] - order[b.member.kind] || a.index - b.index);
  const accessorRows = [];
  for (const { member } of sorted) {
    if (member.kind === 'getter' && known.classes.has(member.returnType.trim()) && cls.name === ENTRY_CLASS) {
      accessorRows.push(member); // rendered as the sub-API table below
      continue;
    }
    let signature;
    if (member.kind === 'constructor') signature = `new ${cls.name}(${member.params})`;
    else if (member.kind === 'getter') signature = `${member.static ? 'static ' : ''}get ${member.name}: ${member.returnType}`;
    else if (member.kind === 'setter') signature = `${member.static ? 'static ' : ''}set ${member.name}(${member.params})`;
    else signature = member.signature;
    out.push(`- **\`${signature}\`**`);
    for (const paragraph of renderMemberDoc(member, { table: false })) {
      out.push('');
      for (const line of paragraph.split('\n')) out.push(line ? `  ${line}` : '');
    }
    const refs = referencedTypes(
      member.kind === 'property' || member.kind === 'index' ? member.type : `${member.params ?? ''} ${member.returnType ?? ''}`,
      known.all,
      cls.name,
    );
    if (refs.length) out.push('', `  Types: ${typeLinks(refs, anchors)}`);
    out.push('');
  }
  return accessorRows;
}

/** Compute { path, class } for every class reachable from the entry class, in BFS order. */
function reachableClasses(classes) {
  const byName = new Map(classes.map((c) => [c.name, c]));
  const entry = byName.get(ENTRY_CLASS);
  const result = [];
  const aliases = new Map(); // class name -> additional access paths
  if (!entry) return { result, aliases };
  const visited = new Set([ENTRY_CLASS]);
  const queue = [{ cls: entry, path: ENTRY_VAR }];
  while (queue.length) {
    const { cls, path: base } = queue.shift();
    for (const member of cls.members) {
      const isGetter = member.kind === 'getter';
      const isMethod = member.kind === 'method' || member.kind === 'property';
      if (!isGetter && !isMethod) continue;
      const typeText = member.kind === 'property' ? member.type : member.returnType;
      for (const id of identifiers(typeText)) {
        const target = byName.get(id);
        if (!target) continue;
        const paramNames = isMethod && member.kind === 'method' ? splitTopLevel(member.params).map((p) => p.replace(/[?:].*$/, '')) : [];
        const accessPath = isGetter || member.kind === 'property' ? `${base}.${member.name}` : `${base}.${member.name}(${paramNames.join(', ')})`;
        if (visited.has(id)) {
          if (id !== ENTRY_CLASS) aliases.set(id, [...(aliases.get(id) ?? []), accessPath]);
          continue;
        }
        visited.add(id);
        const entryItem = { cls: target, path: accessPath, viaGetter: isGetter };
        result.push(entryItem);
        queue.push(entryItem);
      }
    }
  }
  return { result, aliases };
}

/**
 * Render the Markdown page.
 *
 * @param {string} source contents of index.d.ts
 * @returns {{ markdown: string, stats: object, warnings: Array<string> }}
 */
export function generateApiReference(source) {
  const { declarations, warnings } = parseDeclarations(source);
  const classes = declarations.filter((d) => d.kind === 'class');
  const functions = declarations.filter((d) => d.kind === 'function');
  const consts = declarations.filter((d) => d.kind === 'const');
  const classNames = new Set(classes.map((c) => c.name));
  // Interfaces sharing a class name are declaration-merged into the class.
  const mergedInterfaces = new Map();
  const interfaces = [];
  for (const d of declarations) {
    if (d.kind !== 'interface') continue;
    if (classNames.has(d.name)) mergedInterfaces.set(d.name, [...(mergedInterfaces.get(d.name) ?? []), d]);
    else interfaces.push(d);
  }
  const typeAliases = declarations.filter((d) => d.kind === 'type');
  const enums = declarations.filter((d) => d.kind === 'enum');
  const typeDecls = [...interfaces, ...typeAliases, ...enums].sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  const known = {
    classes: classNames,
    all: new Set([...classNames, ...typeDecls.map((d) => d.name)]),
  };

  const { result: reachable, aliases } = reachableClasses(classes);
  const reachableNames = new Set(reachable.map((r) => r.cls.name));
  const otherClasses = classes.filter((c) => c.name !== ENTRY_CLASS && !reachableNames.has(c.name));
  const entry = classes.find((c) => c.name === ENTRY_CLASS) ?? null;

  // Register headings in document order so anchors match what mdBook/GitHub emit.
  const anchors = new Anchors();
  anchors.register('h:contents', 'Contents');
  if (entry) anchors.register(`class:${ENTRY_CLASS}`, ENTRY_CLASS);
  anchors.register('h:subapis', 'Sub-APIs');
  for (const r of reachable) anchors.register(`class:${r.cls.name}`, r.path);
  if (otherClasses.length) anchors.register('h:other', 'Other classes');
  for (const c of otherClasses) anchors.register(`class:${c.name}`, c.name);
  anchors.register('h:functions', 'Functions');
  for (const f of functions) anchors.register(`fn:${f.name}`, f.name);
  for (const c of consts) anchors.register(`const:${c.name}`, c.name);
  anchors.register('h:types', 'Types');
  for (const d of typeDecls) anchors.register(`type:${d.name}`, d.name);

  const out = [];
  out.push('# `@stateset/embedded` API reference', '');
  out.push(
    '<!-- GENERATED FILE. Do not edit by hand: run `node scripts/generate-api-reference.mjs` in bindings/node. -->',
    '',
    'Generated from `bindings/node/index.d.ts` by `bindings/node/scripts/generate-api-reference.mjs`.',
    'The declarations are themselves generated from the Rust binding, so this page is the',
    'authoritative list of what the package exports. For guided examples see [Node.js](node.md).',
    '',
    'All monetary fields are exact base-10 decimal strings (field names ending in `Exact`);',
    'float twins, where they still exist, are deprecated. Methods return Promises unless noted.',
    '',
  );

  // Contents
  out.push('## Contents', '');
  if (entry) out.push(`- [${ENTRY_CLASS}](#${anchors.get(`class:${ENTRY_CLASS}`)})`);
  out.push(`- [Sub-APIs](#${anchors.get('h:subapis')})`);
  for (const r of reachable) out.push(`  - [\`${r.path}\`](#${anchors.get(`class:${r.cls.name}`)}) — \`${r.cls.name}\``);
  if (otherClasses.length) {
    out.push(`- [Other classes](#${anchors.get('h:other')})`);
    for (const c of otherClasses) out.push(`  - [\`${c.name}\`](#${anchors.get(`class:${c.name}`)})`);
  }
  out.push(`- [Functions](#${anchors.get('h:functions')})`);
  for (const f of functions) out.push(`  - [\`${f.name}\`](#${anchors.get(`fn:${f.name}`)})`);
  for (const c of consts) out.push(`  - [\`${c.name}\`](#${anchors.get(`const:${c.name}`)})`);
  out.push(`- [Types](#${anchors.get('h:types')})`);
  for (const d of typeDecls) out.push(`  - [\`${d.name}\`](#${anchors.get(`type:${d.name}`)})`);
  out.push('');

  const renderClass = (cls, headingText, headingLevel) => {
    out.push(`${'#'.repeat(headingLevel)} ${headingText}`, '');
    const merged = mergedInterfaces.get(cls.name) ?? [];
    const extendsList = [
      ...(cls.extends ? [cls.extends] : []),
      ...merged.flatMap((m) => m.extends),
    ];
    const meta = [];
    if (headingText !== cls.name) meta.push(`Class \`${cls.name}\`.`);
    if (extendsList.length) meta.push(`Extends ${extendsList.map((e) => `\`${e}\``).join(', ')}.`);
    if (cls.implements) meta.push(`Implements \`${cls.implements}\`.`);
    const alias = aliases.get(cls.name);
    if (alias?.length) meta.push(`Also available as ${alias.map((a) => `\`${a}\``).join(', ')}.`);
    if (meta.length) out.push(meta.join(' '), '');
    renderDocBlock(cls.doc, out);
    for (const m of merged) renderDocBlock(m.doc, out);
    const extraMembers = merged.flatMap((m) => m.members);
    const accessorRows = renderClassMembers(cls, extraMembers, known, anchors, out);
    if (accessorRows.length) {
      out.push('', `${'#'.repeat(headingLevel + 1)} Sub-API accessors`, '');
      out.push('| Accessor | Class | Description |', '|---|---|---|');
      for (const g of accessorRows) {
        const target = g.returnType.trim();
        const id = anchors.get(`class:${target}`);
        const link = id ? `[\`${ENTRY_VAR}.${g.name}\`](#${id})` : `\`${ENTRY_VAR}.${g.name}\``;
        out.push(`| ${link} | \`${target}\` | ${escapeCell(g.doc.text)} |`);
      }
    }
    out.push('');
  };

  if (entry) {
    renderClass(entry, ENTRY_CLASS, 2);
  }

  out.push('## Sub-APIs', '');
  out.push(`Each sub-API is a property of a \`${ENTRY_CLASS}\` instance; the heading is the access path.`, '');
  for (const r of reachable) renderClass(r.cls, r.path, 3);

  if (otherClasses.length) {
    out.push('## Other classes', '');
    for (const c of otherClasses) renderClass(c, c.name, 3);
  }

  // Functions
  out.push('## Functions', '');
  out.push('Free functions exported by the package (cryptography, VES envelopes, x402 signing).', '');
  for (const f of functions) {
    out.push(`### ${f.name}`, '');
    out.push('```ts', `function ${f.signature}`, '```', '');
    renderDocBlock(f.doc, out);
    const refs = referencedTypes(`${f.params} ${f.returnType}`, known.all, null);
    if (refs.length) out.push(`Types: ${typeLinks(refs, anchors)}`, '');
  }
  for (const c of consts) {
    out.push(`### ${c.name}`, '');
    out.push('```ts', `const ${c.signature}`, '```', '');
    renderDocBlock(c.doc, out);
  }

  // Types
  out.push('## Types', '');
  out.push('Interfaces, type aliases and enums, alphabetically. Optional fields are marked `?`.', '');
  for (const d of typeDecls) {
    out.push(`### ${d.name}`, '');
    if (d.kind === 'interface') {
      const meta = [];
      if (d.typeParams) meta.push(`Type parameters \`${d.typeParams}\`.`);
      if (d.extends.length) meta.push(`Extends ${typeLinks(d.extends.map((e) => e.replace(/<.*$/, '')), anchors)}.`);
      if (meta.length) out.push(meta.join(' '), '');
      renderDocBlock(d.doc, out);
      if (d.members.length === 0) out.push('_No members._', '');
      else {
        out.push('| Field | Type | Description |', '|---|---|---|');
        for (const member of d.members) {
          let name;
          let type;
          if (member.kind === 'property') {
            name = `${member.readonly ? 'readonly ' : ''}${member.name}${member.optional ? '?' : ''}`;
            type = member.type;
          } else if (member.kind === 'index') {
            name = member.name;
            type = member.type;
          } else if (member.kind === 'method') {
            name = `${member.name}${member.optional ? '?' : ''}(${member.params})`;
            type = member.returnType;
          } else {
            name = member.signature;
            type = '';
          }
          const refs = referencedTypes(type, known.all, d.name);
          const desc = [renderMemberDoc(member, { table: true }), refs.length ? `Types: ${typeLinks(refs, anchors)}` : ''].filter(Boolean).join(' ');
          out.push(`| \`${escapeCell(name)}\` | \`${escapeCell(type)}\` | ${desc} |`);
        }
        out.push('');
      }
    } else if (d.kind === 'type') {
      renderDocBlock(d.doc, out);
      const unionMembers = splitTopLevel(d.definition, '|');
      out.push('```ts', `type ${d.name}${d.typeParams} = ${d.definition}`, '```', '');
      const literalUnion = unionMembers.length > 1 && unionMembers.every((m) => /^(['"`]).*\1$|^-?\d+(\.\d+)?$|^(true|false|null|undefined)$/.test(m));
      if (literalUnion) {
        out.push(`One of: ${unionMembers.map((m) => `\`${m}\``).join(', ')}.`, '');
      }
      const refs = referencedTypes(d.definition, known.all, d.name);
      if (refs.length) out.push(`Types: ${typeLinks(refs, anchors)}`, '');
    } else if (d.kind === 'enum') {
      renderDocBlock(d.doc, out);
      out.push('| Member | Value | Description |', '|---|---|---|');
      for (const member of d.members) {
        out.push(`| \`${member.name}\` | ${member.value !== null ? `\`${escapeCell(member.value)}\`` : ''} | ${renderMemberDoc(member, { table: true })} |`);
      }
      out.push('');
    }
  }

  const stats = {
    classes: classes.length,
    reachableClasses: reachable.length,
    otherClasses: otherClasses.length,
    methods: classes.reduce(
      (n, c) => n + c.members.filter((m) => m.kind === 'method' || m.kind === 'constructor').length,
      0,
    ) + [...mergedInterfaces.values()].flat().reduce((n, d) => n + d.members.filter((m) => m.kind === 'method').length, 0),
    getters: classes.reduce((n, c) => n + c.members.filter((m) => m.kind === 'getter').length, 0),
    functions: functions.length,
    interfaces: interfaces.length,
    mergedInterfaces: [...mergedInterfaces.values()].flat().length,
    typeAliases: typeAliases.length,
    enums: enums.length,
  };
  return { markdown: out.join('\n').replace(/\n{3,}/g, '\n\n').trimEnd() + '\n', stats, warnings };
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

const invokedDirectly = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  const args = new Set(process.argv.slice(2));
  const source = readFileSync(DECLARATIONS_PATH, 'utf8');
  const { markdown, stats, warnings } = generateApiReference(source);
  for (const w of warnings) console.warn(`generate-api-reference: ${w}`);
  const summary = `${stats.reachableClasses + (stats.otherClasses ?? 0) + 1} classes, ${stats.methods} methods, ${stats.functions} functions, ${stats.interfaces} interfaces, ${stats.typeAliases} type aliases, ${stats.enums} enums`;
  if (args.has('--stdout')) {
    process.stdout.write(markdown);
  } else if (args.has('--check')) {
    let current = null;
    try {
      current = readFileSync(OUTPUT_PATH, 'utf8');
    } catch {
      current = null;
    }
    if (current !== markdown) {
      console.error(`generate-api-reference: ${path.relative(process.cwd(), OUTPUT_PATH)} is stale; run \`node scripts/generate-api-reference.mjs\``);
      process.exit(1);
    }
    console.log(`generate-api-reference: up to date (${summary})`);
  } else {
    writeFileSync(OUTPUT_PATH, markdown);
    console.log(`generate-api-reference: wrote ${path.relative(process.cwd(), OUTPUT_PATH)} (${summary})`);
  }
}
