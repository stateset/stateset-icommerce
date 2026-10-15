/** Address and request-target handling for the Streamable HTTP entrypoint. */

/** Parse a decimal TCP port; zero asks the OS to choose an available port. */
export function parseListenPort(value = '8090') {
  if (typeof value !== 'string' || value.length === 0 || /[^0-9]/.test(value)) {
    throw new Error('--port must be a decimal integer between 0 and 65535');
  }
  const port = Number(value);
  if (!Number.isInteger(port) || port > 65535) {
    throw new Error('--port must be a decimal integer between 0 and 65535');
  }
  return port;
}

/** Format a bound address as a URL, including brackets around IPv6 literals. */
export function listenUrl(host, port) {
  return `http://${host.includes(':') ? `[${host}]` : host}:${port}`;
}

/** Route independently of the bind address; malformed targets return null. */
export function requestPath(target) {
  try {
    return new URL(target || '/', 'http://localhost').pathname;
  } catch {
    return null;
  }
}
