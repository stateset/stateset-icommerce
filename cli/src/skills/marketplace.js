/**
 * Marketplace Client for StateSet iCommerce Skills
 *
 * Fetches the skill catalog (local or remote), installs skills
 * from zip packages, and manages installed skill lifecycle.
 */

import fs from 'fs';
import path from 'path';
import os from 'os';
import crypto from 'crypto';
import net from 'net';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// ============================================================================
// Types
// ============================================================================

/**
 * @typedef {Object} CatalogEntry
 * @property {string} name
 * @property {string} description
 * @property {string} category
 * @property {string[]} tags
 * @property {string} version
 * @property {string} downloadUrl
 * @property {string} [downloadChecksum]
 * @property {string} [checksumAlgorithm]
 * @property {boolean} isPublic
 * @property {boolean} hasReferences
 * @property {boolean} hasScripts
 * @property {string} updatedAt
 */

/**
 * @typedef {Object} MarketplaceCatalog
 * @property {string} version
 * @property {string} generatedAt
 * @property {string} baseUrl
 * @property {CatalogEntry[]} skills
 */

// ============================================================================
// Constants
// ============================================================================

const DEFAULT_INSTALL_DIR = path.join(os.homedir(), '.stateset', 'skills');
const DEFAULT_CATALOG_PATH = path.resolve(__dirname, '..', '..', 'skills', 'marketplace.json');
const DEFAULT_BUNDLED_DIR = path.resolve(__dirname, '..', '..', 'skills');
const ALLOW_INSECURE_DOWNLOADS_ENV = 'STATESET_ALLOW_INSECURE_SKILL_DOWNLOADS';

// ============================================================================
// MarketplaceClient
// ============================================================================

export class MarketplaceClient {
  /**
   * @param {Object} [opts]
   * @param {string} [opts.catalogUrl] - Remote catalog URL
   * @param {string} [opts.catalogPath] - Local catalog path
   * @param {string} [opts.installDir] - Install directory
   * @param {string} [opts.bundledDir] - Bundled skills directory
   * @param {boolean} [opts.allowInsecureDownloads=false] - Allow downloads without checksum metadata.
   *    Can also be enabled with STATESET_ALLOW_INSECURE_SKILL_DOWNLOADS=1.
   */
  constructor(opts = {}) {
    this._catalogUrl = opts.catalogUrl || null;
    this._catalogPath = opts.catalogPath || DEFAULT_CATALOG_PATH;
    this._installDir = opts.installDir || DEFAULT_INSTALL_DIR;
    this._bundledDir = opts.bundledDir || DEFAULT_BUNDLED_DIR;
    this._allowInsecureDownloads =
      opts.allowInsecureDownloads === true || envFlagEnabled(ALLOW_INSECURE_DOWNLOADS_ENV);
    this._catalog = null;
  }

  // --------------------------------------------------------------------------
  // Catalog
  // --------------------------------------------------------------------------

  /**
   * Fetch the catalog (remote first, local fallback).
   *
   * @returns {Promise<MarketplaceCatalog>}
   */
  async fetchCatalog() {
    // Try remote first
    if (this._catalogUrl) {
      try {
        const res = await fetch(this._catalogUrl);
        if (res.ok) {
          this._catalog = await res.json();
          return this._catalog;
        }
      } catch (err) {
        console.warn(`[Marketplace] Failed to fetch remote catalog: ${err.message}`);
      }
    }

    // Fall back to local
    return this.loadLocalCatalog();
  }

  /**
   * Load the local bundled catalog.
   *
   * @returns {MarketplaceCatalog}
   */
  loadLocalCatalog() {
    if (this._catalog) return this._catalog;

    try {
      const raw = fs.readFileSync(this._catalogPath, 'utf-8');
      this._catalog = JSON.parse(raw);
      return this._catalog;
    } catch (err) {
      console.warn(`[Marketplace] Failed to load catalog: ${err.message}`);
      return { version: '0.0.0', generatedAt: '', baseUrl: '', skills: [] };
    }
  }

