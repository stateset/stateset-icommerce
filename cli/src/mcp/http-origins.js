/** Browser-origin policy. Full URLs pin scheme and port; bare hosts are legacy opt-ins. */
function parseOrigin(value, configuration = false) {
  if (typeof value !== 'string' || !/^https?:\/\/[^/\\?#\s,]+\/?$/i.test(value)) return null;
  try {
    const url = new URL(value);
    if (
      url.hostname.includes('*') ||
      url.username ||
      url.password ||
      url.pathname !== '/' ||
      url.search ||
      url.hash ||
      (!configuration && value.endsWith('/'))
    )
      return null;
    return url;
  } catch {
    return null;
  }
}

export function createOriginGuard(values = [], { allowLocalhost = false } = {}) {
  const origins = new Set();
  const hosts = new Set();
  for (const value of values) {
    const trimmed = value.trim();
    if (trimmed === 'null' || trimmed.includes('*')) {
      throw new Error('--allowed-origin must be an HTTP(S) origin or a bare hostname');
    }
    const origin = parseOrigin(trimmed, true);
    if (origin) {
      origins.add(origin.origin);
      continue;
    }
    // Retain explicit bare-host configuration for existing clients. Never
    // downgrade a malformed URL to a host-wide rule.
    const host = !/[\s/:@?#,\\]/.test(trimmed.replace(/^\[::1\]$/, 'localhost'))
      ? parseOrigin(`http://${trimmed}`, true)
      : null;
    if (!host || host.port || host.hostname !== trimmed.toLowerCase()) {
      throw new Error('--allowed-origin must be an HTTP(S) origin or a bare hostname');
    }
    hosts.add(host.hostname);
  }

  return function validateOrigin(req, res) {
    const raw = req.headers.origin;
    if (raw === undefined) return true;
    const duplicates = (req.rawHeaders ?? []).filter(
      (value, index) => index % 2 === 0 && value.toLowerCase() === 'origin',
    ).length;
    const url = duplicates <= 1 ? parseOrigin(raw) : null;
    if (
      url &&
      (origins.has(url.origin) ||
        hosts.has(url.hostname) ||
        (values.length === 0 &&
          allowLocalhost &&
          ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname)))
    )
      return true;
    res.writeHead(403, { 'Content-Type': 'application/json' });
    res.end(
      JSON.stringify({
        jsonrpc: '2.0',
        id: null,
        error: { code: -32000, message: 'Invalid Origin: browser origin is not allowed' },
      }),
    );
    return false;
  };
}
