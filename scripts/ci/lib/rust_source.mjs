// Lightweight, dependency-free source scanning for the binding parity report.
//
// These helpers do not parse Rust or Go. They lean on two facts that hold for
// every file they read: the code is rustfmt/gofmt formatted (item blocks open
// at column 0 and close with a lone `}` at column 0), and comments and string
// literals are blanked out first, so a doc-comment example such as
// `commerce.orders().create(...)` is never mistaken for a real call.

/**
 * Blank out comments and string/char literal contents, preserving every
 * newline and character offset so line numbers stay valid.
 *
 * @param {string} text Rust source
 * @returns {string}
 */
export function maskRust(text) {
  const out = text.split('');
  const n = text.length;
  const blank = (from, to) => {
    for (let k = from; k < to; k += 1) {
      if (out[k] !== '\n') out[k] = ' ';
    }
  };
  let i = 0;
  while (i < n) {
    const c = text[i];
    const next = text[i + 1];
    if (c === '/' && next === '/') {
      let j = i;
      while (j < n && text[j] !== '\n') j += 1;
      blank(i, j);
      i = j;
      continue;
    }
    if (c === '/' && next === '*') {
      let depth = 1;
      let j = i + 2;
      while (j < n && depth > 0) {
        if (text[j] === '/' && text[j + 1] === '*') {
          depth += 1;
          j += 2;
        } else if (text[j] === '*' && text[j + 1] === '/') {
          depth -= 1;
          j += 2;
        } else {
          j += 1;
        }
      }
      blank(i, j);
      i = j;
      continue;
    }
    // Raw strings: r"..", r#".."#, br#".."#
    const rawMatch = /^b?r(#*)"/.exec(text.slice(i, i + 12));
    if (rawMatch && !/[A-Za-z0-9_]/.test(text[i - 1] ?? '')) {
      const hashes = rawMatch[1];
      const start = i + rawMatch[0].length;
      const terminator = `"${hashes}`;
      const end = text.indexOf(terminator, start);
      const stop = end < 0 ? n : end;
      blank(start, stop);
      i = stop + terminator.length;
      continue;
    }
    if (c === '"') {
      let j = i + 1;
      while (j < n && text[j] !== '"') {
        j += text[j] === '\\' ? 2 : 1;
      }
      blank(i + 1, j);
      i = j + 1;
      continue;
    }
    if (c === "'") {
      // Char literal ('x', '\n', '\u{1F600}') versus lifetime ('a).
      if (next === '\\') {
        const end = text.indexOf("'", i + 2);
        if (end > 0) {
          blank(i + 1, end);
          i = end + 1;
          continue;
        }
      } else {
        const cp = text.codePointAt(i + 1);
        const width = cp !== undefined && cp > 0xffff ? 2 : 1;
        if (text[i + 1 + width] === "'") {
          blank(i + 1, i + 1 + width);
          i = i + 2 + width;
          continue;
        }
      }
    }
    i += 1;
  }
  return out.join('');
}

/**
 * Blank out Go comments and string literal contents (interpreted and raw).
 *
 * @param {string} text Go source
 * @returns {string}
 */
export function maskGo(text) {
  const out = text.split('');
  const n = text.length;
  const blank = (from, to) => {
    for (let k = from; k < to; k += 1) {
      if (out[k] !== '\n') out[k] = ' ';
    }
  };
  let i = 0;
  while (i < n) {
    const c = text[i];
    const next = text[i + 1];
    if (c === '/' && next === '/') {
      let j = i;
      while (j < n && text[j] !== '\n') j += 1;
      blank(i, j);
      i = j;
    } else if (c === '/' && next === '*') {
      const end = text.indexOf('*/', i + 2);
      const stop = end < 0 ? n : end + 2;
      blank(i, stop);
      i = stop;
    } else if (c === '`') {
      const end = text.indexOf('`', i + 1);
      const stop = end < 0 ? n : end;
      blank(i + 1, stop);
      i = stop + 1;
    } else if (c === '"') {
      let j = i + 1;
      while (j < n && text[j] !== '"' && text[j] !== '\n') {
        j += text[j] === '\\' ? 2 : 1;
      }
      blank(i + 1, j);
      i = j + 1;
    } else {
      i += 1;
    }
  }
  return out.join('');
}

/** @param {string} text @param {number} index */
export function lineAt(text, index) {
  let line = 1;
  for (let k = 0; k < index && k < text.length; k += 1) {
    if (text[k] === '\n') line += 1;
  }
  return line;
}

/**
 * Given the index of an opening paren, return the index just past its match.
 *
 * @param {string} text
 * @param {number} open
 */
export function skipBalanced(text, open) {
  const pairs = { '(': ')', '[': ']', '{': '}' };
  const closer = pairs[text[open]];
  let depth = 0;
  for (let k = open; k < text.length; k += 1) {
    if (text[k] === text[open]) depth += 1;
    else if (text[k] === closer) {
      depth -= 1;
      if (depth === 0) return k + 1;
    }
  }
  return text.length;
}

/**
 * Top-level `impl Type { ... }` blocks (inherent impls only; `impl Trait for
 * Type` is skipped). Attributes directly above the `impl` line are returned so
 * callers can tell `#[napi]`/`#[pymethods]`/`#[wasm_bindgen]` blocks apart.
 *
 * @param {string} original unmasked source
 * @param {string} masked maskRust(original)
 */
export function parseImplBlocks(original, masked) {
  const lines = masked.split('\n');
  const originalLines = original.split('\n');
  const blocks = [];
  for (let index = 0; index < lines.length; index += 1) {
    const match = /^impl(?:<[^>]*>)?\s+([A-Za-z_][\w:]*)(?:<[^{]*>)?\s*\{\s*$/.exec(lines[index]);
    if (!match || /\sfor\s/.test(lines[index])) continue;
    let end = index + 1;
    while (end < lines.length && lines[end] !== '}') end += 1;
    const attributes = [];
    for (let k = index - 1; k >= 0 && /^#\[/.test(originalLines[k]); k -= 1) {
      attributes.unshift(originalLines[k].trim());
    }
    blocks.push({
      type: match[1].split('::').pop(),
      attributes,
      startLine: index + 1,
      endLine: end + 1,
      bodyLines: lines.slice(index + 1, end),
      originalBodyLines: originalLines.slice(index + 1, end),
    });
  }
  return blocks;
}

const METHOD_RE =
  /^    (pub(?:\([^)]*\))?\s+)?(?:(?:async|const|unsafe)\s+)*(?:extern\s+"[^"]*"\s+)?fn\s+([A-Za-z_]\w*)/;

/**
 * Split an impl block into its methods. Each method carries its attributes,
 * its signature (masked text up to the opening brace) and its body text.
 *
 * @param {{ bodyLines: string[], originalBodyLines: string[], startLine: number }} block
 */
export function splitMethods(block) {
  const methods = [];
  const { bodyLines, originalBodyLines } = block;
  const starts = [];
  for (let k = 0; k < bodyLines.length; k += 1) {
    // Detect on the original line: masking blanks the "C" in extern "C".
    const match = METHOD_RE.exec(originalBodyLines[k]);
    if (match) starts.push({ k, visibility: match[1]?.trim() ?? '', name: match[2] });
  }
  starts.forEach((start, position) => {
    const stop = position + 1 < starts.length ? starts[position + 1].k : bodyLines.length;
    // Everything between the previous item and this `fn` that is not a
    // comment is attribute text (attributes may span several lines).
    const attributes = [];
    for (let a = start.k - 1; a >= 0; a -= 1) {
      const line = originalBodyLines[a].trim();
      if (line.startsWith('//')) continue;
      if (line === '' || /[;{}]$/.test(line)) break;
      attributes.unshift(line);
    }
    const text = bodyLines.slice(start.k, stop).join('\n');
    const brace = text.indexOf('{');
    methods.push({
      name: start.name,
      visibility: start.visibility,
      attributes,
      line: block.startLine + 1 + start.k,
      signature: brace < 0 ? text : text.slice(0, brace),
      body: brace < 0 ? '' : text.slice(brace),
    });
  });
  return methods;
}

/**
 * Top-level free functions (`fn`, `pub fn`, `pub extern "C" fn`, ...), each
 * with its body. Used to follow helper calls and FFI exports.
 *
 * @param {string} original
 * @param {string} masked
 */
export function parseFreeFunctions(original, masked) {
  const lines = masked.split('\n');
  const originalLines = original.split('\n');
  const functions = [];
  for (let index = 0; index < lines.length; index += 1) {
    const match =
      /^(?:pub(?:\([^)]*\))?\s+)?(?:(?:async|const|unsafe)\s+)*(?:extern\s+"[^"]*"\s+)?fn\s+([A-Za-z_]\w*)/.exec(
        originalLines[index],
      );
    if (!match) continue;
    let end = index;
    while (end < lines.length && lines[end] !== '}') end += 1;
    const text = lines.slice(index, end + 1).join('\n');
    const brace = text.indexOf('{');
    functions.push({
      name: match[1],
      line: index + 1,
      signature: brace < 0 ? text : text.slice(0, brace),
      body: brace < 0 ? '' : text.slice(brace),
    });
    index = end;
  }
  return functions;
}

/** `get_by_code` -> `getByCode` */
export function snakeToCamel(name) {
  return name.replace(/_([a-z0-9])/g, (_, ch) => ch.toUpperCase());
}

/** Convention-free key: `get_by_code`, `getByCode`, `GetByCode` -> `getbycode`. */
export function normalizeName(name) {
  return name.replace(/_/g, '').toLowerCase();
}
