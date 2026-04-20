import { readFileSync } from 'node:fs';
import { DOMAIN_TOOL_ARRAYS } from '../tools/domain-registry.js';

export { DOMAIN_TOOL_ARRAYS };
export const ALL_DOMAIN_TOOLS = Object.values(DOMAIN_TOOL_ARRAYS).flat();
const {
  customers: customerTools,
  orders: orderTools,
  products: productTools,
  inventory: inventoryTools,
  'custom-objects': customObjectTools,
  returns: returnTools,
  carts: cartTools,
  analytics: analyticsTools,
  currency: currencyTools,
  tax: taxTools,
  promotions: promotionTools,
  subscriptions: subscriptionTools,
  manufacturing: manufacturingTools,
  payments: paymentTools,
  x402: x402Tools,
  'agent-cards': agentCardTools,
  shipments: shipmentTools,
  suppliers: supplierTools,
  invoices: invoiceTools,
  warranties: warrantyTools,
  quality: qualityTools,
  lots: lotTools,
  serials: serialTools,
  warehouse: warehouseTools,
  receiving: receivingTools,
  fulfillment: fulfillmentTools,
  'accounts-payable': accountsPayableTools,
  'accounts-receivable': accountsReceivableTools,
  'cost-accounting': costAccountingTools,
  credit: creditTools,
  backorders: backorderTools,
  'general-ledger': generalLedgerTools,
} = DOMAIN_TOOL_ARRAYS;
export const COMMERCE_BINDING_INDEX_DTS = new URL('../../../bindings/node/index.d.ts', import.meta.url);

export const COMMERCE_GETTER_TO_MODULE = Object.freeze({
  customers: 'customers',
  orders: 'orders',
  products: 'products',
  customObjects: 'custom-objects',
  inventory: 'inventory',
  returns: 'returns',
  payments: 'payments',
  x402: 'x402',
  shipments: 'shipments',
  warranties: 'warranties',
  purchaseOrders: 'suppliers',
  invoices: 'invoices',
  bom: 'manufacturing',
  workOrders: 'manufacturing',
  carts: 'carts',
  analytics: 'analytics',
  currency: 'currency',
  subscriptions: 'subscriptions',
  promotions: 'promotions',
  tax: 'tax',
  quality: 'quality',
  lots: 'lots',
  serials: 'serials',
  warehouse: 'warehouse',
  receiving: 'receiving',
  fulfillment: 'fulfillment',
  accountsPayable: 'accounts-payable',
  accountsReceivable: 'accounts-receivable',
  costAccounting: 'cost-accounting',
  credit: 'credit',
  backorder: 'backorders',
  generalLedger: 'general-ledger',
});

