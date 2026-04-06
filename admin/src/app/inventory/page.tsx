import dynamic from 'next/dynamic';

const InventoryAnalytics = dynamic(
  () => import('@/components/inventory/inventory-analytics'),
  {
    loading: () => (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500" />
      </div>
    ),
    ssr: false,
  }
);

export default function InventoryPage() {
  return <InventoryAnalytics />;
}
