'use strict';

// The @stateset/embedded/agent-toolkit entrypoint delegates to the
// @stateset/cli agent toolkit (either the published package or the monorepo
// workspace fallback at ../../cli/src/agent-toolkit.js). That stack is an
// *optional* peer dependency: some CI jobs (e.g. the node-bindings coverage
// job) intentionally run without the cli dependency tree installed.
//
// Integration tests that need the real toolkit call `loadToolkitModule()`
// and skip — loudly, with the underlying resolution error in the skip
// message — when the optional stack is unavailable. Module-resolution
// failures raised from *inside* this package are real bugs and are
// rethrown so they fail the test run.

const path = require('node:path');

const packageRoot = path.resolve(__dirname, '..');

function isMissingOptionalToolkitStack(error) {
  if (!error || error.code !== 'ERR_MODULE_NOT_FOUND') {
    return false;
  }
  const message = error instanceof Error ? error.message : String(error);
  if (
    message.includes("'@stateset/cli'") ||
    message.includes('@stateset/cli/agent-toolkit')
  ) {
    return true;
  }
  // ERR_MODULE_NOT_FOUND messages name the importing module:
  //   Cannot find package 'x' imported from /path/to/importer.js
  // Only failures whose importer lives outside this package (i.e. in the
  // cli workspace dependency chain) qualify for skipping.
  const importer = /imported from (.+?)(?:\n|$)/.exec(message);
  if (!importer) {
    return false;
  }
  return !importer[1].startsWith(packageRoot + path.sep);
}

async function loadToolkitModule() {
  try {
    return { toolkitModule: await import('@stateset/embedded/agent-toolkit'), skipReason: null };
  } catch (error) {
    if (isMissingOptionalToolkitStack(error)) {
      const firstLine = String(error.message).split('\n')[0];
      return {
        toolkitModule: null,
        skipReason: `optional @stateset/cli toolkit stack is not installed (${firstLine})`,
      };
    }
    throw error;
  }
}

module.exports = { loadToolkitModule };
