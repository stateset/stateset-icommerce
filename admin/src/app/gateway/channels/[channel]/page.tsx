'use client';

import dynamic from 'next/dynamic';
import { useParams, useRouter } from 'next/navigation';

const ChannelDetail = dynamic(
  () =>
    import('@/components/gateway/channel-detail').then((m) => ({
      default: m.ChannelDetail,
    })),
  {
    loading: () => (
      <div className="flex items-center justify-center h-64">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-500" />
      </div>
    ),
    ssr: false,
  },
);

export default function ChannelDetailPage() {
  const params = useParams();
  const router = useRouter();
  const channel = params.channel as string;

  return <ChannelDetail channelName={channel} onBack={() => router.push('/gateway')} />;
}
