/**
 * Unit tests for skills/loader.js
 *
 * Tests skill discovery from the filesystem: discoverFromDirectory,
 * discoverSkills, getDefaultPaths, and SKILL_ORIGINS.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs';
import path from 'path';
import os from 'os';
import { randomUUID } from 'crypto';

import {
  SKILL_ORIGINS,
  getDefaultPaths,
  discoverFromDirectory,
  discoverSkills,
} from '../src/skills/loader.js';

// ===========================================================================
// Helpers
// ===========================================================================

/**
 * Create a temporary directory with a random name for test isolation.
 * @returns {string} Absolute path to the new directory.
 */
function makeTmpDir() {
  const dir = path.join(os.tmpdir(), `stateset-loader-test-${randomUUID()}`);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

/**
 * Create a complete, valid SKILL.md inside `skillsRoot/<skillName>/`.
 *
 * @param {string} skillsRoot - The root skills directory.
 * @param {string} skillName  - Sub-directory name.
 * @param {Object} [opts]
 * @param {string} [opts.name]        - Frontmatter `name` (defaults to skillName).
 * @param {string} [opts.description] - Frontmatter `description`.
 * @param {string} [opts.body]        - Markdown body.
 * @param {boolean} [opts.references] - Whether to create a `references/` dir.
 * @param {boolean} [opts.scripts]    - Whether to create a `scripts/` dir.
 * @returns {string} Path to the skill directory.
 */
function createSkillDir(skillsRoot, skillName, opts = {}) {
  const {
    name = skillName,
    description = `Description for ${skillName}`,
    body = `# ${skillName}\n\n## Overview\nBody text.`,
    references = false,
    scripts = false,
  } = opts;

  const skillDir = path.join(skillsRoot, skillName);
  fs.mkdirSync(skillDir, { recursive: true });

  const content = `---\nname: ${name}\ndescription: ${description}\n---\n${body}\n`;
  fs.writeFileSync(path.join(skillDir, 'SKILL.md'), content, 'utf-8');

  if (references) fs.mkdirSync(path.join(skillDir, 'references'), { recursive: true });
  if (scripts) fs.mkdirSync(path.join(skillDir, 'scripts'), { recursive: true });

  return skillDir;
}

/** Remove a directory tree, ignoring errors (e.g. already deleted). */
function removeTmpDir(dir) {
  try {
    fs.rmSync(dir, { recursive: true, force: true });
  } catch {
    // ignore
  }
}

// ===========================================================================
// SKILL_ORIGINS
// ===========================================================================

describe('SKILL_ORIGINS', () => {
  it('exports bundled, installed, and workspace constants', () => {
    assert.strictEqual(SKILL_ORIGINS.BUNDLED, 'bundled');
    assert.strictEqual(SKILL_ORIGINS.INSTALLED, 'installed');
    assert.strictEqual(SKILL_ORIGINS.WORKSPACE, 'workspace');
  });

  it('has exactly three entries', () => {
    assert.strictEqual(Object.keys(SKILL_ORIGINS).length, 3);
  });
});

// ===========================================================================
// getDefaultPaths
// ===========================================================================

describe('getDefaultPaths', () => {
  it('returns an object with bundled, installed, and workspace keys', () => {
    const paths = getDefaultPaths();
    assert.ok(typeof paths.bundled === 'string');
    assert.ok(typeof paths.installed === 'string');
    assert.ok(typeof paths.workspace === 'string');
  });

  it('installed path is inside the user home directory', () => {
    const paths = getDefaultPaths();
    assert.ok(paths.installed.startsWith(os.homedir()));
    assert.ok(paths.installed.includes('.stateset'));
  });

  it('bundled path contains "skills"', () => {
    const paths = getDefaultPaths();
    assert.ok(paths.bundled.endsWith('skills') || paths.bundled.includes('skills'));
  });

  it('workspace path ends with .stateset/skills', () => {
    const paths = getDefaultPaths();
    assert.ok(paths.workspace.endsWith(path.join('.stateset', 'skills')));
  });

  it('returns a fresh object each call', () => {
    const a = getDefaultPaths();
    const b = getDefaultPaths();
    assert.notStrictEqual(a, b);
    assert.deepEqual(a, b);
  });
});

// ===========================================================================
// discoverFromDirectory — basic happy-path
// ===========================================================================

describe('discoverFromDirectory — basic', () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => removeTmpDir(tmpDir));

  it('returns empty array for non-existent directory', () => {
    const result = discoverFromDirectory('/tmp/does-not-exist-' + randomUUID(), 'bundled');
    assert.deepEqual(result, []);
  });

  it('returns empty array for an empty directory', () => {
    const result = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.deepEqual(result, []);
  });

  it('discovers a single valid skill', () => {
    createSkillDir(tmpDir, 'commerce-orders');
    const result = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(result.length, 1);
    assert.strictEqual(result[0].name, 'commerce-orders');
  });

  it('returned skill has correct shape', () => {
    createSkillDir(tmpDir, 'commerce-orders');
    const [skill] = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);

    assert.strictEqual(skill.name, 'commerce-orders');
    assert.strictEqual(skill.origin, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(skill.dirPath, path.join(tmpDir, 'commerce-orders'));
    assert.strictEqual(skill.skillMdPath, path.join(tmpDir, 'commerce-orders', 'SKILL.md'));
    assert.ok(skill.parsed);
    assert.strictEqual(typeof skill.hasReferences, 'boolean');
    assert.strictEqual(typeof skill.hasScripts, 'boolean');
  });

  it('correctly labels origin as workspace', () => {
    createSkillDir(tmpDir, 'my-skill');
    const [skill] = discoverFromDirectory(tmpDir, SKILL_ORIGINS.WORKSPACE);
    assert.strictEqual(skill.origin, SKILL_ORIGINS.WORKSPACE);
  });

  it('discovers multiple skills in the same directory', () => {
    createSkillDir(tmpDir, 'commerce-orders');
    createSkillDir(tmpDir, 'commerce-checkout');
    createSkillDir(tmpDir, 'commerce-inventory');
    const result = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(result.length, 3);
  });

  it('ignores plain files at the root level', () => {
    fs.writeFileSync(path.join(tmpDir, 'README.md'), '# ignore me', 'utf-8');
    createSkillDir(tmpDir, 'commerce-orders');
    const result = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(result.length, 1);
  });

  it('ignores sub-directories that lack SKILL.md', () => {
    fs.mkdirSync(path.join(tmpDir, 'no-skill-here'));
    createSkillDir(tmpDir, 'commerce-orders');
    const result = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(result.length, 1);
  });

  it('ignores sub-directories with an invalid SKILL.md (missing name)', () => {
    const badDir = path.join(tmpDir, 'bad-skill');
    fs.mkdirSync(badDir);
    // Valid YAML frontmatter but missing `name`
    fs.writeFileSync(
      path.join(badDir, 'SKILL.md'),
      '---\ndescription: No name field\n---\n# Body\n',
      'utf-8',
    );
    const result = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(result.length, 0);
  });

  it('ignores sub-directories with a malformed SKILL.md (no frontmatter)', () => {
    const badDir = path.join(tmpDir, 'bad-frontmatter');
    fs.mkdirSync(badDir);
    fs.writeFileSync(path.join(badDir, 'SKILL.md'), '# No frontmatter here\nJust text.\n', 'utf-8');
    const result = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(result.length, 0);
  });

  it('ignores sub-directories with a completely empty SKILL.md', () => {
    const emptyDir = path.join(tmpDir, 'empty-skill');
    fs.mkdirSync(emptyDir);
    fs.writeFileSync(path.join(emptyDir, 'SKILL.md'), '', 'utf-8');
    const result = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(result.length, 0);
  });
});

