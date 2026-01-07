#!/usr/bin/env node
/**
 * StateSet iCommerce - Multi-Currency Example
 *
 * This example demonstrates currency management:
 * - Setting exchange rates
 * - Currency conversion
 * - Store currency settings
 * - Formatting amounts
 * - Multi-currency pricing
 *
 * Run with: node 06_currency.js
 */

const { Commerce } = require('@stateset/embedded');

async function main() {
  console.log('=== StateSet iCommerce - Multi-Currency ===\n');

  const commerce = new Commerce(':memory:');

  // ============================================
  // 1. Configure Store Currency Settings
  // ============================================

  console.log('[1] Configuring store currency settings...');

  // Set up store currency settings
  const settings = await commerce.currency.updateSettings({
    baseCurrency: 'USD',
    enabledCurrencies: ['USD', 'EUR', 'GBP', 'CAD', 'JPY', 'AUD'],
    autoConvert: true,
    roundingMode: 'half_up'
  });

  console.log(`    Base Currency: ${settings.baseCurrency}`);
  console.log(`    Enabled Currencies: ${settings.enabledCurrencies.join(', ')}`);
  console.log(`    Auto Convert: ${settings.autoConvert}`);
  console.log(`    Rounding Mode: ${settings.roundingMode}\n`);

  // Get current settings
  const currentSettings = await commerce.currency.getSettings();
  console.log(`    Retrieved settings - Base: ${currentSettings.baseCurrency}\n`);

  // ============================================
  // 2. Set Exchange Rates
  // ============================================

  console.log('[2] Setting exchange rates...');

  // Set individual exchange rates
  const usdToEur = await commerce.currency.setRate({
    baseCurrency: 'USD',
    quoteCurrency: 'EUR',
    rate: 0.92,
    source: 'manual'
  });
  console.log(`    USD -> EUR: ${usdToEur.rate}`);

  const usdToGbp = await commerce.currency.setRate({
    baseCurrency: 'USD',
    quoteCurrency: 'GBP',
    rate: 0.79,
    source: 'manual'
  });
  console.log(`    USD -> GBP: ${usdToGbp.rate}`);

  // Set multiple rates at once
  const batchRates = await commerce.currency.setRates([
    { baseCurrency: 'USD', quoteCurrency: 'CAD', rate: 1.36, source: 'api' },
    { baseCurrency: 'USD', quoteCurrency: 'JPY', rate: 149.50, source: 'api' },
    { baseCurrency: 'USD', quoteCurrency: 'AUD', rate: 1.53, source: 'api' },
    { baseCurrency: 'EUR', quoteCurrency: 'GBP', rate: 0.86, source: 'api' },
    { baseCurrency: 'EUR', quoteCurrency: 'USD', rate: 1.09, source: 'api' }
  ]);
  console.log(`    Batch set ${batchRates.length} exchange rates\n`);

  // ============================================
  // 3. Query Exchange Rates
  // ============================================

  console.log('[3] Querying exchange rates...');

  // Get specific rate
  const eurRate = await commerce.currency.getRate('USD', 'EUR');
  console.log(`    USD -> EUR: ${eurRate.rate} (Source: ${eurRate.source})`);
  console.log(`      Rate at: ${eurRate.rateAt}`);

  // Get all rates for a base currency
  const usdRates = await commerce.currency.getRatesFor('USD');
  console.log(`    Rates from USD: ${usdRates.length}`);
  for (const rate of usdRates) {
    console.log(`      -> ${rate.quoteCurrency}: ${rate.rate}`);
  }

  // List all rates
  const allRates = await commerce.currency.listRates();
  console.log(`    Total exchange rates: ${allRates.length}\n`);

  // ============================================
  // 4. Currency Conversion
  // ============================================

  console.log('[4] Converting currencies...');

  // Convert USD to EUR
  const usdToEurConversion = await commerce.currency.convert({
    from: 'USD',
    to: 'EUR',
    amount: 100.00
  });
  console.log(`    $100.00 USD = €${usdToEurConversion.convertedAmount.toFixed(2)} EUR`);
  console.log(`      Rate: ${usdToEurConversion.rate}`);
  console.log(`      Inverse: ${usdToEurConversion.inverseRate.toFixed(4)}`);

  // Convert USD to JPY
  const usdToJpyConversion = await commerce.currency.convert({
    from: 'USD',
    to: 'JPY',
    amount: 500.00
  });
  console.log(`    $500.00 USD = ¥${usdToJpyConversion.convertedAmount.toFixed(0)} JPY`);

  // Convert EUR to GBP
  const eurToGbpConversion = await commerce.currency.convert({
    from: 'EUR',
    to: 'GBP',
    amount: 250.00
  });
  console.log(`    €250.00 EUR = £${eurToGbpConversion.convertedAmount.toFixed(2)} GBP\n`);

  // ============================================
  // 5. Format Currency Amounts
  // ============================================

  console.log('[5] Formatting currency amounts...');

  const amounts = [
    { amount: 1234.56, currency: 'USD' },
    { amount: 1234.56, currency: 'EUR' },
    { amount: 1234.56, currency: 'GBP' },
    { amount: 1234.56, currency: 'JPY' },
    { amount: 1234.56, currency: 'CAD' }
  ];

  console.log('    Formatted amounts:');
  for (const { amount, currency } of amounts) {
    const formatted = await commerce.currency.format(amount, currency);
    console.log(`      ${currency}: ${formatted}`);
  }
  console.log('');

  // ============================================
  // 6. Check Currency Status
  // ============================================

  console.log('[6] Checking currency status...');

  // Check if currencies are enabled
  const isEurEnabled = await commerce.currency.isEnabled('EUR');
  const isBtcEnabled = await commerce.currency.isEnabled('BTC');
  console.log(`    EUR enabled: ${isEurEnabled}`);
  console.log(`    BTC enabled: ${isBtcEnabled}`);

  // Get base currency
  const baseCurrency = await commerce.currency.getBaseCurrency();
  console.log(`    Base currency: ${baseCurrency}`);

  // Get enabled currencies
  const enabledCurrencies = await commerce.currency.getEnabledCurrencies();
  console.log(`    Enabled currencies: ${enabledCurrencies.join(', ')}\n`);

  // ============================================
  // 7. Update Currency Settings
  // ============================================

  console.log('[7] Updating currency settings...');

  // Enable additional currencies
  const newSettings = await commerce.currency.enableCurrencies(['CHF', 'MXN', 'BRL']);
  console.log(`    Added currencies: CHF, MXN, BRL`);
  console.log(`    Now enabled: ${newSettings.enabledCurrencies.length} currencies`);

  // Change base currency
  const changedBase = await commerce.currency.setBaseCurrency('EUR');
  console.log(`    Changed base currency to: ${changedBase.baseCurrency}\n`);

  // ============================================
  // 8. Multi-Currency Order Example
  // ============================================

  console.log('[8] Multi-currency order example...');

  // Create customer
  const customer = await commerce.customers.create({
    email: 'international@example.com',
    firstName: 'International',
    lastName: 'Customer'
  });

  // Create product
  const product = await commerce.products.create({
    name: 'Global Widget',
    variants: [{ sku: 'WIDGET-GLOBAL', name: 'Standard', price: 99.99 }]
  });

  // Order in USD
  const usdOrder = await commerce.orders.create({
    customerId: customer.id,
    items: [{ sku: 'WIDGET-GLOBAL', name: 'Global Widget', quantity: 2, unitPrice: 99.99 }],
    currency: 'USD'
  });
  console.log(`    USD Order: ${usdOrder.orderNumber} - $${usdOrder.totalAmount} USD`);

  // Convert order total to other currencies for display
  const eurEquiv = await commerce.currency.convert({ from: 'USD', to: 'EUR', amount: usdOrder.totalAmount });
  const gbpEquiv = await commerce.currency.convert({ from: 'USD', to: 'GBP', amount: usdOrder.totalAmount });
  const jpyEquiv = await commerce.currency.convert({ from: 'USD', to: 'JPY', amount: usdOrder.totalAmount });

  console.log('    Equivalent amounts:');
  console.log(`      €${eurEquiv.convertedAmount.toFixed(2)} EUR`);
  console.log(`      £${gbpEquiv.convertedAmount.toFixed(2)} GBP`);
  console.log(`      ¥${jpyEquiv.convertedAmount.toFixed(0)} JPY\n`);

  // ============================================
  // 9. Delete Exchange Rate
  // ============================================

  console.log('[9] Managing exchange rates...');

  // Delete a rate
  const deleted = await commerce.currency.deleteRate(usdToEur.id);
  console.log(`    Deleted USD -> EUR rate: ${deleted}`);

  // Re-add it
  await commerce.currency.setRate({
    baseCurrency: 'USD',
    quoteCurrency: 'EUR',
    rate: 0.93, // Updated rate
    source: 'updated'
  });
  console.log(`    Re-added USD -> EUR with new rate: 0.93`);

  // Verify
  const newRate = await commerce.currency.getRate('USD', 'EUR');
  console.log(`    Verified new rate: ${newRate.rate}\n`);

  // ============================================
  // 10. Filter Exchange Rates
  // ============================================

  console.log('[10] Filtering exchange rates...');

  // Filter by base currency
  const usdBasedRates = await commerce.currency.listRates({ baseCurrency: 'USD' });
  console.log(`    Rates with USD base: ${usdBasedRates.length}`);

  // Filter by quote currency
  const eurQuoteRates = await commerce.currency.listRates({ quoteCurrency: 'EUR' });
  console.log(`    Rates to EUR: ${eurQuoteRates.length}`);

  // Filter by both
  const specificRate = await commerce.currency.listRates({
    baseCurrency: 'EUR',
    quoteCurrency: 'GBP'
  });
  console.log(`    EUR -> GBP rates: ${specificRate.length}`);

  console.log('\n=== Multi-Currency Example Complete ===');
}

main().catch(console.error);
