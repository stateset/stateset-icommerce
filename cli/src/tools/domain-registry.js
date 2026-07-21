import { customerTools } from './customers.js';
import { orderTools } from './orders.js';
import { productTools } from './products.js';
import { inventoryTools } from './inventory.js';
import { customObjectTools } from './custom-objects.js';
import { returnTools } from './returns.js';
import { cartTools } from './carts.js';
import { analyticsTools } from './analytics.js';
import { currencyTools } from './currency.js';
import { taxTools } from './tax.js';
import { promotionTools } from './promotions.js';
import { subscriptionTools } from './subscriptions.js';
import { syncTools } from './sync.js';
import { manufacturingTools } from './manufacturing.js';
import { paymentTools } from './payments.js';
import { stablecoinTools } from './stablecoin.js';
import { treasuryTools } from './treasury.js';
import { erc8004Tools } from './erc8004.js';
import { x402Tools } from './x402.js';
import { agentCardTools } from './agent-cards.js';
import { a2aTools } from './a2a.js';
import { agentRuntimeTools } from './agent-runtime.js';
import { shipmentTools } from './shipments.js';
import { supplierTools } from './suppliers.js';
import { invoiceTools } from './invoices.js';
import { warrantyTools } from './warranties.js';
import { importTools } from './import.js';
import { policyTools } from './policies.js';
import { vectorTools } from './vector.js';
import { giftCardTools } from './gift-cards.js';
import { storeCreditTools } from './store-credits.js';
import { segmentTools } from './segments.js';
import { shippingZoneTools } from './shipping-zones.js';
import { unitOfMeasureTools } from './units-of-measure.js';
import { stockSnapshotTools } from './stock-snapshots.js';
import { printStationTools } from './print-stations.js';
import { integrationMappingTools } from './integration-mappings.js';
import { integrationFieldMappingTools } from './integration-field-mappings.js';
import { paymentObligationTools } from './payment-obligations.js';
import { purgatoryTools } from './purgatory.js';
import { topologySnapshotTools } from './topology-snapshots.js';
import { vendorReturnTools } from './vendor-returns.js';
import { reviewTools } from './reviews.js';
import { wishlistTools } from './wishlists.js';
import { loyaltyTools } from './loyalty.js';
import { fraudTools } from './fraud.js';
import { connectorTools } from './connectors.js';
import { auditTools } from './audit.js';
import { proofTools } from './proofs.js';
import { circuitBreakerTools } from './circuit-breaker.js';
import { checkoutTools } from './checkout.js';
import { complianceTools } from './compliance.js';
import { catalogTools } from './catalog.js';
import { a2aAutomationTools } from './a2a-automation.js';
import { a2aObservabilityTools } from './a2a-observability.js';
import { a2aPlatformTools } from './a2a-platform.js';
import { a2aIntelligenceTools } from './a2a-intelligence.js';
import { qualityTools } from './quality.js';
import { lotTools } from './lots.js';
import { searchConfigTools } from './search-config.js';
import { serialTools } from './serials.js';
import { warehouseTools } from './warehouse.js';
import { receivingTools } from './receiving.js';
import { fulfillmentTools } from './fulfillment.js';
import { accountsPayableTools } from './accounts-payable.js';
import { accountsReceivableTools } from './accounts-receivable.js';
import { costAccountingTools } from './cost-accounting.js';
import { creditTools } from './credit.js';
import { backorderTools } from './backorders.js';
import { generalLedgerTools } from './general-ledger.js';
import { agentReceiptTools } from './agent-receipt.js';
import { fixedAssetTools } from './fixed-assets.js';
import { maintenanceTools } from './maintenance.js';
import { revenueRecognitionTools } from './revenue-recognition.js';
import { cycleCountTools } from './cycle-counts.js';
import { ediDocumentTools } from './edi-documents.js';
import { prepaymentTools } from './prepayments.js';
import { activityLogTools } from './activity-logs.js';
import { channelTools } from './channels.js';
import { companyTools } from './companies.js';
import { vendorCreditTools } from './vendor-credits.js';
import { priceScheduleTools } from './price-schedules.js';
import { priceLevelTools } from './price-levels.js';
import { transferOrderTools } from './transfer-orders.js';
import { productionBatchTools } from './production-batches.js';
import { supplierSkuTools } from './supplier-skus.js';
import { inboundShipmentTools } from './inbound-shipments.js';

