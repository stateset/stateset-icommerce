import { stripVTControlCharacters } from 'node:util';

const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

// Store values are untrusted terminal text, including OSC hyperlinks, clipboard
// escapes, carriage returns and bidi overrides. Only our renderer emits ANSI.
export function clean(value) {
  return stripVTControlCharacters(String(value ?? '')).replace(/[\p{Cc}\p{Bidi_Control}]/gu, ' ');
}

function cellWidth(text) {
  if (/^\p{Mark}+$/u.test(text)) return 0;
  const cp = text.codePointAt(0);
  return /\p{Extended_Pictographic}/u.test(text) ||
    (cp >= 0x1100 &&
      (cp <= 0x115f ||
        cp === 0x2329 ||
        cp === 0x232a ||
        (cp >= 0x2e80 && cp <= 0xa4cf) ||
        (cp >= 0xac00 && cp <= 0xd7a3) ||
        (cp >= 0xf900 && cp <= 0xfaff) ||
        (cp >= 0xfe10 && cp <= 0xfe6f) ||
        (cp >= 0xff01 && cp <= 0xff60) ||
        (cp >= 0xffe0 && cp <= 0xffe6) ||
        cp >= 0x20000))
    ? 2
    : 1;
}

export function width(value) {
  return [...segmenter.segment(clean(value))].reduce((n, { segment }) => n + cellWidth(segment), 0);
}

export function fit(value, size, pad = true) {
  size = Math.max(0, size);
  let out = '',
    used = 0;
  for (const { segment } of segmenter.segment(clean(value))) {
    const n = cellWidth(segment);
    if (used + n > size) break;
    out += segment;
    used += n;
  }
  return out + (pad ? ' '.repeat(size - used) : '');
}

export function wrap(value, size) {
  const result = [];
  let line = '',
    used = 0;
  for (const { segment } of segmenter.segment(clean(value))) {
    const n = cellWidth(segment);
    if (used + n > size && line) {
      result.push(line);
      line = '';
      used = 0;
    }
    if (n <= size) {
      line += segment;
      used += n;
    }
  }
  return [...result, line];
}
