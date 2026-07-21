'use client';

import { cn } from '@/lib/utils';

interface SkeletonProps {
  className?: string;
}

export function Skeleton({ className }: SkeletonProps) {
  return (
    <div className={cn('animate-pulse rounded-md bg-ds-muted', className)} aria-hidden="true" />
  );
}

interface LoadingSkeletonProps {
  type?: 'card' | 'chart' | 'table' | 'metric' | 'list';
  count?: number;
}

export default function LoadingSkeleton({ type = 'card', count = 1 }: LoadingSkeletonProps) {
  const renderSkeleton = () => {
    switch (type) {
      case 'metric':
        return (
          <div className="p-4 rounded-lg border border-ds-enterprise-line bg-ds-card">
            <Skeleton className="h-4 w-24 mb-2" />
            <Skeleton className="h-8 w-32" />
            <Skeleton className="h-3 w-16 mt-2" />
          </div>
        );

      case 'chart':
        return (
          <div className="p-6 rounded-lg border border-ds-enterprise-line bg-ds-card">
            <Skeleton className="h-6 w-48 mb-2" />
            <Skeleton className="h-4 w-64 mb-6" />
            <Skeleton className="h-64 w-full rounded" />
          </div>
        );

      case 'table':
        return (
          <div className="p-6 rounded-lg border border-ds-enterprise-line bg-ds-card">
            <Skeleton className="h-6 w-48 mb-4" />
            <div className="space-y-3">
              <Skeleton className="h-10 w-full" />
              <Skeleton className="h-10 w-full" />
              <Skeleton className="h-10 w-full" />
              <Skeleton className="h-10 w-full" />
              <Skeleton className="h-10 w-full" />
            </div>
          </div>
        );

      case 'list':
        return (
          <div className="p-6 rounded-lg border border-ds-enterprise-line bg-ds-card">
            <Skeleton className="h-6 w-48 mb-4" />
            <div className="space-y-3">
              {[...Array(4)].map((_, i) => (
                <div key={i} className="flex items-center space-x-3">
                  <Skeleton className="h-10 w-10 rounded-full" />
                  <div className="flex-1">
                    <Skeleton className="h-4 w-full mb-1" />
                    <Skeleton className="h-3 w-2/3" />
                  </div>
                </div>
              ))}
            </div>
          </div>
        );

      case 'card':
      default:
        return (
          <div className="p-6 rounded-lg border border-ds-enterprise-line bg-ds-card">
            <Skeleton className="h-6 w-48 mb-2" />
            <Skeleton className="h-4 w-64 mb-4" />
            <Skeleton className="h-32 w-full rounded" />
          </div>
        );
    }
  };

  if (count === 1) {
    return renderSkeleton();
  }

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
      {[...Array(count)].map((_, i) => (
        <div key={i}>{renderSkeleton()}</div>
      ))}
    </div>
  );
}

/**
 * Skeleton loader for agent config sidebar.
 */
export function AgentConfigSidebarSkeleton() {
  return (
    <div className="flex flex-col w-72 border-l border-ds-enterprise-line bg-ds-muted p-4 space-y-4">
      {/* Header */}
      <div className="space-y-2">
        <Skeleton className="h-5 w-32" />
        <Skeleton className="h-3 w-48" />
      </div>

      {/* Config sections */}
      {[...Array(4)].map((_, i) => (
        <div key={i} className="space-y-2">
          <Skeleton className="h-4 w-24" />
          <Skeleton className="h-10 w-full rounded-md" />
        </div>
      ))}

      {/* Toggle rows */}
      {[...Array(3)].map((_, i) => (
        <div key={i} className="flex items-center justify-between">
          <Skeleton className="h-4 w-28" />
          <Skeleton className="h-6 w-10 rounded-full" />
        </div>
      ))}

      {/* Action button */}
      <Skeleton className="h-10 w-full rounded-md mt-4" />
    </div>
  );
}

/**
 * Skeleton loader for conversation area / message list.
 */
export function ConversationSkeleton() {
  return (
    <div className="flex-1 p-4 space-y-4" aria-label="Loading conversation">
      {[...Array(4)].map((_, i) => (
        <div key={i} className={cn('flex', i % 2 === 0 ? 'justify-start' : 'justify-end')}>
          <div
            className={cn(
              'flex items-start space-x-2',
              i % 2 !== 0 && 'flex-row-reverse space-x-reverse',
            )}
          >
            <Skeleton className="w-8 h-8 rounded-full flex-shrink-0" />
            <div className="space-y-1">
              <Skeleton className={cn('h-16 rounded-lg', i % 2 === 0 ? 'w-64' : 'w-48')} />
              <Skeleton className="h-3 w-16" />
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
