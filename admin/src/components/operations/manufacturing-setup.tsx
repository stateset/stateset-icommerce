'use client';
import { Card, CardContent } from '@stateset/design';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import {
  getManufacturingSetupData,
  createManufacturingBom,
  addManufacturingComponent,
  publishManufacturingBom,
  createManufacturingBatch,
  setManufacturingCost,
} from '@/app/actions/erp';
import ErpActionForm from '@/components/finance/erp-action-form';
import { formatMoney } from '@/lib/finance/format';

export default function ManufacturingSetup() {
  const { data, error, refetch } = useEmbeddedData(getManufacturingSetupData);
  if (error) return <p role="alert">Manufacturing setup unavailable: {error.message}</p>;
  if (!data) return <p>Loading BOMs, batches and costs…</p>;
  const drafts = data.boms
    .filter((b) => b.status.toLowerCase() === 'draft')
    .map((b) => ({ value: b.id, label: `${b.bomNumber} · ${b.name} · revision ${b.revision}` }));
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-semibold">BOMs, batches and costs</h1>
        <p className="text-sm text-ds-muted-foreground">
          Prepare component recipes, group work orders and maintain item costs.
        </p>
      </div>
      <Card>
        <CardContent>
          <h2 className="font-semibold">Bills of materials</h2>
          {data.boms.length === 0 && <p>No bills of materials.</p>}
          {data.boms.map((bom) => (
            <details key={bom.id} className="border-b border-ds-border py-3">
              <summary>
                {bom.bomNumber} · {bom.name} · revision {bom.revision} · {bom.status}
              </summary>
              <ul className="mt-2 space-y-1">
                {bom.components.map((c) => (
                  <li key={c.id}>
                    {c.componentSku ?? c.name} — {c.quantity} {c.unitOfMeasure}
                  </li>
                ))}
              </ul>
            </details>
          ))}
        </CardContent>
      </Card>
      <div className="grid gap-4 lg:grid-cols-3">
        <ErpActionForm
          title="Create BOM"
          description="Create a draft recipe for a product. Add components before publishing."
          fields={[
            { name: 'name', label: 'Name' },
            {
              name: 'productId',
              label: 'Product',
              options: data.products.map((p) => ({ value: p.id, label: p.name })),
            },
          ]}
          submit={createManufacturingBom}
          onComplete={refetch}
        />
        <ErpActionForm
          title="Add component"
          description="Only draft revisions can be edited. Quantities must be positive."
          fields={[
            { name: 'bomId', label: 'Draft BOM', options: drafts },
            { name: 'componentSku', label: 'Component SKU' },
            { name: 'name', label: 'Component name' },
            { name: 'quantity', label: 'Quantity per finished unit' },
          ]}
          submit={addManufacturingComponent}
          onComplete={refetch}
        />
        <ErpActionForm
          title="Publish BOM"
          description="Publishing makes this revision immutable. Later changes require a new revision."
          fields={[{ name: 'bomId', label: 'Draft BOM', options: drafts }]}
          submit={publishManufacturingBom}
          onComplete={refetch}
        />
      </div>
      <Card>
        <CardContent>
          <h2 className="font-semibold">Production batches</h2>
          {data.batches.length === 0 && <p>No production batches.</p>}
          {data.batches.map((batch) => (
            <div key={batch.id} className="border-b border-ds-border py-3">
              <p>
                {batch.name} · {batch.status}
              </p>
              <ul>
                {batch.workOrderIds.map((id) => (
                  <li key={id}>
                    {data.workOrders.find((w) => w.id === id)?.workOrderNumber ?? id}
                  </li>
                ))}
              </ul>
            </div>
          ))}
          <details className="mt-4">
            <summary>Available work order IDs</summary>
            <ul>
              {data.workOrders.map((w) => (
                <li key={w.id}>
                  {w.workOrderNumber} · {w.status} · <code>{w.id}</code>
                </li>
              ))}
            </ul>
          </details>
        </CardContent>
      </Card>
      <ErpActionForm
        title="Create production batch"
        description="Link existing work orders. The engine derives batch progress from its members."
        fields={[
          { name: 'name', label: 'Batch name' },
          {
            name: 'workOrderIds',
            label: 'Work order IDs',
            hint: 'Separate IDs with commas or spaces.',
          },
        ]}
        submit={createManufacturingBatch}
        onComplete={refetch}
      />
      <Card>
        <CardContent>
          <h2 className="font-semibold">Item costs</h2>
          <div className="overflow-x-auto">
            <table className="min-w-full text-sm">
              <thead>
                <tr>
                  {['SKU', 'Method', 'Standard', 'Average', 'Material', 'Labor', 'Overhead'].map(
                    (h) => (
                      <th key={h} className="p-2 text-left">
                        {h}
                      </th>
                    ),
                  )}
                </tr>
              </thead>
              <tbody>
                {data.costs.map((c) => (
                  <tr key={c.id}>
                    <td className="p-2">{c.sku}</td>
                    <td className="p-2">{c.costMethod}</td>
                    {[
                      c.standardCost,
                      c.averageCost,
                      c.materialCost,
                      c.laborCost,
                      c.overheadCost,
                    ].map((v, i) => (
                      <td key={i} className="p-2 font-mono">
                        {formatMoney(v, 'Base')}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="mt-2 text-sm text-ds-muted-foreground">
            Costs are in the store's base currency. Stock receipts and issues maintain carrying
            values.
          </p>
        </CardContent>
      </Card>
      <ErpActionForm
        title="Set item cost"
        description="Maintain standard cost and its components. The engine refuses unsafe cost-method changes while stock is held."
        fields={[
          { name: 'sku', label: 'SKU' },
          {
            name: 'costMethod',
            label: 'Cost method',
            options: ['fifo', 'lifo', 'average', 'standard'].map((value) => ({
              value,
              label: value.toUpperCase(),
            })),
          },
          ...['standardCost', 'materialCost', 'laborCost', 'overheadCost'].map((name, i) => ({
            name,
            label: ['Standard cost', 'Material cost', 'Labor cost', 'Overhead cost'][i],
            hint: 'Exact decimal in base currency; enter 0 when unused.',
          })),
        ]}
        submit={setManufacturingCost}
        onComplete={refetch}
      />
    </div>
  );
}
