#!/usr/bin/env node
/**
 * StateSet iCommerce - Tax Calculation Example
 *
 * This example demonstrates the tax system:
 * - Tax settings configuration
 * - Creating tax jurisdictions and rates
 * - Tax exemptions
 * - Calculating taxes for transactions
 * - US, EU, and Canadian tax lookups
 *
 * Run with: node 07_tax.js
 */

const { Commerce } = require('@stateset/embedded');

async function main() {
  console.log('=== StateSet iCommerce - Tax Calculation ===\n');

  const commerce = new Commerce(':memory:');

  // ============================================
  // 1. Configure Tax Settings
  // ============================================

  console.log('[1] Configuring tax settings...');

  const settings = await commerce.tax.updateSettings({
    enabled: true,
    calculationMethod: 'line_item', // Calculate per line item
    compoundMethod: 'sequential', // How compound taxes are applied
    taxShipping: true,
    taxHandling: false,
    taxGiftWrap: true,
    defaultProductCategory: 'general',
    roundingMode: 'half_up',
    decimalPlaces: 2,
    validateAddresses: true
  });

  console.log(`    Tax enabled: ${settings.enabled}`);
  console.log(`    Calculation method: ${settings.calculationMethod}`);
  console.log(`    Tax shipping: ${settings.taxShipping}`);
  console.log(`    Default category: ${settings.defaultProductCategory}\n`);

  // ============================================
  // 2. Create Tax Jurisdictions
  // ============================================

  console.log('[2] Creating tax jurisdictions...');

  // US - California
  const usCA = await commerce.tax.createJurisdiction({
    name: 'California',
    code: 'US-CA',
    level: 'state',
    countryCode: 'US',
    stateCode: 'CA'
  });
  console.log(`    Created: ${usCA.name} (${usCA.code})`);

  // US - California - San Francisco
  const usCASF = await commerce.tax.createJurisdiction({
    parentId: usCA.id,
    name: 'San Francisco',
    code: 'US-CA-SF',
    level: 'city',
    countryCode: 'US',
    stateCode: 'CA',
    city: 'San Francisco',
    postalCodes: ['94102', '94103', '94104', '94105', '94107', '94108', '94109', '94110']
  });
  console.log(`    Created: ${usCASF.name} (${usCASF.code})`);

  // US - New York
  const usNY = await commerce.tax.createJurisdiction({
    name: 'New York',
    code: 'US-NY',
    level: 'state',
    countryCode: 'US',
    stateCode: 'NY'
  });
  console.log(`    Created: ${usNY.name} (${usNY.code})`);

  // EU - Germany
  const deDE = await commerce.tax.createJurisdiction({
    name: 'Germany',
    code: 'DE',
    level: 'country',
    countryCode: 'DE'
  });
  console.log(`    Created: ${deDE.name} (${deDE.code})`);

  // Canada - Ontario
  const caON = await commerce.tax.createJurisdiction({
    name: 'Ontario',
    code: 'CA-ON',
    level: 'state',
    countryCode: 'CA',
    stateCode: 'ON'
  });
  console.log(`    Created: ${caON.name} (${caON.code})\n`);

  // ============================================
  // 3. Create Tax Rates
  // ============================================

  console.log('[3] Creating tax rates...');

  // California state tax
  const caStateRate = await commerce.tax.createRate({
    jurisdictionId: usCA.id,
    taxType: 'sales_tax',
    productCategory: 'general',
    rate: 0.0725, // 7.25%
    name: 'California State Tax',
    description: 'California state sales tax',
    effectiveFrom: '2024-01-01'
  });
  console.log(`    Created: ${caStateRate.name} - ${(caStateRate.rate * 100).toFixed(2)}%`);

  // San Francisco local tax
  const sfLocalRate = await commerce.tax.createRate({
    jurisdictionId: usCASF.id,
    taxType: 'sales_tax',
    productCategory: 'general',
    rate: 0.0125, // 1.25%
    name: 'San Francisco Local Tax',
    description: 'San Francisco district tax',
    effectiveFrom: '2024-01-01'
  });
  console.log(`    Created: ${sfLocalRate.name} - ${(sfLocalRate.rate * 100).toFixed(2)}%`);

  // New York state tax
  const nyStateRate = await commerce.tax.createRate({
    jurisdictionId: usNY.id,
    taxType: 'sales_tax',
    productCategory: 'general',
    rate: 0.04, // 4%
    name: 'New York State Tax',
    effectiveFrom: '2024-01-01'
  });
  console.log(`    Created: ${nyStateRate.name} - ${(nyStateRate.rate * 100).toFixed(2)}%`);

  // Germany VAT
  const deVatRate = await commerce.tax.createRate({
    jurisdictionId: deDE.id,
    taxType: 'vat',
    productCategory: 'general',
    rate: 0.19, // 19%
    name: 'German VAT',
    description: 'Standard VAT rate',
    effectiveFrom: '2024-01-01'
  });
  console.log(`    Created: ${deVatRate.name} - ${(deVatRate.rate * 100).toFixed(0)}%`);

  // Germany reduced VAT for food
  const deVatReducedRate = await commerce.tax.createRate({
    jurisdictionId: deDE.id,
    taxType: 'vat',
    productCategory: 'food',
    rate: 0.07, // 7%
    name: 'German VAT (Reduced)',
    description: 'Reduced VAT for food',
    effectiveFrom: '2024-01-01'
  });
  console.log(`    Created: ${deVatReducedRate.name} - ${(deVatReducedRate.rate * 100).toFixed(0)}%`);

  // Ontario HST
  const onHstRate = await commerce.tax.createRate({
    jurisdictionId: caON.id,
    taxType: 'hst',
    productCategory: 'general',
    rate: 0.13, // 13%
    name: 'Ontario HST',
    description: 'Harmonized Sales Tax',
    effectiveFrom: '2024-01-01'
  });
  console.log(`    Created: ${onHstRate.name} - ${(onHstRate.rate * 100).toFixed(0)}%\n`);

  // ============================================
  // 4. Create Tax Exemptions
  // ============================================

  console.log('[4] Creating tax exemptions...');

  // Create a customer
  const customer = await commerce.customers.create({
    email: 'reseller@example.com',
    firstName: 'Resale',
    lastName: 'Company'
  });

  // Create exemption for reseller
  const exemption = await commerce.tax.createExemption({
    customerId: customer.id,
    exemptionType: 'resale',
    certificateNumber: 'RES-2024-12345',
    issuingAuthority: 'California BOE',
    jurisdictionIds: [usCA.id, usCASF.id],
    exemptCategories: ['general'],
    effectiveFrom: '2024-01-01',
    expiresAt: '2025-12-31',
    notes: 'California reseller certificate'
  });

  console.log(`    Created exemption: ${exemption.exemptionType}`);
  console.log(`      Certificate: ${exemption.certificateNumber}`);
  console.log(`      Valid: ${exemption.effectiveFrom} to ${exemption.expiresAt}`);

  // Check if customer is exempt
  const isExempt = await commerce.tax.customerIsExempt(customer.id);
  console.log(`    Customer is exempt: ${isExempt}`);

  // Get customer exemptions
  const customerExemptions = await commerce.tax.getCustomerExemptions(customer.id);
  console.log(`    Customer exemptions: ${customerExemptions.length}\n`);

  // ============================================
  // 5. Calculate Tax for Transaction
  // ============================================

  console.log('[5] Calculating tax for transaction...');

  // San Francisco order
  const sfTaxCalc = await commerce.tax.calculate({
    lineItems: [
      {
        id: 'item1',
        sku: 'LAPTOP-001',
        quantity: 1,
        unitPrice: 999.99,
        taxCategory: 'general'
      },
      {
        id: 'item2',
        sku: 'MOUSE-001',
        quantity: 2,
        unitPrice: 49.99,
        discountAmount: 10.00,
        taxCategory: 'general'
      }
    ],
    shippingAddress: {
      line1: '123 Market St',
      city: 'San Francisco',
      state: 'CA',
      postalCode: '94105',
      country: 'US'
    },
    shippingAmount: 15.99,
    currency: 'USD'
  });

  console.log('    San Francisco Order Tax:');
  console.log(`      Subtotal: $${sfTaxCalc.subtotal.toFixed(2)}`);
  console.log(`      Shipping Tax: $${sfTaxCalc.shippingTax.toFixed(2)}`);
  console.log(`      Total Tax: $${sfTaxCalc.totalTax.toFixed(2)}`);
  console.log(`      Grand Total: $${sfTaxCalc.total.toFixed(2)}`);
  console.log(`      Is Estimate: ${sfTaxCalc.isEstimate}`);

  console.log('      Tax breakdown:');
  for (const breakdown of sfTaxCalc.taxBreakdown) {
    console.log(`        ${breakdown.jurisdictionName} (${breakdown.taxType}): ${(breakdown.rate * 100).toFixed(2)}% = $${breakdown.taxAmount.toFixed(2)}`);
  }

  console.log('      Line item taxes:');
  for (const itemTax of sfTaxCalc.lineItemTaxes) {
    console.log(`        ${itemTax.lineItemId}: $${itemTax.taxAmount.toFixed(2)} (${(itemTax.effectiveRate * 100).toFixed(2)}%)`);
  }

  console.log('      Jurisdictions:');
  for (const juris of sfTaxCalc.jurisdictions) {
    console.log(`        ${juris.name}: ${(juris.totalRate * 100).toFixed(2)}% = $${juris.totalTax.toFixed(2)}`);
  }
  console.log('');

  // ============================================
  // 6. Calculate Tax for Exempt Customer
  // ============================================

  console.log('[6] Tax calculation for exempt customer...');

  const exemptCalc = await commerce.tax.calculate({
    lineItems: [
      { id: 'item1', quantity: 1, unitPrice: 500.00, taxCategory: 'general' }
    ],
    shippingAddress: {
      city: 'San Francisco',
      state: 'CA',
      postalCode: '94105',
      country: 'US'
    },
    customerId: customer.id, // Exempt customer
    currency: 'USD'
  });

  console.log('    Exempt customer tax calculation:');
  console.log(`      Subtotal: $${exemptCalc.subtotal.toFixed(2)}`);
  console.log(`      Total Tax: $${exemptCalc.totalTax.toFixed(2)}`);
  console.log(`      Exemptions Applied: ${exemptCalc.exemptionsApplied}`);
  if (exemptCalc.exemptionDetails) {
    console.log(`      Tax Saved: $${exemptCalc.exemptionDetails.taxSaved.toFixed(2)}`);
  }
  console.log('');

  // ============================================
  // 7. Get Effective Tax Rate
  // ============================================

  console.log('[7] Getting effective tax rates...');

  // California rate
  const caRate = await commerce.tax.getEffectiveRate(
    { state: 'CA', country: 'US' },
    'general'
  );
  console.log(`    California effective rate: ${(caRate * 100).toFixed(2)}%`);

  // San Francisco rate (includes local)
  const sfRate = await commerce.tax.getEffectiveRate(
    { city: 'San Francisco', state: 'CA', postalCode: '94105', country: 'US' },
    'general'
  );
  console.log(`    San Francisco effective rate: ${(sfRate * 100).toFixed(2)}%`);

  // Germany rate
  const deRate = await commerce.tax.getEffectiveRate(
    { country: 'DE' },
    'general'
  );
  console.log(`    Germany effective rate: ${(deRate * 100).toFixed(2)}%\n`);

  // ============================================
  // 8. Calculate Tax for Single Item
  // ============================================

  console.log('[8] Single item tax calculation...');

  const singleItemTax = await commerce.tax.calculateForItem(
    99.99, // unit price
    2,     // quantity
    'general',
    { state: 'CA', country: 'US' }
  );
  console.log(`    Single item: 2 x $99.99`);
  console.log(`    Tax: $${singleItemTax.toFixed(2)}\n`);

  // ============================================
  // 9. US State Tax Reference
  // ============================================

  console.log('[9] US state tax reference...');

  const states = ['CA', 'NY', 'TX', 'FL', 'OR', 'DE'];
  console.log('    US State Tax Information:');
  for (const stateCode of states) {
    const stateInfo = commerce.tax.constructor.getUsStateInfo(stateCode);
    if (stateInfo) {
      console.log(`      ${stateInfo.stateName} (${stateCode}):`);
      console.log(`        State rate: ${(stateInfo.stateRate * 100).toFixed(2)}%`);
      console.log(`        Has local taxes: ${stateInfo.hasLocalTaxes}`);
      console.log(`        Tax shipping: ${stateInfo.taxShipping}`);
      console.log(`        Tax clothing: ${stateInfo.taxClothing}`);
    }
  }
  console.log('');

  // ============================================
  // 10. EU VAT Reference
  // ============================================

  console.log('[10] EU VAT reference...');

  const euCountries = ['DE', 'FR', 'IT', 'ES', 'NL'];
  console.log('    EU VAT Rates:');
  for (const countryCode of euCountries) {
    const vatInfo = commerce.tax.constructor.getEuVatInfo(countryCode);
    if (vatInfo) {
      console.log(`      ${vatInfo.countryName} (${countryCode}):`);
      console.log(`        Standard: ${(vatInfo.standardRate * 100).toFixed(0)}%`);
      if (vatInfo.reducedRate) {
        console.log(`        Reduced: ${(vatInfo.reducedRate * 100).toFixed(0)}%`);
      }
    }
  }

  // Check if country is in EU
  const isGermanyEU = commerce.tax.constructor.isEuCountry('DE');
  const isUSEU = commerce.tax.constructor.isEuCountry('US');
  console.log(`\n    Germany in EU: ${isGermanyEU}`);
  console.log(`    US in EU: ${isUSEU}\n`);

  // ============================================
  // 11. Canadian Tax Reference
  // ============================================

  console.log('[11] Canadian tax reference...');

  const provinces = ['ON', 'BC', 'QC', 'AB'];
  console.log('    Canadian Provincial Taxes:');
  for (const provCode of provinces) {
    const provInfo = commerce.tax.constructor.getCanadianTaxInfo(provCode);
    if (provInfo) {
      console.log(`      ${provInfo.provinceName} (${provCode}):`);
      console.log(`        GST: ${(provInfo.gstRate * 100).toFixed(0)}%`);
      if (provInfo.hstRate) {
        console.log(`        HST: ${(provInfo.hstRate * 100).toFixed(0)}%`);
      }
      if (provInfo.pstRate) {
        console.log(`        PST: ${(provInfo.pstRate * 100).toFixed(0)}%`);
      }
      if (provInfo.qstRate) {
        console.log(`        QST: ${(provInfo.qstRate * 100).toFixed(2)}%`);
      }
      console.log(`        Total: ${(provInfo.totalRate * 100).toFixed(2)}%`);
    }
  }
  console.log('');

  // ============================================
  // 12. List and Query Tax Data
  // ============================================

  console.log('[12] Querying tax data...');

  // List jurisdictions
  const allJurisdictions = await commerce.tax.listJurisdictions();
  console.log(`    Total jurisdictions: ${allJurisdictions.length}`);

  // Filter by country
  const usJurisdictions = await commerce.tax.listJurisdictions({
    countryCode: 'US'
  });
  console.log(`    US jurisdictions: ${usJurisdictions.length}`);

  // List tax rates
  const allRates = await commerce.tax.listRates();
  console.log(`    Total tax rates: ${allRates.length}`);

  // Filter rates by tax type
  const salesTaxRates = await commerce.tax.listRates({
    taxType: 'sales_tax'
  });
  console.log(`    Sales tax rates: ${salesTaxRates.length}`);

  // Get specific jurisdiction
  const caJuris = await commerce.tax.getJurisdictionByCode('US-CA');
  console.log(`    Found jurisdiction: ${caJuris.name}`);

  // ============================================
  // 13. Enable/Disable Tax
  // ============================================

  console.log('\n[13] Tax enable/disable...');

  // Disable tax
  await commerce.tax.setEnabled(false);
  const isEnabled = await commerce.tax.isEnabled();
  console.log(`    Tax enabled: ${isEnabled}`);

  // Re-enable tax
  await commerce.tax.setEnabled(true);
  const isEnabledAgain = await commerce.tax.isEnabled();
  console.log(`    Tax enabled: ${isEnabledAgain}`);

  console.log('\n=== Tax Calculation Example Complete ===');
}

main().catch(console.error);
