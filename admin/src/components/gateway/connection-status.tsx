'use client';

import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getGatewayHealth } from '@/lib/gateway-client';
import type { GatewayHealth } from '@/lib/types/gateway';

export function GatewayConnectionStatus() {
  const { data, error, isLoading } = useEmbeddedData<GatewayHealth>(
    getGatewayHealth,
    { refreshInterval: 15_000 }
  );

  const isConnected = !error && data?.status === 'ok';

  return (
    <div className="flex items-center space-x-2">
      <div
        className={`w-2 h-2 rounded-full ${
          isLoading
            ? 'bg-gray-400 animate-pulse'
            : isConnected
              ? 'bg-emerald-500 animate-pulse'
              : 'bg-red-500'
        }`}
      />
      <span className="text-xs font-medium text-gray-500 dark:text-gray-400">
        {isLoading
          ? 'Connecting...'
          : isConnected
            ? `Gateway v${data!.version}`
            : 'Gateway Offline'}
      </span>
    </div>
  );
}
