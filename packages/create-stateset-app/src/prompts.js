import { createInterface } from 'node:readline/promises';
import { stdin, stdout } from 'node:process';

async function prompt(question, defaultValue) {
  const rl = createInterface({ input: stdin, output: stdout });
  const suffix = defaultValue ? ` (${defaultValue})` : '';
  try {
    const answer = await rl.question(`  ${question}${suffix}: `);
    return answer.trim() || defaultValue || '';
  } finally {
    rl.close();
  }
}

async function confirm(question, defaultYes = true) {
  const rl = createInterface({ input: stdin, output: stdout });
  const suffix = defaultYes ? ' (Y/n)' : ' (y/N)';
  try {
    const answer = await rl.question(`  ${question}${suffix}: `);
    const normalized = answer.trim().toLowerCase();
    if (normalized === '') return defaultYes;
    return normalized === 'y' || normalized === 'yes';
  } finally {
    rl.close();
  }
}

export async function promptProjectName() {
  return prompt('What is your project named?', 'my-store');
}

export async function promptStoreName(defaultName) {
  return prompt('What is your store name?', defaultName);
}

export async function promptInstall(pm) {
  return confirm(`Install dependencies with ${pm}?`, true);
}
