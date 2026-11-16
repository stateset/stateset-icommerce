import { describe, it, expect, vi, beforeEach } from 'vitest';
const mocks = vi.hoisted(() => ({
  actor: vi.fn(),
  engine: vi.fn(),
  pay: vi.fn(),
  approve: vi.fn(),
  apply: vi.fn(),
  writeOff: vi.fn(),
  cost: vi.fn(),
}));
vi.mock('@/lib/shared/finance-actor', () => ({ requireFinanceActor: mocks.actor }));
vi.mock('@/lib/shared/auth-session', () => ({ requireAdminSession: vi.fn() }));
vi.mock('@/lib/erp', () => ({ getErpEngine: mocks.engine }));
vi.mock('next/cache', () => ({ revalidatePath: vi.fn() }));
import {
  paySupplierBill,
  approveSupplierBill,
  applyReceivablePayment,
  writeOffReceivable,
  setManufacturingCost,
} from '@/app/actions/erp';
const billId = '11111111-1111-4111-8111-111111111111';
const invoiceId = '22222222-2222-4222-8222-222222222222';
beforeEach(() => {
  vi.clearAllMocks();
  mocks.actor.mockResolvedValue('verified-admin');
  mocks.engine.mockResolvedValue({
    accountsPayable: {
      getBill: vi.fn().mockResolvedValue({ id: billId, supplierId: 'supplier', currency: 'EUR' }),
      createPayment: mocks.pay,
      approveBill: mocks.approve,
    },
    accountsReceivable: { applyPaymentToInvoices: mocks.apply, createWriteOff: mocks.writeOff },
    costAccounting: { setItemCost: mocks.cost },
  });
});
describe('ERP administrator actions', () => {
  it('requires elevated authentication before loading or mutating the engine', async () => {
    mocks.actor.mockRejectedValue(new Error('Finance actions require an administrator'));
    await expect(paySupplierBill({})).rejects.toThrow(/administrator/);
    await expect(writeOffReceivable({})).rejects.toThrow(/administrator/);
    expect(mocks.engine).not.toHaveBeenCalled();
  });
  it('preserves an amount above 2^53 and derives currency and supplier from the bill', async () => {
    const amount = '9007199254740993.01';
    await paySupplierBill({
      billId,
      amount,
      paymentMethod: 'wire',
      referenceNumber: 'BANK-1',
      currency: 'USD',
      supplierId: 'spoofed',
    });
    expect(mocks.pay).toHaveBeenCalledWith({
      supplierId: 'supplier',
      currency: 'EUR',
      amount,
      paymentMethod: 'wire',
      referenceNumber: 'BANK-1',
      allocations: [{ billId, amount }],
    });
  });
  it('refuses numeric money before reaching the engine', async () => {
    await expect(
      paySupplierBill({
        billId,
        amount: 0.1,
        paymentMethod: 'wire',
        referenceNumber: '',
      } as unknown as Record<string, string>),
    ).rejects.toThrow();
    expect(mocks.engine).not.toHaveBeenCalled();
  });
  it('stamps approval and write-off with the verified principal', async () => {
    await approveSupplierBill({ billId, overrideReason: 'Accepted shortage', actor: 'spoofed' });
    expect(mocks.approve).toHaveBeenCalledWith(billId, 'Accepted shortage', 'verified-admin');
    await writeOffReceivable({
      invoiceId,
      amount: '0.01',
      reason: 'small_balance',
      approvedBy: 'spoofed',
    });
    expect(mocks.writeOff).toHaveBeenCalledWith(
      { invoiceId, amount: '0.01', reason: 'small_balance' },
      'verified-admin',
    );
  });
  it('passes exact allocation and cost values through unchanged', async () => {
    await applyReceivablePayment({ invoiceId, paymentId: billId, amount: '12.3456' });
    expect(mocks.apply).toHaveBeenCalledWith({
      paymentId: billId,
      applications: [{ invoiceId, amount: '12.3456' }],
    });
    const input = {
      sku: 'WIDGET',
      costMethod: 'fifo',
      standardCost: '12.3456',
      materialCost: '10.1',
      laborCost: '2.2456',
      overheadCost: '0',
    };
    await setManufacturingCost(input);
    expect(mocks.cost).toHaveBeenCalledWith(input);
  });
});
