import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';

export async function PATCH(
  request: NextRequest,
  { params }: { params: Promise<{ id: string }> }
) {
  const { id } = await params;

  try {
    const body = await request.json();
    const { action } = body;

    const commerce = getCommerce();

    if (action === 'cancel') {
      const updated = await commerce.subscriptions.update(id, {
        status: 'cancelled',
      });
      return NextResponse.json({ subscription: updated });
    }

    if (action === 'pause') {
      const updated = await commerce.subscriptions.update(id, {
        status: 'paused',
      });
      return NextResponse.json({ subscription: updated });
    }

    if (action === 'resume') {
      const updated = await commerce.subscriptions.update(id, {
        status: 'active',
      });
      return NextResponse.json({ subscription: updated });
    }

    return NextResponse.json({ error: 'Unknown action' }, { status: 400 });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to update subscription' },
      { status: 500 }
    );
  }
}
