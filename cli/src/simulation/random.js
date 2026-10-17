import { createHash } from 'node:crypto';

// Keys define independent, versioned streams. Keep the original demand key
// unchanged so adding supply uncertainty cannot perturb existing demand paths.
export function createSeededSampler(key) {
  let state = createHash('sha256').update(key).digest().readUInt32LE(0);
  return (length) => {
    const limit = 0x100000000 - (0x100000000 % length);
    let sample;
    do {
      state = (state + 0x6d2b79f5) >>> 0;
      let value = Math.imul(state ^ (state >>> 15), state | 1);
      value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
      sample = (value ^ (value >>> 14)) >>> 0;
    } while (sample >= limit);
    return sample % length;
  };
}
