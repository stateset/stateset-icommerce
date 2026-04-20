/**
 * Credit Commands Module
 */

function parseAmount(value, usage) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || parsed <= 0) throw new Error(usage);
  return parsed;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'accounts': {
      const creditAccounts = await commerce.credit.listCreditAccounts();
      return formatAccounts(creditAccounts, { output, jsonOutput });
    }

    case 'account': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: credit account <creditAccountId|customerId>');
      const creditAccount = identifier.includes('-')
        ? await commerce.credit.getCreditAccount(identifier)
        : await commerce.credit.getCreditAccountByCustomer(identifier);
      if (!creditAccount) throw new Error(`Credit account not found: ${identifier}`);
      return formatAccount(creditAccount, { jsonOutput });
    }

    case 'create-account': {
      const [customerId, creditLimitRaw, paymentTerms, ...noteParts] = args;
      if (!customerId || !creditLimitRaw) {
        throw new Error(
          'Usage: credit create-account <customerId> <creditLimit> [paymentTerms] [notes]',
        );
      }
      const creditAccount = await commerce.credit.createCreditAccount({
        customerId,
        creditLimit: parseAmount(
          creditLimitRaw,
          'Usage: credit create-account <customerId> <creditLimit> [paymentTerms] [notes]',
        ),
        paymentTerms: paymentTerms || undefined,
        notes: noteParts.join(' ') || undefined,
      });
      return { creditAccount, formatted: `Created credit account ${creditAccount.id}` };
    }

    case 'check': {
      const [customerId, orderAmountRaw] = args;
      if (!customerId || !orderAmountRaw) {
        throw new Error('Usage: credit check <customerId> <orderAmount>');
      }
      const creditCheck = await commerce.credit.checkCredit(
        customerId,
        parseAmount(orderAmountRaw, 'Usage: credit check <customerId> <orderAmount>'),
      );
      return jsonOutput
        ? creditCheck
        : {
            creditCheck,
            formatted:
              `Credit check for ${customerId}\n` +
              `${'-'.repeat(30)}\n` +
              `Approved:      ${creditCheck.approved ? 'yes' : 'no'}\n` +
              `Available:     ${creditCheck.availableCredit ?? 'N/A'}\n` +
              `Requested:     ${creditCheck.requestedAmount ?? orderAmountRaw}`,
          };
    }

    case 'adjust-limit': {
      const [customerId, newLimitRaw, ...reasonParts] = args;
      if (!customerId || !newLimitRaw || reasonParts.length === 0) {
        throw new Error('Usage: credit adjust-limit <customerId> <newLimit> <reason>');
      }
      const creditAccount = await commerce.credit.adjustCreditLimit(
        customerId,
        parseAmount(newLimitRaw, 'Usage: credit adjust-limit <customerId> <newLimit> <reason>'),
        reasonParts.join(' '),
      );
      return { creditAccount, formatted: `Adjusted credit limit for customer ${customerId}` };
    }

    case 'suspend': {
      const [customerId, ...reasonParts] = args;
      if (!customerId || reasonParts.length === 0) {
        throw new Error('Usage: credit suspend <customerId> <reason>');
      }
      const creditAccount = await commerce.credit.suspendCreditAccount(
        customerId,
        reasonParts.join(' '),
      );
      return { creditAccount, formatted: `Suspended credit account for customer ${customerId}` };
    }

    case 'reactivate': {
      const customerId = args[0];
      if (!customerId) throw new Error('Usage: credit reactivate <customerId>');
      const creditAccount = await commerce.credit.reactivateCreditAccount(customerId);
      return { creditAccount, formatted: `Reactivated credit account for customer ${customerId}` };
    }

    case 'over-limit': {
      const creditAccounts = await commerce.credit.getOverLimitCustomers();
      return formatAccounts(creditAccounts, { output, jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: credit ${action}\n\n` +
          'Available actions:\n' +
          '  accounts                                                               List credit accounts\n' +
          '  account <creditAccountId|customerId>                                   Get credit account\n' +
          '  create-account <customerId> <creditLimit> [paymentTerms] [notes]       Create credit account\n' +
          '  check <customerId> <orderAmount>                                       Check customer credit\n' +
          '  adjust-limit <customerId> <newLimit> <reason>                          Adjust credit limit\n' +
          '  suspend <customerId> <reason>                                          Suspend credit account\n' +
          '  reactivate <customerId>                                                Reactivate credit account\n' +
          '  over-limit                                                             List over-limit accounts',
      );
  }
}

function formatAccounts(creditAccounts, { output, jsonOutput }) {
  if (jsonOutput) return creditAccounts;
  if (creditAccounts.length === 0) return { formatted: 'No credit accounts found.' };
  const formatted = output.table(creditAccounts, [
    { key: 'id', header: 'ID' },
    { key: 'customerId', header: 'Customer' },
    { key: 'status', header: 'Status' },
    { key: 'creditLimit', header: 'Limit', align: 'right' },
    { key: 'availableCredit', header: 'Available', align: 'right' },
  ]);
  return { creditAccounts, formatted };
}

function formatAccount(creditAccount, { jsonOutput }) {
  if (jsonOutput) return creditAccount;
  return {
    creditAccount,
    formatted:
      `Credit account: ${creditAccount.id}\n` +
      `${'-'.repeat(36)}\n` +
      `Customer:        ${creditAccount.customerId}\n` +
      `Status:          ${creditAccount.status}\n` +
      `Credit limit:    ${creditAccount.creditLimit}\n` +
      `Available:       ${creditAccount.availableCredit ?? 'N/A'}\n` +
      `Payment terms:   ${creditAccount.paymentTerms || 'N/A'}`,
  };
}

export const metadata = {
  name: 'credit',
  aliases: ['credit-accounts', 'lending'],
  description: 'Customer credit accounts and credit check commands',
  actions: {
    accounts: { description: 'List credit accounts', args: [] },
    account: { description: 'Get credit account', args: ['<creditAccountId|customerId>'] },
    'create-account': {
      description: 'Create credit account',
      args: ['<customerId>', '<creditLimit>', '[paymentTerms]', '[notes]'],
    },
    check: { description: 'Check customer credit', args: ['<customerId>', '<orderAmount>'] },
    'adjust-limit': {
      description: 'Adjust credit limit',
      args: ['<customerId>', '<newLimit>', '<reason>'],
    },
    suspend: { description: 'Suspend credit account', args: ['<customerId>', '<reason>'] },
    reactivate: { description: 'Reactivate credit account', args: ['<customerId>'] },
    'over-limit': { description: 'List over-limit accounts', args: [] },
  },
};

export default { execute, metadata };
