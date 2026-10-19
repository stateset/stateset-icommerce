import { stripVTControlCharacters } from 'node:util';

const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

// Store values are untrusted terminal text, including OSC hyperlinks, clipboard
// escapes, carriage returns and bidi overrides. Only our renderer emits ANSI.
export function clean(value) {
  return stripVTControlCharacters(String(value ?? '')).replace(/[\p{Cc}\p{Bidi_Control}]/gu, ' ');
}

export const graphemes = (value) =>
  [...segmenter.segment(String(value ?? ''))].map((item) => item.segment);

function cellWidth(text) {
  if (/^\p{Mark}+$/u.test(text)) return 0;
  const cp = text.codePointAt(0);
  return /[\p{Extended_Pictographic}\p{Regional_Indicator}\u20e3\ufe0f]/u.test(text) ||
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
  size = Math.max(1, size);
  const parts = graphemes(clean(value));
  const result = [];
  let line = [],
    used = 0;
  for (const part of parts) {
    const n = cellWidth(part);
    let wrapped = false;
    if (used + n > size && line.length) {
      wrapped = true;
      // Prefer a whitespace boundary; split long identifiers and CJK text
      // only when no word boundary fits. Never divide an emoji/grapheme.
      let boundary = -1;
      for (let i = line.length - 1; i >= 0; i--)
        if (/\s/u.test(line[i])) {
          boundary = i;
          break;
        }
      if (boundary > 0) {
        result.push(line.slice(0, boundary).join(''));
        line = line.slice(boundary + 1);
        used = line.reduce((sum, s) => sum + cellWidth(s), 0);
      } else {
        result.push(line.join(''));
        line = [];
        used = 0;
      }
    }
    if (n <= size && !(wrapped && !line.length && /\s/u.test(part))) {
      line.push(part);
      used += n;
    }
  }
  return [...result, line.join('')];
}
