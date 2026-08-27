'use client';

import { useAccount } from 'wagmi';
import { useCustomer } from '@/contexts/CustomerContext';
import { ConnectWallet } from '@/components/commerce/ConnectWallet';
import Link from 'next/link';
import { usePathname } from 'next/navigation';

export default function AccountLayout({ children }: { children: React.ReactNode }) {
  const { isConnected } = useAccount();
  const { isLoading, isAuthenticated, authenticate } = useCustomer();
  const pathname = usePathname();

  if (!isConnected) {
    return (
      <div className="container mx-auto px-4 py-8">
        <div className="max-w-md mx-auto text-center">
          <h1 className="text-3xl font-bold mb-4">My Account</h1>
          <p className="text-gray-600 mb-6">
            Connect your wallet to view your account and order history.
          </p>
          <ConnectWallet />
        </div>
      </div>
    );
  }

  if (!isAuthenticated) {
    return (
      <div className="container mx-auto px-4 py-8">
        <div className="max-w-md mx-auto text-center">
          <h1 className="text-3xl font-bold mb-4">Verify your wallet</h1>
          <p className="text-gray-600 mb-6">
            Sign a short-lived message to view private account data. This does not submit a
            transaction.
          </p>
          <button
            onClick={() => void authenticate()}
            className="px-5 py-3 rounded-lg bg-black text-white"
          >
            Sign in with wallet
          </button>
        </div>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="container mx-auto px-4 py-8">
        <p className="text-gray-600 text-center">Loading your account...</p>
      </div>
    );
  }

  const navItems = [
    { href: '/account', label: 'Dashboard' },
    { href: '/account/orders', label: 'Order History' },
    { href: '/account/subscriptions', label: 'Subscriptions' },
  ];

  return (
    <div className="container mx-auto px-4 py-8">
      <div className="flex flex-col md:flex-row gap-8">
        <aside className="w-full md:w-64 flex-shrink-0">
          <h1 className="text-2xl font-bold mb-4">My Account</h1>
          <nav className="space-y-1">
            {navItems.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className={`block px-4 py-2 rounded-lg transition-colors ${
                  pathname === item.href ? 'bg-black text-white' : 'text-gray-700 hover:bg-gray-100'
                }`}
              >
                {item.label}
              </Link>
            ))}
          </nav>
        </aside>
        <main className="flex-grow">{children}</main>
      </div>
    </div>
  );
}
