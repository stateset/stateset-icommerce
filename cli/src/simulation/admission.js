import { isProxy } from 'node:util/types';
import { z } from 'zod';
import { SIMULATION_LIMITS as limits } from './limits.js';

class SimulationInputError extends Error {}

// Inspect data before Zod can expand arrays or accumulate validation issues.
// Only bounded primitive strings are serialized; no caller toJSON/getter runs.
// Optional undefined values count as null, conservatively retaining their cost.
export function checkSimulationInput(input) {
  let bytes = 0;
  let nodes = 0;
  let depth = 0;
  const ancestors = new Set();
  const refuse = (message) => {
    throw new SimulationInputError(`Simulation input ${message}`);
  };
  const addBytes = (count) => {
    bytes += count;
    if (bytes > limits.maxInputBytes) refuse('exceeds 128 KiB');
  };
  const stringBytes = (value) => {
    if (value.length > limits.maxInputBytes - bytes) refuse('exceeds 128 KiB');
    addBytes(Buffer.byteLength(JSON.stringify(value)));
  };
  const dataProperty = (value, key) => {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor) return undefined; // Sparse entries are still counted.
    if (!Object.hasOwn(descriptor, 'value')) refuse('must not contain accessor properties');
    if (!descriptor.enumerable) refuse('must contain only enumerable data properties');
    return descriptor.value;
  };
  const visit = (value, level) => {
    nodes += 1;
    if (nodes > limits.maxInputNodes) refuse(`exceeds ${limits.maxInputNodes} data nodes`);
    if (level > limits.maxInputDepth) refuse(`exceeds ${limits.maxInputDepth} nesting levels`);
    depth = Math.max(depth, level);
    if (value === null || value === undefined) {
      addBytes(4);
      return;
    }
    if (typeof value === 'string') {
      stringBytes(value);
      return;
    }
    if (typeof value === 'boolean') {
      addBytes(value ? 4 : 5);
      return;
    }
    if (typeof value === 'number') {
      if (!Number.isFinite(value)) refuse('must contain only finite numbers');
      addBytes(String(value).length);
      return;
    }
    if (typeof value !== 'object' || isProxy(value)) refuse('must contain only plain data');
    const array = Array.isArray(value);
    const prototype = Object.getPrototypeOf(value);
    if (
      array ? prototype !== Array.prototype : prototype !== Object.prototype && prototype !== null
    )
      refuse('must contain only plain objects and arrays');
    if (ancestors.has(value)) refuse('must not contain cycles');
    // Reject wide arrays before enumerating keys or reading any element.
    if (array && value.length > limits.maxInputNodes - nodes)
      refuse(`exceeds ${limits.maxInputNodes} data nodes`);
    const keys = Reflect.ownKeys(value);
    if (keys.length - (array ? 1 : 0) > limits.maxInputNodes - nodes)
      refuse(`exceeds ${limits.maxInputNodes} data nodes`);
    if (keys.some((key) => typeof key !== 'string')) refuse('must not contain symbol properties');
    ancestors.add(value);
    try {
      addBytes(2); // Container delimiters.
      if (array) {
        const maximumIndexLength = String(value.length).length;
        if (
          keys.some(
            (key) =>
              key !== 'length' &&
              (key.length > maximumIndexLength ||
                !/^(0|[1-9]\d*)$/.test(key) ||
                Number(key) >= value.length),
          )
        )
          refuse('arrays must not contain custom properties');
        for (let index = 0; index < value.length; index += 1) {
          if (index) addBytes(1);
          visit(dataProperty(value, String(index)), level + 1);
        }
      } else {
        for (const [index, key] of keys.entries()) {
          if (index) addBytes(1);
          stringBytes(key);
          addBytes(1);
          visit(dataProperty(value, key), level + 1);
        }
      }
    } finally {
      ancestors.delete(value);
    }
  };
  visit(input, 1);
  return { bytes, nodes, depth };
}

export function boundedSimulationSchema(schema) {
  return z.preprocess((value, context) => {
    try {
      checkSimulationInput(value);
      return value;
    } catch (error) {
      if (!(error instanceof SimulationInputError)) throw error;
      context.addIssue({ code: z.ZodIssueCode.custom, message: error.message, fatal: true });
      return z.NEVER;
    }
  }, schema);
}
