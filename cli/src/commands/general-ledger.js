/**
 * General Ledger Commands Module
 */

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'accounts': {
      const accounts = await commerce.generalLedger.listAccounts();
      return formatAccounts(accounts, { output, jsonOutput });
    }

    case 'account': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: general-ledger account <accountId|accountNumber>');
      const account = identifier.includes('-')
        ? await commerce.generalLedger.getAccount(identifier)
        : await commerce.generalLedger.getAccountByNumber(identifier);
      if (!account) throw new Error(`GL account not found: ${identifier}`);
      return formatAccount(account, { jsonOutput });
    }

    case 'create-account': {
      const [accountNumber, name, accountType, description, currency] = args;
      if (!accountNumber || !name || !accountType) {
        throw new Error(
          'Usage: general-ledger create-account <accountNumber> <name> <accountType> [description] [currency]',
        );
      }
      const account = await commerce.generalLedger.createAccount({
        accountNumber,
        name,
        accountType,
        description: description || undefined,
        currency: currency || undefined,
      });
      return { account, formatted: `Created GL account ${account.accountNumber}` };
    }

    case 'init-coa': {
      const accounts = await commerce.generalLedger.initializeChartOfAccounts();
      return jsonOutput
        ? accounts
        : { accounts, formatted: `Initialized chart of accounts with ${accounts.length} accounts` };
    }

    case 'entries': {
      const journalEntries = await commerce.generalLedger.listJournalEntries();
      return formatJournalEntries(journalEntries, { output, jsonOutput });
    }

    case 'entry': {
      const journalEntryId = args[0];
      if (!journalEntryId) throw new Error('Usage: general-ledger entry <journalEntryId>');
      const journalEntry = await commerce.generalLedger.getJournalEntry(journalEntryId);
      if (!journalEntry) throw new Error(`Journal entry not found: ${journalEntryId}`);
      return formatJournalEntry(journalEntry, { jsonOutput });
    }

    case 'post-entry': {
      const [journalEntryId, postedBy] = args;
      if (!journalEntryId || !postedBy) {
        throw new Error('Usage: general-ledger post-entry <journalEntryId> <postedBy>');
      }
      const journalEntry = await commerce.generalLedger.postJournalEntry(journalEntryId, postedBy);
      return { journalEntry, formatted: `Posted journal entry ${journalEntry.id}` };
    }

    case 'void-entry': {
      const journalEntryId = args[0];
      if (!journalEntryId) throw new Error('Usage: general-ledger void-entry <journalEntryId>');
      const journalEntry = await commerce.generalLedger.voidJournalEntry(journalEntryId);
      return { journalEntry, formatted: `Voided journal entry ${journalEntry.id}` };
    }

    case 'trial-balance': {
      const asOfDate = args[0];
      if (!asOfDate) throw new Error('Usage: general-ledger trial-balance <asOfDate>');
      const trialBalance = await commerce.generalLedger.getTrialBalance(asOfDate);
      return jsonOutput
        ? trialBalance
        : { trialBalance, formatted: `Trial balance as of ${asOfDate}` };
    }

    case 'balance-sheet': {
      const asOfDate = args[0];
      if (!asOfDate) throw new Error('Usage: general-ledger balance-sheet <asOfDate>');
      const balanceSheet = await commerce.generalLedger.getBalanceSheet(asOfDate);
      return jsonOutput
        ? balanceSheet
        : { balanceSheet, formatted: `Balance sheet as of ${asOfDate}` };
    }

    case 'income-statement': {
      const [startDate, endDate] = args;
      if (!startDate || !endDate) {
        throw new Error('Usage: general-ledger income-statement <startDate> <endDate>');
      }
      const incomeStatement = await commerce.generalLedger.getIncomeStatement(startDate, endDate);
      return jsonOutput
        ? incomeStatement
        : { incomeStatement, formatted: `Income statement for ${startDate} to ${endDate}` };
    }

    case 'balance': {
      const [accountId, asOfDate] = args;
      if (!accountId) throw new Error('Usage: general-ledger balance <accountId> [asOfDate]');
      const balance = await commerce.generalLedger.getAccountBalance(
        accountId,
        asOfDate || undefined,
      );
      return jsonOutput
        ? { accountId, asOfDate, balance }
        : {
            formatted: `GL account ${accountId} balance${asOfDate ? ` as of ${asOfDate}` : ''}: ${balance}`,
          };
    }

    default:
      throw new Error(
        `Unknown action: general-ledger ${action}\n\n` +
          'Available actions:\n' +
          '  accounts                                                               List GL accounts\n' +
          '  account <accountId|accountNumber>                                      Get GL account\n' +
          '  create-account <accountNumber> <name> <accountType> [description] [currency]\n' +
          '  init-coa                                                               Initialize chart of accounts\n' +
          '  entries                                                                List journal entries\n' +
          '  entry <journalEntryId>                                                 Get journal entry\n' +
          '  post-entry <journalEntryId> <postedBy>                                 Post journal entry\n' +
          '  void-entry <journalEntryId>                                            Void journal entry\n' +
          '  trial-balance <asOfDate>                                               Get trial balance\n' +
          '  balance-sheet <asOfDate>                                               Get balance sheet\n' +
          '  income-statement <startDate> <endDate>                                 Get income statement\n' +
          '  balance <accountId> [asOfDate]                                         Get account balance',
      );
  }
}

