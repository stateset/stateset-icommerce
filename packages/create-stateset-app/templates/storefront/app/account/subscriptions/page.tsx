'use client';

import { useCustomer } from '@/contexts/CustomerContext';
import Link from 'next/link';
import { useAccount } from 'wagmi';

export default function SubscriptionsPage() {
  const { subscriptions, isLoading, refreshSubscriptions, authenticatedFetch } = useCustomer();
  const { address } = useAccount();

  const handleCancel = async (subscriptionId: string) => {
    if (!confirm('Are you sure you want to cancel this subscription?')) return;
    try {
      const response = await authenticatedFetch(`/api/subscriptions/${subscriptionId}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action: 'cancel', walletAddress: address }),
      });
      if (response.ok) {
        refreshSubscriptions();
      }
    } catch {
      alert('Failed to cancel subscription');
    }
  };

  if (isLoading) {
    return (
      <div>
        <h2 className="text-2xl font-bold mb-6">Subscriptions</h2>
        <p className="text-gray-600">Loading subscriptions...</p>
      </div>
    );
  }

  return (
    <div>
      <h2 className="text-2xl font-bold mb-6">Subscriptions</h2>
      {subscriptions.length === 0 ? (
        <div className="bg-white border rounded-lg p-8 text-center">
          <p className="text-gray-500 mb-4">No active subscriptions.</p>
          <Link
            href="/products"
            className="inline-block px-6 py-2 bg-black text-white rounded-lg hover:bg-gray-800"
          >
            Browse Products
          </Link>
        </div>
      ) : (
        <div className="space-y-4">
          {subscriptions.map((sub: any) => (
            <div key={sub.id} className="bg-white border rounded-lg p-6">
              <div className="flex justify-between items-start">
                <div>
                  <h3 className="font-semibold text-lg">
                    {sub.planName || sub.sku || 'Subscription'}
                  </h3>
                  <p className="text-gray-500 text-sm mt-1">${(sub.price || 0).toFixed(2)}/month</p>
                </div>
                <span
                  className={`px-3 py-1 text-sm rounded-full ${
                    sub.status === 'active'
                      ? 'bg-green-100 text-green-800'
                      : sub.status === 'cancelled'
                        ? 'bg-red-100 text-red-800'
                        : 'bg-yellow-100 text-yellow-800'
                  }`}
                >
                  {sub.status}
                </span>
              </div>
              {sub.status === 'active' && (
                <button
                  onClick={() => handleCancel(sub.id)}
                  className="mt-4 text-sm text-red-600 hover:text-red-800"
                >
                  Cancel Subscription
                </button>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
