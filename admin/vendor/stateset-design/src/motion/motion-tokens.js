/**
 * Motion tokens — the single source of truth for animation timing across the
 * platform. The app uses framer-motion in ~86 files today with ad-hoc durations
 * and easings; converge them on these so motion feels like one system.
 *
 * Durations are in milliseconds. EASING values are CSS cubic-beziers; TRANSITION
 * presets are framer-motion-shaped (seconds + bezier array) for `transition={}`.
 */

export const DURATION = {
  fast: 180,
  base: 320,
  slow: 560,
  page: 800,
};

export const EASING = {
  /** General-purpose ease-out — the default. */
  standard: 'cubic-bezier(0.2, 0.7, 0.2, 1)',
  /** Decisive entrances. */
  emphasized: 'cubic-bezier(0.2, 0, 0, 1)',
  /** Exits / dismissals. */
  exit: 'cubic-bezier(0.4, 0, 1, 1)',
};

// Typed as 4-tuples so framer-motion's `ease` (a cubic-bezier definition)
// accepts them in .tsx consumers — a plain number[] would not satisfy its types.
/** @type {[number, number, number, number]} */
const STANDARD_BEZIER = [0.2, 0.7, 0.2, 1];
/** @type {[number, number, number, number]} */
const EMPHASIZED_BEZIER = [0.2, 0, 0, 1];

/** framer-motion `transition` presets (seconds + bezier tuple). */
export const TRANSITION = {
  fast: { duration: DURATION.fast / 1000, ease: STANDARD_BEZIER },
  standard: { duration: DURATION.base / 1000, ease: STANDARD_BEZIER },
  slow: { duration: DURATION.slow / 1000, ease: STANDARD_BEZIER },
  emphasized: { duration: DURATION.base / 1000, ease: EMPHASIZED_BEZIER },
};
