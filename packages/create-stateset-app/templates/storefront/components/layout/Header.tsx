'use client';

import Link from 'next/link';
import { useAccount } from 'wagmi';
import { CartIcon } from '@/components/commerce/CartIcon';

export function Header() {
  const { isConnected } = useAccount();

  return (
    <header className="border-b">
      <div className="container mx-auto px-4 py-4 flex items-center justify-between">
        <Link href="/" className="text-xl font-bold">
          {{STORE_NAME}}
        </Link>
        <nav className="hidden md:flex items-center gap-6">
          <Link href="/products" className="text-gray-600 hover:text-black transition-colors">
            Products
          </Link>
          <Link href="/collections" className="text-gray-600 hover:text-black transition-colors">
            Collections
          </Link>
        </nav>
        <div className="flex items-center gap-4">
          <CartIcon />
          <Link
            href="/account"
            className="text-gray-600 hover:text-black transition-colors"
          >
            {isConnected ? 'Account' : 'Sign In'}
          </Link>
        </div>
      </div>
    </header>
  );
}