// ===========================================================================
// discoverFromDirectory — hasReferences / hasScripts flags
// ===========================================================================

describe('discoverFromDirectory — optional sub-directories', () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => removeTmpDir(tmpDir));

  it('hasReferences is false when references/ does not exist', () => {
    createSkillDir(tmpDir, 'commerce-orders', { references: false });
    const [skill] = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(skill.hasReferences, false);
  });

  it('hasReferences is true when references/ exists', () => {
    createSkillDir(tmpDir, 'commerce-orders', { references: true });
    const [skill] = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(skill.hasReferences, true);
  });

  it('hasScripts is false when scripts/ does not exist', () => {
    createSkillDir(tmpDir, 'commerce-orders', { scripts: false });
    const [skill] = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(skill.hasScripts, false);
  });

  it('hasScripts is true when scripts/ exists', () => {
    createSkillDir(tmpDir, 'commerce-orders', { scripts: true });
    const [skill] = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(skill.hasScripts, true);
  });

  it('skill can have both references and scripts', () => {
    createSkillDir(tmpDir, 'full-skill', { references: true, scripts: true });
    const [skill] = discoverFromDirectory(tmpDir, SKILL_ORIGINS.INSTALLED);
    assert.strictEqual(skill.hasReferences, true);
    assert.strictEqual(skill.hasScripts, true);
  });
});

