'use client';

import { useEffect, useState } from 'react';

/**
 * usePrefersReducedMotion — true when the user has requested reduced motion.
 * Every motion primitive must honor it (skip/short-circuit animations).
 *
 * @returns {boolean}
 */
export function usePrefersReducedMotion() {
  const [reduced, setReduced] = useState(false);

  useEffect(() => {
    if (typeof window === 'undefined' || !window.matchMedia) return undefined;
    const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
    const update = () => setReduced(mq.matches);
    update();
    mq.addEventListener('change', update);
    return () => mq.removeEventListener('change', update);
  }, []);

  return reduced;
}
