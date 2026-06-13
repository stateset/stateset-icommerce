// Component test for the per-channel status card. The card is pure
// presentational — it derives its UI entirely from props (no hooks, no
// network) — so we mock @tremor/react down to <div>s and assert that the
// stats and online/idle decorations match the input.

import { render, screen, fireEvent } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tremor/react', () => {
  const Wrapper = ({ children, onClick }: { children?: ReactNode; onClick?: () => void }) => (
    <div onClick={onClick} role={onClick ? 'button' : undefined}>
      {children}
    </div>
  );
  return { Card: Wrapper, Text: Wrapper, Badge: Wrapper };
});

import { ChannelStatusCard } from '@/components/gateway/channel-status-card';
import type { ChannelStats } from '@/lib/types/gateway';

const ACTIVE_STATS: ChannelStats = {
  messagesReceived: 100,
  responsesSent: 95,
  errors: 5,
  blocked: 0,
  avgResponseMs: 142.6,
  lastMessageAt: new Date(Date.now() - 60_000).toISOString(),
};

const IDLE_STATS: ChannelStats = {
  messagesReceived: 0,
  responsesSent: 0,
  errors: 0,
  blocked: 0,
  avgResponseMs: 0,
  lastMessageAt: null,
};

describe('ChannelStatusCard', () => {
  it('renders the friendly channel name for known channels', () => {
    render(<ChannelStatusCard name="discord" stats={ACTIVE_STATS} />);
    expect(screen.getByText('Discord')).toBeInTheDocument();
  });

  it('falls back to the raw name for unknown channels', () => {
    render(<ChannelStatusCard name="custom-channel" stats={ACTIVE_STATS} />);
    expect(screen.getByText('custom-channel')).toBeInTheDocument();
  });

  it('shows the Online badge and counts for active channels', () => {
    render(<ChannelStatusCard name="slack" stats={ACTIVE_STATS} />);
    expect(screen.getByText('Online')).toBeInTheDocument();
    expect(screen.getByText('100')).toBeInTheDocument();
    // 5 errors / 100 messages = 5.0%
    expect(screen.getByText('5 (5.0%)')).toBeInTheDocument();
    // Math.round(142.6) = 143
    expect(screen.getByText('143ms')).toBeInTheDocument();
  });

  it('shows the Idle badge and zero error rate when no traffic', () => {
    render(<ChannelStatusCard name="telegram" stats={IDLE_STATS} />);
    expect(screen.getByText('Idle')).toBeInTheDocument();
    expect(screen.getByText('No activity')).toBeInTheDocument();
    expect(screen.getByText('0 (0.0%)')).toBeInTheDocument();
  });

  it('invokes onClick when the card is clicked', () => {
    const onClick = vi.fn();
    render(<ChannelStatusCard name="webchat" stats={ACTIVE_STATS} onClick={onClick} />);
    fireEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });
});
