'use client';

import { createContext, useContext, useState, useCallback, useEffect, ReactNode, useRef } from 'react';

interface CartItem {
  id: string;
  sku: string;
  name?: string;
  price: number;
  quantity: number;
}

interface Cart {
  id: string;
  items: CartItem[];
  subtotal: number;
  tax: number;
  taxRate: number;
  total: number;
  itemCount: number;
}

interface CartContextType {
  cart: Cart | null;
  isLoading: boolean;
  itemCount: number;
  shippingState: string | null;
  addItem: (sku: string, name: string, price: number) => Promise<void>;
  removeItem: (itemId: string) => Promise<void>;
  updateQuantity: (itemId: string, quantity: number) => Promise<void>;
  setShippingState: (state: string) => void;
  clearCart: () => void;
}

const CartContext = createContext<CartContextType>({
  cart: null,
  isLoading: false,
  itemCount: 0,
  shippingState: null,
  addItem: async () => {},
  removeItem: async () => {},
  updateQuantity: async () => {},
  setShippingState: () => {},
  clearCart: () => {},
});

function computeCart(raw: any, tax = 0, taxRate = 0): Cart {
  const items: CartItem[] = (raw.items || []).map((item: any) => ({
    id: item.id,
    sku: item.sku,
    name: item.name,
    price: item.unitPrice || item.price || 0,
    quantity: item.quantity || 1,
  }));
  const subtotal = items.reduce((sum, i) => sum + i.price * i.quantity, 0);
  return {
    id: raw.id,
    items,
    subtotal,
    tax,
    taxRate,
    total: subtotal + tax,
    itemCount: items.reduce((sum, i) => sum + i.quantity, 0),
  };
}

export function CartProvider({ children }: { children: ReactNode }) {
  const [cart, setCart] = useState<Cart | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [shippingState, setShippingStateRaw] = useState<string | null>(null);
  const taxTimerRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    const savedCartId = localStorage.getItem('stateset_cart_id');
    const savedState = localStorage.getItem('stateset_shipping_state');
    if (savedState) setShippingStateRaw(savedState);
    if (savedCartId) fetchCart(savedCartId);
  }, []);

  const fetchCart = async (cartId: string) => {
    setIsLoading(true);
    try {
      const res = await fetch(`/api/cart?cartId=${cartId}`);
      const data = await res.json();
      if (data.cart) setCart(computeCart(data.cart));
    } catch {} finally {
      setIsLoading(false);
    }
  };

  const recalculateTax = useCallback(async (currentCart: Cart, state: string) => {
    if (!state || !currentCart || currentCart.items.length === 0) return;
    try {
      const items = currentCart.items.map((i) => ({
        sku: i.sku,
        quantity: i.quantity,
        unitPrice: i.price,
      }));
      const res = await fetch('/api/tax', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ items, stateCode: state }),
      });
      const data = await res.json();
      setCart((prev) => prev ? {
        ...prev,
        tax: data.taxAmount || 0,
        taxRate: data.taxRate || 0,
        total: prev.subtotal + (data.taxAmount || 0),
      } : prev);
    } catch {}
  }, []);

  const setShippingState = useCallback((state: string) => {
    setShippingStateRaw(state);
    localStorage.setItem('stateset_shipping_state', state);
    if (taxTimerRef.current) clearTimeout(taxTimerRef.current);
    taxTimerRef.current = setTimeout(() => {
      if (cart) recalculateTax(cart, state);
    }, 500);
  }, [cart, recalculateTax]);

  const addItem = useCallback(async (sku: string, name: string, price: number) => {
    const cartId = cart?.id || localStorage.getItem('stateset_cart_id');
    const res = await fetch('/api/cart', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ cartId, sku, name, unitPrice: price, quantity: 1 }),
    });
    const data = await res.json();
    if (data.cart) {
      const newCart = computeCart(data.cart);
      setCart(newCart);
      localStorage.setItem('stateset_cart_id', newCart.id);
      if (shippingState) recalculateTax(newCart, shippingState);
    }
  }, [cart, shippingState, recalculateTax]);

  const removeItem = useCallback(async (itemId: string) => {
    if (!cart) return;
    const res = await fetch('/api/cart', {
      method: 'DELETE',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ cartId: cart.id, itemId }),
    });
    const data = await res.json();
    if (data.cart) {
      const newCart = computeCart(data.cart);
      setCart(newCart);
      if (shippingState) recalculateTax(newCart, shippingState);
    }
  }, [cart, shippingState, recalculateTax]);

  const updateQuantity = useCallback(async (itemId: string, quantity: number) => {
    if (!cart) return;
    if (quantity <= 0) return removeItem(itemId);
    const res = await fetch('/api/cart', {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ cartId: cart.id, itemId, quantity }),
    });
    const data = await res.json();
    if (data.cart) {
      const newCart = computeCart(data.cart);
      setCart(newCart);
      if (shippingState) recalculateTax(newCart, shippingState);
    }
  }, [cart, shippingState, recalculateTax, removeItem]);

  const clearCart = useCallback(() => {
    setCart(null);
    localStorage.removeItem('stateset_cart_id');
  }, []);

  return (
    <CartContext.Provider
      value={{
        cart,
        isLoading,
        itemCount: cart?.itemCount || 0,
        shippingState,
        addItem,
        removeItem,
        updateQuantity,
        setShippingState,
        clearCart,
      }}
    >
      {children}
    </CartContext.Provider>
  );
}

export function useCart() {
  return useContext(CartContext);
}
