'use client';

import {
  createContext,
  useContext,
  useState,
  useCallback,
  useEffect,
  ReactNode,
  useRef,
} from 'react';

interface CartItem {
  id: string;
  sku: string;
  name?: string;
  price: number;
  priceExact: string;
  quantity: number;
}

interface Cart {
  id: string;
  items: CartItem[];
  subtotal: number;
  subtotalExact: string;
  tax: number;
  taxExact: string;
  taxRate: number;
  taxRateExact: string;
  taxConfigured: boolean;
  total: number;
  totalExact: string;
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

function computeCart(
  raw: any,
  taxExact = '0',
  taxRateExact = '0',
  totalExact?: string,
  taxConfigured = false,
): Cart {
  const items: CartItem[] = (raw.items || []).map((item: any) => ({
    id: item.id,
    sku: item.sku,
    name: item.name,
    price: item.unitPrice || item.price || 0,
    priceExact: item.unitPriceExact || String(item.unitPrice || item.price || 0),
    quantity: item.quantity || 1,
  }));
  const subtotalExact = raw.subtotalExact || String(raw.subtotal || 0);
  const subtotal = Number(subtotalExact);
  const tax = Number(taxExact);
  const resolvedTotalExact = totalExact || raw.grandTotalExact || String(subtotal + tax);
  return {
    id: raw.id,
    items,
    subtotal,
    subtotalExact,
    tax,
    taxExact,
    taxRate: Number(taxRateExact),
    taxRateExact,
    taxConfigured,
    total: Number(resolvedTotalExact),
    totalExact: resolvedTotalExact,
    itemCount: items.reduce((sum, i) => sum + i.quantity, 0),
  };
}

export function CartProvider({ children }: { children: ReactNode }) {
  const [cart, setCart] = useState<Cart | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [shippingState, setShippingStateRaw] = useState<string | null>(null);
  const taxTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

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
    } catch {
    } finally {
      setIsLoading(false);
    }
  };

  const recalculateTax = useCallback(async (currentCart: Cart, state: string) => {
    if (!state || !currentCart || currentCart.items.length === 0) return;
    try {
      const res = await fetch('/api/tax', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ cartId: currentCart.id, stateCode: state }),
      });
      const data = await res.json();
      if (!res.ok) throw new Error(data.error || 'Tax calculation failed');
      setCart((prev) =>
        prev
          ? computeCart(
              {
                ...prev,
                items: prev.items.map((item) => ({ ...item, unitPriceExact: item.priceExact })),
              },
              data.taxAmountExact,
              data.taxRateExact,
              data.totalExact,
              data.configured === true,
            )
          : prev,
      );
    } catch {
      setCart((prev) => (prev ? computeCart(prev, '0', '0', prev.subtotalExact, false) : prev));
    }
  }, []);

  useEffect(() => {
    if (cart && shippingState) void recalculateTax(cart, shippingState);
  }, [cart?.id, shippingState, recalculateTax]);

  const setShippingState = useCallback(
    (state: string) => {
      setShippingStateRaw(state);
      localStorage.setItem('stateset_shipping_state', state);
      if (taxTimerRef.current) clearTimeout(taxTimerRef.current);
      taxTimerRef.current = setTimeout(() => {
        if (cart) recalculateTax(cart, state);
      }, 500);
    },
    [cart, recalculateTax],
  );

  const addItem = useCallback(
    async (sku: string, name: string, price: number) => {
      const cartId = cart?.id || localStorage.getItem('stateset_cart_id');
      const res = await fetch('/api/cart', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ cartId, sku, quantity: 1 }),
      });
      const data = await res.json();
      if (data.cart) {
        const newCart = computeCart(data.cart);
        setCart(newCart);
        localStorage.setItem('stateset_cart_id', newCart.id);
        if (shippingState) recalculateTax(newCart, shippingState);
      }
    },
    [cart, shippingState, recalculateTax],
  );

  const removeItem = useCallback(
    async (itemId: string) => {
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
    },
    [cart, shippingState, recalculateTax],
  );

  const updateQuantity = useCallback(
    async (itemId: string, quantity: number) => {
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
    },
    [cart, shippingState, recalculateTax, removeItem],
  );

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
