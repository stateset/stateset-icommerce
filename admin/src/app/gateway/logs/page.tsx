import dynamic from 'next/dynamic';

const LogsDashboard = dynamic(() => import('@/components/gateway/logs-dashboard'), {
  loading: () => (
    <div className="flex items-center justify-center h-64">
      <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-500" />
    </div>
  ),
  ssr: false,
});

export default function GatewayLogsPage() {
  return <LogsDashboard />;
}
