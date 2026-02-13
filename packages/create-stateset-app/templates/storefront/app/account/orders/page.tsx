'use client';

import { useCustomer } from '@/contexts/CustomerContext';
import Link from 'next/link';

export default function OrderHistoryPage() {
  const { orders, isLoading } = useCustomer();

  if (isLoading) {
    return (
      <div>
        <h2 className="text-2xl font-bold mb-6">Order History</h2>
        <p className="text-gray-600">Loading orders...</p>
      </div>
    );
  }

  return (
    <div>
      <h2 className="text-2xl font-bold mb-6">Order History</h2>
      {orders.length === 0 ? (
        <div className="bg-white border rounded-lg p-8 text-center">
          <p className="text-gray-500 mb-4">You haven&apos;t placed any orders yet.</p>
          <Link
            href="/products"
            className="inline-block px-6 py-2 bg-black text-white rounded-lg hover:bg-gray-800"
          >
            Start Shopping
          </Link>
        </div>
      ) : (
        <div className="space-y-4">
          {orders.map((order: any) => (
            <Link
              key={order.id}
              href={`/account/orders/${order.id}`}
              className="block bg-white border rounded-lg p-6 hover:shadow-md transition-shadow"
            >
              <div className="flex flex-col md:flex-row md:justify-between md:items-start gap-4">
                <div>
                  <h3 className="font-semibold text-lg">
                    Order #{order.orderNumber || order.id.slice(0, 8)}
                  </h3>
                  <p className="text-gray-500 text-sm">
                    {order.createdAt
                      ? new Date(order.createdAt).toLocaleDateString('en-US', {
                          year: 'numeric',
                          month: 'long',
                          day: 'numeric',
                        })
                      : 'Date unknown'}
                  </p>
                </div>
                <div className="flex items-center gap-4">
                  <span
                    className={`px-3 py-1 text-sm rounded-full ${
                      order.status === 'completed' || order.status === 'delivered'
                        ? 'bg-green-100 text-green-800'
                        : order.status === 'cancelled'
                        ? 'bg-red-100 text-red-800'
                        : 'bg-blue-100 text-blue-800'
                    }`}
                  >
                    {order.status
                      ? order.status.charAt(0).toUpperCase() + order.status.slice(1)
                      : 'Pending'}
                  </span>
                  <p className="font-semibold text-lg">
                    ${(order.totalAmount || order.grandTotal || order.subtotal || 0).toFixed(2)}
                  </p>
                </div>
              </div>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
