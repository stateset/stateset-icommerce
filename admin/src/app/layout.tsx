import type { Metadata } from 'next';
import { cookies } from 'next/headers';
import { Inter } from 'next/font/google';
import { Suspense } from 'react';
import '@stateset/design/tokens.css';
import './globals.css';
import { Sidebar } from '@/components/sidebar';
import { MobileNav } from '@/components/mobile-nav';
import { SessionsSidebar } from '@/components/sessions-sidebar';
import { TopBar } from '@/components/shared/top-bar';
import { AdminLoginGate } from '@/lib/shared/admin-login-gate';
import {
  ADMIN_SESSION_COOKIE,
  isAdminAuthDisabled,
  validateSessionToken,
} from '@/lib/shared/auth-session';

const inter = Inter({ subsets: ['latin'] });

export const metadata: Metadata = {
  title: 'StateSet Admin Dashboard',
  description: 'Commerce operations dashboard powered by embedded StateSet engine',
};

function SidebarSkeleton() {
  return (
    <div className="hidden w-ds-sidebar flex-col border-r border-ds-sidebar-border bg-ds-sidebar lg:flex">
      <div className="space-y-3 p-4">
        <div className="h-6 w-3/4 animate-pulse rounded bg-ds-sidebar-foreground/10" />
        <div className="h-8 animate-pulse rounded bg-ds-sidebar-foreground/10" />
        <div className="h-8 animate-pulse rounded bg-ds-sidebar-foreground/10" />
      </div>
      <div className="flex-1 space-y-2 p-4">
        {[...Array(5)].map((_, i) => (
          <div key={i} className="h-10 animate-pulse rounded-lg bg-ds-sidebar-foreground/10" />
        ))}
      </div>
    </div>
  );
}

export default async function RootLayout({ children }: { children: React.ReactNode }) {
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
        <body
          className={`${inter.className} ds-app-frame min-h-screen bg-ds-enterprise-canvas text-ds-foreground`}
        >
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
      <body
        className={`${inter.className} ds-app-frame bg-ds-enterprise-canvas text-ds-foreground`}
      >
        {/* Skip to content link for accessibility */}
        <a
          href="#main-content"
          className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:rounded-full focus:bg-ds-primary focus:px-4 focus:py-2 focus:text-ds-primary-foreground focus:outline-none"
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
            <Suspense
              fallback={
                <div className="container mx-auto animate-pulse p-6">
                  <div className="mb-4 h-8 w-1/3 rounded bg-ds-muted" />
                  <div className="h-64 rounded bg-ds-muted" />
                </div>
              }
            >
              <div className="container mx-auto p-6 flex-1">{children}</div>
            </Suspense>
          </main>
        </div>
      </body>
    </html>
  );
}