export const DOMAIN_TOOL_ENTRIES = Object.freeze([
  ['customers', customerTools],
  ['orders', orderTools],
  ['products', productTools],
  ['inventory', inventoryTools],
  ['custom-objects', customObjectTools],
  ['returns', returnTools],
  ['carts', cartTools],
  ['analytics', analyticsTools],
  ['currency', currencyTools],
  ['tax', taxTools],
  ['promotions', promotionTools],
  ['subscriptions', subscriptionTools],
  ['sync', syncTools],
  ['manufacturing', manufacturingTools],
  ['payments', paymentTools],
  ['stablecoin', stablecoinTools],
  ['treasury', treasuryTools],
  ['erc8004', erc8004Tools],
  ['x402', x402Tools],
  ['agent-cards', agentCardTools],
  ['a2a', a2aTools],
  ['agent-runtime', agentRuntimeTools],
  ['shipments', shipmentTools],
  ['suppliers', supplierTools],
  ['invoices', invoiceTools],
  ['warranties', warrantyTools],
  ['import', importTools],
  ['policies', policyTools],
  ['vector', vectorTools],
  ['gift-cards', giftCardTools],
  ['store-credits', storeCreditTools],
  ['segments', segmentTools],
  ['shipping-zones', shippingZoneTools],
  ['units-of-measure', unitOfMeasureTools],
  ['stock-snapshots', stockSnapshotTools],
  ['print-stations', printStationTools],
  ['integration-mappings', integrationMappingTools],
  ['integration-field-mappings', integrationFieldMappingTools],
  ['payment-obligations', paymentObligationTools],
  ['purgatory', purgatoryTools],
  ['topology-snapshots', topologySnapshotTools],
  ['vendor-returns', vendorReturnTools],
  ['reviews', reviewTools],
  ['wishlists', wishlistTools],
  ['loyalty', loyaltyTools],
  ['fraud', fraudTools],
  ['connectors', connectorTools],
  ['audit', auditTools],
  ['proofs', proofTools],
  ['circuit-breaker', circuitBreakerTools],
  ['checkout', checkoutTools],
  ['compliance', complianceTools],
  ['catalog', catalogTools],
  ['a2a-automation', a2aAutomationTools],
  ['a2a-observability', a2aObservabilityTools],
  ['a2a-platform', a2aPlatformTools],
  ['a2a-intelligence', a2aIntelligenceTools],
  ['quality', qualityTools],
  ['lots', lotTools],
  ['search-config', searchConfigTools],
  ['serials', serialTools],
  ['warehouse', warehouseTools],
  ['receiving', receivingTools],
  ['fulfillment', fulfillmentTools],
  ['accounts-payable', accountsPayableTools],
  ['accounts-receivable', accountsReceivableTools],
  ['cost-accounting', costAccountingTools],
  ['credit', creditTools],
  ['backorders', backorderTools],
  ['general-ledger', generalLedgerTools],
  ['agent-receipt', agentReceiptTools],
  ['fixed-assets', fixedAssetTools],
  ['maintenance', maintenanceTools],
  ['revenue-recognition', revenueRecognitionTools],
  ['cycle-counts', cycleCountTools],
  ['edi-documents', ediDocumentTools],
  ['prepayments', prepaymentTools],
  ['activity-logs', activityLogTools],
  ['channels', channelTools],
  ['companies', companyTools],
  ['vendor-credits', vendorCreditTools],
  ['price-schedules', priceScheduleTools],
  ['price-levels', priceLevelTools],
  ['transfer-orders', transferOrderTools],
  ['production-batches', productionBatchTools],
  ['supplier-skus', supplierSkuTools],
  ['inbound-shipments', inboundShipmentTools],
]);

export const DOMAIN_TOOL_ARRAYS = Object.freeze(Object.fromEntries(DOMAIN_TOOL_ENTRIES));
export const ALL_DOMAIN_TOOLS = Object.freeze(DOMAIN_TOOL_ENTRIES.flatMap(([, tools]) => tools));
export const TOOL_MODULE_NAMES = Object.freeze(DOMAIN_TOOL_ENTRIES.map(([name]) => name));

export const TOOL_POLICY_DOMAIN_BY_NAME = Object.freeze(
  Object.fromEntries(
    DOMAIN_TOOL_ENTRIES.flatMap(([moduleName, tools]) =>
      tools
        .filter((tool) => tool?.name)
        .map((tool) => [tool.name, tool.policyDomain ?? moduleName.replace(/-/g, '_')]),
    ),
  ),
);
