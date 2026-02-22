/**
 * Error recovery hints for common CLI failures.
 *
 * Maps known error patterns to actionable suggestions so users
 * can self-recover without reading docs.
 */

/** @type {Array<{ pattern: RegExp, hint: string }>} */
const ERROR_HINTS = [
  {
    pattern: /Failed to load @stateset\/embedded/i,
    hint: 'Run: npm install (from the project root)',
  },
  {
    pattern: /ANTHROPIC_API_KEY|api key.*required|authentication.*failed|401.*unauthorized/i,
    hint: 'Set your API key:\n  stateset-config set-key anthropic\n  Or: export ANTHROPIC_API_KEY="sk-ant-..."',
  },
  {
    pattern: /database directory does not exist|ENOENT.*\.db|no such file.*\.db/i,
    hint: 'Initialize the database:\n  stateset-init --demo\n  Or create the directory: mkdir -p <path>',
  },
  {
    pattern: /ECONNREFUSED|fetch failed|network error|ETIMEDOUT/i,
    hint: 'Check your internet connection.\n  Run: stateset-doctor --checks api',
  },
  {
    pattern: /rate limit|429|too many requests/i,
    hint: 'Wait a moment and retry. Use --budget to limit costs.',
  },
  {
    pattern: /overloaded|503|service unavailable/i,
    hint: 'The API is temporarily overloaded. Retry in a few seconds.',
  },
  {
    pattern: /permission denied|EACCES/i,
    hint: 'Check file permissions: ls -la <path>',
  },
  {
    pattern: /requires --apply|preview mode|write operations? (?:are )?blocked/i,
    hint: 'Add --apply to enable write operations:\n  stateset --apply "<your request>"',
  },
  {
    pattern: /invalid model|model.*not found/i,
    hint: 'Check available models with: stateset-config show\n  Common models: claude-sonnet-4-5, gpt-4o, gemini-2.0-flash',
  },
  {
    pattern: /budget exceeded|spending limit/i,
    hint: 'Increase your budget: --budget <amount>\n  Or use a cheaper model: --model claude-haiku-3-5',
  },
];

/**
 * Look up a recovery hint for the given error.
 *
 * @param {Error|string} error
 * @returns {string|null} hint text, or null if no match
 */
export function getErrorHint(error) {
  const message = error instanceof Error ? error.message : String(error);
  for (const { pattern, hint } of ERROR_HINTS) {
    if (pattern.test(message)) return hint;
  }
  return null;
}
