/**
 * Interactive UI Module for StateSet CLI
 *
 * Wraps @clack/prompts for beautiful terminal interactions —
 * spinners, confirmations, selects, text inputs, password prompts.
 * Degrades gracefully in non-TTY environments.
 */

import * as p from '@clack/prompts';
import { theme } from './theme.js';

// ============================================================================
// Spinners
// ============================================================================

/**
 * Run an async function with an animated spinner.
 *
 * @param {string} label   - Text shown next to the spinner
 * @param {() => Promise<T>} fn  - Async work to perform
 * @returns {Promise<T>}
 */
export async function withSpinner(label, fn) {
  if (!process.stderr.isTTY) {
    // Non-TTY: log start/end, no animation
    console.error(theme.muted(`  ${label}...`));
    try {
      const result = await fn();
      console.error(theme.muted(`  ${label} done`));
      return result;
    } catch (err) {
      console.error(theme.error(`  ${label} failed`));
      throw err;
    }
  }

  const s = p.spinner();
  s.start(label);
  try {
    const result = await fn();
    s.stop(theme.success(`${label} — done`));
    return result;
  } catch (err) {
    s.stop(theme.error(`${label} — failed`));
    throw err;
  }
}

// ============================================================================
// Confirmations
// ============================================================================

/**
 * Ask a yes/no confirmation question.
 *
 * @param {string} message
 * @param {{ assumeYes?: boolean, defaultValue?: boolean }} [options]
 * @returns {Promise<boolean>}
 */
export async function confirm(message, options = {}) {
  if (options.assumeYes) return true;

  if (!process.stdin.isTTY) {
    return options.defaultValue ?? false;
  }

  const result = await p.confirm({ message, initialValue: options.defaultValue ?? false });
  if (p.isCancel(result)) {
    p.cancel('Operation cancelled.');
    process.exit(0);
  }
  return result;
}

// ============================================================================
// Selection
// ============================================================================

/**
 * Present a list of options and return the selected value.
 *
 * @param {string} message
 * @param {{ value: string, label: string, hint?: string }[]} options
 * @returns {Promise<string>}
 */
export async function select(message, options) {
  if (!process.stdin.isTTY) {
    // Non-TTY: return first option
    return options[0]?.value;
  }

  const result = await p.select({ message, options });
  if (p.isCancel(result)) {
    p.cancel('Operation cancelled.');
    process.exit(0);
  }
  return result;
}

// ============================================================================
// Text Input
// ============================================================================

/**
 * Prompt for text input.
 *
 * @param {string} message
 * @param {{ placeholder?: string, defaultValue?: string, validate?: (v: string) => string|void }} [options]
 * @returns {Promise<string>}
 */
export async function text(message, options = {}) {
  if (!process.stdin.isTTY) {
    return options.defaultValue ?? '';
  }

  const result = await p.text({
    message,
    placeholder: options.placeholder,
    defaultValue: options.defaultValue,
    validate: options.validate,
  });
  if (p.isCancel(result)) {
    p.cancel('Operation cancelled.');
    process.exit(0);
  }
  return result;
}

/**
 * Prompt for a password (masked input).
 *
 * @param {string} message
 * @param {{ validate?: (v: string) => string|void }} [options]
 * @returns {Promise<string>}
 */
export async function password(message, options = {}) {
  if (!process.stdin.isTTY) {
    return '';
  }

  const result = await p.password({
    message,
    validate: options.validate,
  });
  if (p.isCancel(result)) {
    p.cancel('Operation cancelled.');
    process.exit(0);
  }
  return result;
}

// ============================================================================
// Banners & Notes
// ============================================================================

/**
 * Display an intro banner.
 *
 * @param {string} title
 */
export function intro(title) {
  p.intro(theme.heading(title));
}

/**
 * Display an outro/closing message.
 *
 * @param {string} message
 */
export function outro(message) {
  p.outro(theme.success(message));
}

/**
 * Display a boxed note.
 *
 * @param {string} message
 * @param {string} [title]
 */
export function note(message, title) {
  p.note(message, title);
}

// ============================================================================
// Task Groups
// ============================================================================

/**
 * Run a series of tasks with animated spinners.
 *
 * @param {{ title: string, task: (message: (msg: string) => void) => Promise<string|void> }[]} taskList
 * @returns {Promise<void>}
 */
export async function tasks(taskList) {
  if (!process.stderr.isTTY) {
    // Non-TTY: run sequentially with log output
    for (const t of taskList) {
      console.error(theme.muted(`  ${t.title}...`));
      try {
        const result = await t.task(() => {});
        console.error(theme.success(`  ${t.title} — ${result || 'done'}`));
      } catch (err) {
        console.error(theme.error(`  ${t.title} — failed: ${err.message}`));
        throw err;
      }
    }
    return;
  }

  await p.tasks(taskList);
}

// ============================================================================
// Log helpers (themed console output)
// ============================================================================

/**
 * Log a styled message to stderr (won't pollute stdout piping).
 */
export function log(message) {
  p.log.message(message);
}

export function logSuccess(message) {
  p.log.success(message);
}

export function logError(message) {
  p.log.error(message);
}

export function logWarning(message) {
  p.log.warning(message);
}

export function logInfo(message) {
  p.log.info(message);
}
