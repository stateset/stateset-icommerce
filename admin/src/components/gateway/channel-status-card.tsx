'use client';

import { Card, Text, Badge } from '@tremor/react';
import {
  ChatBubbleLeftRightIcon,
  HashtagIcon,
  PaperAirplaneIcon,
  DevicePhoneMobileIcon,
  ComputerDesktopIcon,
  GlobeAltIcon,
  CloudIcon,
} from '@heroicons/react/24/outline';
import type { ChannelStats } from '@/lib/types/gateway';
import { formatRelativeTime } from '@/lib/utils';

const CHANNEL_ICONS: Record<string, React.ComponentType<{ className?: string }>> = {
  discord: ChatBubbleLeftRightIcon,
  slack: HashtagIcon,
  telegram: PaperAirplaneIcon,
  whatsapp: DevicePhoneMobileIcon,
  signal: DevicePhoneMobileIcon,
  imessage: DevicePhoneMobileIcon,
  teams: ComputerDesktopIcon,
  matrix: ComputerDesktopIcon,
  'google-chat': CloudIcon,
  webchat: GlobeAltIcon,
  http: GlobeAltIcon,
};

const CHANNEL_DISPLAY: Record<string, string> = {
  discord: 'Discord',
  slack: 'Slack',
  telegram: 'Telegram',
  whatsapp: 'WhatsApp',
  signal: 'Signal',
  imessage: 'iMessage',
  teams: 'Teams',
  matrix: 'Matrix',
  'google-chat': 'Google Chat',
  webchat: 'Webchat',
  http: 'HTTP API',
};

interface ChannelStatusCardProps {
  name: string;
  stats: ChannelStats;
  onClick?: () => void;
}

export function ChannelStatusCard({ name, stats, onClick }: ChannelStatusCardProps) {
  const Icon = CHANNEL_ICONS[name] || GlobeAltIcon;
  const displayName = CHANNEL_DISPLAY[name] || name;
  const isActive = stats.lastMessageAt !== null;
  const errorRate =
    stats.messagesReceived > 0
      ? ((stats.errors / stats.messagesReceived) * 100).toFixed(1)
      : '0.0';

  return (
    <Card
      className="cursor-pointer hover:shadow-md transition-shadow"
      decoration="left"
      decorationColor={isActive ? 'emerald' : 'gray'}
      onClick={onClick}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center space-x-3">
          <Icon className="w-6 h-6 text-gray-500 dark:text-gray-400" />
          <div>
            <Text className="font-medium">{displayName}</Text>
            <Text className="text-xs text-gray-400">
              {isActive
                ? `Active ${formatRelativeTime(stats.lastMessageAt!)}`
                : 'No activity'}
            </Text>
          </div>
        </div>
        <Badge color={isActive ? 'emerald' : 'gray'} size="xs">
          {isActive ? 'Online' : 'Idle'}
        </Badge>
      </div>
      <div className="grid grid-cols-3 gap-4 mt-4">
        <div>
          <Text className="text-xs text-gray-400">Messages</Text>
          <Text className="font-semibold">{stats.messagesReceived}</Text>
        </div>
        <div>
          <Text className="text-xs text-gray-400">Errors</Text>
          <Text
            className={`font-semibold ${stats.errors > 0 ? 'text-red-500' : ''}`}
          >
            {stats.errors} ({errorRate}%)
          </Text>
        </div>
        <div>
          <Text className="text-xs text-gray-400">Avg Response</Text>
          <Text className="font-semibold">{Math.round(stats.avgResponseMs)}ms</Text>
        </div>
      </div>
    </Card>
  );
}
