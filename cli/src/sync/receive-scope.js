/** Resolve scope only from operator configuration, never an incoming event. */
export function resolveReceiveScope(config) {
  const tenantId = config?.tenantId ?? config?.identity?.tenantId;
  const storeId = config?.storeId ?? config?.identity?.storeId;
  if (
    typeof tenantId !== 'string' ||
    !tenantId.trim() ||
    typeof storeId !== 'string' ||
    !storeId.trim()
  ) {
    throw new Error('Sync delivery requires configured tenantId and storeId');
  }
  return { tenantId, storeId };
}

export function matchesReceiveScope(event, scope) {
  return event.tenantId === scope.tenantId && event.storeId === scope.storeId;
}

/** Check the whole batch before issuing any network request. */
export function assertOutgoingScope(events, config) {
  const scope = resolveReceiveScope(config);
  if (events.some((event) => !matchesReceiveScope(event, scope))) {
    throw new Error('Outgoing event tenant/store does not match the configured destination');
  }
}
