/**
 * Auto-load ~/.stateset/.env file
 *
 * This module is imported at the top of config.js to ensure API keys
 * saved by `stateset-config set-key` are automatically available.
 *
 * Environment variables already set take precedence over .env file values.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

const envFile = path.join(os.homedir(), '.stateset', '.env');

if (fs.existsSync(envFile)) {
  try {
    const content = fs.readFileSync(envFile, 'utf-8');
    for (const line of content.split('\n')) {
      const trimmed = line.trim();
      if (trimmed && !trimmed.startsWith('#')) {
        const eqIndex = trimmed.indexOf('=');
        if (eqIndex > 0) {
          const key = trimmed.slice(0, eqIndex).trim();
          let value = trimmed.slice(eqIndex + 1);
          // Remove surrounding quotes
          if ((value.startsWith('"') && value.endsWith('"')) ||
              (value.startsWith("'") && value.endsWith("'"))) {
            value = value.slice(1, -1);
          }
          // Only set if not already in environment (env vars take precedence)
          // Use 'in' to check existence, not truthiness (empty string should count as "set")
          if (!(key in process.env)) {
            process.env[key] = value;
          }
        }
      }
    }
  } catch {
    // Silently ignore errors reading env file
  }
}
