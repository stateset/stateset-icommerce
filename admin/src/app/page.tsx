import dynamic from 'next/dynamic';

const UnifiedDashboard = dynamic(
  () => import('@/components/operations/unified-dashboard'),
  {
    loading: () => (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500" />
      </div>
    ),
    ssr: false,
  }
);

export default function Home() {
  return <UnifiedDashboard />;
}
