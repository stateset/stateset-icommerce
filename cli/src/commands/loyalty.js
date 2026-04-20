/**
 * Loyalty Commands Module
 */

function parseInteger(value, usage) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(usage);
  }
  return parsed;
}

function parsePositiveNumber(value, usage) {
  const parsed = Number.parseFloat(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
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
      const [name, pointsPerDollarRaw = '1', currency = 'USD', description, tiersJson] = args;
      if (!name) {
        throw new Error(
          'Usage: loyalty create-program <name> [pointsPerDollar] [currency] [description] [tiersJson]',
        );
      }
      const program = await commerce.loyalty.createProgram({
        name,
        pointsPerDollar: parseInteger(
          pointsPerDollarRaw,
          'Usage: loyalty create-program <name> [pointsPerDollar] [currency] [description] [tiersJson]',
        ),
        currency: currency.toUpperCase(),
        description: description || undefined,
        tiers: tiersJson ? parseJsonArg(tiersJson, 'tiers') : undefined,
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
      const account = await commerce.loyalty.enrollCustomer(programId, customerId);
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
      const account = await commerce.loyalty.getAccount(programId, customerId);
      if (!account) throw new Error(`Loyalty account not found for customer ${customerId}`);
      return formatAccount(account, { jsonOutput });
    }

    case 'earn': {
      const [programId, customerId, pointsRaw, reason = 'manual', orderId, ...noteParts] = args;
      if (!programId || !customerId || !pointsRaw) {
        throw new Error(
          'Usage: loyalty earn <programId> <customerId> <points> [reason] [orderId] [note]',
        );
      }
      const transaction = await commerce.loyalty.earnPoints({
        programId,
        customerId,
        points: parseInteger(
          pointsRaw,
          'Usage: loyalty earn <programId> <customerId> <points> [reason] [orderId] [note]',
        ),
        reason,
        orderId: orderId || undefined,
        note: noteParts.join(' ') || undefined,
      });
      return {
        transaction,
        formatted: `Awarded ${transaction.points || pointsRaw} points to customer ${customerId}`,
      };
    }

    case 'redeem': {
      const [programId, customerId, pointsRaw, rewardId, orderId, ...noteParts] = args;
      if (!programId || !customerId || !pointsRaw) {
        throw new Error(
          'Usage: loyalty redeem <programId> <customerId> <points> [rewardId] [orderId] [note]',
        );
      }
      const transaction = await commerce.loyalty.redeemPoints({
        programId,
        customerId,
        points: parseInteger(
          pointsRaw,
          'Usage: loyalty redeem <programId> <customerId> <points> [rewardId] [orderId] [note]',
        ),
        rewardId: rewardId || undefined,
        orderId: orderId || undefined,
        note: noteParts.join(' ') || undefined,
      });
      return {
        transaction,
        formatted: `Redeemed ${transaction.points || pointsRaw} points for customer ${customerId}`,
      };
    }

    case 'rewards': {
      const [programId, tier] = args;
      if (!programId) throw new Error('Usage: loyalty rewards <programId> [tier]');
      const rewards = await commerce.loyalty.listRewards(programId, { tier: tier || undefined });
      return formatRewards(rewards, { output, jsonOutput });
    }

    case 'create-reward': {
      const [
        programId,
        name,
        pointsCostRaw,
        type,
        valueRaw,
        tier,
        maxRedemptionsRaw,
        stockRaw,
        description,
      ] = args;
      if (!programId || !name || !pointsCostRaw || !type || !valueRaw) {
        throw new Error(
          'Usage: loyalty create-reward <programId> <name> <pointsCost> <type> <value> [tier] [maxRedemptions] [stock] [description]',
        );
      }
      const reward = await commerce.loyalty.createReward(programId, {
        name,
        description: description || undefined,
        pointsCost: parseInteger(
          pointsCostRaw,
          'Usage: loyalty create-reward <programId> <name> <pointsCost> <type> <value> [tier] [maxRedemptions] [stock] [description]',
        ),
        type,
        value: String(
          parsePositiveNumber(
            valueRaw,
            'Usage: loyalty create-reward <programId> <name> <pointsCost> <type> <value> [tier] [maxRedemptions] [stock] [description]',
          ),
        ),
        tier: tier || undefined,
        maxRedemptions: maxRedemptionsRaw
          ? parseInteger(maxRedemptionsRaw, 'maxRedemptions must be a non-negative integer')
          : undefined,
        stock: stockRaw
          ? parseInteger(stockRaw, 'stock must be a non-negative integer')
          : undefined,
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
          '  create-program <name> [pointsPerDollar] [currency] [description] [tiersJson]\n' +
          '  enroll <programId> <customerId>                                          Enroll customer\n' +
          '  account <programId> <customerId>                                         Get loyalty account\n' +
          '  earn <programId> <customerId> <points> [reason] [orderId] [note]        Award points\n' +
          '  redeem <programId> <customerId> <points> [rewardId] [orderId] [note]    Redeem points\n' +
          '  rewards <programId> [tier]                                               List rewards\n' +
          '  create-reward <programId> <name> <pointsCost> <type> <value> [tier] [maxRedemptions] [stock] [description]',
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
      `Currency:         ${program.currency}\n` +
      `Points/$:         ${program.pointsPerDollar}\n` +
      `Members:          ${program.totalMembers || 0}`,
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
      `Current tier:     ${account.currentTier || 'N/A'}\n` +
      `Next tier:        ${account.nextTier || 'N/A'}`,
  };
}

function formatRewards(rewards, { output, jsonOutput }) {
  if (jsonOutput) return rewards;
  if (rewards.length === 0) return { formatted: 'No loyalty rewards found.' };
  const formatted = output.table(rewards, [
    { key: 'id', header: 'ID' },
    { key: 'name', header: 'Name' },
    { key: 'pointsCost', header: 'Points', align: 'right' },
    { key: 'type', header: 'Type' },
    { key: 'tier', header: 'Tier' },
    { key: 'status', header: 'Status' },
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
      args: ['<name>', '[pointsPerDollar]', '[currency]', '[description]', '[tiersJson]'],
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
    rewards: { description: 'List loyalty rewards', args: ['<programId>', '[tier]'] },
    'create-reward': {
      description: 'Create loyalty reward',
      args: [
        '<programId>',
        '<name>',
        '<pointsCost>',
        '<type>',
        '<value>',
        '[tier]',
        '[maxRedemptions]',
        '[stock]',
        '[description]',
      ],
    },
  },
};

export default { execute, metadata };
