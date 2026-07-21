'use client';

import { Card, CardContent, StatusPill } from '@stateset/design';
import { MicrophoneIcon, GlobeAltIcon, CpuChipIcon, HeartIcon } from '@heroicons/react/24/outline';
import type { GatewayHealth } from '@/lib/types/gateway';

interface SubsystemPanelProps {
  subsystems: GatewayHealth['subsystems'];
}

const SUBSYSTEM_META = [
  { key: 'voice' as const, label: 'Voice STT/TTS', icon: MicrophoneIcon },
  { key: 'browser' as const, label: 'Browser', icon: GlobeAltIcon },
  { key: 'memory' as const, label: 'Vector Memory', icon: CpuChipIcon },
  { key: 'heartbeat' as const, label: 'Heartbeat', icon: HeartIcon },
];

export function SubsystemPanel({ subsystems }: SubsystemPanelProps) {
  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
      {SUBSYSTEM_META.map(({ key, label, icon: Icon }) => {
        const status = subsystems[key];
        const isEnabled = status === 'enabled';
        return (
          <Card key={key}>
            <CardContent className="p-4">
              <div className="flex items-center space-x-2">
                <Icon
                  className={`w-5 h-5 ${isEnabled ? 'text-ds-status-ok' : 'text-ds-muted-foreground'}`}
                />
                <p className="text-sm font-medium text-ds-foreground">{label}</p>
              </div>
              <div className="mt-2">
                <StatusPill status={isEnabled ? 'ok' : 'idle'}>{status}</StatusPill>
              </div>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}
