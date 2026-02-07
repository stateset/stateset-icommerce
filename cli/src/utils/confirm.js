import * as readline from 'node:readline';

const defaultOutput = {
  yellow: (text) => text,
  bold: (text) => text,
};

export function normalizeConfirmContext(ctx = {}) {
  const operation = ctx.operation || ctx.tool || ctx.name || 'operation';
  const details = ctx.details || ctx.message || null;
  const amountRaw = ctx.amount;
  const amount = typeof amountRaw === 'number' ? amountRaw : Number(amountRaw);
  return {
    operation,
    details,
    amount: Number.isFinite(amount) ? amount : null,
  };
}

export function createConfirmPrompt({ input = process.stdin, output = process.stdout } = {}) {
  return (message) =>
    new Promise((resolve) => {
      const rl = readline.createInterface({ input, output });
      rl.question(`${message} [y/N] `, (answer) => {
        rl.close();
        resolve(answer.toLowerCase() === 'y' || answer.toLowerCase() === 'yes');
      });
    });
}

export function createConfirmHandler({
  output = null,
  assumeYes = false,
  nonInteractive = false,
  confirmPrompt = null,
} = {}) {
  if (assumeYes) return async () => true;

  const style = output
    ? {
        yellow: output.yellow?.bind(output) || defaultOutput.yellow,
        bold: output.bold?.bind(output) || defaultOutput.bold,
      }
    : defaultOutput;

  if (nonInteractive) {
    let warned = false;
    return async (ctx = {}) => {
      if (!warned) {
        const { operation, details, amount } = normalizeConfirmContext(ctx);
        let message = `Error: Confirmation required for ${operation}. Re-run with --yes to proceed.`;
        if (details) message += ` ${details}`;
        if (amount !== null) message += ` Amount: $${amount.toFixed(2)}.`;
        console.error(message);
        warned = true;
      }
      return false;
    };
  }

  const prompt = confirmPrompt || createConfirmPrompt();
  return async (ctx = {}) => {
    const { operation, details, amount } = normalizeConfirmContext(ctx);
    let message = `\n${style.yellow('WARNING: Confirmation required')}\n`;
    message += `   Operation: ${operation}\n`;
    if (details) message += `   Details: ${details}\n`;
    if (amount !== null) message += `   Amount: ${style.bold('$' + amount.toFixed(2))}\n`;
    message += `\n   Proceed?`;

    console.log(message);
    return await prompt('');
  };
}
