export async function loadStoreEnvironment({ cwd = process.cwd(), development = false } = {}) {
  const nextEnv = await import('@next/env');
  let failed = false;
  const loadEnvConfig = nextEnv.loadEnvConfig || nextEnv.default.loadEnvConfig;
  loadEnvConfig(cwd, development, {
    info() {},
    error() {
      failed = true;
    },
  });
  if (failed) throw new Error('Store environment files could not be loaded');
}
