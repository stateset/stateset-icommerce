import { z } from 'zod';

/** Monetary values must arrive as strings before JavaScript can round them. */
export const decimalString = z.string().regex(/^-?\d+(\.\d+)?$/, 'Use an exact decimal string');
export const nonnegativeDecimalString = decimalString.refine(
  (value) => !value.startsWith('-'),
  'Must be non-negative',
);
export const positiveDecimalString = nonnegativeDecimalString.refine(
  (value) => /[1-9]/.test(value),
  'Must be positive',
);
