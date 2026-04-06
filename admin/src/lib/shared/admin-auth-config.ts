export const ADMIN_AUTH_DISABLE_FLAG = 'STATESET_ADMIN_DISABLE_AUTH';

function isTruthyFlag(value: string | undefined): boolean {
  return typeof value === 'string' && /^(1|true|yes|on)$/i.test(value.trim());
}

export function isProductionRuntime(): boolean {
  return process.env.NODE_ENV === 'production';
}

export function isAdminAuthDisabled(): boolean {
  return isTruthyFlag(process.env[ADMIN_AUTH_DISABLE_FLAG]) && !isProductionRuntime();
}

export function getBypassAdminUser() {
  return {
    id: 'stateset-admin-local',
    email: 'local@stateset.dev',
    role: 'admin',
    authMode: 'disabled' as const,
  };
}
