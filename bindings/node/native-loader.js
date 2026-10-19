// Keep diagnostics outside the generated NAPI shim: builds overwrite that file.
const { engines, version } = require('./package.json');

function nodeBelowMinimum(nodeVersion) {
  const minimum = /^>=(\d+)\.(\d+)\.(\d+)$/.exec(engines.node);
  const current = /^(\d+)\.(\d+)\.(\d+)$/.exec(nodeVersion);
  if (!minimum) return false;
  if (!current) return true;
  for (let index = 1; index <= 3; index++) {
    if (Number(current[index]) !== Number(minimum[index]))
      return Number(current[index]) < Number(minimum[index]);
  }
  return false;
}

function loadNativeBinding(load, nodeVersion = process.versions.node) {
  try {
    return load();
  } catch (cause) {
    if (!['MODULE_NOT_FOUND', 'ERR_DLOPEN_FAILED'].includes(cause?.code)) throw cause;
    const packageName = cause.message.match(/@stateset\/embedded-[a-z0-9-]+/)?.[0];
    if (nodeBelowMinimum(nodeVersion)) {
      const error = new Error(
        `@stateset/embedded requires Node ${engines.node} (found ${nodeVersion}). ` +
          'npm may have skipped the optional native binary because this Node version is unsupported. ' +
          `Upgrade Node, then reinstall with: npm install --include=optional @stateset/embedded@${version}`,
        { cause },
      );
      error.code = 'STATESET_UNSUPPORTED_NODE_VERSION';
      throw error;
    }
    const error = new Error(
      `Unable to load the @stateset/embedded native engine on ${process.platform}/${process.arch}` +
        (packageName ? ` (expected ${packageName})` : '') +
        '. ' +
        `Reinstall with: npm install --include=optional @stateset/embedded@${version}. ` +
        'Check that the binary matches your OS, architecture and libc. ' +
        cause.message,
      { cause },
    );
    error.code = 'STATESET_NATIVE_BINDING_LOAD_FAILED';
    throw error;
  }
}

module.exports = { loadNativeBinding };