  /**
   * Search the catalog.
   *
   * @param {string} query
   * @returns {CatalogEntry[]}
   */
  searchCatalog(query) {
    const catalog = this.loadLocalCatalog();
    if (!query) return catalog.skills;

    const q = query.toLowerCase();
    return catalog.skills.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        s.description.toLowerCase().includes(q) ||
        s.category.includes(q) ||
        s.tags.some((t) => t.includes(q)),
    );
  }

  /**
   * Get a single catalog entry.
   *
   * @param {string} name
   * @returns {CatalogEntry|null}
   */
  getCatalogEntry(name) {
    const catalog = this.loadLocalCatalog();
    return catalog.skills.find((s) => s.name === name) || null;
  }

  /**
   * List catalog categories.
   *
   * @returns {string[]}
   */
  listCategories() {
    const catalog = this.loadLocalCatalog();
    return [...new Set(catalog.skills.map((s) => s.category))].sort();
  }

  /**
   * List skills in a category.
   *
   * @param {string} category
   * @returns {CatalogEntry[]}
   */
  listByCategory(category) {
    const catalog = this.loadLocalCatalog();
    return catalog.skills.filter((s) => s.category === category);
  }

  // --------------------------------------------------------------------------
  // Install / Uninstall
  // --------------------------------------------------------------------------

  /**
   * Install a skill from the marketplace.
   * Copies from bundled directory if available, otherwise downloads.
   *
   * @param {string} name
   * @param {Object} [opts]
   * @param {boolean} [opts.force=false]
   * @returns {Promise<{ installed: boolean, path: string, error?: string }>}
   */
  async install(name, opts = {}) {
    const { force = false } = opts;
    const destDir = path.join(this._installDir, name);

    // Check if already installed
    if (fs.existsSync(destDir) && !force) {
      return {
        installed: false,
        path: destDir,
        error: 'Already installed. Use --force to overwrite.',
      };
    }

    // Look up in catalog
    const entry = this.getCatalogEntry(name);
    if (!entry) {
      return {
        installed: false,
        path: '',
        error: `Skill "${name}" not found in marketplace catalog.`,
      };
    }

    // Try to copy from bundled directory first (offline-friendly)
    const bundledDir = path.join(this._bundledDir, name);
    if (fs.existsSync(bundledDir)) {
      try {
        // Remove existing if force
        if (fs.existsSync(destDir) && force) {
          fs.rmSync(destDir, { recursive: true, force: true });
        }

        fs.mkdirSync(this._installDir, { recursive: true });
        copyDir(bundledDir, destDir);

        // Verify
        const skillMd = path.join(destDir, 'SKILL.md');
        if (!fs.existsSync(skillMd)) {
          fs.rmSync(destDir, { recursive: true, force: true });
          return { installed: false, path: destDir, error: 'Installed skill missing SKILL.md' };
        }

        return { installed: true, path: destDir };
      } catch (err) {
        return { installed: false, path: destDir, error: `Copy failed: ${err.message}` };
      }
    }

    // Not bundled — download from remote URL
    if (entry.downloadUrl) {
      const sourceError = this._validateRemoteDownloadSource(entry);
      if (sourceError) {
        return { installed: false, path: '', error: sourceError };
      }

      try {
        const result = await this._downloadAndInstall(name, entry, destDir, force);
        return result;
      } catch (err) {
        return { installed: false, path: '', error: `Download failed: ${err.message}` };
      }
    }

    return {
      installed: false,
      path: '',
      error: `Skill "${name}" is not bundled and has no download URL.`,
    };
  }

  /**
   * Uninstall an installed skill.
   *
   * @param {string} name
   * @returns {{ removed: boolean, error?: string }}
   */
  uninstall(name) {
    const destDir = path.join(this._installDir, name);

    if (!fs.existsSync(destDir)) {
      return { removed: false, error: `Skill "${name}" is not installed.` };
    }

    // Check if it's in the install dir (not bundled)
    try {
      fs.rmSync(destDir, { recursive: true, force: true });
      return { removed: true };
    } catch (err) {
      return { removed: false, error: `Failed to remove: ${err.message}` };
    }
  }

  /**
   * Check if a skill is installed (in the install dir).
   *
   * @param {string} name
   * @returns {boolean}
   */
  isInstalled(name) {
    return fs.existsSync(path.join(this._installDir, name, 'SKILL.md'));
  }

  /**
   * List all installed skill names.
   *
   * @returns {string[]}
   */
  listInstalled() {
    if (!fs.existsSync(this._installDir)) return [];

    try {
      return fs
        .readdirSync(this._installDir, { withFileTypes: true })
        .filter(
          (e) => e.isDirectory() && fs.existsSync(path.join(this._installDir, e.name, 'SKILL.md')),
        )
        .map((e) => e.name)
        .sort();
    } catch (err) {
      console.debug('[marketplace] Installed skills listing failed:', err.message || err);
      return [];
    }
  }

  // --------------------------------------------------------------------------
  // Remote Download
  // --------------------------------------------------------------------------

  /**
   * Download a skill from a remote URL and install it.
   * Supports .zip and .tar.gz packages, or a direct SKILL.md file.
   *
   * @param {string} name
   * @param {CatalogEntry} entry
   * @param {string} destDir
   * @param {boolean} force
   * @returns {Promise<{ installed: boolean, path: string, error?: string }>}
   * @private
   */
  async _downloadAndInstall(name, entry, destDir, force) {
    const url = entry.downloadUrl;
    const res = await fetch(url, { redirect: 'error' });
    if (!res.ok) {
      return { installed: false, path: '', error: `HTTP ${res.status}: ${res.statusText}` };
    }

    // Remove existing if force
    if (fs.existsSync(destDir) && force) {
      fs.rmSync(destDir, { recursive: true, force: true });
    }

    fs.mkdirSync(destDir, { recursive: true });

    try {
      const contentType = res.headers.get('content-type') || '';

      if (
        url.endsWith('.md') ||
        contentType.includes('text/markdown') ||
        contentType.includes('text/plain')
      ) {
        // Direct SKILL.md download
        const text = await res.text();
        const payload = Buffer.from(text, 'utf8');
        const checksumError = this._verifyDownloadChecksum(entry, payload);
        if (checksumError) {
          fs.rmSync(destDir, { recursive: true, force: true });
          return { installed: false, path: destDir, error: checksumError };
        }
        fs.writeFileSync(path.join(destDir, 'SKILL.md'), text);
      } else {
        // Binary package — save as temp file
        const tmpFile = path.join(os.tmpdir(), `stateset-skill-${name}-${Date.now()}.bin`);
        const arrayBuf = await res.arrayBuffer();
        const payload = Buffer.from(arrayBuf);
        const checksumError = this._verifyDownloadChecksum(entry, payload);
        if (checksumError) {
          fs.rmSync(destDir, { recursive: true, force: true });
          return { installed: false, path: destDir, error: checksumError };
        }
        fs.writeFileSync(tmpFile, payload);

        try {
          // Extract archive — use execFileSync (no shell) for safety
          const { execFileSync } = await import('child_process');
          if (url.endsWith('.tar.gz') || url.endsWith('.tgz')) {
            // --strip-components=1 prevents directory traversal at top level
            execFileSync('tar', ['-xzf', tmpFile, '-C', destDir, '--strip-components=1'], {
              stdio: 'pipe',
              timeout: 30000,
            });
          } else {
            execFileSync('unzip', ['-o', '-q', tmpFile, '-d', destDir], {
              stdio: 'pipe',
              timeout: 30000,
            });
          }
          // Verify no files escaped destDir (defence against zip path traversal)
          const resolvedDest = path.resolve(destDir);
          const { execFileSync: efs2 } = await import('child_process');
          const listing = efs2('find', [resolvedDest, '-type', 'f'], {
            encoding: 'utf8',
            timeout: 5000,
          });
          for (const line of listing.split('\n').filter(Boolean)) {
            if (!path.resolve(line).startsWith(resolvedDest)) {
              fs.rmSync(resolvedDest, { recursive: true, force: true });
              throw new Error('Extracted archive contains path traversal — installation aborted');
            }
          }
        } finally {
          fs.unlinkSync(tmpFile);
        }
      }

      // Verify
      const skillMd = path.join(destDir, 'SKILL.md');
      if (!fs.existsSync(skillMd)) {
        fs.rmSync(destDir, { recursive: true, force: true });
        return { installed: false, path: destDir, error: 'Downloaded package missing SKILL.md' };
      }

      return { installed: true, path: destDir };
    } catch (err) {
      fs.rmSync(destDir, { recursive: true, force: true });
      return { installed: false, path: destDir, error: `Install failed: ${err.message}` };
    }
  }

  /**
   * Validate remote download source and integrity metadata.
   *
   * @param {CatalogEntry} entry
   * @returns {string|null}
   * @private
   */
  _validateRemoteDownloadSource(entry) {
    let parsed;
    try {
      parsed = new URL(entry.downloadUrl);
    } catch {
      return 'Invalid remote skill download URL.';
    }

    if (parsed.protocol !== 'https:') {
      return 'Remote skill download URL must use HTTPS.';
    }

    const host = parsed.hostname.toLowerCase();
    if (!host || host === 'localhost' || host.endsWith('.localhost') || net.isIP(host) !== 0) {
      return 'Remote skill download URL must use a public DNS hostname.';
    }

    const catalog = this.loadLocalCatalog();
    if (catalog.baseUrl) {
      try {
        const baseUrl = new URL(catalog.baseUrl);
        if (baseUrl.protocol !== 'https:') {
          return 'Marketplace catalog baseUrl must use HTTPS.';
        }
        if (parsed.origin !== baseUrl.origin || !parsed.pathname.startsWith(baseUrl.pathname)) {
          return 'Remote skill download URL must be within marketplace baseUrl.';
        }
      } catch {
        return 'Marketplace catalog baseUrl is invalid.';
      }
    }

    if (!this._allowInsecureDownloads) {
      if (typeof entry.downloadChecksum !== 'string' || !entry.downloadChecksum.trim()) {
        return 'Remote skill download requires downloadChecksum metadata.';
      }
      const parsedChecksum = parseChecksum(
        entry.downloadChecksum,
        entry.checksumAlgorithm || 'sha256',
      );
      if (!parsedChecksum) {
        return 'Invalid downloadChecksum format. Use "<algorithm>:<hex>" or "<hex>".';
      }
      const configuredAlgorithm =
        typeof entry.checksumAlgorithm === 'string'
          ? entry.checksumAlgorithm.trim().toLowerCase()
          : '';
      if (configuredAlgorithm && configuredAlgorithm !== parsedChecksum.algorithm) {
        return 'downloadChecksum algorithm does not match checksumAlgorithm.';
      }
    }

    return null;
  }

  /**
   * Verify downloaded payload integrity using catalog checksum.
   *
   * @param {CatalogEntry} entry
   * @param {Buffer} payload
   * @returns {string|null}
   * @private
   */
  _verifyDownloadChecksum(entry, payload) {
    if (this._allowInsecureDownloads) return null;

    const parsedChecksum = parseChecksum(
      entry.downloadChecksum,
      entry.checksumAlgorithm || 'sha256',
    );
    if (!parsedChecksum) {
      return 'Invalid checksum metadata for remote skill download.';
    }
    const configuredAlgorithm =
      typeof entry.checksumAlgorithm === 'string'
        ? entry.checksumAlgorithm.trim().toLowerCase()
        : '';
    if (configuredAlgorithm && configuredAlgorithm !== parsedChecksum.algorithm) {
      return 'Checksum algorithm metadata mismatch for remote skill download.';
    }

    const { algorithm, digest: expectedDigest } = parsedChecksum;
    let actualDigest;
    try {
      actualDigest = crypto.createHash(algorithm).update(payload).digest('hex');
    } catch (err) {
      return `Unsupported checksum algorithm: ${algorithm} (${err.message})`;
    }

    if (!safeCompareHex(actualDigest, expectedDigest)) {
      return `Checksum mismatch for downloaded skill package (${algorithm}).`;
    }

    return null;
  }

  // --------------------------------------------------------------------------
  // Versioning & Auto-Update
  // --------------------------------------------------------------------------

  /**
   * Check if a skill has an update available.
   *
   * @param {string} name
   * @returns {{ hasUpdate: boolean, installed: string|null, latest: string|null }}
   */
  checkForUpdate(name) {
    const entry = this.getCatalogEntry(name);
    if (!entry) return { hasUpdate: false, installed: null, latest: null };

    const installedDir = path.join(this._installDir, name);
    if (!fs.existsSync(installedDir))
      return { hasUpdate: true, installed: null, latest: entry.version };

    // Read installed version from SKILL.md frontmatter
    try {
      const skillMd = fs.readFileSync(path.join(installedDir, 'SKILL.md'), 'utf-8');
      const versionMatch = skillMd.match(/^version:\s*(.+)$/m);
      const installedVersion = versionMatch ? versionMatch[1].trim() : '0.0.0';
      const hasUpdate = entry.version !== installedVersion;
      return { hasUpdate, installed: installedVersion, latest: entry.version };
    } catch (err) {
      console.debug('[marketplace] Skill version check failed:', err.message || err);
      return { hasUpdate: true, installed: null, latest: entry.version };
    }
  }

  /**
   * Check for updates across all installed skills.
   *
   * @returns {{ name: string, installed: string|null, latest: string|null }[]}
   */
  checkAllUpdates() {
    const installed = this.listInstalled();
    const updates = [];

    for (const name of installed) {
      const { hasUpdate, installed: instVer, latest } = this.checkForUpdate(name);
      if (hasUpdate) {
        updates.push({ name, installed: instVer, latest });
      }
    }

    return updates;
  }

  /**
   * Update a skill to the latest version.
   *
   * @param {string} name
   * @returns {Promise<{ updated: boolean, error?: string }>}
   */
  async update(name) {
    const { hasUpdate } = this.checkForUpdate(name);
    if (!hasUpdate) {
      return { updated: false, error: 'Already up to date.' };
    }
    const result = await this.install(name, { force: true });
    return { updated: result.installed, error: result.error };
  }

  /**
   * Update all installed skills.
   *
   * @returns {Promise<{ updated: string[], failed: { name: string, error: string }[] }>}
   */
  async updateAll() {
    const updates = this.checkAllUpdates();
    const updated = [];
    const failed = [];

    for (const { name } of updates) {
      const result = await this.update(name);
      if (result.updated) {
        updated.push(name);
      } else {
        failed.push({ name, error: result.error || 'Unknown error' });
      }
    }

    return { updated, failed };
  }

  /**
   * Get catalog summary stats.
   *
   * @returns {{ total: number, public: number, internal: number, categories: Object<string, number> }}
   */
  getCatalogStats() {
    const catalog = this.loadLocalCatalog();
    const stats = {
      total: catalog.skills.length,
      public: catalog.skills.filter((s) => s.isPublic).length,
      internal: catalog.skills.filter((s) => !s.isPublic).length,
      categories: {},
    };

    for (const s of catalog.skills) {
      stats.categories[s.category] = (stats.categories[s.category] || 0) + 1;
    }

    return stats;
  }
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Recursively copy a directory.
 *
 * @param {string} src
 * @param {string} dest
 */
