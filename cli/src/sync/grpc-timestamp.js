// Protobuf timestamps preserve an instant, not the original signed RFC3339 text.
// Keep the fraction separate from Date: Date truncates beyond milliseconds.
export function toProtoTimestamp(value) {
  let seconds;
  let nanos;
  if (typeof value === 'string') {
    const match = /^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.(\d{1,9}))?(Z|[+-]\d{2}:\d{2})$/.exec(
      value,
    );
    if (!match)
      throw new TypeError(
        'Invalid event timestamp: expected RFC3339 with at most 9 fractional digits',
      );
    const [, local, fraction = '', offset] = match;
    const localMs = Date.parse(`${local}Z`);
    // Date.parse normalizes impossible days such as February 30; reject them.
    if (!Number.isFinite(localMs) || new Date(localMs).toISOString().slice(0, 19) !== local) {
      throw new TypeError('Invalid event timestamp calendar value');
    }
    let offsetSeconds = 0;
    if (offset !== 'Z') {
      const hours = Number(offset.slice(1, 3));
      const minutes = Number(offset.slice(4, 6));
      if (hours > 23 || minutes > 59) throw new TypeError('Invalid event timestamp offset');
      offsetSeconds = (hours * 3600 + minutes * 60) * (offset[0] === '+' ? 1 : -1);
    }
    seconds = localMs / 1000 - offsetSeconds;
    nanos = Number(fraction.padEnd(9, '0'));
  } else {
    const ms = value instanceof Date ? value.getTime() : value;
    if (!Number.isSafeInteger(ms)) throw new TypeError('Invalid event timestamp milliseconds');
    seconds = Math.floor(ms / 1000);
    nanos = (ms - seconds * 1000) * 1_000_000;
  }
  if (seconds < -62135596800 || seconds > 253402300799) {
    throw new RangeError('Event timestamp must be within years 0001 through 9999');
  }
  return { seconds, nanos };
}
