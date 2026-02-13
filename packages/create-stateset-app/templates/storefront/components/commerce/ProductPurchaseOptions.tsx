'use client';

import { useState } from 'react';
import { SubscriptionSelector } from './SubscriptionSelector';
import { AddToCartButton } from './AddToCartButton';

interface Props {
  sku: string;
  productName: string;
  price: number;
}

export function ProductPurchaseOptions({ sku, productName, price }: Props) {
  const [isSubscription, setIsSubscription] = useState(false);

  return (
    <div className="space-y-4">
      <p className="text-2xl font-bold">${price.toFixed(2)}</p>
      <SubscriptionSelector
        price={price}
        isSubscription={isSubscription}
        onChange={setIsSubscription}
      />
      <AddToCartButton
        sku={sku}
        productName={productName}
        price={price}
        isSubscription={isSubscription}
      />
    </div>
  );
}
