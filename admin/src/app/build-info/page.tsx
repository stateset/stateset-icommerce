import {
  BuildInfoView,
  type FetchResult,
  type VersionResponse,
} from '@/components/build-info-view';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';

async function fetchVersion(): Promise<FetchResult> {
  const apiUrl = getServerStateSetApiUrl();
  try {
    const response = await fetch(`${apiUrl}/version`, {
      // Build info changes only on deploy, but the page should reflect the
      // running binary, so refresh the server-side result every 60 seconds.
      next: { revalidate: 60 },
      headers: { Accept: 'application/json' },
    });
    if (!response.ok) {
      return { error: `Engine returned ${response.status}` };
    }
    const data = (await response.json()) as VersionResponse;
    if (typeof data?.version !== 'string') {
      return { error: 'Engine returned a malformed /version response' };
    }
    return data;
  } catch (error) {
    return { error: error instanceof Error ? error.message : String(error) };
  }
}

export const dynamic = 'force-dynamic';

export default async function BuildInfoPage() {
  return <BuildInfoView result={await fetchVersion()} />;
}
