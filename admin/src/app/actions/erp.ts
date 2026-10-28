'use server';

import { z } from 'zod';
import { revalidatePath } from 'next/cache';
import { getErpEngine } from '@/lib/erp';
import { requireAdminSession } from '@/lib/shared/auth-session';
import { requireFinanceActor } from '@/lib/shared/finance-actor';

const id = z.string().uuid();
const amount = z
  .string()
  .regex(/^\d+(\.\d+)?$/, 'Enter an exact decimal amount')
  .refine((v) => /[1-9]/.test(v), 'Amount must be positive');
const cost = z.string().regex(/^\d+(\.\d+)?$/, 'Enter an exact decimal cost');
const method = z.enum(['ach', 'wire', 'check', 'cash', 'credit_card', 'other']);
type FormValues = Record<string, string>;

export async function approveSupplierBill(input: FormValues) {
  const actor = await requireFinanceActor();
  const args = z
    .object({ billId: id, overrideReason: z.string().trim().max(2000).optional() })
    .parse(input);
  await (
    await getErpEngine()
  ).accountsPayable.approveBill(args.billId, args.overrideReason || undefined, actor);
  revalidatePath('/finance/bills');
}

export async function paySupplierBill(input: FormValues) {
  await requireFinanceActor();
  const args = z
    .object({
      billId: id,
      amount,
      paymentMethod: method,
      referenceNumber: z.string().trim().max(200),
    })
    .parse(input);
  const engine = await getErpEngine();
  const bill = await engine.accountsPayable.getBill(args.billId);
  if (!bill) throw new Error('Bill not found');
  await engine.accountsPayable.createPayment({
    supplierId: bill.supplierId,
    currency: bill.currency,
    amount: args.amount,
    paymentMethod: args.paymentMethod,
    referenceNumber: args.referenceNumber || undefined,
    allocations: [{ billId: bill.id, amount: args.amount }],
  });
  revalidatePath('/finance/bills');
}

export async function applyReceivablePayment(input: FormValues) {
  await requireFinanceActor();
  const args = z.object({ invoiceId: id, paymentId: id, amount }).parse(input);
  await (
    await getErpEngine()
  ).accountsReceivable.applyPaymentToInvoices({
    paymentId: args.paymentId,
    applications: [{ invoiceId: args.invoiceId, amount: args.amount }],
  });
  revalidatePath('/finance/receivables');
}

export async function applyReceivableCredit(input: FormValues) {
  await requireFinanceActor();
  const args = z.object({ invoiceId: id, creditMemoId: id, amount }).parse(input);
  await (await getErpEngine()).accountsReceivable.applyCreditMemo(args);
  revalidatePath('/finance/receivables');
}

export async function writeOffReceivable(input: FormValues) {
  const actor = await requireFinanceActor();
  const args = z
    .object({
      invoiceId: id,
      amount,
      reason: z.enum([
        'uncollectible',
        'bankruptcy',
        'customer_dispute',
        'small_balance',
        'account_closed',
        'deceased',
        'other',
      ]),
      notes: z.string().max(2000).optional(),
    })
    .parse(input);
  await (await getErpEngine()).accountsReceivable.createWriteOff(args, actor);
  revalidatePath('/finance/receivables');
}

export async function reverseReceivableWriteOff(input: FormValues) {
  await requireFinanceActor();
  await (await getErpEngine()).accountsReceivable.reverseWriteOff(id.parse(input.writeOffId));
  revalidatePath('/finance/receivables');
}

export async function getReceivableActionsData() {
  await requireAdminSession();
  const engine = await getErpEngine();
  const [invoices, credits, writeOffs] = await Promise.all([
    engine.invoices.list(),
    engine.accountsReceivable.listCreditMemos(),
    engine.accountsReceivable.listWriteOffs({ includeReversed: false }),
  ]);
  return { invoices, credits, writeOffs };
}

export async function getManufacturingSetupData() {
  await requireAdminSession();
  const engine = await getErpEngine();
  const [boms, batches, costs, workOrders, products] = await Promise.all([
    engine.bom.list(),
    engine.productionBatches.list(),
    engine.costAccounting.listItemCosts(),
    engine.workOrders.list(),
    engine.products.list(),
  ]);
  const components = await Promise.all(boms.map((bom) => engine.bom.getComponents(bom.id)));
  return {
    boms: boms.map((bom, index) => ({ ...bom, components: components[index] })),
    batches,
    costs,
    workOrders,
    products,
  };
}

export async function createManufacturingBom(input: FormValues) {
  await requireFinanceActor();
  const args = z.object({ name: z.string().trim().min(1), productId: id }).parse(input);
  await (await getErpEngine()).bom.create(args);
  revalidatePath('/operations/manufacturing/setup');
}

export async function addManufacturingComponent(input: FormValues) {
  await requireFinanceActor();
  const args = z
    .object({
      bomId: id,
      componentSku: z.string().trim().min(1),
      name: z.string().trim().min(1),
      quantity: z.coerce.number().positive().finite(),
    })
    .parse(input);
  await (
    await getErpEngine()
  ).bom.addComponent(args.bomId, {
    componentSku: args.componentSku,
    name: args.name,
    quantity: args.quantity,
  });
  revalidatePath('/operations/manufacturing/setup');
}

export async function publishManufacturingBom(input: FormValues) {
  await requireFinanceActor();
  await (await getErpEngine()).bom.activate(id.parse(input.bomId));
  revalidatePath('/operations/manufacturing/setup');
}

export async function createManufacturingBatch(input: FormValues) {
  await requireFinanceActor();
  const args = z
    .object({
      name: z.string().trim().min(1),
      workOrderIds: z
        .string()
        .transform((v) => v.split(/[\s,]+/).filter(Boolean))
        .pipe(z.array(id).min(1)),
    })
    .parse(input);
  await (await getErpEngine()).productionBatches.create(args);
  revalidatePath('/operations/manufacturing/setup');
}

export async function setManufacturingCost(input: FormValues) {
  await requireFinanceActor();
  const args = z
    .object({
      sku: z.string().trim().min(1),
      costMethod: z.enum(['fifo', 'lifo', 'average', 'standard']),
      standardCost: cost,
      materialCost: cost,
      laborCost: cost,
      overheadCost: cost,
    })
    .parse(input);
  await (await getErpEngine()).costAccounting.setItemCost(args);
  revalidatePath('/operations/manufacturing/setup');
}
