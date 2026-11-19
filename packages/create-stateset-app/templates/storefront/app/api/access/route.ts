import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import { checkDigitalAccess } from '@/lib/digital-access.js';

export async function POST(request: NextRequest) {
  const { resource } = await request.json();
  const key = (request.headers.get('authorization') || '').replace(/^Bearer /, '');
  const { file, ...result } = await checkDigitalAccess(getCommerce(), process.env, key, resource);
  return NextResponse.json(result, {
    status: result.allowed ? 200 : 403,
    headers: { 'Cache-Control': 'no-store' },
  });
}
