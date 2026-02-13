'use client';

import { createContext, useContext, useState, useEffect, useCallback, ReactNode } from 'react';

interface WishlistContextType {
  wishlist: string[];
  addToWishlist: (productId: string) => void;
  removeFromWishlist: (productId: string) => void;
  isInWishlist: (productId: string) => boolean;
}

const WishlistContext = createContext<WishlistContextType>({
  wishlist: [],
  addToWishlist: () => {},
  removeFromWishlist: () => {},
  isInWishlist: () => false,
});

export function WishlistProvider({ children }: { children: ReactNode }) {
  const [wishlist, setWishlist] = useState<string[]>([]);

  useEffect(() => {
    try {
      const saved = localStorage.getItem('stateset_wishlist');
      if (saved) setWishlist(JSON.parse(saved));
    } catch {}
  }, []);

  const persist = useCallback((items: string[]) => {
    setWishlist(items);
    localStorage.setItem('stateset_wishlist', JSON.stringify(items));
  }, []);

  const addToWishlist = useCallback((productId: string) => {
    persist([...new Set([...wishlist, productId])]);
  }, [wishlist, persist]);

  const removeFromWishlist = useCallback((productId: string) => {
    persist(wishlist.filter((id) => id !== productId));
  }, [wishlist, persist]);

  const isInWishlist = useCallback((productId: string) => {
    return wishlist.includes(productId);
  }, [wishlist]);

  return (
    <WishlistContext.Provider value={{ wishlist, addToWishlist, removeFromWishlist, isInWishlist }}>
      {children}
    </WishlistContext.Provider>
  );
}

export function useWishlist() {
  return useContext(WishlistContext);
}
