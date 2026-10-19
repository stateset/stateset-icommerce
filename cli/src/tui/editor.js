import { clean, graphemes, width } from './text.js';

// Cursor positions count complete graphemes, never UTF-16 code units. Editing
// returns data only; text is never interpreted as a command or a filename here.
export function editText(
  value,
  cursor,
  text,
  key = {},
  { limit = 120, accept = (s) => !/\p{Cc}/u.test(s) } = {},
) {
  const parts = graphemes(value);
  let position = Math.max(
    0,
    Math.min(parts.length, Number.isInteger(cursor) ? cursor : parts.length),
  );
  const previous = parts.join('');
  const name = key.name;
  if (name === 'left') position = Math.max(0, position - 1);
  else if (name === 'right') position = Math.min(parts.length, position + 1);
  else if (name === 'home' || (key.ctrl && name === 'a')) position = 0;
  else if (name === 'end' || (key.ctrl && name === 'e')) position = parts.length;
  else if (key.ctrl && name === 'u') {
    parts.length = 0;
    position = 0;
  } else if (key.ctrl && name === 'k') parts.splice(position);
  else if (name === 'backspace') {
    if (position) {
      parts.splice(position - 1, 1);
      position -= 1;
    }
  } else if (name === 'delete') parts.splice(position, 1);
  else if (text && !key.ctrl && !key.meta && accept(text)) {
    const inserted = graphemes(text).slice(0, Math.max(0, limit - parts.length));
    parts.splice(position, 0, ...inserted);
    position += inserted.length;
  } else return null;
  const result = parts.join('');
  const graphemePosition = Math.min(
    graphemes(result).length,
    graphemes(parts.slice(0, position).join('')).length,
  );
  return { text: result, cursor: graphemePosition, changed: result !== previous };
}

export function editorWindow(value, cursor, cells) {
  cells = Math.max(1, cells);
  if (cells === 1) return '▏';
  const parts = graphemes(clean(value)),
    sizes = parts.map((part) => width(part));
  const position = Math.max(
    0,
    Math.min(parts.length, Number.isInteger(cursor) ? cursor : parts.length),
  );
  const budget = position === parts.length ? cells - 1 : Math.floor((cells - 1) * 0.6);
  let start = position,
    used = 0;
  while (start > 0 && used + sizes[start - 1] + (start > 1 ? 1 : 0) <= budget) {
    used += sizes[--start];
  }
  let result = (start ? '…' : '') + parts.slice(start, position).join('') + '▏';
  used = (start ? 1 : 0) + used + 1;
  let end = position;
  while (end < parts.length && used + sizes[end] + (end + 1 < parts.length ? 1 : 0) <= cells) {
    used += sizes[end];
    result += parts[end++];
  }
  if (end < parts.length && used < cells) result += '…';
  return result;
}