function formatAccounts(accounts, { output, jsonOutput }) {
  if (jsonOutput) return accounts;
  if (accounts.length === 0) return { formatted: 'No GL accounts found.' };
  const formatted = output.table(accounts, [
    { key: 'id', header: 'ID' },
    { key: 'accountNumber', header: 'Account #' },
    { key: 'name', header: 'Name' },
    { key: 'accountType', header: 'Type' },
    { key: 'currency', header: 'Currency' },
  ]);
  return { accounts, formatted };
}

function formatAccount(account, { jsonOutput }) {
  if (jsonOutput) return account;
  return {
    account,
    formatted:
      `GL account: ${account.accountNumber}\n` +
      `${'-'.repeat(34)}\n` +
      `ID:            ${account.id}\n` +
      `Name:          ${account.name}\n` +
      `Type:          ${account.accountType}\n` +
      `Currency:      ${account.currency || 'N/A'}\n` +
      `Description:   ${account.description || 'N/A'}`,
  };
}

function formatJournalEntries(journalEntries, { output, jsonOutput }) {
  if (jsonOutput) return journalEntries;
  if (journalEntries.length === 0) return { formatted: 'No journal entries found.' };
  const formatted = output.table(journalEntries, [
    { key: 'id', header: 'ID' },
    { key: 'status', header: 'Status' },
    { key: 'postedBy', header: 'Posted By' },
    { key: 'postedAt', header: 'Posted At' },
  ]);
  return { journalEntries, formatted };
}

function formatJournalEntry(journalEntry, { jsonOutput }) {
  if (jsonOutput) return journalEntry;
  return {
    journalEntry,
    formatted:
      `Journal entry: ${journalEntry.id}\n` +
      `${'-'.repeat(38)}\n` +
      `Status:        ${journalEntry.status}\n` +
      `Posted by:     ${journalEntry.postedBy || 'N/A'}\n` +
      `Posted at:     ${journalEntry.postedAt || 'N/A'}`,
  };
}

export const metadata = {
  name: 'general-ledger',
  aliases: ['gl', 'ledger'],
  description: 'General ledger accounts, entries, and statements',
  actions: {
    accounts: { description: 'List GL accounts', args: [] },
    account: { description: 'Get GL account', args: ['<accountId|accountNumber>'] },
    'create-account': {
      description: 'Create GL account',
      args: ['<accountNumber>', '<name>', '<accountType>', '[description]', '[currency]'],
    },
    'init-coa': { description: 'Initialize chart of accounts', args: [] },
    entries: { description: 'List journal entries', args: [] },
    entry: { description: 'Get journal entry', args: ['<journalEntryId>'] },
    'post-entry': { description: 'Post journal entry', args: ['<journalEntryId>', '<postedBy>'] },
    'void-entry': { description: 'Void journal entry', args: ['<journalEntryId>'] },
    'trial-balance': { description: 'Get trial balance', args: ['<asOfDate>'] },
    'balance-sheet': { description: 'Get balance sheet', args: ['<asOfDate>'] },
    'income-statement': { description: 'Get income statement', args: ['<startDate>', '<endDate>'] },
    balance: { description: 'Get account balance', args: ['<accountId>', '[asOfDate]'] },
  },
};

export default { execute, metadata };
