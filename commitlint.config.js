// @ts-check
/** @type {import('@commitlint/types').UserConfig} */
module.exports = {
  extends: ['@commitlint/config-conventional'],
  rules: {
    'type-enum': [
      2,
      'always',
      [
        'feat',     // New feature
        'fix',      // Bug fix
        'docs',     // Documentation
        'style',    // Formatting (no code change)
        'refactor', // Code restructuring
        'perf',     // Performance improvement
        'test',     // Adding/updating tests
        'chore',    // Maintenance
        'ci',       // CI/CD changes
        'build',    // Build system changes
        'revert',   // Revert a commit
      ],
    ],
    'scope-enum': [1, 'always', ['cli', 'admin', 'core', 'ci', 'docs', 'deps']],
    'subject-case': [2, 'never', ['upper-case']],
    'subject-empty': [2, 'never'],
    'type-empty': [2, 'never'],
  },
};
