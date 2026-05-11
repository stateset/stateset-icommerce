// Healthcheck shared by handler and settler. Exits 0 if the local server's
// /healthz returns ok, else exits 1. Used by Docker HEALTHCHECK.

const port = process.env.PORT;
if (!port) {
  process.stderr.write('healthcheck: PORT env var not set\n');
  process.exit(1);
}

try {
  const r = await fetch(`http://127.0.0.1:${port}/healthz`);
  if (!r.ok) {
    process.stderr.write(`healthcheck: status ${r.status}\n`);
    process.exit(1);
  }
  const j = await r.json();
  if (!j.ok) {
    process.stderr.write(`healthcheck: not ok: ${JSON.stringify(j)}\n`);
    process.exit(1);
  }
  process.exit(0);
} catch (err) {
  process.stderr.write(`healthcheck: ${err.message}\n`);
  process.exit(1);
}
