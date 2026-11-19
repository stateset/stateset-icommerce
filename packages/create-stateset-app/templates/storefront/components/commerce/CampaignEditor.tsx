'use client';

import { useState } from 'react';

type Variant = {
  id: string;
  title: string;
  sku: string;
  weight: number;
  description?: string;
  bumpSku?: string;
  upsellSku?: string;
  couponCode?: string;
};
export type Campaign = { id: string; revision: number; variants: Variant[] };
type Product = { sku: string; name: string; price: string };
const initial = (): Campaign => ({
  id: '',
  revision: 0,
  variants: [{ id: 'control', weight: 1, sku: '', title: '' }],
});
const fields: [Exclude<keyof Variant, 'weight'>, string][] = [
  ['id', 'Variant ID'],
  ['title', 'Landing page title'],
  ['description', 'Description'],
  ['sku', 'Product SKU'],
  ['bumpSku', 'Checkout add-on SKU'],
  ['upsellSku', 'Post-purchase offer SKU'],
  ['couponCode', 'Coupon code'],
];

export function CampaignEditor({
  catalog,
  onPublish,
}: {
  catalog: Campaign[];
  onPublish: () => Promise<void>;
}) {
  const [draft, setDraft] = useState<Campaign>(initial);
  const [selected, setSelected] = useState('');
  const [preview, setPreview] = useState<Product[] | null>(null);
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  function edit(next: Campaign) {
    setDraft(next);
    setPreview(null);
    setMessage('');
  }
  function editVariant(index: number, update: Partial<Variant>) {
    edit({
      ...draft,
      variants: draft.variants.map((v, i) => (i === index ? { ...v, ...update } : v)),
    });
  }
  async function submit(apply: boolean) {
    setBusy(true);
    setMessage('');
    try {
      const variants = draft.variants.map((v) =>
        Object.fromEntries(Object.entries(v).filter(([, value]) => value !== '')),
      );
      const response = await fetch('/api/portal', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ campaign: { ...draft, variants }, apply }),
      });
      const result = await response.json();
      if (!response.ok) throw new Error(result.error || 'Unable to save this campaign');
      if (apply) {
        setDraft({ ...draft, revision: result.revision });
        setSelected(draft.id);
        setPreview(null);
        await onPublish();
        setMessage('Published revision ' + result.revision);
      } else setPreview(result.products);
    } catch (e) {
      setMessage(e instanceof Error ? e.message : 'Unable to save this campaign');
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="space-y-4">
      <h2 className="text-lg font-semibold">Campaign editor</h2>
      <label className="block">
        Campaign
        <select
          className="ml-2 rounded border p-2"
          value={selected}
          onChange={(event) => {
            setSelected(event.target.value);
            edit(catalog.find((c) => c.id === event.target.value) || initial());
          }}
        >
          <option value="">New campaign</option>
          {catalog.map((c) => (
            <option value={c.id} key={c.id}>
              {c.id}
            </option>
          ))}
        </select>
      </label>
      <form
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          void submit(false);
        }}
      >
        <label className="block">
          Campaign ID
          <input
            required
            readOnly={Boolean(selected)}
            className="ml-2 rounded border p-2"
            value={draft.id}
            pattern="[a-z0-9][a-z0-9_\-]{0,47}"
            onChange={(event) => edit({ ...draft, id: event.target.value })}
          />
        </label>
        {draft.variants.map((v, index) => (
          <fieldset key={index} className="grid gap-3 rounded border p-4 sm:grid-cols-2">
            <legend>Variant {index + 1}</legend>
            {fields.map(([key, label]) => (
              <label key={key}>
                {label}
                <input
                  className="block w-full rounded border p-2"
                  required={['sku', 'title', 'id'].includes(key)}
                  value={v[key] || ''}
                  onChange={(event) => editVariant(index, { [key]: event.target.value })}
                />
              </label>
            ))}
            <label>
              Traffic weight
              <input
                type="number"
                required
                min="1"
                max="10000"
                className="block rounded border p-2"
                value={v.weight || ''}
                onChange={(event) => editVariant(index, { weight: Number(event.target.value) })}
              />
            </label>
            {draft.variants.length > 1 && (
              <button
                type="button"
                className="underline"
                onClick={() =>
                  edit({ ...draft, variants: draft.variants.filter((_, i) => i !== index) })
                }
              >
                Remove variant
              </button>
            )}
          </fieldset>
        ))}
        <div className="flex gap-4">
          <button
            type="button"
            className="underline"
            disabled={draft.variants.length === 32}
            onClick={() =>
              edit({
                ...draft,
                variants: [...draft.variants, { id: '', sku: '', title: '', weight: 1 }],
              })
            }
          >
            Add variant
          </button>
          <button disabled={busy} className="rounded border px-3 py-2">
            Preview campaign
          </button>
        </div>
      </form>
      {preview && (
        <div className="space-y-2">
          <h3 className="font-semibold">Preview</h3>
          {draft.variants.map((v) => (
            <article className="rounded border p-3" key={v.id}>
              <h4>{v.title}</h4>
              <p>{v.description}</p>
              {[
                ['Product', v.sku],
                ['Checkout add-on', v.bumpSku],
                ['Post-purchase offer', v.upsellSku],
              ]
                .filter(([, sku]) => sku)
                .map(([label, sku]) => {
                  const item = preview.find((p) => p.sku === sku);
                  return (
                    <p key={label}>
                      {label} · {item?.name} · USD {item?.price}
                    </p>
                  );
                })}
              {v.couponCode && <p>Coupon · {v.couponCode}</p>}
            </article>
          ))}
          <button
            disabled={busy}
            className="rounded bg-black px-4 py-2 text-white"
            onClick={() => submit(true)}
          >
            Publish campaign
          </button>
        </div>
      )}
      {message && (
        <p role="status">
          {message}{' '}
          {message.startsWith('Published') && (
            <a className="underline" href={'/c/' + draft.id}>
              Open campaign
            </a>
          )}
        </p>
      )}
    </section>
  );
}
