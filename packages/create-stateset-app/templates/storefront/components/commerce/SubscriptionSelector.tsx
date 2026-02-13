'use client';

interface Props {
  price: number;
  isSubscription: boolean;
  onChange: (isSubscription: boolean) => void;
  discountPercent?: number;
}

export function SubscriptionSelector({
  price,
  isSubscription,
  onChange,
  discountPercent = 10,
}: Props) {
  const discountedPrice = price * (1 - discountPercent / 100);
  const savings = price - discountedPrice;

  return (
    <div className="space-y-2">
      <button
        onClick={() => onChange(false)}
        className={`w-full p-3 border rounded-lg text-left flex items-center gap-3 transition-colors ${
          !isSubscription ? 'border-black bg-gray-50' : 'border-gray-200 hover:border-gray-300'
        }`}
      >
        <div className={`w-4 h-4 rounded-full border-2 flex items-center justify-center ${
          !isSubscription ? 'border-black' : 'border-gray-300'
        }`}>
          {!isSubscription && <div className="w-2 h-2 rounded-full bg-black" />}
        </div>
        <div>
          <p className="font-medium">One-time purchase</p>
          <p className="text-sm text-gray-600">${price.toFixed(2)}</p>
        </div>
      </button>

      <button
        onClick={() => onChange(true)}
        className={`w-full p-3 border rounded-lg text-left flex items-center gap-3 transition-colors ${
          isSubscription ? 'border-black bg-gray-50' : 'border-gray-200 hover:border-gray-300'
        }`}
      >
        <div className={`w-4 h-4 rounded-full border-2 flex items-center justify-center ${
          isSubscription ? 'border-black' : 'border-gray-300'
        }`}>
          {isSubscription && <div className="w-2 h-2 rounded-full bg-black" />}
        </div>
        <div>
          <p className="font-medium">Subscribe & Save {discountPercent}%</p>
          <p className="text-sm text-gray-600">
            ${discountedPrice.toFixed(2)}/month — Save ${savings.toFixed(2)}
          </p>
        </div>
      </button>
    </div>
  );
}
