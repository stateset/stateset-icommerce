'use client';

import { useAccount } from 'wagmi';
import { useCustomer } from '@/contexts/CustomerContext';
import Link from 'next/link';

export default function AccountDashboard() {
  const { address } = useAccount();
  const { customer, orders, subscriptions } = useCustomer();

  const totalOrders = orders.length;
  const totalSpent = orders.reduce((sum: number, order: any) => {
    return sum + (order.totalAmount || order.grandTotal || order.subtotal || 0);
  }, 0);
  const activeSubscriptions = subscriptions.filter(
    (sub: any) => sub.status === 'active' || sub.status === 'trialing'
  ).length;

  return (
    <div>
      <h2 className="text-2xl font-bold mb-6">Dashboard</h2>
      <div className="bg-white border rounded-lg p-6 mb-6">
        <h3 className="font-semibold mb-4">Account Information</h3>
        <div className="space-y-3">
          {customer ? (
            <>
              <div className="flex justify-between">
                <span className="text-gray-600">Email</span>
                <span className="font-medium">{customer.email}</span>
              </div>
              {customer.firstName && (
                <div className="flex justify-between">
                  <span className="text-gray-600">Name</span>
                  <span className="font-medium">
                    {customer.firstName} {customer.lastName}
                  </span>
                </div>
              )}
            </>
          ) : (
            <p className="text-gray-500">
              No account found. Place an order to create your account.
            </p>
          )}
          <div className="flex justify-between">
            <span className="text-gray-600">Wallet</span>
            <span className="font-mono text-sm">
              {address?.slice(0, 6)}...{address?.slice(-4)}
            </span>
          </div>
        </div>
      </div>
      <div className="grid grid-cols-3 gap-4 mb-6">
        <div className="bg-white border rounded-lg p-6 text-center">
          <p className="text-3xl font-bold">{totalOrders}</p>
          <p className="text-gray-600">Total Orders</p>
        </div>
        <div className="bg-white border rounded-lg p-6 text-center">
          <p className="text-3xl font-bold">${totalSpent.toFixed(2)}</p>
          <p className="text-gray-600">Total Spent</p>
        </div>
        <div className="bg-white border rounded-lg p-6 text-center">
          <p className="text-3xl font-bold">{activeSubscriptions}</p>
          <p className="text-gray-600">Active Subscriptions</p>
        </div>
      </div>
      <div className="bg-white border rounded-lg p-6">
        <div className="flex justify-between items-center mb-4">
          <h3 className="font-semibold">Recent Orders</h3>
          <Link href="/account/orders" className="text-blue-600 hover:underline text-sm">
            View All
          </Link>
        </div>
        {orders.length === 0 ? (
          <p className="text-gray-500 text-center py-4">
            No orders yet.{' '}
            <Link href="/products" className="text-blue-600 hover:underline">
              Start shopping
            </Link>
          </p>
        ) : (
          <div className="space-y-3">
            {orders.slice(0, 3).map((order: any) => (
              <Link
                key={order.id}
                href={`/account/orders/${order.id}`}
                className="block p-3 bg-gray-50 rounded-lg hover:bg-gray-100 transition-colors"
              >
                <div className="flex justify-between items-center">
                  <div>
                    <p className="font-medium">
                      Order #{order.orderNumber || order.id.slice(0, 8)}
                    </p>
                    <p className="text-sm text-gray-500">
                      {order.createdAt
                        ? new Date(order.createdAt).toLocaleDateString()
                        : 'Date unknown'}
                    </p>
                  </div>
                  <div className="text-right">
                    <p className="font-medium">
                      ${(order.totalAmount || order.grandTotal || order.subtotal || 0).toFixed(2)}
                    </p>
                    <span
                      className={`inline-block px-2 py-0.5 text-xs rounded-full ${
                        order.status === 'completed' || order.status === 'delivered'
                          ? 'bg-green-100 text-green-800'
                          : order.status === 'cancelled'
                          ? 'bg-red-100 text-red-800'
                          : 'bg-blue-100 text-blue-800'
                      }`}
                    >
                      {order.status || 'pending'}
                    </span>
                  </div>
                </div>
              </Link>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
