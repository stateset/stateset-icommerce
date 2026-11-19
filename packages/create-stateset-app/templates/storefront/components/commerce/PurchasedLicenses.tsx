export interface PurchasedLicense {
  resource: string;
  licenseKey: string;
  hasDownload: boolean;
  active?: boolean;
  expires?: number | null;
}

export function PurchasedLicenses({ licenses }: { licenses: PurchasedLicense[] }) {
  return (
    <>
      {licenses.map((license) => (
        <section key={license.resource} className="space-y-2">
          <h2 className="font-semibold">{license.resource}</h2>
          <p>
            {license.active === false ? 'Access inactive' : 'Access active'}
            {license.expires ? ' · Expires ' + new Date(license.expires).toLocaleDateString() : ''}
          </p>
          <label className="block">
            License key
            <input
              readOnly
              value={license.licenseKey}
              onFocus={(e) => e.target.select()}
              className="w-full rounded border p-2 font-mono"
            />
          </label>
          {license.hasDownload && (
            <form action="/api/access/download" method="post">
              <input type="hidden" name="resource" value={license.resource} />
              <input type="hidden" name="license" value={license.licenseKey} />
              <button className="underline">Download</button>
            </form>
          )}
        </section>
      ))}
    </>
  );
}
