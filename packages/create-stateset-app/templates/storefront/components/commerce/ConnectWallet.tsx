'use client';

import { useAccount, useConnect, useDisconnect, useSwitchChain } from 'wagmi';
import { activeChain } from '@/lib/wagmi';

export function ConnectWallet() {
  const { address, isConnected, chain } = useAccount();
  const { connectors, connect } = useConnect();
  const { disconnect } = useDisconnect();
  const { switchChain } = useSwitchChain();

  const isWrongNetwork = isConnected && chain?.id !== activeChain.id;

  if (isConnected) {
    return (
      <div className="space-y-2">
        <div className={`p-3 rounded-lg ${isWrongNetwork ? 'bg-red-50' : 'bg-gray-50'}`}>
          <div className="flex justify-between items-center">
            <div>
              <p className="text-sm font-medium">
                {address?.slice(0, 6)}...{address?.slice(-4)}
              </p>
              <p className={`text-xs ${isWrongNetwork ? 'text-red-600' : 'text-gray-500'}`}>
                {isWrongNetwork ? `Wrong network — switch to ${activeChain.name}` : chain?.name}
              </p>
            </div>
            <div className="flex gap-2">
              {isWrongNetwork && (
                <button
                  onClick={() => switchChain({ chainId: activeChain.id })}
                  className="text-xs px-3 py-1 bg-blue-600 text-white rounded hover:bg-blue-700"
                >
                  Switch
                </button>
              )}
              <button
                onClick={() => disconnect()}
                className="text-xs px-3 py-1 border rounded hover:bg-gray-100"
              >
                Disconnect
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      {connectors.map((connector) => (
        <button
          key={connector.uid}
          onClick={() => connect({ connector })}
          className="w-full py-2 px-4 border rounded-lg hover:bg-gray-50 transition-colors text-sm"
        >
          Connect {connector.name}
        </button>
      ))}
    </div>
  );
}
