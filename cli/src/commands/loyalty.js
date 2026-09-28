/**
 * Loyalty Commands Module
 */

function parseInteger(value, usage, { positive = false } = {}) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < (positive ? 1 : 0)) {
    throw new Error(usage);
  }
  return parsed;
}

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

/** Exact decimal amount, as a string (money is never a float). */
const DECIMAL_RE = /^\d+(\.\d+)?$/;

const USAGE = {
  createProgram: 'Usage: loyalty create-program <name> [pointsPerDollar] [description] [tiersJson]',
  earn: 'Usage: loyalty earn <programId> <customerId> <points> [reason] [orderId] [note]',
  redeem: 'Usage: loyalty redeem <programId> <customerId> <points> [rewardId] [orderId] [note]',
  createReward:
    'Usage: loyalty create-reward <programId> <name> <pointsCost> <rewardType> [value] [description]',
};

/**
 * The customer's account in the program. Points live on the account (one per
 * customer per program), so every earn/redeem needs it first.
 */
async function requireAccount(commerce, programId, customerId) {
  const account = await commerce.loyalty.getAccountByCustomer(customerId, programId);
  if (!account) {
    throw new Error(`Customer ${customerId} is not enrolled in loyalty program ${programId}`);
  }
  return account;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'program': {
      const programId = args[0];
      if (!programId) throw new Error('Usage: loyalty program <programId>');
      const program = await commerce.loyalty.getProgram(programId);
      if (!program) throw new Error(`Loyalty program not found: ${programId}`);
      return formatProgram(program, { jsonOutput });
    }

    case 'create-program': {
      const [name, pointsPerDollarRaw = '1', description, tiersJson] = args;
      if (!name) throw new Error(USAGE.createProgram);
      const tiers = tiersJson ? parseJsonArg(tiersJson, 'tiers') : undefined;
      if (tiers !== undefined && !Array.isArray(tiers)) {
        throw new Error('tiers must be a JSON array');
      }
      const program = await commerce.loyalty.createProgram({
        name,
        description: description || undefined,
        pointsPerDollar: parseInteger(pointsPerDollarRaw, USAGE.createProgram, { positive: true }),
        tiers: tiers?.map((t) => ({
          name: t.name,
          minPoints: t.minPoints,
          multiplier: t.multiplier ?? 1,
          perks: t.perks ?? [],
        })),
      });
      return {
        program,
        formatted: `Created loyalty program ${program.name || program.id}`,
      };
    }

    case 'enroll': {
      const [programId, customerId] = args;
      if (!programId || !customerId) {
        throw new Error('Usage: loyalty enroll <programId> <customerId>');
      }
      const account = await commerce.loyalty.enroll({ customerId, programId });
      return {
        account,
        formatted: `Enrolled customer ${customerId} in loyalty program ${programId}`,
      };
    }

    case 'account': {
      const [programId, customerId] = args;
      if (!programId || !customerId) {
        throw new Error('Usage: loyalty account <programId> <customerId>');
      }
      const account = await commerce.loyalty.getAccountByCustomer(customerId, programId);
      if (!account) throw new Error(`Loyalty account not found for customer ${customerId}`);
      return formatAccount(account, { jsonOutput });
    }

    case 'earn': {
      const [programId, customerId, pointsRaw, reason = 'manual', orderId, ...noteParts] = args;
      if (!programId || !customerId || !pointsRaw) throw new Error(USAGE.earn);
      const points = parseInteger(pointsRaw, USAGE.earn, { positive: true });
      const account = await requireAccount(commerce, programId, customerId);
      const note = noteParts.join(' ');
      const transaction = await commerce.loyalty.adjustPoints({
        accountId: account.id,
        points,
        transactionType: 'earn',
        referenceId: orderId || undefined,
        description: note ? `${reason}: ${note}` : reason,
      });
      return {
        transaction,
        formatted: `Awarded ${transaction.points} points to customer ${customerId}`,
      };
    }

    case 'redeem': {
      const [programId, customerId, pointsRaw, rewardId, orderId, ...noteParts] = args;
      if (!programId || !customerId || !pointsRaw) throw new Error(USAGE.redeem);
      const points = parseInteger(pointsRaw, USAGE.redeem, { positive: true });
      const account = await requireAccount(commerce, programId, customerId);
      let reward = null;
      if (rewardId) {
        reward = await commerce.loyalty.getReward(rewardId);
        if (!reward || reward.programId !== programId) {
          throw new Error(`Reward ${rewardId} not found in loyalty program ${programId}`);
        }
        if (!reward.isActive) throw new Error(`Reward ${rewardId} is not active`);
      }
      const parts = [];
      if (reward) parts.push(`reward ${reward.id} (${reward.name})`);
      if (noteParts.length) parts.push(noteParts.join(' '));
      // The engine refuses a redemption larger than the balance.
      const transaction = await commerce.loyalty.adjustPoints({
        accountId: account.id,
        points: -points,
        transactionType: 'redeem',
        referenceId: orderId || reward?.id,
        description: parts.length ? parts.join(': ') : undefined,
      });
      return {
        transaction,
        formatted: `Redeemed ${points} points for customer ${customerId}`,
      };
    }

    case 'rewards': {
      const [programId, rewardType] = args;
      if (!programId) throw new Error('Usage: loyalty rewards <programId> [rewardType]');
      const rewards = await commerce.loyalty.listRewards({
        programId,
        rewardType: rewardType || undefined,
      });
      return formatRewards(rewards, { output, jsonOutput });
    }

    case 'create-reward': {
      const [programId, name, pointsCostRaw, rewardType, value, description] = args;
      if (!programId || !name || !pointsCostRaw || !rewardType) {
        throw new Error(USAGE.createReward);
      }
      if (value !== undefined && value !== '' && !DECIMAL_RE.test(value)) {
        throw new Error(`value must be a non-negative decimal string, e.g. "10.00": ${value}`);
      }
      const reward = await commerce.loyalty.createReward({
        programId,
        name,
        description: description || undefined,
        pointsCost: parseInteger(pointsCostRaw, USAGE.createReward, { positive: true }),
        rewardType,
        value: value || undefined,
      });
      return {
        reward,
        formatted: `Created loyalty reward ${reward.name || reward.id}`,
      };
    }

    default:
      throw new Error(
        `Unknown action: loyalty ${action}\n\n` +
          'Available actions:\n' +
          '  program <programId>                                                      Get loyalty program\n' +
          '  create-program <name> [pointsPerDollar] [description] [tiersJson]\n' +
          '  enroll <programId> <customerId>                                          Enroll customer\n' +
          '  account <programId> <customerId>                                         Get loyalty account\n' +
          '  earn <programId> <customerId> <points> [reason] [orderId] [note]        Award points\n' +
          '  redeem <programId> <customerId> <points> [rewardId] [orderId] [note]    Redeem points\n' +
          '  rewards <programId> [rewardType]                                         List rewards\n' +
          '  create-reward <programId> <name> <pointsCost> <rewardType> [value] [description]',
      );
  }
}

