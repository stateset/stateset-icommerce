'use client';

import { useState } from 'react';
import { useCart } from '@/hooks/useCart';
import { useAccount } from 'wagmi';
import { useCustomer } from '@/contexts/CustomerContext';

interface Props {
  sku: string;
  productName: string;
  price: number;
  isSubscription?: boolean;
}

export function AddToCartButton({ sku, productName, price, isSubscription = false }: Props) {
  const { addItem } = useCart();
  const { isConnected } = useAccount();
  const { customer } = useCustomer();
  const [isLoading, setIsLoading] = useState(false);
  const [success, setSuccess] = useState(false);

  const handleClick = async () => {
    if (isSubscription) {
      if (!isConnected) {
        alert('Please connect your wallet to subscribe.');
        return;
      }
      if (!customer) {
        alert('Please place an order first to create your account, then you can subscribe.');
        return;
      }
      setIsLoading(true);
      try {
        const discountedPrice = price * 0.9;
        const response = await fetch('/api/subscriptions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            customerId: customer.id,
            sku,
            productName,
            price: discountedPrice,
          }),
        });
        if (response.ok) {
          setSuccess(true);
          setTimeout(() => setSuccess(false), 3000);
        } else {
          const data = await response.json();
          alert(data.error || 'Failed to create subscription');
        }
      } catch {
        alert('Failed to create subscription');
      } finally {
        setIsLoading(false);
      }
      return;
    }

    setIsLoading(true);
    try {
      await addItem(sku, productName, price);
      setSuccess(true);
      setTimeout(() => setSuccess(false), 2000);
    } catch {
      alert('Failed to add to cart');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <button
      onClick={handleClick}
      disabled={isLoading}
      className={`w-full py-3 px-4 rounded-lg font-medium transition-colors ${
        success
          ? 'bg-green-600 text-white'
          : 'bg-black text-white hover:bg-gray-800'
      } disabled:opacity-50`}
    >
      {isLoading
        ? 'Adding...'
        : success
        ? isSubscription ? 'Subscribed!' : 'Added to Cart!'
        : isSubscription ? 'Subscribe' : 'Add to Cart'}
    </button>
  );
}
