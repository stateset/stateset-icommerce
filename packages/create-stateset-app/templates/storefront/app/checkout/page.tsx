'use client';

import { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';
import { useAccount } from 'wagmi';
import { formatUnits, parseUnits } from 'viem';
import { useCart } from '@/hooks/useCart';
import { useUSDCPayment } from '@/hooks/useUSDCPayment';
import { ConnectWallet } from '@/components/commerce/ConnectWallet';
import { activeChain, USDC_DECIMALS, getExplorerTxUrl } from '@/lib/wagmi';
import { addDecimals } from '@/lib/money.js';
import Link from 'next/link';

interface ShippingMethod {
  id: string;
  label: string;
  amount: string;
  carrier: string;
  estimatedDays?: string;
}

const US_STATES = [
  { code: 'AL', name: 'Alabama' },
  { code: 'AK', name: 'Alaska' },
  { code: 'AZ', name: 'Arizona' },
  { code: 'AR', name: 'Arkansas' },
  { code: 'CA', name: 'California' },
  { code: 'CO', name: 'Colorado' },
  { code: 'CT', name: 'Connecticut' },
  { code: 'DE', name: 'Delaware' },
  { code: 'FL', name: 'Florida' },
  { code: 'GA', name: 'Georgia' },
  { code: 'HI', name: 'Hawaii' },
  { code: 'ID', name: 'Idaho' },
  { code: 'IL', name: 'Illinois' },
  { code: 'IN', name: 'Indiana' },
  { code: 'IA', name: 'Iowa' },
  { code: 'KS', name: 'Kansas' },
  { code: 'KY', name: 'Kentucky' },
  { code: 'LA', name: 'Louisiana' },
  { code: 'ME', name: 'Maine' },
  { code: 'MD', name: 'Maryland' },
  { code: 'MA', name: 'Massachusetts' },
  { code: 'MI', name: 'Michigan' },
  { code: 'MN', name: 'Minnesota' },
  { code: 'MS', name: 'Mississippi' },
  { code: 'MO', name: 'Missouri' },
  { code: 'MT', name: 'Montana' },
  { code: 'NE', name: 'Nebraska' },
  { code: 'NV', name: 'Nevada' },
  { code: 'NH', name: 'New Hampshire' },
  { code: 'NJ', name: 'New Jersey' },
  { code: 'NM', name: 'New Mexico' },
  { code: 'NY', name: 'New York' },
  { code: 'NC', name: 'North Carolina' },
  { code: 'ND', name: 'North Dakota' },
  { code: 'OH', name: 'Ohio' },
  { code: 'OK', name: 'Oklahoma' },
  { code: 'OR', name: 'Oregon' },
  { code: 'PA', name: 'Pennsylvania' },
  { code: 'RI', name: 'Rhode Island' },
  { code: 'SC', name: 'South Carolina' },
  { code: 'SD', name: 'South Dakota' },
  { code: 'TN', name: 'Tennessee' },
  { code: 'TX', name: 'Texas' },
  { code: 'UT', name: 'Utah' },
  { code: 'VT', name: 'Vermont' },
  { code: 'VA', name: 'Virginia' },
  { code: 'WA', name: 'Washington' },
  { code: 'WV', name: 'West Virginia' },
  { code: 'WI', name: 'Wisconsin' },
  { code: 'WY', name: 'Wyoming' },
  { code: 'DC', name: 'District of Columbia' },
];

export default function CheckoutPage() {
  const router = useRouter();
  const { cart, isLoading: cartLoading, clearCart, shippingState, setShippingState } = useCart();
  const { address, isConnected, chain } = useAccount();
  const { paymentStatus, txHash, error, usdcBalance, sendPayment, reset } = useUSDCPayment();

  const [email, setEmail] = useState('');
  const [shippingAddress, setShippingAddress] = useState({
    firstName: '',
    lastName: '',
    line1: '',
    line2: '',
    city: '',
    postalCode: '',
    country: 'US',
  });
  const [isProcessing, setIsProcessing] = useState(false);
  const [orderError, setOrderError] = useState<string | null>(null);
  const [shippingMethods, setShippingMethods] = useState<ShippingMethod[]>([]);
  const [shippingMethodId, setShippingMethodId] = useState('');
  const [shippingError, setShippingError] = useState<string | null>(null);
  const [shippingLoading, setShippingLoading] = useState(false);

  const formattedBalance =
    usdcBalance !== undefined ? Number(formatUnits(usdcBalance, USDC_DECIMALS)).toFixed(2) : '0.00';

  const isWrongNetwork = isConnected && chain?.id !== activeChain.id;

  useEffect(() => {
    if (paymentStatus === 'success' && txHash) {
      createOrder(txHash);
    }
  }, [paymentStatus, txHash]);

  useEffect(() => {
    const addressReady =
      shippingAddress.firstName.trim() &&
      shippingAddress.lastName.trim() &&
      shippingAddress.line1.trim() &&
      shippingAddress.city.trim() &&
      shippingAddress.postalCode.trim() &&
      shippingState;
    if (!addressReady) {
      setShippingMethods([]);
      setShippingMethodId('');
      setShippingError(null);
      return;
    }
    const controller = new AbortController();
    const timer = setTimeout(async () => {
      setShippingLoading(true);
      setShippingError(null);
      try {
        const response = await fetch('/api/shipping', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          signal: controller.signal,
          body: JSON.stringify({
            shippingAddress: { ...shippingAddress, state: shippingState },
            shippingMethodId: shippingMethodId || undefined,
          }),
        });
        const data = await response.json();
        if (!response.ok) throw new Error(data.error || 'Shipping quote failed');
        setShippingMethods(data.methods);
        setShippingMethodId(data.selected.id);
      } catch (quoteError) {
        if (!controller.signal.aborted) {
          setShippingMethods([]);
          setShippingMethodId('');
          setShippingError(
            quoteError instanceof Error ? quoteError.message : 'Shipping quote failed',
          );
        }
      } finally {
        if (!controller.signal.aborted) setShippingLoading(false);
      }
    }, 400);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [
    shippingAddress.firstName,
    shippingAddress.lastName,
    shippingAddress.line1,
    shippingAddress.line2,
    shippingAddress.city,
    shippingAddress.postalCode,
    shippingAddress.country,
    shippingState,
    shippingMethodId,
  ]);

  const createOrder = async (transactionHash: string) => {
    setIsProcessing(true);
    setOrderError(null);
    try {
      const response = await fetch('/api/checkout', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          cartId: cart?.id,
          email,
          txHash: transactionHash,
          walletAddress: address,
          shippingAddress: { ...shippingAddress, state: shippingState },
          shippingMethodId,
        }),
      });

      const data = await response.json();

      if (response.ok && data.orderId) {
        clearCart();
        const params = new URLSearchParams({
          orderId: data.orderId,
          txHash: transactionHash,
          ...(data.orderNumber && { orderNumber: data.orderNumber }),
        });
        router.push(`/checkout/confirmation?${params.toString()}`);
      } else {
        setOrderError(data.error || 'Order finalization failed');
      }
    } catch (err) {
      setOrderError(err instanceof Error ? err.message : 'Order finalization failed');
    } finally {
      setIsProcessing(false);
    }
  };

  const handlePayment = async () => {
    if (!cart || !email) return;
    const shippingAmount =
      shippingMethods.find((method) => method.id === shippingMethodId)?.amount || '0';
    await sendPayment(addDecimals([cart.totalExact, shippingAmount]));
  };

  if (cartLoading) {
    return (
      <div className="container mx-auto px-4 py-8">
        <h1 className="text-3xl font-bold mb-8">Checkout</h1>
        <p className="text-gray-600">Loading...</p>
      </div>
    );
  }

  if (!cart || cart.items.length === 0) {
    return (
      <div className="container mx-auto px-4 py-8">
        <h1 className="text-3xl font-bold mb-8">Checkout</h1>
        <p className="text-gray-600 mb-4">Your cart is empty.</p>
        <Link href="/products" className="text-blue-600 hover:underline">
          Continue Shopping
        </Link>
      </div>
    );
  }

  const shippingComplete =
    shippingAddress.firstName.trim() &&
    shippingAddress.lastName.trim() &&
    shippingAddress.line1.trim() &&
    shippingAddress.city.trim() &&
    shippingAddress.postalCode.trim() &&
    shippingState;
  const selectedShippingMethod = shippingMethods.find((method) => method.id === shippingMethodId);
  const shippingAmountExact = selectedShippingMethod?.amount || '0';
  const checkoutTotalExact = addDecimals([cart.totalExact, shippingAmountExact]);
  const canPay =
    isConnected &&
    !isWrongNetwork &&
    email &&
    shippingComplete &&
    selectedShippingMethod &&
    !shippingLoading &&
    !shippingError &&
    cart.taxConfigured &&
    paymentStatus === 'idle';
  const hasEnoughBalance =
    usdcBalance !== undefined && usdcBalance >= parseUnits(checkoutTotalExact, USDC_DECIMALS);

  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">Checkout</h1>

      <div className="grid lg:grid-cols-2 gap-8">
        <div className="order-2 lg:order-1">
          <div className="border rounded-lg p-6">
            <h2 className="text-xl font-bold mb-4">Order Summary</h2>
            <div className="space-y-4 mb-6">
              {cart.items.map((item) => (
                <div key={item.id} className="flex items-center gap-3">
                  <div className="w-16 h-16 bg-gray-100 rounded-lg flex-shrink-0" />
                  <div className="flex-grow">
                    <p className="font-medium">{item.name || item.sku}</p>
                    <p className="text-sm text-gray-500">Qty: {item.quantity}</p>
                  </div>
                  <p className="font-medium">${(item.price * item.quantity).toFixed(2)}</p>
                </div>
              ))}
            </div>
            <div className="border-t pt-4 space-y-2">
              <div className="flex justify-between">
                <span>Subtotal</span>
                <span>${cart.subtotal.toFixed(2)}</span>
              </div>
              <div className="flex justify-between text-gray-500">
                <span>Shipping</span>
                <span>
                  {selectedShippingMethod
                    ? Number(shippingAmountExact) === 0
                      ? 'Free'
                      : `$${Number(shippingAmountExact).toFixed(2)}`
                    : 'Enter address'}
                </span>
              </div>
              <div className="flex justify-between text-gray-500">
                <span>
                  Tax
                  {cart.taxRate > 0 && (
                    <span className="text-xs ml-1">({(cart.taxRate * 100).toFixed(2)}%)</span>
                  )}
                </span>
                <span>
                  {cart.tax > 0
                    ? `$${cart.tax.toFixed(2)}`
                    : shippingState
                      ? '$0.00'
                      : 'Select state'}
                </span>
              </div>
              <div className="flex justify-between font-bold text-lg pt-2 border-t">
                <span>Total</span>
                <span>${Number(checkoutTotalExact).toFixed(2)} USDC</span>
              </div>
            </div>
          </div>
        </div>

        <div className="order-1 lg:order-2">
          <div className="border rounded-lg p-6 space-y-6">
            <h2 className="text-xl font-bold">Shipping &amp; Payment</h2>
            <div>
              <label htmlFor="email" className="block text-sm font-medium mb-1">
                Email for receipt
              </label>
              <input
                type="email"
                id="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="your@email.com"
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                required
              />
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div>
                <label htmlFor="firstName" className="block text-sm font-medium mb-1">
                  First name
                </label>
                <input
                  id="firstName"
                  autoComplete="shipping given-name"
                  value={shippingAddress.firstName}
                  onChange={(event) =>
                    setShippingAddress((current) => ({
                      ...current,
                      firstName: event.target.value,
                    }))
                  }
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                  required
                />
              </div>
              <div>
                <label htmlFor="lastName" className="block text-sm font-medium mb-1">
                  Last name
                </label>
                <input
                  id="lastName"
                  autoComplete="shipping family-name"
                  value={shippingAddress.lastName}
                  onChange={(event) =>
                    setShippingAddress((current) => ({
                      ...current,
                      lastName: event.target.value,
                    }))
                  }
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                  required
                />
              </div>
            </div>
            <div>
              <label htmlFor="addressLine1" className="block text-sm font-medium mb-1">
                Street address
              </label>
              <input
                id="addressLine1"
                autoComplete="shipping address-line1"
                value={shippingAddress.line1}
                onChange={(event) =>
                  setShippingAddress((current) => ({ ...current, line1: event.target.value }))
                }
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                required
              />
            </div>
            <div>
              <label htmlFor="addressLine2" className="block text-sm font-medium mb-1">
                Apartment, suite, etc. <span className="text-gray-400">(optional)</span>
              </label>
              <input
                id="addressLine2"
                autoComplete="shipping address-line2"
                value={shippingAddress.line2}
                onChange={(event) =>
                  setShippingAddress((current) => ({ ...current, line2: event.target.value }))
                }
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
              />
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div>
                <label htmlFor="city" className="block text-sm font-medium mb-1">
                  City
                </label>
                <input
                  id="city"
                  autoComplete="shipping address-level2"
                  value={shippingAddress.city}
                  onChange={(event) =>
                    setShippingAddress((current) => ({ ...current, city: event.target.value }))
                  }
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                  required
                />
              </div>
              <div>
                <label htmlFor="postalCode" className="block text-sm font-medium mb-1">
                  ZIP code
                </label>
                <input
                  id="postalCode"
                  autoComplete="shipping postal-code"
                  value={shippingAddress.postalCode}
                  onChange={(event) =>
                    setShippingAddress((current) => ({
                      ...current,
                      postalCode: event.target.value,
                    }))
                  }
                  className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500"
                  required
                />
              </div>
            </div>
            <div>
              <label htmlFor="shippingState" className="block text-sm font-medium mb-1">
                Shipping State (for tax calculation)
              </label>
              <select
                id="shippingState"
                value={shippingState || ''}
                onChange={(e) => setShippingState(e.target.value)}
                autoComplete="shipping address-level1"
                className="w-full px-3 py-2 border rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-white"
                required
              >
                <option value="">Select a state...</option>
                {US_STATES.map((state) => (
                  <option key={state.code} value={state.code}>
                    {state.name}
                  </option>
                ))}
              </select>
              {!shippingState && (
                <p className="text-xs text-amber-600 mt-1">
                  Please select a state to calculate applicable taxes
                </p>
              )}
              {shippingState && !cart.taxConfigured && (
                <p className="text-xs text-red-600 mt-1" role="alert">
                  Tax is not configured for this state. The store operator must configure a tax
                  service before checkout can continue.
                </p>
              )}
            </div>
            <div>
              <label htmlFor="country" className="block text-sm font-medium mb-1">
                Country
              </label>
              <input
                id="country"
                value="United States"
                className="w-full px-3 py-2 border rounded-lg bg-gray-50 text-gray-600"
                readOnly
              />
            </div>
            <div>
              <h3 className="text-sm font-medium mb-2">Shipping method</h3>
              {shippingLoading && <p className="text-sm text-gray-500">Loading rates...</p>}
              {shippingError && (
                <p className="text-sm text-red-600" role="alert">
                  {shippingError}
                </p>
              )}
              {!shippingLoading && !shippingError && shippingMethods.length === 0 && (
                <p className="text-sm text-gray-500">Enter a complete address to view rates.</p>
              )}
              <div className="space-y-2">
                {shippingMethods.map((method) => (
                  <label
                    key={method.id}
                    className="flex items-center justify-between gap-3 border rounded-lg p-3 cursor-pointer"
                  >
                    <span className="flex items-center gap-3">
                      <input
                        type="radio"
                        name="shippingMethod"
                        value={method.id}
                        checked={shippingMethodId === method.id}
                        onChange={() => setShippingMethodId(method.id)}
                      />
                      <span>
                        <span className="block text-sm font-medium">{method.label}</span>
                        {method.estimatedDays && (
                          <span className="block text-xs text-gray-500">
                            {method.carrier} · {method.estimatedDays}
                          </span>
                        )}
                      </span>
                    </span>
                    <span className="text-sm font-medium">
                      {Number(method.amount) === 0
                        ? 'Free'
                        : `$${Number(method.amount).toFixed(2)}`}
                    </span>
                  </label>
                ))}
              </div>
            </div>
            <div>
              <h3 className="text-sm font-medium mb-2">Connect Wallet</h3>
              <ConnectWallet />
            </div>
            {isConnected && !isWrongNetwork && (
              <div className="p-3 bg-gray-50 rounded-lg">
                <div className="flex justify-between text-sm">
                  <span className="text-gray-600">Your USDC Balance</span>
                  <span className={hasEnoughBalance ? 'text-green-600' : 'text-red-600'}>
                    ${formattedBalance}
                  </span>
                </div>
                {!hasEnoughBalance && (
                  <p className="text-sm text-red-600 mt-1">
                    Insufficient balance. Need ${Number(checkoutTotalExact).toFixed(2)} USDC.
                  </p>
                )}
              </div>
            )}
            {paymentStatus !== 'idle' && (
              <div
                className={`p-4 rounded-lg ${
                  paymentStatus === 'success'
                    ? 'bg-green-50 text-green-800'
                    : paymentStatus === 'error'
                      ? 'bg-red-50 text-red-800'
                      : 'bg-blue-50 text-blue-800'
                }`}
              >
                {paymentStatus === 'confirming' && (
                  <p>Please confirm the transaction in your wallet...</p>
                )}
                {paymentStatus === 'pending' && (
                  <div>
                    <p className="mb-2">Transaction submitted! Waiting for confirmation...</p>
                    {txHash && (
                      <a
                        href={getExplorerTxUrl(txHash)}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-sm underline"
                      >
                        View on BaseScan
                      </a>
                    )}
                  </div>
                )}
                {paymentStatus === 'success' && !orderError && (
                  <p>Payment confirmed! Creating your order...</p>
                )}
                {paymentStatus === 'error' && (
                  <div>
                    <p className="mb-2">{error || 'Payment failed'}</p>
                    <button onClick={reset} className="text-sm underline">
                      Try again
                    </button>
                  </div>
                )}
              </div>
            )}
            {paymentStatus === 'success' && orderError && txHash && (
              <div className="p-4 rounded-lg bg-red-50 text-red-800" role="alert">
                <p className="font-medium">Payment confirmed, but order finalization paused.</p>
                <p className="text-sm mt-1">{orderError}</p>
                <button
                  type="button"
                  onClick={() => void createOrder(txHash)}
                  disabled={isProcessing}
                  className="mt-3 text-sm underline disabled:opacity-50"
                >
                  Retry order finalization
                </button>
              </div>
            )}
            <button
              onClick={handlePayment}
              disabled={!canPay || !hasEnoughBalance || isProcessing}
              className={`w-full py-3 px-4 rounded-lg font-medium transition-colors ${
                canPay && hasEnoughBalance && !isProcessing
                  ? 'bg-blue-600 text-white hover:bg-blue-700'
                  : 'bg-gray-300 text-gray-500 cursor-not-allowed'
              }`}
            >
              {isProcessing
                ? 'Processing...'
                : paymentStatus === 'confirming'
                  ? 'Confirm in Wallet...'
                  : paymentStatus === 'pending'
                    ? 'Waiting for Confirmation...'
                    : `Pay ${Number(checkoutTotalExact).toFixed(2)} USDC`}
            </button>
            <p className="text-xs text-gray-500 text-center">
              Payment on {activeChain.name} network
            </p>
          </div>
          <Link href="/cart" className="block text-center mt-4 text-gray-600 hover:text-gray-800">
            Back to Cart
          </Link>
        </div>
      </div>
    </div>
  );
}
