/**
 * Skill Discovery for StateSet iCommerce
 *
 * Discovers skills from three origins:
 * - Bundled: cli/skills/ (shipped with package)
 * - Installed: ~/.stateset/skills/ (user-installed from marketplace)
 * - Workspace: .stateset/skills/ (project-specific)
 *
 * Higher-priority origins override lower: workspace > installed > bundled.
 */

import fs from 'fs';
import path from 'path';
import os from 'os';
import { fileURLToPath } from 'url';
import { parseSkillMd } from './parser.js';

// ============================================================================
// Constants
// ============================================================================

export const SKILL_ORIGINS = {
  BUNDLED: 'bundled',
  INSTALLED: 'installed',
  WORKSPACE: 'workspace',
};

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// ============================================================================
// Types
// ============================================================================

/**
 * @typedef {Object} DiscoveredSkill
 * @property {string} name - Skill name from frontmatter
 * @property {string} origin - One of SKILL_ORIGINS
 * @property {string} dirPath - Absolute path to skill directory
 * @property {string} skillMdPath - Absolute path to SKILL.md
 * @property {import('./parser.js').ParsedSkill} parsed
 * @property {boolean} hasReferences - Whether references/ exists
 * @property {boolean} hasScripts - Whether scripts/ exists
 */

// ============================================================================
// Default Paths
// ============================================================================

/**
 * Get default skill directory paths.
 *
 * @returns {{ bundled: string, installed: string, workspace: string }}
 */
export function getDefaultPaths() {
  return {
    bundled: path.resolve(__dirname, '..', '..', 'skills'),
    installed: path.join(os.homedir(), '.stateset', 'skills'),
    workspace: path.resolve('.stateset', 'skills'),
  };
}

// ============================================================================
// Discovery
// ============================================================================

/**
 * Discover skills from a single directory.
 *
 * @param {string} dirPath - Directory to scan
 * @param {string} origin - Origin label
 * @returns {DiscoveredSkill[]}
 */
export function discoverFromDirectory(dirPath, origin) {
  const skills = [];

  if (!fs.existsSync(dirPath)) return skills;

  let entries;
  try {
    entries = fs.readdirSync(dirPath, { withFileTypes: true });
  } catch {
    return skills;
  }

  for (const entry of entries) {
    if (!entry.isDirectory()) continue;

    const skillDir = path.join(dirPath, entry.name);
    const skillMdPath = path.join(skillDir, 'SKILL.md');

    if (!fs.existsSync(skillMdPath)) continue;

    const parsed = parseSkillMd(skillMdPath);
    if (!parsed) continue;

    skills.push({
      name: parsed.name,
      origin,
      dirPath: skillDir,
      skillMdPath,
      parsed,
      hasReferences: fs.existsSync(path.join(skillDir, 'references')),
      hasScripts: fs.existsSync(path.join(skillDir, 'scripts')),
    });
  }

  return skills;
}

/**
 * Discover skills from all configured origins.
 * Higher-priority origins override lower ones for duplicate names.
 *
 * @param {Object} [opts]
 * @param {string} [opts.bundledDir] - Bundled skills directory
 * @param {string} [opts.installedDir] - Installed skills directory
 * @param {string} [opts.workspaceDir] - Workspace skills directory
 * @param {boolean} [opts.verbose=false]
 * @returns {DiscoveredSkill[]}
 */
export function discoverSkills(opts = {}) {
  const defaults = getDefaultPaths();
  const {
    bundledDir = defaults.bundled,
    installedDir = defaults.installed,
    workspaceDir = defaults.workspace,
    verbose = false,
  } = opts;

  const seenNames = new Set();
  const result = [];

  // Scan in priority order: workspace > installed > bundled
  // We scan highest-priority first so the Set prevents lower-priority duplicates.
  const origins = [
    { dir: workspaceDir, origin: SKILL_ORIGINS.WORKSPACE },
    { dir: installedDir, origin: SKILL_ORIGINS.INSTALLED },
    { dir: bundledDir, origin: SKILL_ORIGINS.BUNDLED },
  ];

  for (const { dir, origin } of origins) {
    if (!dir) continue;

    const discovered = discoverFromDirectory(dir, origin);
    for (const skill of discovered) {
      if (seenNames.has(skill.name)) {
        if (verbose) {
          console.log(
            `[SkillLoader] Skipping ${skill.name} from ${origin} (overridden by higher-priority origin)`,
          );
        }
        continue;
      }
      seenNames.add(skill.name);
      result.push(skill);
    }
  }

  // Sort by name for deterministic ordering
  result.sort((a, b) => a.name.localeCompare(b.name));

  if (verbose) {
    console.log(
      `[SkillLoader] Discovered ${result.length} skills from ${new Set(result.map((s) => s.origin)).size} origin(s)`,
    );
  }

  return result;
}
