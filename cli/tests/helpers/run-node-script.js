import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

function shellQuote(value) {
  return `'${String(value).replace(/'/g, `'\\''`)}'`;
}

/**
 * Run a Node CLI script in tests with robust output capture.
 *
 * In this sandboxed runner, nested Node output is unreliable with pipe stdio.
 * We always execute through shell redirection to capture deterministic output
 * without running commands twice.
 */
export function runNodeScript(scriptPath, args = [], options = {}) {
  const env = { ...process.env, NODE_NO_WARNINGS: '1', ...(options.env || {}) };
  const captureDir = mkdtempSync(join(tmpdir(), 'stateset-cli-test-'));
  const capturePath = join(captureDir, 'capture.txt');
  const commandParts = [
    shellQuote(process.execPath),
    shellQuote(scriptPath),
    ...args.map((arg) => shellQuote(arg)),
    '>',
    shellQuote(capturePath),
    '2>&1',
  ];

  const commandBody = commandParts.join(' ');
  const command =
    options.input !== undefined
      ? `printf %s ${shellQuote(options.input)} | ${commandBody}`
      : commandBody;

  const result = spawnSync('bash', ['-lc', command], {
    encoding: 'utf-8',
    env,
    cwd: options.cwd || process.cwd(),
  });

  let capturedOutput = '';
  try {
    capturedOutput = readFileSync(capturePath, 'utf-8');
  } catch {
    capturedOutput = '';
  } finally {
    rmSync(captureDir, { recursive: true, force: true });
  }

  if (result.error && result.status === null) {
    throw result.error;
  }

  return {
    status: result.status,
    signal: result.signal,
    stdout: capturedOutput,
    stderr: result.stderr || '',
  };
}
