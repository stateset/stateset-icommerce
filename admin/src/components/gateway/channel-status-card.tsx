'use client';

import { Card, CardContent, StatusPill } from '@stateset/design';
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
    stats.messagesReceived > 0 ? ((stats.errors / stats.messagesReceived) * 100).toFixed(1) : '0.0';

  return (
    <Card
      className="cursor-pointer transition-shadow hover:shadow-ds-card-hover"
      onClick={onClick}
      role={onClick ? 'button' : undefined}
      tabIndex={onClick ? 0 : undefined}
      onKeyDown={
        onClick
          ? (e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onClick();
              }
            }
          : undefined
      }
    >
      <CardContent className="p-5">
        <div className="flex items-center justify-between">
          <div className="flex items-center space-x-3">
            <Icon className="w-6 h-6 text-ds-muted-foreground" />
            <div>
              <p className="text-sm font-medium text-ds-foreground">{displayName}</p>
              <p className="text-xs text-ds-muted-foreground">
                {isActive ? `Active ${formatRelativeTime(stats.lastMessageAt!)}` : 'No activity'}
              </p>
            </div>
          </div>
          <StatusPill status={isActive ? 'ok' : 'idle'}>{isActive ? 'Online' : 'Idle'}</StatusPill>
        </div>
        <div className="grid grid-cols-3 gap-4 mt-4">
          <div>
            <p className="text-xs text-ds-muted-foreground">Messages</p>
            <p className="text-sm font-semibold text-ds-foreground">{stats.messagesReceived}</p>
          </div>
          <div>
            <p className="text-xs text-ds-muted-foreground">Errors</p>
            <p
              className={`text-sm font-semibold ${stats.errors > 0 ? 'text-ds-status-fail' : 'text-ds-foreground'}`}
            >
              {stats.errors} ({errorRate}%)
            </p>
          </div>
          <div>
            <p className="text-xs text-ds-muted-foreground">Avg Response</p>
            <p className="text-sm font-semibold text-ds-foreground">
              {Math.round(stats.avgResponseMs)}ms
            </p>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
