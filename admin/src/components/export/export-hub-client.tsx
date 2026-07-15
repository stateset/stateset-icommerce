'use client';

import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { CsvExportButton } from '@/components/export/csv-export-button';
import { getCustomers, getInventory, getOrders } from '@/app/actions/commerce';
import {
  CUSTOMER_CSV_COLUMNS,
  INVENTORY_CSV_COLUMNS,
  ORDER_CSV_COLUMNS,
} from '@/lib/csv/specs';

interface EntityCardProps {
  title: string;
  description: string;
  columnCount: number;
  exportButton: React.ReactNode;
}

function EntityCard({ title, description, columnCount, exportButton }: EntityCardProps) {
  return (
    <Card decoration="left" decorationColor="indigo">
      <CardHeader>
        <div className="flex items-start justify-between gap-3">
          <div>
            <h2 className="text-lg font-semibold">{title}</h2>
            <p className="text-sm text-ds-muted-foreground mt-1">{description}</p>
          </div>
          <Badge color="indigo">{columnCount} cols</Badge>
        </div>
      </CardHeader>
      <CardContent className="flex items-center justify-end">{exportButton}</CardContent>
    </Card>
  );
}

export function ExportHubClient() {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
      <EntityCard
        title="Orders"
        description="Status, totals, item counts, timestamps. Sortable in any spreadsheet."
        columnCount={ORDER_CSV_COLUMNS.length}
        exportButton={
          <CsvExportButton
            fetchRows={() => getOrders({ limit: 1000 })}
            columns={ORDER_CSV_COLUMNS}
            filenamePrefix="orders"
            label="Export orders CSV"
          />
        }
      />
      <EntityCard
        title="Customers"
        description="Lifetime spend, order count, tags, last-order date. CRM-ready."
        columnCount={CUSTOMER_CSV_COLUMNS.length}
        exportButton={
          <CsvExportButton
            fetchRows={() => getCustomers({ limit: 1000 })}
            columns={CUSTOMER_CSV_COLUMNS}
            filenamePrefix="customers"
            label="Export customers CSV"
          />
        }
      />
      <EntityCard
        title="Inventory"
        description="On-hand, reserved, available, reorder points by SKU + warehouse."
        columnCount={INVENTORY_CSV_COLUMNS.length}
        exportButton={
          <CsvExportButton
            fetchRows={() => getInventory()}
            columns={INVENTORY_CSV_COLUMNS}
            filenamePrefix="inventory"
            label="Export inventory CSV"
          />
        }
      />
    </div>
  );
}
