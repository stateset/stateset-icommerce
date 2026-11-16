'use client';

import { useState, type FormEvent } from 'react';
import { Card, CardContent } from '@stateset/design';

export interface ErpField {
  name: string;
  label: string;
  optional?: boolean;
  hint?: string;
  options?: { value: string; label: string }[];
}

/** Review the exact submitted values before performing a financial action. */
export default function ErpActionForm({
  title,
  description,
  fields,
  submit,
  onComplete,
}: {
  title: string;
  description: string;
  fields: ErpField[];
  submit: (values: Record<string, string>) => Promise<unknown>;
  onComplete?: () => Promise<void>;
}) {
  const [review, setReview] = useState<Record<string, string> | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  const [complete, setComplete] = useState(false);
  function preview(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError('');
    setComplete(false);
    setReview(
      Object.fromEntries(new FormData(event.currentTarget).entries()) as Record<string, string>,
    );
  }
  async function confirm() {
    if (!review || pending) return;
    setPending(true);
    setError('');
    try {
      await submit(review);
      setReview(null);
      setComplete(true);
      await onComplete?.();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Action failed');
    } finally {
      setPending(false);
    }
  }
  return (
    <Card>
      <CardContent>
        <h2 className="font-semibold">{title}</h2>
        <p className="mb-4 text-sm text-ds-muted-foreground">{description}</p>
        <form onSubmit={preview} className="space-y-3">
          <fieldset disabled={pending || !!review} className="space-y-3">
            {fields.map((field) => (
              <label key={field.name} className="block text-sm">
                <span>{field.label}</span>
                {field.options ? (
                  <select
                    name={field.name}
                    required={!field.optional}
                    className="mt-1 block w-full rounded border border-ds-border bg-ds-background p-2"
                  >
                    <option value="">Choose {field.label.toLowerCase()}</option>
                    {field.options.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                ) : (
                  <input
                    name={field.name}
                    required={!field.optional}
                    autoComplete="off"
                    className="mt-1 block w-full rounded border border-ds-border bg-ds-background p-2"
                  />
                )}
                {field.hint && (
                  <span className="text-xs text-ds-muted-foreground">{field.hint}</span>
                )}
              </label>
            ))}
          </fieldset>
          {!review && (
            <button className="rounded border border-ds-border px-3 py-2" type="submit">
              Review {title.toLowerCase()}
            </button>
          )}
        </form>
        {review && (
          <div className="mt-4 space-y-3" aria-label={`Review ${title.toLowerCase()}`}>
            <dl>
              {fields.map((field) => (
                <div key={field.name} className="text-sm">
                  <dt className="font-medium">{field.label}</dt>
                  <dd className="break-all">
                    {field.options?.find((v) => v.value === review[field.name])?.label ??
                      (review[field.name] || '—')}
                  </dd>
                </div>
              ))}
            </dl>
            <button
              type="button"
              disabled={pending}
              onClick={confirm}
              className="rounded bg-ds-primary px-3 py-2 text-ds-primary-foreground"
            >
              {pending ? 'Saving…' : `Confirm ${title.toLowerCase()}`}
            </button>{' '}
            <button
              type="button"
              disabled={pending}
              onClick={() => setReview(null)}
              className="rounded border border-ds-border px-3 py-2"
            >
              Edit
            </button>
          </div>
        )}
        {error && (
          <p className="mt-3 text-sm text-ds-status-fail" role="alert">
            {error}
          </p>
        )}
        {complete && (
          <p className="mt-3 text-sm" role="status">
            {title} completed.
          </p>
        )}
      </CardContent>
    </Card>
  );
}
