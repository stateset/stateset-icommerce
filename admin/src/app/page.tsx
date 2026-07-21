import dynamic from 'next/dynamic';

const UnifiedDashboard = dynamic(() => import('@/components/operations/unified-dashboard'), {
  loading: () => (
    <div className="flex h-64 items-center justify-center">
      <div className="h-8 w-8 animate-spin rounded-full border-b-2 border-ds-primary" />
    </div>
  ),
  ssr: false,
});

export default function Home() {
  return <UnifiedDashboard />;
}
