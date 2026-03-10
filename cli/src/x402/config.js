import fs from 'node:fs';
import path from 'node:path';

/**
 * @typedef {Record<string, unknown>} X402Config
 * @typedef {{ env?: NodeJS.ProcessEnv, configDir?: string, configFile?: string }} ResolveX402ConfigPathOptions
 */

/**
 * @param {string} [configDir]
 * @returns {string}
 */
export function getDefaultX402ConfigPath(configDir = '.stateset') {
  return path.join(configDir, 'x402.json');
}

/**
 * @param {string | null | undefined} filePath
 * @returns {X402Config | null}
 */
export function loadX402Config(filePath) {
  if (!filePath) return null;
  try {
    if (!fs.existsSync(filePath)) return null;
    const raw = fs.readFileSync(filePath, 'utf8');
    return /** @type {X402Config} */ (JSON.parse(raw));
  } catch (error) {
    const err = error instanceof Error ? error : new Error(String(error));
    throw new Error(`Failed to load x402 config: ${err.message}`);
  }
}

/**
 * @param {string} filePath
 * @param {X402Config} config
 */
export function saveX402Config(filePath, config) {
  const dir = path.dirname(filePath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  fs.writeFileSync(filePath, JSON.stringify(config, null, 2));

  const baseDir = path.basename(dir);
  if (baseDir === '.stateset') {
    const gitignorePath = path.join(process.cwd(), '.gitignore');
    if (fs.existsSync(gitignorePath)) {
      const content = fs.readFileSync(gitignorePath, 'utf8');
      if (!content.includes('.stateset')) {
        fs.appendFileSync(gitignorePath, '\n# StateSet local config\n.stateset/\n');
      }
    }
  }
}

/**
 * @param {ResolveX402ConfigPathOptions} [options]
 * @returns {string}
 */
export function resolveX402ConfigPath({
  env = process.env,
  configDir = '.stateset',
  configFile,
} = {}) {
  return configFile || env.X402_CONFIG_FILE || getDefaultX402ConfigPath(configDir);
}

/**
 * @param {NodeJS.ProcessEnv | null | undefined} env
 * @param {X402Config | null | undefined} config
 * @param {string | null | undefined} envKey
 * @param {...string} configKeys
 * @returns {unknown}
 */
export function pickConfigValue(env, config, envKey, ...configKeys) {
  if (envKey && env && env[envKey] !== undefined) return env[envKey];
  if (!config) return undefined;
  for (const key of configKeys) {
    if (config[key] !== undefined) return config[key];
  }
  return undefined;
}
