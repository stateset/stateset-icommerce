import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';

/**
 * Build & Release Info
 *
 * Surfaces the metadata baked into the running binary at compile time
 * — version, commit SHA, release tag, build timestamp — and most
 * importantly whether the binary was signed via the release pipeline's
 * sigstore step. Operators use this to verify they're running a
 * known-good binary in production rather than a local/dev build that
 * happened to have the right version number.
 *
 * The corresponding HTTP endpoint is `GET /version` on the engine.
 * `signed: false` means the binary did not come from a verified release
 * pipeline (local builds, dev builds, or releases where signing was
 * skipped or failed).
 */
export interface VersionResponse {
  version: string;
  git_commit?: string;
  git_ref?: string;
  release_tag?: string;
  built_at?: string;
  signed: boolean;
}

export type FetchResult = VersionResponse | { error: string };

const REPO_URL = 'https://github.com/stateset/stateset-icommerce';

function commitHref(sha: string): string {
  return `${REPO_URL}/commit/${sha}`;
}

function releaseHref(tag: string): string {
  return `${REPO_URL}/releases/tag/${tag}`;
}

function shortenCommit(sha: string): string {
  return sha.length > 12 ? sha.slice(0, 12) : sha;
}

/**
 * Pure renderer split out from the async page so component tests can exercise
 * it directly without mocking `fetch` or the Next runtime.
 */
export function BuildInfoView({ result }: { result: FetchResult }) {
  const failed = 'error' in result;
  const version = failed ? null : result;

  return (
    <div className="container mx-auto py-8 max-w-3xl space-y-6">
      <div>
        <h1 className="text-2xl font-bold">Build &amp; Release</h1>
        <p className="text-sm text-ds-muted-foreground mt-1">
          What binary is running, where it came from, and whether the release pipeline signed it.
        </p>
      </div>

      {failed ? (
        <Card>
          <CardContent className="py-6">
            <div className="text-sm">
              <Badge color="red">Engine unreachable</Badge>
            </div>
            <p className="mt-3 text-sm text-ds-muted-foreground">
              Could not fetch <code>/version</code>: {result.error}. Check that the engine is
              running and that
              <code className="mx-1">STATESET_API_URL</code> points to it.
            </p>
          </CardContent>
        </Card>
      ) : (
        <Card>
          <CardHeader className="flex flex-row items-center justify-between gap-4">
            <div>
              <h2 className="text-lg font-semibold">Verification</h2>
              <p className="text-sm text-ds-muted-foreground">
                Trust signal for the running binary
              </p>
            </div>
            {version!.signed ? (
              <Badge color="emerald" data-testid="trust-badge">
                Signed release
              </Badge>
            ) : (
              <Badge color="amber" data-testid="trust-badge">
                Unsigned build
              </Badge>
            )}
          </CardHeader>
          <CardContent className="text-sm text-ds-foreground">
            {version!.signed ? (
              <p>
                This binary was built by the StateSet release pipeline and signed via sigstore. The
                artifacts can be verified against the public transparency log.
              </p>
            ) : (
              <p>
                <strong>This binary did not come from a verified release pipeline.</strong> It may
                be a local build, a development build, or a release where signing was skipped. Do
                not rely on it for production audits.
              </p>
            )}
          </CardContent>
        </Card>
      )}

      {!failed && (
        <Card>
          <CardHeader>
            <h2 className="text-lg font-semibold">Build metadata</h2>
          </CardHeader>
          <CardContent className="text-sm">
            <dl className="grid grid-cols-1 sm:grid-cols-3 gap-y-3 gap-x-4">
              <dt className="font-medium text-ds-muted-foreground">Version</dt>
              <dd className="sm:col-span-2 font-mono">{version!.version}</dd>

              <dt className="font-medium text-ds-muted-foreground">Release tag</dt>
              <dd className="sm:col-span-2 font-mono">
                {version!.release_tag ? (
                  <a
                    href={releaseHref(version!.release_tag)}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-ds-primary hover:underline"
                  >
                    {version!.release_tag}
                  </a>
                ) : (
                  <span className="text-ds-muted-foreground">Not set</span>
                )}
              </dd>

              <dt className="font-medium text-ds-muted-foreground">Git commit</dt>
              <dd className="sm:col-span-2 font-mono">
                {version!.git_commit ? (
                  <a
                    href={commitHref(version!.git_commit)}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-ds-primary hover:underline"
                  >
                    {shortenCommit(version!.git_commit)}
                  </a>
                ) : (
                  <span className="text-ds-muted-foreground">Not set</span>
                )}
              </dd>

              <dt className="font-medium text-ds-muted-foreground">Git ref</dt>
              <dd className="sm:col-span-2 font-mono">
                {version!.git_ref ?? <span className="text-ds-muted-foreground">Not set</span>}
              </dd>

              <dt className="font-medium text-ds-muted-foreground">Built at</dt>
              <dd className="sm:col-span-2 font-mono">
                {version!.built_at ? (
                  <time dateTime={version!.built_at}>{version!.built_at}</time>
                ) : (
                  <span className="text-ds-muted-foreground">Not set</span>
                )}
              </dd>
            </dl>
          </CardContent>
        </Card>
      )}

      <Card>
        <CardHeader>
          <h2 className="text-lg font-semibold">How signing works</h2>
        </CardHeader>
        <CardContent className="text-sm text-ds-foreground space-y-2">
          <p>
            Releases are built via GitHub Actions and signed via sigstore using OIDC keyless
            signing. The public transparency log anchors each signature so anyone can verify a
            binary came from a specific commit on a specific workflow run.
          </p>
          <p>
            Local <code>cargo build</code> runs do not pass through the release pipeline, so they
            show as <em>Unsigned build</em> here even when the version number matches a real
            release. To run a verified binary, install from the release artifacts published to
            GitHub Releases.
          </p>
        </CardContent>
      </Card>
    </div>
  );
}
