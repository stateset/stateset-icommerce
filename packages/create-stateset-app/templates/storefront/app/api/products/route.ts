import { NextRequest, NextResponse } from 'next/server';
import { getProducts } from '@/lib/commerce';

export async function GET(request: NextRequest) {
  try {
    const search = request.nextUrl.searchParams.get('search') || undefined;
    const category = request.nextUrl.searchParams.get('category') || undefined;
    const limit = Number(request.nextUrl.searchParams.get('limit') || '20');
    if (!Number.isSafeInteger(limit) || limit < 1 || limit > 100) {
      return NextResponse.json({ error: 'limit must be between 1 and 100' }, { status: 400 });
    }
    return NextResponse.json(await getProducts({ search, category, limit }));
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Failed to list products' },
      { status: 500 },
    );
  }
}
