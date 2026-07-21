import type { ReactNode } from 'react';
import OperationsNav from '@/components/operations/operations-nav';

export default function OperationsLayout({ children }: { children: ReactNode }) {
  return (
    <div className="space-y-6">
      <OperationsNav />
      {children}
    </div>
  );
}
