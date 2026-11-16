'use client';

import { Card, CardContent } from '@stateset/design';

/**
 * Shown in place of a widget whose data the connected engine build cannot
 * provide (see `EngineFeatureUnavailable`). States what is missing instead
 * of failing or inventing numbers.
 */
export function EngineFeatureNotice({ title, message }: { title: string; message: string }) {
  return (
    <Card className="border-ds-status-warn/30">
      <CardContent className="p-5">
        <p className="text-sm font-medium text-ds-foreground">{title} unavailable</p>
        <p className="mt-1 text-sm text-ds-muted-foreground" role="status">
          {message}
        </p>
      </CardContent>
    </Card>
  );
}