// ===========================================================================
// discoverFromDirectory — parsed skill passthrough
// ===========================================================================

describe('discoverFromDirectory — parsed data passthrough', () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => removeTmpDir(tmpDir));

  it('parsed.name comes from frontmatter, not dir-name', () => {
    // Dir name is 'my-dir-name', frontmatter name is different
    createSkillDir(tmpDir, 'my-dir-name', { name: 'commerce-actual-name' });
    const [skill] = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(skill.name, 'commerce-actual-name');
    assert.strictEqual(skill.parsed.name, 'commerce-actual-name');
  });

  it('parsed.description reflects SKILL.md description', () => {
    createSkillDir(tmpDir, 'commerce-orders', { description: 'Custom description text' });
    const [skill] = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.strictEqual(skill.parsed.description, 'Custom description text');
  });

  it('parsed includes mcpTools extracted from body', () => {
    const body = '# Tools\n\nUse `list_orders` and `create_order` here.';
    createSkillDir(tmpDir, 'commerce-orders', { body });
    const [skill] = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.ok(skill.parsed.mcpTools.includes('list_orders'));
    assert.ok(skill.parsed.mcpTools.includes('create_order'));
  });

  it('parsed includes sections from body headings', () => {
    const body = '# Main Title\n\n## Overview\nText.\n\n## Advanced\nMore text.';
    createSkillDir(tmpDir, 'commerce-orders', { body });
    const [skill] = discoverFromDirectory(tmpDir, SKILL_ORIGINS.BUNDLED);
    assert.ok(skill.parsed.sections.includes('Overview'));
    assert.ok(skill.parsed.sections.includes('Advanced'));
  });
});

// ===========================================================================
// discoverSkills — priority / deduplication
// ===========================================================================

