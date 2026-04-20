import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const workspaceToolkitPath = path.resolve(__dirname, '../../cli/src/agent-toolkit.js');

function isMissingToolkitModule(error) {
  const message = error instanceof Error ? error.message : String(error);
  return (
    error &&
    error.code === 'ERR_MODULE_NOT_FOUND' &&
    (message.includes('@stateset/cli/agent-toolkit') || message.includes("package '@stateset/cli'"))
  );
}

async function loadPublishedToolkit() {
  return import('@stateset/cli/agent-toolkit');
}

async function loadWorkspaceToolkit() {
  return import(pathToFileURL(workspaceToolkitPath).href);
}

function createMissingPeerDependencyError(originalError) {
  const error = new Error(
    'The @stateset/embedded/agent-toolkit entrypoint requires @stateset/cli to be installed alongside @stateset/embedded. Run `npm install @stateset/cli` in the host app.',
  );
  error.cause = originalError;
  return error;
}

async function loadToolkitModule() {
  try {
    return await loadPublishedToolkit();
  } catch (error) {
    if (!isMissingToolkitModule(error)) {
      throw error;
    }
    if (existsSync(workspaceToolkitPath)) {
      return loadWorkspaceToolkit();
    }
    throw createMissingPeerDependencyError(error);
  }
}

const toolkitModule = await loadToolkitModule();

export const createEmbeddedAgentToolkit = toolkitModule.createEmbeddedAgentToolkit;
export const createEmbeddedAgentKit =
  toolkitModule.createEmbeddedAgentKit || toolkitModule.createEmbeddedAgentToolkit;

export default createEmbeddedAgentToolkit;
