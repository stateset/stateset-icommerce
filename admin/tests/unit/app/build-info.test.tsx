/**
 * Component tests for the Build Info page renderer.
 *
 * The page itself is an async server component that awaits a `/version`
 * fetch; we extract the rendering logic into a pure `BuildInfoView`
 * component (exported alongside the default page export) so tests can
 * drive it directly without mocking the Next runtime or fetch.
 *
 * Coverage:
 *  - happy path (signed release with all metadata)
 *  - unsigned local-build path
 *  - missing optional fields render as "Not set"
 *  - engine-unreachable error path
 *  - commit and release links resolve to the correct GitHub URLs
 */

import React from 'react';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { BuildInfoView } from '@/app/build-info/page';

afterEach(() => {
  cleanup();
});

describe('<BuildInfoView />', () => {
  it('renders the signed-release happy path with all metadata', () => {
    render(
      <BuildInfoView
        result={{
          version: '1.0.4',
          git_commit: 'abc123def456ghi789',
          git_ref: 'main',
          release_tag: 'v1.0.4',
          built_at: '2026-05-08T00:00:00Z',
          signed: true,
        }}
      />,
    );

    // Trust badge says signed.
    expect(screen.getByTestId('trust-badge').textContent).toBe('Signed release');
    // Version is rendered.
    expect(screen.getByText('1.0.4')).toBeInTheDocument();
    // Release tag links to the GitHub release page.
    const releaseLink = screen.getByRole('link', { name: 'v1.0.4' });
    expect(releaseLink).toHaveAttribute(
      'href',
      'https://github.com/stateset/stateset-icommerce/releases/tag/v1.0.4',
    );
    // Commit SHA is shortened in the visible text but the href has the full SHA.
    const commitLink = screen.getByRole('link', { name: /^abc123def456$/ });
    expect(commitLink).toHaveAttribute(
      'href',
      'https://github.com/stateset/stateset-icommerce/commit/abc123def456ghi789',
    );
    // Build timestamp is rendered as a <time> element.
    const builtAt = screen.getByText('2026-05-08T00:00:00Z');
    expect(builtAt.tagName.toLowerCase()).toBe('time');
    expect(builtAt).toHaveAttribute('datetime', '2026-05-08T00:00:00Z');
  });

  it('renders the unsigned-build warning prominently', () => {
    render(
      <BuildInfoView
        result={{
          version: '1.0.4',
          signed: false,
        }}
      />,
    );

    expect(screen.getByTestId('trust-badge').textContent).toBe('Unsigned build');
    // The warning copy makes "did not come from a verified release pipeline" prominent.
    expect(
      screen.getByText(/did not come from a verified release pipeline/i),
    ).toBeInTheDocument();
  });

  it('renders "Not set" for missing optional fields', () => {
    render(
      <BuildInfoView
        result={{
          version: '0.0.0-dev',
          signed: false,
          // git_commit, git_ref, release_tag, built_at all absent
        }}
      />,
    );

    // "Not set" appears once per missing optional field (4 of them).
    const notSet = screen.getAllByText('Not set');
    expect(notSet.length).toBe(4);
    // No external links rendered when commit/release_tag are missing.
    expect(screen.queryByRole('link', { name: /releases\/tag/i })).toBeNull();
  });

  it('renders the engine-unreachable error path', () => {
    render(
      <BuildInfoView
        result={{ error: 'connect ECONNREFUSED 127.0.0.1:8080' }}
      />,
    );

    expect(screen.getByText('Engine unreachable')).toBeInTheDocument();
    expect(
      screen.getByText(/Could not fetch.*ECONNREFUSED/i),
    ).toBeInTheDocument();
    // Build-metadata card should be suppressed on error.
    expect(screen.queryByText('Build metadata')).toBeNull();
  });

  it('shortens long commit SHAs to 12 chars in the link text', () => {
    render(
      <BuildInfoView
        result={{
          version: '1.0.4',
          git_commit: '0123456789abcdef0123456789abcdef01234567', // 40 chars
          signed: true,
        }}
      />,
    );

    const link = screen.getByRole('link', { name: /^0123456789ab$/ });
    expect(link).toBeInTheDocument();
    // Full SHA is preserved in the href.
    expect(link).toHaveAttribute(
      'href',
      'https://github.com/stateset/stateset-icommerce/commit/0123456789abcdef0123456789abcdef01234567',
    );
  });

  it('shows "How signing works" educational copy regardless of state', () => {
    // Even on error, the "how signing works" section explains the model.
    render(<BuildInfoView result={{ error: 'down' }} />);
    expect(screen.getByText('How signing works')).toBeInTheDocument();
    expect(
      screen.getByText(/sigstore using OIDC keyless signing/i),
    ).toBeInTheDocument();
  });
});
