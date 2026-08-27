'use client';

import React, { useEffect, useRef, useState } from 'react';

import { DURATION, EASING } from './motion-tokens.js';
import { usePrefersReducedMotion } from './use-reduced-motion.js';

/**
 * Reveal — the canonical scroll-into-view entrance. Fades + lifts its children
 * the first time they enter the viewport. SSR-safe (children always render),
 * honors reduced-motion (shows immediately), and replaces the ad-hoc
 * IntersectionObserver + CSS that surfaces reimplement.
 *
 * @param {Object} props
 * @param {React.ElementType} [props.as]      Element/component to render (default 'div').
 * @param {number} [props.delay]              Stagger delay in ms.
 * @param {number} [props.y]                  Initial translateY offset in px (default 24).
 * @param {boolean} [props.once]              Reveal only once (default true).
 * @param {string} [props.className]
 * @param {React.ReactNode} [props.children]
 */
export function Reveal({
  as: Tag = 'div',
  delay = 0,
  y = 24,
  once = true,
  className = '',
  children,
  ...props
}) {
  const ref = useRef(null);
  const reduced = usePrefersReducedMotion();
  const [inView, setInView] = useState(false);

  useEffect(() => {
    if (reduced || typeof IntersectionObserver === 'undefined') {
      setInView(true);
      return undefined;
    }
    const el = ref.current;
    if (!el) return undefined;
    const io = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setInView(true);
          if (once) io.disconnect();
        } else if (!once) {
          setInView(false);
        }
      },
      { threshold: 0.15, rootMargin: '0px 0px -8% 0px' },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [reduced, once]);

  const style = reduced
    ? undefined
    : {
        opacity: inView ? 1 : 0,
        transform: inView ? 'none' : `translateY(${y}px)`,
        transition: `opacity ${DURATION.slow}ms ${EASING.standard} ${delay}ms, transform ${DURATION.slow}ms ${EASING.standard} ${delay}ms`,
        willChange: 'opacity, transform',
      };

  return (
    <Tag ref={ref} className={className} style={style} {...props}>
      {children}
    </Tag>
  );
}

export default Reveal;
