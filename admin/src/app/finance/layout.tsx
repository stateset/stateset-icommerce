import type { ReactNode } from 'react';
import FinanceNav from '@/components/finance/finance-nav';

export default function FinanceLayout({ children }: { children: ReactNode }) {
  return (
    <div className="space-y-6">
      <FinanceNav />
      {children}
    </div>
  );
}
