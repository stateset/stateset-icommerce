/**
 * Currency Commands Module
 */

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'rate': {
      const [from, to] = args;
      if (!from || !to) throw new Error('Usage: currency rate <from> <to>');
      const rate = await commerce.currency.getRate(from.toUpperCase(), to.toUpperCase());
      if (!rate) throw new Error(`No exchange rate found for ${from} -> ${to}`);
      return formatRate(rate, { jsonOutput });
    }

    case 'rates': {
      const baseCurrency = args[0];
      const rates = baseCurrency
        ? await commerce.currency.getRatesFor(baseCurrency.toUpperCase())
        : await commerce.currency.listRates();
      return formatRates(rates, { output, jsonOutput });
    }

    case 'convert': {
      const [from, to, amountRaw] = args;
      const amount = Number.parseFloat(amountRaw);
      if (!from || !to || !Number.isFinite(amount)) {
        throw new Error('Usage: currency convert <from> <to> <amount>');
      }
      const result = await commerce.currency.convert({
        from: from.toUpperCase(),
        to: to.toUpperCase(),
        amount,
      });
      return formatConversion(result, { jsonOutput });
    }

    case 'settings': {
      const settings = await commerce.currency.getSettings();
      return formatSettings(settings, { jsonOutput });
    }

    case 'enabled': {
      const currency = args[0];
      if (!currency) throw new Error('Usage: currency enabled <currency>');
      const enabled = await commerce.currency.isEnabled(currency.toUpperCase());
      return jsonOutput
        ? { currency: currency.toUpperCase(), enabled }
        : { formatted: `${currency.toUpperCase()} enabled: ${enabled ? 'yes' : 'no'}` };
    }

    case 'set-rate': {
      const [baseCurrency, quoteCurrency, rateRaw, source = 'manual'] = args;
      const rate = Number.parseFloat(rateRaw);
      if (!baseCurrency || !quoteCurrency || !Number.isFinite(rate)) {
        throw new Error('Usage: currency set-rate <base> <quote> <rate> [source]');
      }
      const result = await commerce.currency.setRate({
        baseCurrency: baseCurrency.toUpperCase(),
        quoteCurrency: quoteCurrency.toUpperCase(),
        rate,
        source,
      });
      return {
        rate: result,
        formatted: `Set exchange rate 1 ${result.baseCurrency} = ${result.rate} ${result.quoteCurrency}`,
      };
    }

    case 'base': {
      const currency = args[0];
      if (!currency) throw new Error('Usage: currency base <currency>');
      const settings = await commerce.currency.setBaseCurrency(currency.toUpperCase());
      return {
        settings,
        formatted: `Base currency set to ${settings.baseCurrency || currency.toUpperCase()}`,
      };
    }

    default:
      throw new Error(
        `Unknown action: currency ${action}\n\n` +
          'Available actions:\n' +
          '  rate <from> <to>           Get exchange rate\n' +
          '  rates [baseCurrency]       List exchange rates\n' +
          '  convert <from> <to> <amount>  Convert amount\n' +
          '  settings                   Get currency settings\n' +
          '  enabled <currency>         Check if currency is enabled\n' +
          '  set-rate <base> <quote> <rate> [source]  Set exchange rate\n' +
          '  base <currency>            Set base currency',
      );
  }
}

function formatRate(rate, { jsonOutput }) {
  if (jsonOutput) return rate;
  return {
    rate,
    formatted:
      `Exchange rate\n` +
      `${'-'.repeat(24)}\n` +
      `Base:      ${rate.baseCurrency}\n` +
      `Quote:     ${rate.quoteCurrency}\n` +
      `Rate:      ${rate.rate}\n` +
      `Source:    ${rate.source || 'N/A'}\n` +
      `Rate at:   ${rate.rateAt || 'N/A'}`,
  };
}

function formatRates(rates, { output, jsonOutput }) {
  if (jsonOutput) return rates;
  if (rates.length === 0) return { formatted: 'No exchange rates found.' };
  const formatted = output.table(rates, [
    { key: 'baseCurrency', header: 'Base' },
    { key: 'quoteCurrency', header: 'Quote' },
    { key: 'rate', header: 'Rate', align: 'right' },
    { key: 'source', header: 'Source' },
  ]);
  return { rates, formatted };
}

function formatConversion(result, { jsonOutput }) {
  if (jsonOutput) return result;
  return {
    result,
    formatted:
      `Conversion\n` +
      `${'-'.repeat(20)}\n` +
      `${result.originalAmount} ${result.originalCurrency} = ${result.convertedAmount} ${result.targetCurrency}\n` +
      `Rate: ${result.rate}`,
  };
}

function formatSettings(settings, { jsonOutput }) {
  if (jsonOutput) return settings;
  return {
    settings,
    formatted:
      `Currency settings\n` +
      `${'-'.repeat(28)}\n` +
      `Base currency:      ${settings.baseCurrency}\n` +
      `Enabled currencies: ${(settings.enabledCurrencies || []).join(', ')}\n` +
      `Auto convert:       ${settings.autoConvert ? 'yes' : 'no'}\n` +
      `Rounding mode:      ${settings.roundingMode || 'N/A'}`,
  };
}

export const metadata = {
  name: 'currency',
  aliases: ['curr', 'fx'],
  description: 'Currency and exchange rate commands',
  actions: {
    rate: { description: 'Get an exchange rate', args: ['<from>', '<to>'] },
    rates: { description: 'List exchange rates', args: ['[baseCurrency]'] },
    convert: { description: 'Convert an amount', args: ['<from>', '<to>', '<amount>'] },
    settings: { description: 'Get currency settings', args: [] },
    enabled: { description: 'Check if currency is enabled', args: ['<currency>'] },
    'set-rate': {
      description: 'Set an exchange rate',
      args: ['<base>', '<quote>', '<rate>', '[source]'],
    },
    base: { description: 'Set base currency', args: ['<currency>'] },
  },
};

export default { execute, metadata };