describe('discoverSkills — priority and deduplication', () => {
  let bundledDir, installedDir, workspaceDir;

  beforeEach(() => {
    bundledDir = makeTmpDir();
    installedDir = makeTmpDir();
    workspaceDir = makeTmpDir();
  });

  afterEach(() => {
    removeTmpDir(bundledDir);
    removeTmpDir(installedDir);
    removeTmpDir(workspaceDir);
  });

  it('returns empty array when all directories are empty', () => {
    const result = discoverSkills({ bundledDir, installedDir, workspaceDir });
    assert.deepEqual(result, []);
  });

  it('discovers skills from a single origin', () => {
    createSkillDir(bundledDir, 'commerce-orders');
    const result = discoverSkills({ bundledDir, installedDir, workspaceDir });
    assert.strictEqual(result.length, 1);
    assert.strictEqual(result[0].name, 'commerce-orders');
  });

  it('merges skills from all three origins', () => {
    createSkillDir(bundledDir, 'commerce-orders');
    createSkillDir(installedDir, 'commerce-analytics');
    createSkillDir(workspaceDir, 'commerce-custom');
    const result = discoverSkills({ bundledDir, installedDir, workspaceDir });
    assert.strictEqual(result.length, 3);
    const names = result.map((s) => s.name);
    assert.ok(names.includes('commerce-orders'));
    assert.ok(names.includes('commerce-analytics'));
    assert.ok(names.includes('commerce-custom'));
  });

  it('workspace overrides bundled for the same skill name', () => {
    createSkillDir(bundledDir, 'commerce-orders', { description: 'Bundled version' });
    createSkillDir(workspaceDir, 'commerce-orders', { description: 'Workspace version' });
    const result = discoverSkills({ bundledDir, installedDir, workspaceDir });
    assert.strictEqual(result.length, 1);
    assert.strictEqual(result[0].origin, SKILL_ORIGINS.WORKSPACE);
    assert.strictEqual(result[0].parsed.description, 'Workspace version');
  });

  it('workspace overrides installed for the same skill name', () => {
    createSkillDir(installedDir, 'commerce-orders', { description: 'Installed version' });
    createSkillDir(workspaceDir, 'commerce-orders', { description: 'Workspace version' });
    const result = discoverSkills({ bundledDir, installedDir, workspaceDir });
    assert.strictEqual(result.length, 1);
    assert.strictEqual(result[0].origin, SKILL_ORIGINS.WORKSPACE);
  });

  it('installed overrides bundled for the same skill name', () => {
    createSkillDir(bundledDir, 'commerce-orders', { description: 'Bundled version' });
    createSkillDir(installedDir, 'commerce-orders', { description: 'Installed version' });
    const result = discoverSkills({ bundledDir, installedDir, workspaceDir });
    assert.strictEqual(result.length, 1);
    assert.strictEqual(result[0].origin, SKILL_ORIGINS.INSTALLED);
    assert.strictEqual(result[0].parsed.description, 'Installed version');
  });

  it('returns skills sorted alphabetically by name', () => {
    createSkillDir(bundledDir, 'commerce-zoo');
    createSkillDir(bundledDir, 'commerce-alpha');
    createSkillDir(bundledDir, 'commerce-middle');
    const result = discoverSkills({ bundledDir, installedDir, workspaceDir });
    const names = result.map((s) => s.name);
    assert.deepEqual(names, [...names].sort());
  });

  it('does not return duplicate entries even with same name across origins', () => {
    createSkillDir(bundledDir, 'shared-skill');
    createSkillDir(installedDir, 'shared-skill');
    createSkillDir(workspaceDir, 'shared-skill');
    const result = discoverSkills({ bundledDir, installedDir, workspaceDir });
    assert.strictEqual(result.length, 1);
  });

  it('handles non-existent directories gracefully', () => {
    const missing = '/tmp/does-not-exist-' + randomUUID();
    createSkillDir(bundledDir, 'commerce-orders');
    const result = discoverSkills({
      bundledDir,
      installedDir: missing,
      workspaceDir: missing,
    });
    assert.strictEqual(result.length, 1);
    assert.strictEqual(result[0].name, 'commerce-orders');
  });

  it('returns empty array when all directories are non-existent', () => {
    const result = discoverSkills({
      bundledDir: '/tmp/no-' + randomUUID(),
      installedDir: '/tmp/no-' + randomUUID(),
      workspaceDir: '/tmp/no-' + randomUUID(),
    });
    assert.deepEqual(result, []);
  });
});

// ===========================================================================
// discoverSkills — verbose flag (smoke test)
// ===========================================================================

describe('discoverSkills — verbose option', () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => removeTmpDir(tmpDir));

  it('runs without error when verbose is true', () => {
    createSkillDir(tmpDir, 'commerce-orders');
    assert.doesNotThrow(() => {
      discoverSkills({ bundledDir: tmpDir, verbose: true });
    });
  });

  it('runs without error when verbose is false', () => {
    createSkillDir(tmpDir, 'commerce-orders');
    assert.doesNotThrow(() => {
      discoverSkills({ bundledDir: tmpDir, verbose: false });
    });
  });
});

// ===========================================================================
// discoverSkills — null / falsy directory opts
// ===========================================================================

describe('discoverSkills — null directory handling', () => {
  let tmpDir;

  beforeEach(() => {
    tmpDir = makeTmpDir();
  });

  afterEach(() => removeTmpDir(tmpDir));

  it('skips origins with null dir value', () => {
    createSkillDir(tmpDir, 'commerce-orders');
    // Pass null for two origins — should still find the bundled one
    const result = discoverSkills({ bundledDir: tmpDir, installedDir: null, workspaceDir: null });
    assert.strictEqual(result.length, 1);
  });

  it('skips origins with undefined dir value', () => {
    createSkillDir(tmpDir, 'commerce-orders');
    const result = discoverSkills({
      bundledDir: tmpDir,
      installedDir: undefined,
      workspaceDir: undefined,
    });
    assert.strictEqual(result.length, 1);
  });
});
