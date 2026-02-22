/**
 * StateSet CLI Theme System
 *
 * Provides a branded color palette and theme helpers that respect
 * NO_COLOR, FORCE_COLOR, and TTY detection.
 */

// ============================================================================
// Branded Palette — StateSet Blue
// ============================================================================

/** ANSI 24-bit color codes keyed by semantic name. */
export const PALETTE = {
  // Brand
  accent: '\x1b[38;2;75;120;255m', // #4B78FF — StateSet blue
  accentBright: '\x1b[38;2;120;160;255m', // #78A0FF
  accentDim: '\x1b[38;2;55;90;200m', // #375AC8

  // Semantic
  success: '\x1b[38;2;47;191;113m', // #2FBF71
  warn: '\x1b[38;2;255;176;32m', // #FFB020
  error: '\x1b[38;2;226;61;45m', // #E23D2D
  info: '\x1b[38;2;100;180;255m', // #64B4FF

  // Neutral
  muted: '\x1b[90m', // Gray (standard dim)
  white: '\x1b[37m',

  // Formatting
  bold: '\x1b[1m',
  dim: '\x1b[2m',
  italic: '\x1b[3m',
  underline: '\x1b[4m',
  reset: '\x1b[0m',
};

// ============================================================================
// Theme Factory
// ============================================================================

/**
 * Create a theme object with color functions.
 *
 * Each property is a function `(text: string) => string` that wraps text
 * in ANSI color codes when color is enabled, or returns plain text when not.
 *
 * @param {{ color?: boolean }} [options]
 * @returns {Theme}
 */
export function createTheme(options = {}) {
  const enabled =
    options.color !== undefined
      ? options.color
      : !process.env.NO_COLOR &&
        (Boolean(process.env.FORCE_COLOR) || Boolean(process.stdout.isTTY));

  const wrap = (code) => (text) => (enabled ? `${code}${text}${PALETTE.reset}` : String(text));

  return {
    // Brand
    accent: wrap(PALETTE.accent),
    accentBright: wrap(PALETTE.accentBright),
    accentDim: wrap(PALETTE.accentDim),

    // Semantic
    success: wrap(PALETTE.success),
    warn: wrap(PALETTE.warn),
    error: wrap(PALETTE.error),
    info: wrap(PALETTE.info),

    // Neutral
    muted: wrap(PALETTE.muted),

    // Formatting
    bold: wrap(PALETTE.bold),
    dim: wrap(PALETTE.dim),
    italic: wrap(PALETTE.italic),
    underline: wrap(PALETTE.underline),

    // Compound helpers
    heading: (text) =>
      enabled ? `${PALETTE.bold}${PALETTE.accent}${text}${PALETTE.reset}` : String(text),
    command: wrap(PALETTE.accentBright),
    option: wrap(PALETTE.warn),
    label: (text) =>
      enabled ? `${PALETTE.bold}${PALETTE.white}${text}${PALETTE.reset}` : String(text),

    // Introspection
    isRich: () => enabled,
  };
}

// ============================================================================
// Default singleton (auto-detects environment)
// ============================================================================

export const theme = createTheme();