export const AUDITED_CLASS_METHOD_TOOL_COVERAGE = Object.freeze({
  Customers: {
    tools: customerTools,
    methodToTools: {
      create: ['create_customer'],
      get: ['get_customer'],
      getByEmail: ['get_customer'],
      list: ['list_customers'],
      count: ['list_customers'],
    },
  },
  Orders: {
    tools: orderTools,
    methodToTools: {
      create: ['create_order'],
      get: ['get_order'],
      list: ['list_orders'],
      updateStatus: ['update_order_status'],
      ship: ['ship_order'],
      cancel: ['cancel_order'],
      count: ['list_orders'],
    },
  },
  Products: {
    tools: productTools,
    methodToTools: {
      create: ['create_product'],
      get: ['get_product'],
      getVariantBySku: ['get_product_variant'],
      list: ['list_products'],
      count: ['list_products'],
    },
  },
  CustomObjects: {
    tools: customObjectTools,
    methodToTools: {
      createType: ['create_custom_object_type'],
      getType: ['get_custom_object_type'],
      getTypeByHandle: ['get_custom_object_type_by_handle'],
      updateType: ['update_custom_object_type'],
      listTypes: ['list_custom_object_types'],
      deleteType: ['delete_custom_object_type'],
      createObject: ['create_custom_object'],
      getObject: ['get_custom_object'],
      getObjectByHandle: ['get_custom_object_by_handle'],
      updateObject: ['update_custom_object'],
      listObjects: ['list_custom_objects'],
      deleteObject: ['delete_custom_object'],
    },
  },
  Inventory: {
    tools: inventoryTools,
    methodToTools: {
      createItem: ['create_inventory_item'],
      getStock: ['get_stock'],
      adjust: ['adjust_inventory'],
      reserve: ['reserve_inventory'],
      confirmReservation: ['confirm_reservation'],
      releaseReservation: ['release_reservation'],
    },
  },
  Returns: {
    tools: returnTools,
    methodToTools: {
      create: ['create_return'],
      get: ['get_return'],
      approve: ['approve_return'],
      reject: ['reject_return'],
      list: ['list_returns'],
      count: ['list_returns'],
    },
  },
  Warranties: {
    tools: warrantyTools,
    methodToTools: {
      create: ['create_warranty'],
      get: ['get_warranty'],
      list: ['list_warranties'],
      createClaim: ['create_warranty_claim'],
      approveClaim: ['approve_warranty_claim'],
      denyClaim: ['deny_warranty_claim'],
      completeClaim: ['complete_warranty_claim'],
      count: ['list_warranties'],
    },
  },
  PurchaseOrders: {
    tools: supplierTools,
    methodToTools: {
      createSupplier: ['create_supplier'],
      getSupplier: ['get_supplier'],
      listSuppliers: ['list_suppliers'],
      create: ['create_purchase_order'],
      get: ['get_purchase_order'],
      list: ['list_purchase_orders'],
      submit: ['submit_purchase_order'],
      approve: ['approve_purchase_order'],
      send: ['send_purchase_order'],
      cancel: ['cancel_purchase_order'],
      count: ['list_purchase_orders'],
    },
  },
  GeneralLedger: {
    tools: generalLedgerTools,
    methodToTools: {
      createAccount: ['create_gl_account'],
      getAccount: ['get_gl_account'],
      getAccountByNumber: ['get_gl_account'],
      listAccounts: ['list_gl_accounts'],
      initializeChartOfAccounts: ['initialize_chart_of_accounts'],
      getJournalEntry: ['get_journal_entry'],
      listJournalEntries: ['list_journal_entries'],
      postJournalEntry: ['post_journal_entry'],
      voidJournalEntry: ['void_journal_entry'],
      getTrialBalance: ['get_trial_balance'],
      getBalanceSheet: ['get_balance_sheet'],
      getIncomeStatement: ['get_income_statement'],
      getAccountBalance: ['get_gl_account_balance'],
    },
  },
  AccountsPayable: {
    tools: accountsPayableTools,
    methodToTools: {
      createBill: ['create_bill'],
      getBill: ['get_bill'],
      getBillByNumber: ['get_bill'],
      listBills: ['list_bills'],
      approveBill: ['approve_bill'],
      cancelBill: ['cancel_bill'],
      getOverdueBills: ['list_overdue_bills'],
      getBillsDueSoon: ['list_bills_due_soon'],
      getAgingSummary: ['get_accounts_payable_aging_summary'],
      getTotalOutstanding: ['get_accounts_payable_total_outstanding'],
      countBills: ['count_accounts_payable_bills'],
    },
  },
  AccountsReceivable: {
    tools: accountsReceivableTools,
    methodToTools: {
      getAgingSummary: ['get_accounts_receivable_aging_summary'],
      getTotalOutstanding: ['get_accounts_receivable_total_outstanding'],
      getDso: ['get_days_sales_outstanding'],
      createCreditMemo: ['create_credit_memo'],
      getCreditMemo: ['get_credit_memo'],
      listCreditMemos: ['list_credit_memos'],
      voidCreditMemo: ['void_credit_memo'],
      getUnappliedCredits: ['list_unapplied_credits'],
    },
  },
  Payments: {
    tools: paymentTools,
    methodToTools: {
      create: ['create_payment'],
      get: ['get_payment'],
      list: ['list_payments'],
      markCompleted: ['complete_payment'],
      markFailed: ['mark_failed_payment'],
      cancel: ['cancel_payment'],
      createRefund: ['create_refund'],
      count: ['list_payments'],
    },
  },
  Shipments: {
    tools: shipmentTools,
    methodToTools: {
      create: ['create_shipment'],
      get: ['get_shipment'],
      list: ['list_shipments'],
      ship: ['ship_shipment'],
      deliver: ['deliver_shipment'],
      cancel: ['cancel_shipment'],
      count: ['list_shipments'],
    },
  },
  Invoices: {
    tools: invoiceTools,
    methodToTools: {
      create: ['create_invoice'],
      get: ['get_invoice'],
      list: ['list_invoices'],
      send: ['send_invoice'],
      void: ['void_invoice'],
      recordPayment: ['record_invoice_payment'],
      getOverdue: ['get_overdue_invoices'],
      count: ['list_invoices'],
    },
  },
  Bom: {
    tools: manufacturingTools,
    methodToTools: {
      create: ['create_bom'],
      get: ['get_bom'],
      list: ['list_boms'],
      addComponent: ['add_bom_component'],
      getComponents: ['get_bom'],
      activate: ['activate_bom'],
      count: ['list_boms'],
    },
  },
  WorkOrders: {
    tools: manufacturingTools,
    methodToTools: {
      create: ['create_work_order'],
      get: ['get_work_order'],
      list: ['list_work_orders'],
      start: ['start_work_order'],
      complete: ['complete_work_order'],
      cancel: ['cancel_work_order'],
      count: ['list_work_orders'],
    },
  },
  X402: {
    tools: [...x402Tools, ...agentCardTools],
    methodToTools: {
      createIntent: ['x402_create_payment_intent'],
      signIntent: ['x402_sign_intent'],
      getIntent: ['x402_get_intent'],
      listIntents: ['x402_list_intents'],
      markSettled: ['x402_mark_settled'],
      getNextNonce: ['x402_get_next_nonce'],
      registerAgent: ['register_agent_card'],
      discoverAgents: ['discover_agents'],
      getAgent: ['get_agent_card'],
      getAgentByWallet: ['get_agent_card'],
      verifyAgent: ['verify_agent'],
      listAgents: ['list_agent_cards'],
      getCreditBalance: ['x402_credit_balance'],
      getCreditAccount: ['x402_get_credit_account'],
      creditAccount: ['x402_credit_deposit'],
      debitAccount: ['x402_credit_debit'],
      listCreditTransactions: ['x402_credit_transactions'],
    },
  },
  Analytics: {
    tools: analyticsTools,
    methodToTools: {
      salesSummary: ['get_sales_summary'],
      revenueByPeriod: ['get_revenue_by_period'],
      topProducts: ['get_top_products'],
      productPerformance: ['get_product_performance'],
      customerMetrics: ['get_customer_metrics'],
      topCustomers: ['get_top_customers'],
      inventoryHealth: ['get_inventory_health'],
      lowStockItems: ['get_low_stock_items'],
      inventoryMovement: ['get_inventory_movement'],
      demandForecast: ['get_demand_forecast'],
      revenueForecast: ['get_revenue_forecast'],
      orderStatusBreakdown: ['get_order_status_breakdown'],
      fulfillmentMetrics: ['get_fulfillment_metrics'],
      returnMetrics: ['get_return_metrics'],
    },
  },
  CurrencyOperations: {
    tools: currencyTools,
    methodToTools: {
      getRate: ['get_exchange_rate'],
      getRatesFor: ['list_exchange_rates'],
      listRates: ['list_exchange_rates'],
      setRate: ['set_exchange_rate'],
      setRates: ['set_exchange_rates'],
      deleteRate: ['delete_exchange_rate'],
      convert: ['convert_currency'],
      getSettings: ['get_currency_settings'],
      updateSettings: ['update_currency_settings'],
      setBaseCurrency: ['set_base_currency'],
      enableCurrencies: ['enable_currencies'],
      isEnabled: ['check_currency_enabled'],
      getBaseCurrency: ['get_currency_settings'],
      getEnabledCurrencies: ['get_currency_settings'],
      format: ['format_currency'],
    },
  },
  Subscriptions: {
    tools: subscriptionTools,
    methodToTools: {
      createPlan: ['create_subscription_plan'],
      getPlan: ['get_subscription_plan'],
      getPlanByCode: ['get_subscription_plan'],
      listPlans: ['list_subscription_plans'],
      updatePlan: ['update_subscription_plan'],
      activatePlan: ['activate_subscription_plan'],
      archivePlan: ['archive_subscription_plan'],
      subscribe: ['create_subscription'],
      get: ['get_subscription'],
      getByNumber: ['get_subscription'],
      list: ['list_subscriptions'],
      update: ['update_subscription'],
      pause: ['pause_subscription'],
      resume: ['resume_subscription'],
      cancel: ['cancel_subscription'],
      skipBilling: ['skip_billing_cycle'],
      listBillingCycles: ['list_billing_cycles'],
      getBillingCycle: ['get_billing_cycle'],
      getEvents: ['get_subscription_events'],
    },
  },
  Promotions: {
    tools: promotionTools,
    methodToTools: {
      create: ['create_promotion'],
      get: ['get_promotion'],
      getByCode: ['get_promotion'],
      list: ['list_promotions'],
      update: ['update_promotion'],
      delete: ['delete_promotion'],
      activate: ['activate_promotion'],
      deactivate: ['deactivate_promotion'],
      getActive: ['get_active_promotions'],
      isValid: ['check_promotion_validity'],
      createCoupon: ['create_coupon'],
      getCoupon: ['get_coupon'],
      getCouponByCode: ['get_coupon'],
      listCoupons: ['list_coupons'],
      validateCoupon: ['validate_coupon'],
      apply: ['apply_cart_promotions'],
      recordUsage: ['record_promotion_usage'],
    },
  },
  Tax: {
    tools: taxTools,
    methodToTools: {
      calculate: ['calculate_tax'],
      calculateForItem: ['calculate_item_tax'],
      getEffectiveRate: ['get_tax_rate'],
      getJurisdiction: ['get_tax_jurisdiction'],
      getJurisdictionByCode: ['get_tax_jurisdiction'],
      listJurisdictions: ['list_tax_jurisdictions'],
      createJurisdiction: ['create_tax_jurisdiction'],
      getRate: ['get_tax_rate_record'],
      listRates: ['list_tax_rates'],
      createRate: ['create_tax_rate'],
      getExemption: ['get_tax_exemption'],
      getCustomerExemptions: ['get_customer_tax_exemptions'],
      createExemption: ['create_tax_exemption'],
      customerIsExempt: ['check_customer_tax_exempt'],
      getSettings: ['get_tax_settings'],
      updateSettings: ['update_tax_settings'],
      setEnabled: ['set_tax_enabled'],
      isEnabled: ['check_tax_enabled'],
    },
  },
  CostAccounting: {
    tools: costAccountingTools,
    methodToTools: {
      getItemCost: ['get_item_cost'],
      setItemCost: ['set_item_cost'],
      listItemCosts: ['list_item_costs'],
      updateAverageCost: ['update_average_item_cost'],
      getTotalInventoryValue: ['get_total_inventory_value'],
    },
  },
  Credit: {
    tools: creditTools,
    methodToTools: {
      createCreditAccount: ['create_credit_account'],
      getCreditAccount: ['get_credit_account'],
      getCreditAccountByCustomer: ['get_credit_account'],
      listCreditAccounts: ['list_credit_accounts'],
      checkCredit: ['check_customer_credit'],
      adjustCreditLimit: ['adjust_credit_limit'],
      suspendCreditAccount: ['suspend_credit_account'],
      reactivateCreditAccount: ['reactivate_credit_account'],
      getOverLimitCustomers: ['list_over_limit_credit_accounts'],
    },
  },
  Backorders: {
    tools: backorderTools,
    methodToTools: {
      createBackorder: ['create_backorder'],
      getBackorder: ['get_backorder'],
      getBackorderByNumber: ['get_backorder'],
      listBackorders: ['list_backorders'],
      cancelBackorder: ['cancel_backorder'],
      getBackordersForOrder: ['list_backorders_for_order'],
      getBackordersForSku: ['list_backorders_for_sku'],
      getOverdueBackorders: ['list_overdue_backorders'],
      getSummary: ['get_backorder_summary'],
      countPending: ['count_pending_backorders'],
    },
  },
  Quality: {
    tools: qualityTools,
    methodToTools: {
      createInspection: ['create_inspection'],
      getInspection: ['get_inspection'],
      listInspections: ['list_inspections'],
      startInspection: ['start_inspection'],
      completeInspection: ['complete_inspection'],
      createNcr: ['create_ncr'],
      getNcr: ['get_ncr'],
      listNcrs: ['list_ncrs'],
      closeNcr: ['close_ncr'],
      createHold: ['create_quality_hold'],
      getHold: ['get_quality_hold'],
      listHolds: ['list_quality_holds'],
      releaseHold: ['release_quality_hold'],
      getActiveHolds: ['list_active_quality_holds'],
      countActiveHolds: ['count_active_quality_holds'],
    },
  },
  Lots: {
    tools: lotTools,
    methodToTools: {
      create: ['create_lot'],
      get: ['get_lot'],
      getByNumber: ['get_lot'],
      list: ['list_lots'],
      getActiveLots: ['list_active_lots'],
      getAvailableLotsForSku: ['list_available_lots_for_sku'],
      quarantine: ['quarantine_lot'],
      releaseQuarantine: ['release_lot_quarantine'],
      getExpiringLots: ['list_expiring_lots'],
      getExpiredLots: ['list_expired_lots'],
      getQuarantined: ['list_quarantined_lots'],
      count: ['count_lots'],
    },
  },
  Serials: {
    tools: serialTools,
    methodToTools: {
      create: ['create_serial'],
      get: ['get_serial'],
      getBySerial: ['get_serial'],
      list: ['list_serials'],
      getAvailable: ['list_available_serials'],
      markSold: ['mark_serial_sold'],
      quarantine: ['quarantine_serial'],
      isAvailable: ['check_serial_availability'],
      count: ['count_serials'],
    },
  },
  Warehouse: {
    tools: warehouseTools,
    methodToTools: {
      createWarehouse: ['create_warehouse'],
      getWarehouse: ['get_warehouse'],
      getWarehouseByCode: ['get_warehouse'],
      listWarehouses: ['list_warehouses'],
      createLocation: ['create_location'],
      getLocation: ['get_location'],
      listLocations: ['list_locations'],
      getPickableLocations: ['list_pickable_locations'],
      getTotalAvailable: ['get_warehouse_sku_available_quantity'],
      countWarehouses: ['count_warehouses'],
    },
  },
  Receiving: {
    tools: receivingTools,
    methodToTools: {
      createReceipt: ['create_receipt'],
      getReceipt: ['get_receipt'],
      getReceiptByNumber: ['get_receipt'],
      listReceipts: ['list_receipts'],
      startReceiving: ['start_receiving'],
      completeReceiving: ['complete_receiving'],
      cancelReceipt: ['cancel_receipt'],
      createReceiptFromPo: ['create_receipt_from_purchase_order'],
      countReceipts: ['count_receipts'],
    },
  },
  Fulfillment: {
    tools: fulfillmentTools,
    methodToTools: {
      createWave: ['create_fulfillment_wave'],
      getWave: ['get_fulfillment_wave'],
      listWaves: ['list_fulfillment_waves'],
      releaseWave: ['release_fulfillment_wave'],
      completeWave: ['complete_fulfillment_wave'],
      cancelWave: ['cancel_fulfillment_wave'],
      getPick: ['get_pick_task'],
      listPicks: ['list_pick_tasks'],
      assignPick: ['assign_pick_task'],
      startPick: ['start_pick_task'],
      cancelPick: ['cancel_pick_task'],
      isOrderReadyToPack: ['check_order_ready_to_pack'],
      isOrderReadyToShip: ['check_order_ready_to_ship'],
      countWaves: ['count_fulfillment_waves'],
    },
  },
  Carts: {
    tools: cartTools,
    methodToTools: {
      create: ['create_cart'],
      get: ['get_cart'],
      getByNumber: ['get_cart'],
      update: ['update_cart'],
      list: ['list_carts'],
      forCustomer: ['list_customer_carts'],
      delete: ['delete_cart'],
      addItem: ['add_cart_item'],
      updateItem: ['update_cart_item'],
      removeItem: ['remove_cart_item'],
      getItems: ['list_cart_items'],
      clearItems: ['clear_cart_items'],
      setShippingAddress: ['set_cart_shipping_address'],
      setShipping: ['set_cart_shipping'],
      setBillingAddress: ['set_cart_billing_address'],
      getShippingRates: ['get_shipping_rates'],
      setPayment: ['set_cart_payment'],
      applyDiscount: ['apply_cart_discount'],
      removeDiscount: ['remove_cart_discount'],
      markReadyForPayment: ['mark_cart_ready_for_payment'],
      beginCheckout: ['begin_cart_checkout'],
      complete: ['complete_checkout'],
      cancel: ['cancel_cart'],
      abandon: ['abandon_cart'],
      expire: ['expire_cart'],
      reserveInventory: ['reserve_cart_inventory'],
      releaseInventory: ['release_cart_inventory'],
      recalculate: ['recalculate_cart'],
      setTax: ['set_cart_tax'],
      getAbandoned: ['get_abandoned_carts'],
      getExpired: ['get_expired_carts'],
      count: ['list_carts'],
    },
  },
});

