/**
 * Tax Commands Module
 */

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'rate': {
      const [country, state, city, category = 'standard'] = args;
      if (!country) throw new Error('Usage: tax rate <country> [state] [city] [category]');
      const rate = await commerce.tax.getEffectiveRate({ country, state, city }, category);
      return jsonOutput
        ? { country, state, city, category, rate }
        : {
            formatted: `Effective tax rate for ${country}${state ? `-${state}` : ''}: ${(rate * 100).toFixed(2)}%`,
          };
    }

    case 'item': {
      const [country, unitPriceRaw, quantityRaw, state, postalCode, category] = args;
      const unitPrice = Number.parseFloat(unitPriceRaw);
      const quantity = Number.parseInt(quantityRaw, 10);
      if (!country || !Number.isFinite(unitPrice) || !Number.isInteger(quantity)) {
        throw new Error(
          'Usage: tax item <country> <unitPrice> <quantity> [state] [postalCode] [category]',
        );
      }
      const taxAmount = await commerce.tax.calculateForItem(unitPrice, quantity, category, {
        country,
        state,
        postalCode,
      });
      return jsonOutput
        ? { country, unitPrice, quantity, taxAmount }
        : { formatted: `Tax amount: ${taxAmount}` };
    }

    case 'jurisdictions': {
      const [countryCode, level] = args;
      const jurisdictions = await commerce.tax.listJurisdictions({
        countryCode,
        level,
        activeOnly: true,
      });
      return formatJurisdictions(jurisdictions, { output, jsonOutput });
    }

    case 'providers': {
      const providers = await import('../tools/providers/tax.js').then((mod) =>
        mod.listTaxProviders({}),
      );
      return formatProviders(providers, { output, jsonOutput });
    }

    case 'settings': {
      const settings = await commerce.tax.getSettings();
      return formatSettings(settings, { jsonOutput });
    }

    case 'state': {
      const stateCode = args[0];
      if (!stateCode) throw new Error('Usage: tax state <stateCode>');
      const upper = stateCode.toUpperCase();
      const staticStateMap = {
        CA: 'California',
        TX: 'Texas',
        NY: 'New York',
        FL: 'Florida',
        WA: 'Washington',
        OR: 'Oregon',
        DE: 'Delaware',
        MT: 'Montana',
        NH: 'New Hampshire',
        AK: 'Alaska',
      };
      const rate = await commerce.tax.getEffectiveRate({ country: 'US', state: upper }, 'standard');
      return jsonOutput
        ? { stateCode: upper, stateName: staticStateMap[upper] || upper, effectiveRate: rate }
        : {
            formatted: `${staticStateMap[upper] || upper} effective state rate: ${(rate * 100).toFixed(2)}%`,
          };
    }

    case 'exemptions': {
      const customerId = args[0];
      if (!customerId) throw new Error('Usage: tax exemptions <customerId>');
      const exemptions = await commerce.tax.getCustomerExemptions(customerId);
      return formatExemptions(exemptions, { output, jsonOutput });
    }

    case 'exempt': {
      const customerId = args[0];
      if (!customerId) throw new Error('Usage: tax exempt <customerId>');
      const exempt = await commerce.tax.customerIsExempt(customerId);
      return jsonOutput
        ? { customerId, exempt }
        : { formatted: `Customer ${customerId} tax exempt: ${exempt ? 'yes' : 'no'}` };
    }

    default:
      throw new Error(
        `Unknown action: tax ${action}\n\n` +
          'Available actions:\n' +
          '  rate <country> [state] [city] [category]  Get effective rate\n' +
          '  item <country> <unitPrice> <quantity> [state] [postalCode] [category]  Calculate item tax\n' +
          '  jurisdictions [countryCode] [level]       List tax jurisdictions\n' +
          '  providers                                 List tax providers\n' +
          '  settings                                  Get tax settings\n' +
          '  state <stateCode>                         Inspect US state tax rate\n' +
          '  exemptions <customerId>                   List customer exemptions\n' +
          '  exempt <customerId>                       Check if customer is exempt',
      );
  }
}

function formatJurisdictions(jurisdictions, { output, jsonOutput }) {
  if (jsonOutput) return jurisdictions;
  if (jurisdictions.length === 0) return { formatted: 'No tax jurisdictions found.' };
  const formatted = output.table(jurisdictions, [
    { key: 'id', header: 'ID' },
    { key: 'code', header: 'Code' },
    { key: 'name', header: 'Name' },
    { key: 'level', header: 'Level' },
    { key: 'countryCode', header: 'Country' },
    { key: 'stateCode', header: 'State' },
  ]);
  return { jurisdictions, formatted };
}

function formatProviders(providers, { output, jsonOutput }) {
  if (jsonOutput) return providers;
  if (providers.length === 0) return { formatted: 'No tax providers found.' };
  const formatted = output.table(
    providers.map((provider) => ({
      id: provider.id,
      mode: provider.mode,
      countries: (provider.countryCodes || []).join(','),
      capabilities: (provider.capabilities || []).slice(0, 4).join(','),
    })),
    [
      { key: 'id', header: 'Provider' },
      { key: 'mode', header: 'Mode' },
      { key: 'countries', header: 'Countries' },
      { key: 'capabilities', header: 'Capabilities' },
    ],
  );
  return { providers, formatted };
}

function formatSettings(settings, { jsonOutput }) {
  if (jsonOutput) return settings;
  return {
    settings,
    formatted:
      `Tax settings\n` +
      `${'-'.repeat(22)}\n` +
      `Enabled:             ${settings.enabled ? 'yes' : 'no'}\n` +
      `Calculation method:  ${settings.calculationMethod}\n` +
      `Tax shipping:        ${settings.taxShipping ? 'yes' : 'no'}\n` +
      `Tax handling:        ${settings.taxHandling ? 'yes' : 'no'}\n` +
      `Provider:            ${settings.taxProvider || 'N/A'}`,
  };
}

function formatExemptions(exemptions, { output, jsonOutput }) {
  if (jsonOutput) return exemptions;
  if (exemptions.length === 0) return { formatted: 'No tax exemptions found.' };
  const formatted = output.table(exemptions, [
    { key: 'id', header: 'ID' },
    { key: 'exemptionType', header: 'Type' },
    { key: 'certificateNumber', header: 'Certificate' },
    { key: 'verified', header: 'Verified' },
    { key: 'expiresAt', header: 'Expires' },
  ]);
  return { exemptions, formatted };
}

export const metadata = {
  name: 'tax',
  aliases: ['t', 'vat'],
  description: 'Tax calculation and configuration commands',
  actions: {
    rate: {
      description: 'Get effective tax rate',
      args: ['<country>', '[state]', '[city]', '[category]'],
    },
    item: {
      description: 'Calculate tax for an item',
      args: ['<country>', '<unitPrice>', '<quantity>', '[state]', '[postalCode]', '[category]'],
    },
    jurisdictions: { description: 'List tax jurisdictions', args: ['[countryCode]', '[level]'] },
    providers: { description: 'List tax providers', args: [] },
    settings: { description: 'Get tax settings', args: [] },
    state: { description: 'Inspect US state rate', args: ['<stateCode>'] },
    exemptions: { description: 'List customer tax exemptions', args: ['<customerId>'] },
    exempt: { description: 'Check if customer is tax exempt', args: ['<customerId>'] },
  },
};

export default { execute, metadata };
