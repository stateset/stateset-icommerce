#!/usr/bin/env node
// Test fixture: an adapter that follows the stdio contract (reads the vector
// inputs from stdin, writes one JSON object to stdout, exits 0) but whose
// output never matches any vector's expected.json. Used by runner.test.mjs to
// prove a divergent implementation fails the run and is recorded in the
// --report output. Not a real IUT; never add it to iut-adapters/registry.json.

let raw = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => {
  raw += chunk;
});
process.stdin.on('end', () => {
  process.stdout.write(
    JSON.stringify({ fixture: 'failing-adapter', vector: process.argv[2], input_bytes: raw.length }),
  );
});