export function readCommerceBindingSource() {
  return readFileSync(COMMERCE_BINDING_INDEX_DTS, 'utf8');
}

export function getBindingClassMethodNames(source, className) {
  const escapedClassName = className.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const classMatch = source.match(
    new RegExp(`export declare class ${escapedClassName} \\{([\\s\\S]*?)\\n\\}`, 'm'),
  );

  if (!classMatch) {
    throw new Error(`Unable to locate Commerce binding class "${className}"`);
  }

  return new Set(
    [...classMatch[1].matchAll(/^\s+([A-Za-z0-9]+)\(/gm)].map((match) => match[1]),
  );
}

export function buildMcpApiCoverage() {
  const source = readCommerceBindingSource();
  const getterNames = new Set(
    [...source.matchAll(/get\s+([A-Za-z0-9]+)\(\):\s+[A-Za-z0-9]+/g)].map((match) => match[1]),
  );

  getterNames.delete('customStates');
  getterNames.delete('events');

  const getters = [...getterNames]
    .sort()
    .map((getter) => {
      const moduleName = COMMERCE_GETTER_TO_MODULE[getter] ?? null;
      return {
        getter,
        module: moduleName,
        toolCount: moduleName ? DOMAIN_TOOL_ARRAYS[moduleName].length : 0,
      };
    });

  const uncoveredCommerceGetters = getters
    .filter((entry) => !entry.module)
    .map((entry) => entry.getter);

  const staleGetterMappings = Object.keys(COMMERCE_GETTER_TO_MODULE)
    .filter((getter) => !getterNames.has(getter))
    .sort();

  const auditedClasses = Object.entries(AUDITED_CLASS_METHOD_TOOL_COVERAGE)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([className, coverage]) => {
      const bindingMethods = [...getBindingClassMethodNames(source, className)].sort();
      const mappedMethods = Object.keys(coverage.methodToTools).sort();
      const exportedToolNames = new Set(coverage.tools.map((tool) => tool.name));
      const uncoveredMethods = bindingMethods.filter((method) => !coverage.methodToTools[method]);
      const staleMappedMethods = mappedMethods.filter((method) => !bindingMethods.includes(method));
      const invalidToolReferences = mappedMethods.flatMap((method) =>
        (coverage.methodToTools[method] ?? [])
          .filter((toolName) => !exportedToolNames.has(toolName))
          .map((toolName) => `${className}.${method}:${toolName}`),
      );

      return {
        className,
        methodCount: bindingMethods.length,
        mappedMethodCount: bindingMethods.length - uncoveredMethods.length,
        uncoveredMethods,
        staleMappedMethods,
        invalidToolReferences,
      };
    });

  const uncoveredAuditedMethods = auditedClasses.flatMap((entry) =>
    entry.uncoveredMethods.map((method) => `${entry.className}.${method}`),
  );
  const staleAuditedMethodMappings = auditedClasses.flatMap((entry) =>
    entry.staleMappedMethods.map((method) => `${entry.className}.${method}`),
  );
  const invalidAuditedToolReferences = auditedClasses.flatMap((entry) => entry.invalidToolReferences);

  return {
    source: {
      binding: 'bindings/node/index.d.ts',
      coverageModel: 'cli/src/coverage/mcp-api-coverage.js',
    },
    totalDomainModules: Object.keys(DOMAIN_TOOL_ARRAYS).length,
    totalDomainTools: ALL_DOMAIN_TOOLS.length,
    totalCommerceGetters: getters.length,
    mappedCommerceGetters: getters.length - uncoveredCommerceGetters.length,
    uncoveredCommerceGetters,
    staleGetterMappings,
    getters,
    totalAuditedClasses: auditedClasses.length,
    totalAuditedMethods: auditedClasses.reduce((sum, entry) => sum + entry.methodCount, 0),
    mappedAuditedMethods: auditedClasses.reduce((sum, entry) => sum + entry.mappedMethodCount, 0),
    uncoveredAuditedMethods,
    staleAuditedMethodMappings,
    invalidAuditedToolReferences,
    auditedClasses,
    fullyCovered:
      uncoveredCommerceGetters.length === 0 &&
      staleGetterMappings.length === 0 &&
      uncoveredAuditedMethods.length === 0 &&
      staleAuditedMethodMappings.length === 0 &&
      invalidAuditedToolReferences.length === 0,
  };
}
