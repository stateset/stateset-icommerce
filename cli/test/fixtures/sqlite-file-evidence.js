import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

const execute = promisify(execFile);
const script = `
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const hashes = await Promise.all(process.argv.slice(1).map(async (file) =>
  createHash('sha256').update(await readFile(file)).digest('hex')));
console.log(JSON.stringify(hashes));
`;

// On Unix, closing any descriptor for a locked file releases that process's
// POSIX locks, even when SQLite owns a different descriptor. Hashing a live
// database with fs.readFile() in its writer process can therefore orphan its
// WAL and manufacture a lost-update failure. Observe bytes from a child only.
export async function hashFiles(files) {
  const { stdout } = await execute(
    process.execPath,
    ['--input-type=module', '-e', script, ...files],
    { timeout: 45000 },
  );
  return JSON.parse(stdout);
}

export async function hashFile(file) {
  return (await hashFiles([file]))[0];
}
