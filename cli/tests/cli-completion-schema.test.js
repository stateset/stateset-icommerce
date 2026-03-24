import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { MAIN_CLI_LONG_FLAGS } from '../src/cli-schema.js';
import { runNodeScript } from './helpers/run-node-script.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const COMPLETION_BIN = join(__dirname, '..', 'bin', 'stateset-completion.js');

function runCompletion(shell) {
  return runNodeScript(COMPLETION_BIN, [shell]);
}

describe('stateset-completion schema parity', () => {
  it('bash completion includes all main stateset flags from shared schema', () => {
    const result = runCompletion('bash');
    assert.equal(result.status, 0, result.stderr);

    for (const flag of MAIN_CLI_LONG_FLAGS) {
      assert.match(result.stdout, new RegExp(`\\${flag}\\b`), `Missing ${flag} in bash completion`);
    }
  });

  it('zsh completion includes all main stateset flags from shared schema', () => {
    const result = runCompletion('zsh');
    assert.equal(result.status, 0, result.stderr);

    for (const flag of MAIN_CLI_LONG_FLAGS) {
      assert.match(result.stdout, new RegExp(`\\${flag}\\[`), `Missing ${flag} in zsh completion`);
    }
  });

  it('fish completion includes all main stateset flags from shared schema', () => {
    const result = runCompletion('fish');
    assert.equal(result.status, 0, result.stderr);

    for (const flag of MAIN_CLI_LONG_FLAGS) {
      const longName = flag.replace(/^--/, '');
      assert.match(result.stdout, new RegExp(`-l\\s+${longName}\\b`), `Missing ${flag} in fish completion`);
    }
  });

  it('chat completion surfaces the --stats flag across shells', () => {
    const bash = runCompletion('bash');
    const zsh = runCompletion('zsh');
    const fish = runCompletion('fish');

    assert.equal(bash.status, 0, bash.stderr);
    assert.equal(zsh.status, 0, zsh.stderr);
    assert.equal(fish.status, 0, fish.stderr);

    assert.match(bash.stdout, /stateset-chat[\s\S]*--stats/);
    assert.match(zsh.stdout, /--stats\[Show execution stats\]/);
    assert.match(fish.stdout, /complete -c stateset-chat -l stats -d 'Show execution stats'/);
  });

});
