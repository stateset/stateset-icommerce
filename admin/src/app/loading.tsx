// Root app-router loading state. Mirrors the Suspense fallback used in
// layout.tsx so route transitions and initial loads look consistent.

import { Skeleton } from '@/components/ui/loading-skeleton';

export default function Loading() {
  return (
    <div aria-busy="true" aria-label="Loading page">
      <Skeleton className="mb-4 h-8 w-1/3" />
      <div className="mb-6 grid grid-cols-1 gap-4 md:grid-cols-2 lg:grid-cols-4">
        {[...Array(4)].map((_, i) => (
          <div key={i} className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-800 dark:bg-gray-900">
            <Skeleton className="mb-2 h-4 w-24" />
            <Skeleton className="h-8 w-32" />
          </div>
        ))}
      </div>
      <Skeleton className="h-64 w-full" />
    </div>
  );
}
