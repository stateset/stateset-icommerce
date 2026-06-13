import type { Metadata } from 'next';
import { cookies } from 'next/headers';
import { Inter } from 'next/font/google';
import { Suspense } from 'react';
import './globals.css';
import { Sidebar } from '@/components/sidebar';
import { MobileNav } from '@/components/mobile-nav';
import { SessionsSidebar } from '@/components/sessions-sidebar';
import { TopBar } from '@/components/shared/top-bar';
import { AdminLoginGate } from '@/lib/shared/admin-login-gate';
import { ADMIN_SESSION_COOKIE, isAdminAuthDisabled, validateSessionToken } from '@/lib/shared/auth-session';

const inter = Inter({ subsets: ['latin'] });

export const metadata: Metadata = {
  title: 'StateSet Admin Dashboard',
  description: 'Commerce operations dashboard powered by embedded StateSet engine',
};

function SidebarSkeleton() {
  return (
    <div className="hidden lg:flex flex-col w-64 border-r border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900/50 animate-pulse">
      <div className="p-4 space-y-3">
        <div className="h-6 bg-gray-200 dark:bg-gray-800 rounded w-3/4" />
        <div className="h-8 bg-gray-200 dark:bg-gray-800 rounded" />
        <div className="h-8 bg-gray-200 dark:bg-gray-800 rounded" />
      </div>
      <div className="flex-1 p-4 space-y-2">
        {[...Array(5)].map((_, i) => (
          <div key={i} className="h-20 bg-gray-200 dark:bg-gray-800 rounded-lg" />
        ))}
      </div>
    </div>
  );
}

export default async function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const renderUnauthenticated = async () => {
    if (isAdminAuthDisabled()) {
      return null;
    }

    const cookieStore = await cookies();
    const sessionToken = cookieStore.get(ADMIN_SESSION_COOKIE)?.value?.trim();
    const isAuthenticated = sessionToken ? await validateSessionToken(sessionToken) : false;

    if (isAuthenticated) {
      return null;
    }

    return (
      <html lang="en" suppressHydrationWarning>
        <body className={`${inter.className} min-h-screen bg-gray-50 dark:bg-gray-950`}>
          <main className="flex min-h-screen items-center justify-center p-6">
            <AdminLoginGate />
          </main>
        </body>
      </html>
    );
  };

  const unauthenticatedLayout = await renderUnauthenticated();
  if (unauthenticatedLayout) {
    return unauthenticatedLayout;
  }

  return (
    <html lang="en" suppressHydrationWarning>
      <body className={`${inter.className} bg-gray-50 dark:bg-gray-950`}>
        {/* Skip to content link for accessibility */}
        <a
          href="#main-content"
          className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:px-4 focus:py-2 focus:bg-indigo-600 focus:text-white focus:rounded-md focus:outline-none"
        >
          Skip to content
        </a>
        <div className="flex h-screen overflow-hidden">
          <Suspense fallback={<SidebarSkeleton />}>
            <SessionsSidebar className="hidden lg:flex" />
          </Suspense>
          <Sidebar />
          <main id="main-content" className="flex-1 overflow-y-auto flex flex-col">
            {/* Mobile disclosure nav (the sidebar is hidden below lg). */}
            <MobileNav />
            {/* Top bar (org switcher; hides itself when there's ≤1 org). */}
            <Suspense fallback={null}>
              <TopBar />
            </Suspense>
            <Suspense fallback={
              <div className="container mx-auto p-6 animate-pulse">
                <div className="h-8 bg-gray-200 dark:bg-gray-800 rounded w-1/3 mb-4" />
                <div className="h-64 bg-gray-200 dark:bg-gray-800 rounded" />
              </div>
            }>
              <div className="container mx-auto p-6 flex-1">
                {children}
              </div>
            </Suspense>
          </main>
        </div>
      </body>
    </html>
  );
}