function copyDir(src, dest) {
  fs.mkdirSync(dest, { recursive: true });
  const entries = fs.readdirSync(src, { withFileTypes: true });

  for (const entry of entries) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);

    if (entry.isDirectory()) {
      copyDir(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

/**
 * Parse checksum metadata.
 *
 * Accepted formats:
 * - "<hex>" (uses fallback algorithm)
 * - "<algorithm>:<hex>"
 *
 * @param {string} checksum
 * @param {string} fallbackAlgorithm
 * @returns {{ algorithm: string, digest: string }|null}
 */
function parseChecksum(checksum, fallbackAlgorithm) {
  if (typeof checksum !== 'string') return null;
  const trimmed = checksum.trim().toLowerCase();
  if (!trimmed) return null;

  let algorithm = String(fallbackAlgorithm || 'sha256')
    .trim()
    .toLowerCase();
  let digest = trimmed;

  const parts = trimmed.split(':');
  if (parts.length === 2) {
    algorithm = parts[0].trim();
    digest = parts[1].trim();
  } else if (parts.length > 2) {
    return null;
  }

  if (!algorithm || !/^[a-z0-9-]+$/.test(algorithm)) return null;
  if (!/^[a-f0-9]+$/.test(digest) || digest.length % 2 !== 0) return null;

  return { algorithm, digest };
}

/**
 * Constant-time hex digest comparison.
 *
 * @param {string} left
 * @param {string} right
 * @returns {boolean}
 */
function safeCompareHex(left, right) {
  if (left.length !== right.length) return false;
  const leftBuf = Buffer.from(left, 'hex');
  const rightBuf = Buffer.from(right, 'hex');
  if (leftBuf.length !== rightBuf.length) return false;
  return crypto.timingSafeEqual(leftBuf, rightBuf);
}

/**
 * Parse an environment flag.
 *
 * @param {string} key
 * @returns {boolean}
 */
function envFlagEnabled(key) {
  const raw = process.env[key];
  if (typeof raw !== 'string') return false;
  const value = raw.trim().toLowerCase();
  return value === '1' || value === 'true' || value === 'yes' || value === 'on';
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the shared MarketplaceClient singleton.
 *
 * @param {Object} [opts]
 * @param {boolean} [opts.allowInsecureDownloads=false]
 * @returns {MarketplaceClient}
 */
export function getMarketplaceClient(opts = {}) {
  if (!_instance) {
    _instance = new MarketplaceClient(opts);
  }
  return _instance;
}

/**
 * Reset the singleton (for testing).
 */
export function resetMarketplaceClient() {
  _instance = null;
}
