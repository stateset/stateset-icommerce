import { NextRequest } from 'next/server';
import { realpath, open } from 'node:fs/promises';
import path from 'node:path';
import { Readable } from 'node:stream';
import { getCommerce } from '@/lib/commerce';
import { checkDigitalAccess } from '@/lib/digital-access.js';

export async function POST(request: NextRequest) {
  const form = await request.formData();
  const { allowed, file } = await checkDigitalAccess(
    getCommerce(),
    process.env,
    form.get('license'),
    form.get('resource'),
  );
  if (!allowed || !file) return new Response('Access denied', { status: 403 });
  if (!process.env.STATESET_DOWNLOAD_ROOT)
    return new Response('Download is unavailable', { status: 503 });
  try {
    const root = await realpath(process.env.STATESET_DOWNLOAD_ROOT);
    const filename = await realpath(path.resolve(root, file));
    const relative = path.relative(root, filename);
    if (
      !relative ||
      relative === '..' ||
      relative.startsWith('..' + path.sep) ||
      path.isAbsolute(relative)
    )
      return new Response('File unavailable', { status: 404 });
    const handle = await open(filename);
    const stat = await handle.stat();
    if (!stat.isFile()) {
      await handle.close();
      return new Response('File unavailable', { status: 404 });
    }
    return new Response(Readable.toWeb(handle.createReadStream()) as ReadableStream, {
      headers: {
        'Content-Type': 'application/octet-stream',
        'Content-Length': String(stat.size),
        'Content-Disposition':
          "attachment; filename*=UTF-8''" + encodeURIComponent(path.basename(filename)),
        'Cache-Control': 'private, no-store',
      },
    });
  } catch {
    return new Response('File unavailable', { status: 404 });
  }
}
