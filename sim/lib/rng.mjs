/**
 * Seeded randomness for the swarm.
 *
 * Every random choice the harness makes comes from a stream derived from
 * `(seed, label)`, so a choice is a pure function of the seed and of *what* is
 * being decided, never of how many other choices were made first. Actor plans
 * use one stream per actor (`actor:<i>`), schedulers one per process, so the
 * same seed yields the same plans whether the actors run in one process or are
 * split across eight.
 */
import { createHash } from 'node:crypto';

function seedWords(seed, label) {
  const digest = createHash('sha256').update(`${seed}\u0000${label}`).digest();
  return [0, 4, 8, 12].map((offset) => digest.readUInt32LE(offset));
}

/** sfc32: 128-bit state, passes PractRand to 2^40+, trivially portable. */
export function createRng(seed, label = '') {
  let [a, b, c, d] = seedWords(seed, label);
  const next = () => {
    a >>>= 0;
    b >>>= 0;
    c >>>= 0;
    d >>>= 0;
    let t = (a + b) | 0;
    a = b ^ (b >>> 9);
    b = (c + (c << 3)) | 0;
    c = (c << 21) | (c >>> 11);
    d = (d + 1) | 0;
    t = (t + d) | 0;
    c = (c + t) | 0;
    return (t >>> 0) / 4294967296;
  };
  for (let i = 0; i < 16; i += 1) next();
  return {
    next,
    /** Integer in [min, max], both inclusive. */
    int(min, max) {
      return min + Math.floor(next() * (max - min + 1));
    },
    chance(probability) {
      return next() < probability;
    },
    pick(items) {
      return items[Math.floor(next() * items.length)];
    },
  };
}

/**
 * A UUID v4-shaped identifier that is a pure function of its parts. Unique by
 * construction for distinct parts, so seeded command ids never collide across
 * actors or processes (the kernel's `kernel_receipts.command_id` is a primary
 * key).
 */
export function seededUuid(...parts) {
  const bytes = createHash('sha256').update(parts.join('\u0000')).digest().subarray(0, 16);
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = bytes.toString('hex');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
