import Link from 'next/link';

export function Footer() {
  return (
    <footer className="border-t mt-16">
      <div className="container mx-auto px-4 py-12">
        <div className="grid grid-cols-1 md:grid-cols-4 gap-8">
          <div>
            <h3 className="font-bold mb-4">{{STORE_NAME}}</h3>
            <p className="text-sm text-gray-600">
              Premium products powered by decentralized commerce.
            </p>
          </div>
          <div>
            <h4 className="font-semibold mb-3 text-sm">Shop</h4>
            <ul className="space-y-2 text-sm text-gray-600">
              <li><Link href="/products" className="hover:text-black">All Products</Link></li>
              <li><Link href="/collections" className="hover:text-black">Collections</Link></li>
            </ul>
          </div>
          <div>
            <h4 className="font-semibold mb-3 text-sm">Account</h4>
            <ul className="space-y-2 text-sm text-gray-600">
              <li><Link href="/account" className="hover:text-black">Dashboard</Link></li>
              <li><Link href="/account/orders" className="hover:text-black">Orders</Link></li>
              <li><Link href="/cart" className="hover:text-black">Cart</Link></li>
            </ul>
          </div>
          <div>
            <h4 className="font-semibold mb-3 text-sm">Info</h4>
            <ul className="space-y-2 text-sm text-gray-600">
              <li><span>USDC payments on Base</span></li>
              <li><span>Powered by StateSet</span></li>
            </ul>
          </div>
        </div>
        <div className="border-t mt-8 pt-8 text-center text-sm text-gray-500">
          &copy; {new Date().getFullYear()} {{STORE_NAME}}. All rights reserved.
        </div>
      </div>
    </footer>
  );
}
