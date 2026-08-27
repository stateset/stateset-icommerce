'use client';

import { useEffect, useState, use } from 'react';
import { useAccount } from 'wagmi';
import Link from 'next/link';
import { getExplorerTxUrl, activeChain } from '@/lib/wagmi';
import { useCustomer } from '@/contexts/CustomerContext';

interface OrderItem {
  id: string;
  sku: string;
  name: string;
  quantity: number;
  unitPrice: number;
}

interface Order {
  id: string;
  orderNumber?: string;
  status: string;
  subtotal?: number;
  grandTotal?: number;
  currency?: string;
  notes?: string;
  createdAt?: string;
}

export default function OrderDetailPage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = use(params);
  const { address } = useAccount();
  const { authenticatedFetch } = useCustomer();
  const [order, setOrder] = useState<Order | null>(null);
  const [items, setItems] = useState<OrderItem[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const txHash = order?.notes?.match(/Verified Base settlement: (0x[a-fA-F0-9]{64})/)?.[1];

  useEffect(() => {
    async function fetchOrder() {
      if (!id) return;
      setIsLoading(true);
      try {
        const url = address ? `/api/orders/${id}?wallet=${address}` : `/api/orders/${id}`;
        const response = await authenticatedFetch(url);
        const data = await response.json();
        if (response.ok) {
          setOrder(data.order);
          setItems(data.items || []);
        } else {
          setError(data.error || 'Order not found');
        }
      } catch {
        setError('Failed to load order');
      } finally {
        setIsLoading(false);
      }
    }
    fetchOrder();
  }, [id, address, authenticatedFetch]);

  if (isLoading) {
    return (
      <div>
        <h2 className="text-2xl font-bold mb-6">Order Details</h2>
        <p className="text-gray-600">Loading order...</p>
      </div>
    );
  }

  if (error || !order) {
    return (
      <div>
        <h2 className="text-2xl font-bold mb-6">Order Details</h2>
        <div className="bg-red-50 border border-red-200 rounded-lg p-6 text-center">
          <p className="text-red-800 mb-4">{error || 'Order not found'}</p>
          <Link href="/account/orders" className="text-blue-600 hover:underline">
            Back to Order History
          </Link>
        </div>
      </div>
    );
  }

  const total = (order as any).totalAmount || order.grandTotal || order.subtotal || 0;

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-bold">Order #{order.orderNumber || order.id.slice(0, 8)}</h2>
        <span
          className={`px-3 py-1 text-sm rounded-full ${
            order.status === 'completed' || order.status === 'delivered'
              ? 'bg-green-100 text-green-800'
              : order.status === 'cancelled'
                ? 'bg-red-100 text-red-800'
                : 'bg-blue-100 text-blue-800'
          }`}
        >
          {order.status ? order.status.charAt(0).toUpperCase() + order.status.slice(1) : 'Pending'}
        </span>
      </div>

      <div className="bg-white border rounded-lg p-6 mb-6">
        <h3 className="font-semibold mb-4">Order Information</h3>
        <div className="grid md:grid-cols-2 gap-4">
          <div>
            <p className="text-gray-600 text-sm">Order Date</p>
            <p className="font-medium">
              {order.createdAt
                ? new Date(order.createdAt).toLocaleDateString('en-US', {
                    year: 'numeric',
                    month: 'long',
                    day: 'numeric',
                  })
                : 'Unknown'}
            </p>
          </div>
          <div>
            <p className="text-gray-600 text-sm">Payment Method</p>
            <p className="font-medium">USDC on {activeChain.name}</p>
          </div>
        </div>
      </div>

      <div className="bg-white border rounded-lg p-6 mb-6">
        <h3 className="font-semibold mb-4">Items</h3>
        <div className="space-y-3">
          {items.length > 0 ? (
            items.map((item, index) => (
              <div
                key={item.id || index}
                className="flex justify-between items-center py-2 border-b last:border-0"
              >
                <div>
                  <p className="font-medium">{item.name || item.sku}</p>
                  <p className="text-sm text-gray-500">
                    Qty: {item.quantity} @ ${item.unitPrice.toFixed(2)}
                  </p>
                </div>
                <p className="font-medium">${(item.quantity * item.unitPrice).toFixed(2)}</p>
              </div>
            ))
          ) : (
            <p className="text-gray-500">No item details available</p>
          )}
        </div>
        <div className="border-t mt-4 pt-4">
          <div className="flex justify-between font-bold text-lg">
            <span>Total</span>
            <span>
              ${total.toFixed(2)} {order.currency || 'USDC'}
            </span>
          </div>
        </div>
      </div>

      {txHash && (
        <div className="bg-white border rounded-lg p-6 mb-6">
          <h3 className="font-semibold mb-4">Transaction</h3>
          <div className="flex items-center gap-2">
            <code className="text-sm bg-gray-100 px-2 py-1 rounded">
              {txHash.slice(0, 10)}...{txHash.slice(-8)}
            </code>
            <a
              href={getExplorerTxUrl(txHash)}
              target="_blank"
              rel="noopener noreferrer"
              className="text-blue-600 hover:underline text-sm"
            >
              View on BaseScan
            </a>
          </div>
        </div>
      )}

      <Link href="/account/orders" className="inline-block text-gray-600 hover:text-gray-800">
        &larr; Back to Order History
      </Link>
    </div>
  );
}
