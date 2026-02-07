import fs from 'node:fs';
import path from 'node:path';

export function getDefaultX402ConfigPath(configDir = '.stateset') {
  return path.join(configDir, 'x402.json');
}

export function loadX402Config(filePath) {
  if (!filePath) return null;
  try {
    if (!fs.existsSync(filePath)) return null;
    const raw = fs.readFileSync(filePath, 'utf8');
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`Failed to load x402 config: ${error.message}`);
  }
}

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

export function resolveX402ConfigPath({
  env = process.env,
  configDir = '.stateset',
  configFile,
} = {}) {
  return configFile || env.X402_CONFIG_FILE || getDefaultX402ConfigPath(configDir);
}

export function pickConfigValue(env, config, envKey, ...configKeys) {
  if (envKey && env && env[envKey] !== undefined) return env[envKey];
  if (!config) return undefined;
  for (const key of configKeys) {
    if (config[key] !== undefined) return config[key];
  }
  return undefined;
}
