'use client';

import { useCart } from '@/hooks/useCart';
import Link from 'next/link';

export default function CartPage() {
  const { cart, isLoading, removeItem, updateQuantity, clearCart } = useCart();

  if (isLoading) {
    return (
      <div className="container mx-auto px-4 py-8">
        <h1 className="text-3xl font-bold mb-8">Shopping Cart</h1>
        <p className="text-gray-600">Loading cart...</p>
      </div>
    );
  }

  if (!cart || cart.items.length === 0) {
    return (
      <div className="container mx-auto px-4 py-8">
        <h1 className="text-3xl font-bold mb-8">Shopping Cart</h1>
        <p className="text-gray-600 mb-8">Your cart is empty.</p>
        <Link
          href="/products"
          className="bg-black text-white px-6 py-3 rounded-lg hover:bg-gray-800"
        >
          Continue Shopping
        </Link>
      </div>
    );
  }

  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-3xl font-bold mb-8">Shopping Cart</h1>

      <div className="grid lg:grid-cols-3 gap-8">
        <div className="lg:col-span-2">
          <div className="space-y-4">
            {cart.items.map((item) => (
              <div
                key={item.id}
                className="flex items-center gap-4 p-4 border rounded-lg"
              >
                <div className="w-20 h-20 bg-gray-100 rounded-lg flex-shrink-0" />
                <div className="flex-grow">
                  <h3 className="font-medium">{item.name || item.sku}</h3>
                  <p className="text-sm text-gray-500">SKU: {item.sku}</p>
                  <p className="font-semibold">${item.price?.toFixed(2)}</p>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => updateQuantity(item.id, item.quantity - 1)}
                    className="w-8 h-8 border rounded hover:bg-gray-100"
                  >
                    -
                  </button>
                  <span className="w-8 text-center">{item.quantity}</span>
                  <button
                    onClick={() => updateQuantity(item.id, item.quantity + 1)}
                    className="w-8 h-8 border rounded hover:bg-gray-100"
                  >
                    +
                  </button>
                </div>
                <div className="text-right">
                  <p className="font-semibold">
                    ${(item.price * item.quantity).toFixed(2)}
                  </p>
                </div>
                <button
                  onClick={() => removeItem(item.id)}
                  className="text-red-500 hover:text-red-700"
                >
                  Remove
                </button>
              </div>
            ))}
          </div>
          <button
            onClick={clearCart}
            className="mt-4 text-sm text-gray-500 hover:text-gray-700"
          >
            Clear Cart
          </button>
        </div>

        <div className="lg:col-span-1">
          <div className="border rounded-lg p-6 sticky top-4">
            <h2 className="text-xl font-bold mb-4">Order Summary</h2>
            <div className="space-y-2 mb-4">
              <div className="flex justify-between">
                <span>Subtotal ({cart.itemCount} items)</span>
                <span>${cart.subtotal.toFixed(2)}</span>
              </div>
              <div className="flex justify-between text-gray-500">
                <span>Shipping</span>
                <span>Calculated at checkout</span>
              </div>
            </div>
            <div className="border-t pt-4 mb-6">
              <div className="flex justify-between font-bold text-lg">
                <span>Total</span>
                <span>${cart.total.toFixed(2)}</span>
              </div>
            </div>
            <Link
              href="/checkout"
              className="block w-full bg-black text-white text-center px-6 py-3 rounded-lg hover:bg-gray-800"
            >
              Proceed to Checkout
            </Link>
            <Link
              href="/products"
              className="block w-full text-center mt-4 text-gray-600 hover:text-gray-800"
            >
              Continue Shopping
            </Link>
          </div>
        </div>
      </div>
    </div>
  );
}
