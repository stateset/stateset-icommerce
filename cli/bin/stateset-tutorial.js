#!/usr/bin/env node
/**
 * StateSet Tutorial Command
 *
 * Interactive tutorials and onboarding for StateSet CLI.
 *
 * Usage:
 *   stateset-tutorial                    # List available tutorials
 *   stateset-tutorial quickstart         # Run quickstart tutorial
 *   stateset-tutorial orders             # Run orders tutorial
 *   stateset-tutorial inventory          # Run inventory tutorial
 *   stateset-tutorial checkout           # Run checkout tutorial
 *   stateset-tutorial analytics          # Run analytics tutorial
 */

import { parseArgs } from 'node:util';
import {
  TUTORIALS,
  TutorialRunner,
  createTutorialRunner,
  showWelcome
} from '../src/tutorial.js';

const options = {
  list: { type: 'boolean', short: 'l', default: false },
  'non-interactive': { type: 'boolean', default: false },
  help: { type: 'boolean', short: 'h', default: false }
};

const { values, positionals } = parseArgs({ options, allowPositionals: true });

if (values.help) {
  console.log(`
StateSet Tutorial - Interactive Learning

Usage:
  stateset-tutorial [options] [tutorial-name]

Options:
  -l, --list           List available tutorials
  --non-interactive    Don't prompt between steps
  -h, --help           Show this help

Available Tutorials:
  quickstart    Learn the basics in 5 minutes (recommended for new users)
  orders        Order lifecycle management
  inventory     Stock tracking and reservations
  checkout      Shopping cart and checkout flow (ACP)
  analytics     Business intelligence and forecasting

Examples:
  stateset-tutorial                    # List tutorials
  stateset-tutorial quickstart         # Run quickstart
  stateset-tutorial orders             # Learn order management
  stateset-tutorial --non-interactive checkout  # Non-interactive mode
`);
  process.exit(0);
}

const runner = createTutorialRunner({
  interactive: !values['non-interactive']
});

if (values.list || positionals.length === 0) {
  showWelcome();
  runner.listTutorials();
  process.exit(0);
}

const tutorialId = positionals[0];

if (!TUTORIALS[tutorialId]) {
  console.error(`Unknown tutorial: ${tutorialId}\n`);
  runner.listTutorials();
  process.exit(1);
}

try {
  const completed = await runner.run(tutorialId);
  process.exit(completed ? 0 : 1);
} catch (error) {
  console.error('Error running tutorial:', error.message);
  process.exit(1);
}