function formatProgram(program, { jsonOutput }) {
  if (jsonOutput) return program;
  return {
    program,
    formatted:
      `Loyalty program: ${program.name}\n` +
      `${'-'.repeat(42)}\n` +
      `ID:               ${program.id}\n` +
      `Status:           ${program.status}\n` +
      `Points/$:         ${program.pointsPerDollar}\n` +
      `Tiers:            ${program.tiers.map((t) => t.name).join(', ') || 'none'}`,
  };
}

function formatAccount(account, { jsonOutput }) {
  if (jsonOutput) return account;
  return {
    account,
    formatted:
      `Loyalty account: ${account.id}\n` +
      `${'-'.repeat(40)}\n` +
      `Customer:         ${account.customerId}\n` +
      `Program:          ${account.programId}\n` +
      `Points balance:   ${account.pointsBalance}\n` +
      `Lifetime points:  ${account.lifetimePoints}\n` +
      `Tier:             ${account.tier || 'N/A'}`,
  };
}

function formatRewards(rewards, { output, jsonOutput }) {
  if (jsonOutput) return rewards;
  if (rewards.length === 0) return { formatted: 'No loyalty rewards found.' };
  const formatted = output.table(rewards, [
    { key: 'id', header: 'ID' },
    { key: 'name', header: 'Name' },
    { key: 'pointsCost', header: 'Points', align: 'right' },
    { key: 'rewardType', header: 'Type' },
    { key: 'value', header: 'Value', align: 'right' },
    { key: 'isActive', header: 'Active' },
  ]);
  return { rewards, formatted };
}

export const metadata = {
  name: 'loyalty',
  aliases: ['rewards', 'points'],
  description: 'Loyalty programs, accounts, points, and rewards',
  actions: {
    program: { description: 'Get loyalty program', args: ['<programId>'] },
    'create-program': {
      description: 'Create loyalty program',
      args: ['<name>', '[pointsPerDollar]', '[description]', '[tiersJson]'],
    },
    enroll: { description: 'Enroll customer', args: ['<programId>', '<customerId>'] },
    account: { description: 'Get loyalty account', args: ['<programId>', '<customerId>'] },
    earn: {
      description: 'Award loyalty points',
      args: ['<programId>', '<customerId>', '<points>', '[reason]', '[orderId]', '[note]'],
    },
    redeem: {
      description: 'Redeem loyalty points',
      args: ['<programId>', '<customerId>', '<points>', '[rewardId]', '[orderId]', '[note]'],
    },
    rewards: { description: 'List loyalty rewards', args: ['<programId>', '[rewardType]'] },
    'create-reward': {
      description: 'Create loyalty reward',
      args: ['<programId>', '<name>', '<pointsCost>', '<rewardType>', '[value]', '[description]'],
    },
  },
};

export default { execute, metadata };
