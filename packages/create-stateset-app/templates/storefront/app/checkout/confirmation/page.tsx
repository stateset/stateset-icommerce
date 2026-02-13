'use client';

import { useSearchParams } from 'next/navigation';
import Link from 'next/link';
import { getExplorerTxUrl, activeChain } from '@/lib/wagmi';
import { Suspense } from 'react';

function ConfirmationContent() {
  const searchParams = useSearchParams();
  const orderId = searchParams.get('orderId');
  const orderNumber = searchParams.get('orderNumber');
  const txHash = searchParams.get('txHash');

  const explorerUrl = txHash ? getExplorerTxUrl(txHash) : null;

  return (
    <div className="container mx-auto px-4 py-8">
      <div className="max-w-lg mx-auto text-center">
        <div className="w-20 h-20 bg-green-100 rounded-full flex items-center justify-center mx-auto mb-6">
          <svg
            className="w-10 h-10 text-green-600"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M5 13l4 4L19 7"
            />
          </svg>
        </div>

        <h1 className="text-3xl font-bold mb-2">Order Confirmed!</h1>
        {orderNumber && (
          <p className="text-xl text-gray-700 mb-4">Order #{orderNumber}</p>
        )}
        <p className="text-gray-600 mb-8">
          Thank you for your purchase. Your USDC payment has been received and your order is being processed.
        </p>

        <div className="border rounded-lg p-6 text-left mb-8">
          <h2 className="font-semibold mb-4">Order Details</h2>
          {orderNumber && (
            <div className="flex justify-between py-2 border-b">
              <span className="text-gray-600">Order Number</span>
              <span className="font-semibold">{orderNumber}</span>
            </div>
          )}
          {txHash && (
            <div className="py-2 border-b">
              <div className="flex justify-between mb-1">
                <span className="text-gray-600">Transaction</span>
                <span className="font-mono text-sm">
                  {txHash.slice(0, 10)}...{txHash.slice(-8)}
                </span>
              </div>
              {explorerUrl && (
                <a
                  href={explorerUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-sm text-blue-600 hover:underline"
                >
                  View on {activeChain.name === 'Base' ? 'BaseScan' : 'Block Explorer'}
                </a>
              )}
            </div>
          )}
          <div className="flex justify-between py-2">
            <span className="text-gray-600">Payment Method</span>
            <span>USDC on {activeChain.name}</span>
          </div>
        </div>

        <div className="space-y-3">
          <Link
            href="/products"
            className="block w-full py-3 px-4 bg-black text-white rounded-lg hover:bg-gray-800 transition-colors"
          >
            Continue Shopping
          </Link>
          <Link
            href="/"
            className="block w-full py-3 px-4 border rounded-lg hover:bg-gray-50 transition-colors"
          >
            Return Home
          </Link>
        </div>
      </div>
    </div>
  );
}

export default function ConfirmationPage() {
  return (
    <Suspense fallback={<div className="container mx-auto px-4 py-8 text-center">Loading...</div>}>
      <ConfirmationContent />
    </Suspense>
  );
}
