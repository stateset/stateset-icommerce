'use server';

/**
 * Operations server actions (purchasing, warehouse, manufacturing).
 *
 * Every exported action is gated by `requireAdminSession()` — server actions
 * bypass the API middleware, so each one must enforce the admin session
 * itself (skipped in the auth-disabled dev mode, like middleware).
 *
 * Read-only slice: no mutations are exposed from these pages. Amounts and
 * exact decimal quantities from the engine are passed through untouched;
 * formatting is display-only in the client components.
 */

import {
  purchaseOrdersApi,
  warehouseApi,
  cycleCountsApi,
  workOrdersApi,
  qualityApi,
  type PurchaseOrder,
  type Supplier,
  type WarehouseRecord,
  type WarehouseLocation,
  type CycleCount,
  type CycleCountFilter,
  type WorkOrder,
  type QualityInspection,
  type NonConformanceReport,
} from '@/lib/embedded';
import { requireAdminSession } from '@/lib/shared/auth-session';

function assertNonEmpty(value: string, label: string): void {
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new Error(`${label} is required`);
  }
}

function assertWarehouseId(value: number, label: string): void {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${label} must be a positive integer`);
  }
}

// ============================================================================
// Purchasing
// ============================================================================

export async function getPurchaseOrders(): Promise<PurchaseOrder[]> {
  await requireAdminSession();
  return purchaseOrdersApi.list();
}

export async function getPurchaseOrder(id: string): Promise<PurchaseOrder | null> {
  await requireAdminSession();
  assertNonEmpty(id, 'id');
  return purchaseOrdersApi.get(id);
}

export async function getSuppliers(): Promise<Supplier[]> {
  await requireAdminSession();
  return purchaseOrdersApi.listSuppliers();
}

/** Purchase orders + suppliers in one round trip for the purchasing page. */
export async function getPurchasingPageData(): Promise<{
  purchaseOrders: PurchaseOrder[];
  suppliers: Supplier[];
}> {
  await requireAdminSession();
  const [purchaseOrders, suppliers] = await Promise.all([
    purchaseOrdersApi.list(),
    purchaseOrdersApi.listSuppliers(),
  ]);
  return { purchaseOrders, suppliers };
}

// ============================================================================
// Warehouse
// ============================================================================

export async function getWarehouses(): Promise<WarehouseRecord[]> {
  await requireAdminSession();
  return warehouseApi.listWarehouses();
}

export async function getWarehouseLocations(
  warehouseId?: number
): Promise<WarehouseLocation[]> {
  await requireAdminSession();
  if (warehouseId !== undefined) {
    assertWarehouseId(warehouseId, 'warehouseId');
  }
  return warehouseApi.listLocations(warehouseId);
}

export async function getCycleCounts(filter?: CycleCountFilter): Promise<CycleCount[]> {
  await requireAdminSession();
  if (filter?.warehouseId !== undefined) {
    assertWarehouseId(filter.warehouseId, 'warehouseId');
  }
  return cycleCountsApi.list(filter);
}

/** Warehouses + locations + cycle counts in one round trip. */
export async function getWarehousePageData(): Promise<{
  warehouses: WarehouseRecord[];
  locations: WarehouseLocation[];
  cycleCounts: CycleCount[];
}> {
  await requireAdminSession();
  const [warehouses, locations, cycleCounts] = await Promise.all([
    warehouseApi.listWarehouses(),
    warehouseApi.listLocations(),
    cycleCountsApi.list(),
  ]);
  return { warehouses, locations, cycleCounts };
}

// ============================================================================
// Manufacturing + quality
// ============================================================================

export async function getWorkOrders(): Promise<WorkOrder[]> {
  await requireAdminSession();
  return workOrdersApi.list();
}

export async function getWorkOrder(id: string): Promise<WorkOrder | null> {
  await requireAdminSession();
  assertNonEmpty(id, 'id');
  return workOrdersApi.get(id);
}

export async function getQualityInspections(): Promise<QualityInspection[]> {
  await requireAdminSession();
  return qualityApi.listInspections();
}

export async function getNonConformanceReports(): Promise<NonConformanceReport[]> {
  await requireAdminSession();
  return qualityApi.listNcrs();
}

/** Work orders + quality inspections + NCRs in one round trip. */
export async function getManufacturingPageData(): Promise<{
  workOrders: WorkOrder[];
  inspections: QualityInspection[];
  ncrs: NonConformanceReport[];
}> {
  await requireAdminSession();
  const [workOrders, inspections, ncrs] = await Promise.all([
    workOrdersApi.list(),
    qualityApi.listInspections(),
    qualityApi.listNcrs(),
  ]);
  return { workOrders, inspections, ncrs };
}
