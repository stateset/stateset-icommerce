#!/usr/bin/env node

/**
 * StateSet Sync CLI (VES v1.0)
 *
 * Verifiable Event Sync (VES) commands for syncing local SQLite with production sequencer.
 * Supports VES v1.0 agent signatures and encrypted payloads.
 *
 * Usage:
 *   stateset-sync init              # Initialize sync configuration
 *   stateset-sync push              # Push pending events to sequencer
 *   stateset-sync pull              # Pull remote events locally
 *   stateset-sync status            # Show sync status
 *   stateset-sync verify <event-id> # Verify event inclusion
 *   stateset-sync rebase            # Rebase after conflict
 *
 * Key Management (VES v1.0):
 *   stateset-sync keys:generate     # Generate Ed25519/X25519 key pairs
 *   stateset-sync keys:list         # List agent keys
 *   stateset-sync keys:register     # Register signing key with sequencer
 *   stateset-sync keys:rotate       # Rotate keys (generate new, revoke old)
 *   stateset-sync keys:export       # Export public keys for sharing
 */

import { Command } from 'commander';
import chalk from 'chalk';
import Database from 'better-sqlite3';
import ora from 'ora';
import {
  loadSyncConfig,
  saveSyncConfig,
  createSyncConfig,
  isSyncConfigured,
  validateSyncConfig,
  getConfigDir,
  SyncConfig,
} from '../src/sync/config.js';
import { createSyncEngine } from '../src/sync/engine.js';
import { createOutbox } from '../src/sync/outbox.js';
import { createSequencerClient } from '../src/sync/client.js';
import { getKeyManager } from '../src/sync/keys.js';
import { bufferToHex } from '../src/sync/crypto.js';

const program = new Command();

program
  .name('stateset-sync')
  .description('Verifiable Event Sync (VES) for StateSet CLI')
  .version('0.1.0');

// ============================================================================
// init command
// ============================================================================
program
  .command('init')
  .description('Initialize sync configuration for this store')
  .requiredOption('--sequencer-url <url>', 'Sequencer URL (grpc:// or https://)')
  .requiredOption('--tenant-id <uuid>', 'Tenant UUID')
  .requiredOption('--store-id <uuid>', 'Store UUID')
  .option('--api-key <key>', 'API key for authentication')
  .option('--db <path>', 'Database path', './store.db')
  .action(async (options) => {
    const spinner = ora('Initializing sync...').start();

    try {
      // Check if already configured
      if (isSyncConfigured()) {
        spinner.warn('Sync already configured. Use --force to reinitialize.');
        process.exit(1);
      }

      // Create configuration
      spinner.text = 'Creating sync configuration...';
      const config = createSyncConfig({
        sequencerUrl: options.sequencerUrl,
        tenantId: options.tenantId,
        storeId: options.storeId,
        apiKey: options.apiKey,
        dbPath: options.db,
      });

      // Validate
      const validation = validateSyncConfig(config);
      if (!validation.valid) {
        spinner.fail('Invalid configuration');
        console.error(chalk.red('Errors:'));
        validation.errors.forEach(e => console.error(chalk.red(`  - ${e}`)));
        process.exit(1);
      }

      // Initialize database with outbox schema
      spinner.text = 'Initializing outbox tables...';
      const db = new Database(options.db);
      const outbox = createOutbox(db);
      outbox.initialize();

      // Set initial sync state
      outbox.updateSyncState({
        agentId: config.identity.agentId,
        tenantId: config.identity.tenantId,
        storeId: config.identity.storeId,
      });

      // Test connection
      spinner.text = 'Testing sequencer connection...';
      try {
        const client = createSequencerClient(new SyncConfig(config));
        await client.connect();
        spinner.succeed('Sync initialized successfully');
      } catch (error) {
        spinner.warn(`Sync initialized (sequencer offline: ${error.message})`);
      }

      db.close();

      // Output summary
      console.log();
      console.log(chalk.green('Configuration saved to .stateset/sync.json'));
      console.log();
      console.log(chalk.bold('Sync Settings:'));
      console.log(`  Sequencer URL: ${config.sequencer.url}`);
      console.log(`  Tenant ID:     ${config.identity.tenantId}`);
      console.log(`  Store ID:      ${config.identity.storeId}`);
      console.log(`  Agent ID:      ${config.identity.agentId}`);
      console.log(`  Database:      ${options.db}`);
      console.log();
      console.log(chalk.dim('Run "stateset-sync status" to check sync state.'));
    } catch (error) {
      spinner.fail(`Initialization failed: ${error.message}`);
      process.exit(1);
    }
  });

// ============================================================================
// push command
// ============================================================================
program
  .command('push')
  .description('Push pending local events to sequencer')
  .option('--batch-size <n>', 'Max events per batch', '100')
  .option('--dry-run', 'Show what would be pushed without pushing')
  .option('--db <path>', 'Database path', './store.db')
  .option('--verbose', 'Show detailed progress')
  .action(async (options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const db = new Database(options.db || config.local.dbPath);
    const spinner = ora('Pushing events...').start();

    try {
      const engine = createSyncEngine({ db, config });
      await engine.initialize();

      // Get pending count first
      const outbox = createOutbox(db);
      const pendingCount = outbox.getPendingCount();

      if (pendingCount === 0) {
        spinner.info('No pending events to push');
        db.close();
        return;
      }

      spinner.text = `Pushing ${pendingCount} events...`;

      const result = await engine.push({
        batchSize: parseInt(options.batchSize, 10),
        dryRun: options.dryRun,
      });

      if (result.success) {
        if (options.dryRun) {
          spinner.info(`Would push ${result.pushed} events (dry run)`);
        } else {
          spinner.succeed(`Push complete: ${result.pushed} accepted, ${result.rejected} rejected`);

          if (result.receipt && options.verbose) {
            console.log();
            console.log(chalk.bold('Receipt:'));
            console.log(`  Batch ID:  ${result.receipt.batchId}`);
            console.log(`  Sequence:  ${result.receipt.sequenceStart} - ${result.receipt.sequenceEnd}`);
          }
        }
      } else {
        spinner.fail(`Push failed: ${result.error}`);
      }

      await engine.shutdown();
      db.close();
    } catch (error) {
      spinner.fail(`Push failed: ${error.message}`);
      db.close();
      process.exit(1);
    }
  });

// ============================================================================
// pull command
// ============================================================================
program
  .command('pull')
  .description('Pull remote events and apply locally')
  .option('--from <seq>', 'Start from sequence number')
  .option('--limit <n>', 'Max events to pull', '1000')
  .option('--dry-run', 'Show what would be applied without applying')
  .option('--db <path>', 'Database path', './store.db')
  .option('--verbose', 'Show detailed progress')
  .action(async (options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const db = new Database(options.db || config.local.dbPath);
    const spinner = ora('Pulling events...').start();

    try {
      const engine = createSyncEngine({ db, config });
      await engine.initialize();

      // Get current state
      const outbox = createOutbox(db);
      const state = outbox.getSyncState();
      const fromSeq = options.from ? parseInt(options.from, 10) : state.lastPulledSequence;

      spinner.text = `Pulling events from sequence ${fromSeq}...`;

      const result = await engine.pull({
        fromSequence: fromSeq,
        limit: parseInt(options.limit, 10),
        dryRun: options.dryRun,
      });

      if (result.success) {
        if (options.dryRun) {
          spinner.info(`Would pull ${result.pulled} events (dry run)`);
        } else if (result.pulled === 0) {
          spinner.info('Already up to date');
        } else {
          spinner.succeed(`Pull complete: ${result.pulled} events pulled`);
        }
      } else {
        spinner.fail(`Pull failed: ${result.error}`);
      }

      await engine.shutdown();
      db.close();
    } catch (error) {
      spinner.fail(`Pull failed: ${error.message}`);
      db.close();
      process.exit(1);
    }
  });

// ============================================================================
// status command
// ============================================================================
program
  .command('status')
  .description('Show sync status')
  .option('--json', 'Output as JSON')
  .option('--db <path>', 'Database path', './store.db')
  .option('--verbose', 'Show detailed stats')
  .action(async (options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const db = new Database(options.db || config.local.dbPath);

    try {
      const engine = createSyncEngine({ db, config });
      await engine.initialize();

      const status = await engine.getStatus();
      const outbox = createOutbox(db);
      const stats = outbox.getStats();

      if (options.json) {
        console.log(JSON.stringify({ status, stats }, null, 2));
      } else {
        console.log();
        console.log(chalk.bold('Sync Status'));
        console.log();

        // Connection
        const connIcon = status.connected ? chalk.green('✓') : chalk.red('✗');
        console.log(`  Connection:     ${connIcon} ${status.connected ? 'Connected' : 'Disconnected'}`);
        console.log(`  Sequencer:      ${config.sequencer.url}`);
        console.log();

        // Local state
        console.log(chalk.bold('  Local State:'));
        console.log(`    Database:     ${options.db || config.local.dbPath}`);
        console.log(`    Outbox:       ${stats.pending} pending, ${stats.synced} synced, ${stats.failed} failed`);
        console.log(`    Local head:   ${status.localHead}`);
        console.log();

        // Remote state
        console.log(chalk.bold('  Remote State:'));
        console.log(`    Remote head:  ${status.remoteHead}`);
        console.log();

        // Sync gap
        const lagColor = status.lag > 100 ? chalk.red : status.lag > 10 ? chalk.yellow : chalk.green;
        console.log(`  Sync Gap:       ${lagColor(status.lag + ' events')}`);
        console.log();

        // Health indicator
        if (status.lag > 100) {
          console.log(chalk.yellow('  ⚠ Significant sync lag detected'));
        } else if (status.pending > 100) {
          console.log(chalk.yellow('  ⚠ Many pending events to push'));
        } else if (!status.connected) {
          console.log(chalk.yellow('  ⚠ Cannot reach sequencer'));
        } else {
          console.log(chalk.green('  ✓ Sync healthy'));
        }

        if (options.verbose) {
          console.log();
          console.log(chalk.bold('  Detailed Stats:'));
          console.log(`    Total events:    ${stats.total}`);
          console.log(`    Rejected:        ${stats.rejected}`);
          if (stats.oldestPending) {
            console.log(`    Oldest pending:  ${stats.oldestPending.toISOString()}`);
          }
          if (stats.lastSynced) {
            console.log(`    Last synced:     ${stats.lastSynced.toISOString()}`);
          }
        }
      }

      await engine.shutdown();
      db.close();
    } catch (error) {
      console.error(chalk.red(`Status check failed: ${error.message}`));
      db.close();
      process.exit(1);
    }
  });

// ============================================================================
// verify command
// ============================================================================
program
  .command('verify <event-id>')
  .description('Verify event inclusion in commitment')
  .option('--batch-id <id>', 'Verify against specific batch')
  .option('--db <path>', 'Database path', './store.db')
  .option('--verbose', 'Show proof details')
  .action(async (eventId, options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const spinner = ora('Verifying event...').start();

    try {
      const client = createSequencerClient(new SyncConfig(config));
      await client.connect();

      // Get entity history to find the event
      spinner.text = 'Looking up event...';

      // For now, we just verify the event exists
      // Full proof verification requires commitment data
      spinner.info(`Event verification for ${eventId}`);
      console.log();
      console.log(chalk.dim('Note: Full Merkle proof verification will be available in Phase 1'));
      console.log(chalk.dim('when on-chain anchoring is implemented.'));
    } catch (error) {
      spinner.fail(`Verification failed: ${error.message}`);
      process.exit(1);
    }
  });

// ============================================================================
// conflicts command
// ============================================================================
program
  .command('conflicts')
  .description('List unresolved sync conflicts')
  .option('--json', 'Output as JSON')
  .option('--db <path>', 'Database path', './store.db')
  .action(async (options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const db = new Database(options.db || config.local.dbPath);

    try {
      const engine = createSyncEngine({ db, config });
      await engine.initialize();

      const conflicts = await engine.getConflicts();

      if (options.json) {
        console.log(JSON.stringify(conflicts, null, 2));
        await engine.shutdown();
        db.close();
        return;
      }

      if (conflicts.length === 0) {
        console.log(chalk.green('No unresolved conflicts'));
        await engine.shutdown();
        db.close();
        return;
      }

      console.log();
      console.log(chalk.bold(`Unresolved Conflicts (${conflicts.length})`));
      console.log();

      for (const conflict of conflicts) {
        const typeIcon = {
          version: chalk.yellow('V'),
          concurrent: chalk.magenta('C'),
          invariant: chalk.red('I'),
        }[conflict.type] || '?';

        console.log(`${typeIcon} ${chalk.bold(conflict.id.substring(0, 8))}  ${conflict.entityType}/${conflict.entityId}`);
        console.log(`    ${chalk.dim('Type:')} ${conflict.type}`);
        console.log(`    ${chalk.dim('Description:')} ${conflict.description}`);
        console.log(`    ${chalk.dim('Suggested:')} ${chalk.cyan(conflict.suggestedStrategy)}`);
        console.log(`    ${chalk.dim('Detected:')} ${conflict.detectedAt.toISOString()}`);
        if (conflict.localEvent) {
          console.log(`    ${chalk.dim('Local Event:')} ${conflict.localEvent.eventType} (seq ${conflict.localEvent.localSeq})`);
        }
        console.log();
      }

      console.log(chalk.dim('Use "stateset-sync resolve <id> --strategy <strategy>" to resolve'));
      console.log(chalk.dim('Or "stateset-sync rebase --strategy <strategy>" to resolve all'));

      await engine.shutdown();
      db.close();
    } catch (error) {
      console.error(chalk.red(`Failed to list conflicts: ${error.message}`));
      db.close();
      process.exit(1);
    }
  });

// ============================================================================
// resolve command
// ============================================================================
program
  .command('resolve <conflict-id>')
  .description('Resolve a specific conflict')
  .option('--strategy <strategy>', 'Resolution strategy (remote-wins, local-wins, merge)', 'remote-wins')
  .option('--skip', 'Skip this conflict without resolving')
  .option('--db <path>', 'Database path', './store.db')
  .option('--verbose', 'Show resolution details')
  .action(async (conflictId, options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const db = new Database(options.db || config.local.dbPath);
    const spinner = ora('Resolving conflict...').start();

    try {
      const engine = createSyncEngine({ db, config });
      await engine.initialize();

      if (options.skip) {
        engine.skipConflict(conflictId, 'Skipped via CLI');
        spinner.succeed(`Conflict ${conflictId.substring(0, 8)} skipped`);
        await engine.shutdown();
        db.close();
        return;
      }

      const validStrategies = ['remote-wins', 'local-wins', 'merge'];
      if (!validStrategies.includes(options.strategy)) {
        spinner.fail(`Invalid strategy: ${options.strategy}`);
        console.log(chalk.dim(`Valid strategies: ${validStrategies.join(', ')}`));
        await engine.shutdown();
        db.close();
        process.exit(1);
      }

      const result = await engine.resolveConflict(conflictId, options.strategy);

      if (result.success) {
        spinner.succeed(`Conflict resolved using ${chalk.cyan(options.strategy)} strategy`);
        if (options.verbose && result.result) {
          console.log();
          console.log(chalk.bold('Resolution Details:'));
          console.log(`  Action: ${result.result.action}`);
          if (result.result.newEventId) {
            console.log(`  New Event ID: ${result.result.newEventId}`);
          }
          if (result.result.newSeq) {
            console.log(`  New Sequence: ${result.result.newSeq}`);
          }
        }
      } else {
        spinner.fail(`Resolution failed: ${result.error}`);
      }

      await engine.shutdown();
      db.close();
    } catch (error) {
      spinner.fail(`Resolution failed: ${error.message}`);
      db.close();
      process.exit(1);
    }
  });

// ============================================================================
// rebase command
// ============================================================================
program
  .command('rebase')
  .description('Resolve all conflicts with a strategy')
  .option('--strategy <strategy>', 'Resolution strategy (remote-wins, local-wins, merge)', 'remote-wins')
  .option('--force', 'Alias for --strategy=remote-wins')
  .option('--dry-run', 'Show what would happen without applying')
  .option('--db <path>', 'Database path', './store.db')
  .option('--verbose', 'Show detailed rebase steps')
  .action(async (options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const db = new Database(options.db || config.local.dbPath);
    const spinner = ora('Checking for conflicts...').start();

    try {
      const engine = createSyncEngine({ db, config });
      await engine.initialize();

      const conflicts = await engine.getConflicts();

      if (conflicts.length === 0) {
        spinner.info('No conflicts to resolve');
        await engine.shutdown();
        db.close();
        return;
      }

      const strategy = options.force ? 'remote-wins' : options.strategy;
      spinner.text = `Rebasing ${conflicts.length} conflicts with ${strategy} strategy...`;

      if (options.dryRun) {
        spinner.info(`Would resolve ${conflicts.length} conflicts using ${chalk.cyan(strategy)} strategy (dry run)`);
        console.log();
        for (const conflict of conflicts) {
          console.log(`  - ${conflict.id.substring(0, 8)}: ${conflict.entityType}/${conflict.entityId} (${conflict.type})`);
        }
        await engine.shutdown();
        db.close();
        return;
      }

      const result = await engine.rebase({ strategy });

      if (result.success) {
        spinner.succeed(`Rebase complete: ${result.rebased} conflicts resolved`);
      } else {
        spinner.warn(`Rebase partially complete: ${result.rebased} resolved, ${result.failed} failed`);
        if (options.verbose && result.errors.length > 0) {
          console.log();
          console.log(chalk.bold('Errors:'));
          for (const err of result.errors) {
            console.log(`  - ${err.conflictId.substring(0, 8)}: ${err.error}`);
          }
        }
      }

      await engine.shutdown();
      db.close();
    } catch (error) {
      spinner.fail(`Rebase failed: ${error.message}`);
      db.close();
      process.exit(1);
    }
  });

// ============================================================================
// history command
// ============================================================================
program
  .command('history')
  .description('Show sync history')
  .option('--limit <n>', 'Number of events to show', '20')
  .option('--db <path>', 'Database path', './store.db')
  .action(async (options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const db = new Database(options.db || config.local.dbPath);

    try {
      const outbox = createOutbox(db);
      outbox.initialize();

      const stmt = db.prepare(`
        SELECT * FROM _ves_outbox
        ORDER BY local_seq DESC
        LIMIT ?
      `);

      const events = stmt.all(parseInt(options.limit, 10));

      if (events.length === 0) {
        console.log(chalk.dim('No events in outbox'));
        db.close();
        return;
      }

      console.log();
      console.log(chalk.bold('Recent Outbox Events'));
      console.log();

      for (const event of events) {
        const statusIcon = {
          pending: chalk.yellow('○'),
          synced: chalk.green('✓'),
          failed: chalk.red('✗'),
          rejected: chalk.red('⊘'),
        }[event.sync_status] || '?';

        console.log(`${statusIcon} ${chalk.dim(event.local_seq.toString().padStart(6))} ${event.event_type}`);
        console.log(`    ${chalk.dim('Entity:')} ${event.entity_type}/${event.entity_id}`);
        console.log(`    ${chalk.dim('ID:')} ${event.event_id.substring(0, 8)}...`);
        if (event.remote_sequence) {
          console.log(`    ${chalk.dim('Remote Seq:')} ${event.remote_sequence}`);
        }
        if (event.rejection_reason) {
          console.log(`    ${chalk.red('Rejection:')} ${event.rejection_reason}`);
        }
        console.log();
      }

      db.close();
    } catch (error) {
      console.error(chalk.red(`Failed to get history: ${error.message}`));
      db.close();
      process.exit(1);
    }
  });

// ============================================================================
// keys:generate command
// ============================================================================
program
  .command('keys:generate')
  .description('Generate new signing and encryption keys (VES v1.0)')
  .option('--agent-id <uuid>', 'Agent UUID (uses configured agent if not specified)')
  .option('--signing-only', 'Generate only signing key')
  .option('--encryption-only', 'Generate only encryption key')
  .option('--json', 'Output as JSON')
  .action(async (options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const agentId = options.agentId || config.identity.agentId;
    if (!agentId) {
      console.error(chalk.red('No agent ID specified or configured.'));
      process.exit(1);
    }

    const spinner = ora('Generating keys...').start();

    try {
      const keyManager = getKeyManager(getConfigDir());
      const result = {};

      if (!options.encryptionOnly) {
        spinner.text = 'Generating Ed25519 signing key...';
        const signingKey = await keyManager.generateSigningKey(agentId);
        result.signingKey = {
          keyId: signingKey.keyId,
          publicKey: bufferToHex(signingKey.publicKey),
          createdAt: signingKey.createdAt,
        };
      }

      if (!options.signingOnly) {
        spinner.text = 'Generating X25519 encryption key...';
        const encryptionKey = await keyManager.generateEncryptionKey(agentId);
        result.encryptionKey = {
          keyId: encryptionKey.keyId,
          publicKey: bufferToHex(encryptionKey.publicKey),
          createdAt: encryptionKey.createdAt,
        };
      }

      spinner.succeed('Keys generated successfully');

      if (options.json) {
        console.log(JSON.stringify(result, null, 2));
      } else {
        console.log();
        console.log(chalk.bold('Generated Keys:'));
        console.log(`  Agent ID: ${agentId}`);
        console.log();

        if (result.signingKey) {
          console.log(chalk.cyan('  Signing Key (Ed25519):'));
          console.log(`    Key ID:     ${result.signingKey.keyId}`);
          console.log(`    Public Key: ${result.signingKey.publicKey}`);
          console.log();
        }

        if (result.encryptionKey) {
          console.log(chalk.cyan('  Encryption Key (X25519):'));
          console.log(`    Key ID:     ${result.encryptionKey.keyId}`);
          console.log(`    Public Key: ${result.encryptionKey.publicKey}`);
          console.log();
        }

        console.log(chalk.dim('Run "stateset-sync keys:register" to register with sequencer.'));
      }
    } catch (error) {
      spinner.fail(`Key generation failed: ${error.message}`);
      process.exit(1);
    }
  });

// ============================================================================
// keys:list command
// ============================================================================
program
  .command('keys:list')
  .description('List keys for an agent')
  .option('--agent-id <uuid>', 'Agent UUID (uses configured agent if not specified)')
  .option('--json', 'Output as JSON')
  .option('--include-revoked', 'Include revoked keys')
  .action(async (options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const agentId = options.agentId || config.identity.agentId;
    if (!agentId) {
      console.error(chalk.red('No agent ID specified or configured.'));
      process.exit(1);
    }

    try {
      const keyManager = getKeyManager(getConfigDir());

      const signingKeys = await keyManager.listSigningKeys(agentId);
      const encryptionKeys = await keyManager.listEncryptionKeys(agentId);

      const formatKeys = (keys) => keys
        .filter(k => options.includeRevoked || !k.revokedAt)
        .map(k => ({
          keyId: k.keyId,
          publicKey: bufferToHex(k.publicKey),
          createdAt: k.createdAt,
          revokedAt: k.revokedAt || null,
        }));

      const result = {
        agentId,
        signingKeys: formatKeys(signingKeys),
        encryptionKeys: formatKeys(encryptionKeys),
      };

      if (options.json) {
        console.log(JSON.stringify(result, null, 2));
        return;
      }

      console.log();
      console.log(chalk.bold('Agent Keys'));
      console.log(`  Agent ID: ${agentId}`);
      console.log();

      console.log(chalk.cyan('  Signing Keys (Ed25519):'));
      if (result.signingKeys.length === 0) {
        console.log(chalk.dim('    No keys found'));
      } else {
        for (const key of result.signingKeys) {
          const statusIcon = key.revokedAt ? chalk.red('✗') : chalk.green('✓');
          console.log(`    ${statusIcon} Key ${key.keyId}: ${key.publicKey.substring(0, 16)}...`);
          if (key.revokedAt) {
            console.log(`        ${chalk.red('Revoked:')} ${key.revokedAt}`);
          }
        }
      }
      console.log();

      console.log(chalk.cyan('  Encryption Keys (X25519):'));
      if (result.encryptionKeys.length === 0) {
        console.log(chalk.dim('    No keys found'));
      } else {
        for (const key of result.encryptionKeys) {
          const statusIcon = key.revokedAt ? chalk.red('✗') : chalk.green('✓');
          console.log(`    ${statusIcon} Key ${key.keyId}: ${key.publicKey.substring(0, 16)}...`);
          if (key.revokedAt) {
            console.log(`        ${chalk.red('Revoked:')} ${key.revokedAt}`);
          }
        }
      }
    } catch (error) {
      console.error(chalk.red(`Failed to list keys: ${error.message}`));
      process.exit(1);
    }
  });

// ============================================================================
// keys:register command
// ============================================================================
program
  .command('keys:register')
  .description('Register signing public key with sequencer')
  .option('--agent-id <uuid>', 'Agent UUID (uses configured agent if not specified)')
  .option('--key-id <n>', 'Key ID to register (uses latest if not specified)')
  .option('--valid-from <iso>', 'Validity start timestamp')
  .option('--valid-to <iso>', 'Validity end timestamp')
  .action(async (options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const agentId = options.agentId || config.identity.agentId;
    if (!agentId) {
      console.error(chalk.red('No agent ID specified or configured.'));
      process.exit(1);
    }

    const spinner = ora('Registering key...').start();

    try {
      const keyManager = getKeyManager(getConfigDir());

      // Get the key to register
      let signingKey;
      if (options.keyId) {
        signingKey = await keyManager.getSigningKey(agentId, parseInt(options.keyId, 10));
        if (!signingKey) {
          spinner.fail(`Signing key ${options.keyId} not found`);
          process.exit(1);
        }
      } else {
        signingKey = await keyManager.getCurrentSigningKey(agentId);
        if (!signingKey) {
          spinner.fail('No signing key found. Run "stateset-sync keys:generate" first.');
          process.exit(1);
        }
      }

      // Connect to sequencer and register
      spinner.text = 'Connecting to sequencer...';
      const client = createSequencerClient(new SyncConfig(config));
      await client.connect();

      spinner.text = 'Registering public key...';
      const result = await client.registerAgentKey({
        agentId,
        keyId: signingKey.keyId,
        publicKey: bufferToHex(signingKey.publicKey),
        validFrom: options.validFrom,
        validTo: options.validTo,
      });

      if (result.success) {
        spinner.succeed(`Key ${signingKey.keyId} registered successfully`);
        console.log();
        console.log(chalk.bold('Registered Key:'));
        console.log(`  Agent ID:   ${agentId}`);
        console.log(`  Key ID:     ${signingKey.keyId}`);
        console.log(`  Public Key: ${bufferToHex(signingKey.publicKey)}`);
        if (options.validFrom) {
          console.log(`  Valid From: ${options.validFrom}`);
        }
        if (options.validTo) {
          console.log(`  Valid To:   ${options.validTo}`);
        }
      } else {
        spinner.fail('Key registration failed');
        process.exit(1);
      }
    } catch (error) {
      spinner.fail(`Key registration failed: ${error.message}`);
      process.exit(1);
    }
  });

// ============================================================================
// keys:rotate command
// ============================================================================
program
  .command('keys:rotate')
  .description('Rotate keys (generate new, revoke old)')
  .option('--agent-id <uuid>', 'Agent UUID (uses configured agent if not specified)')
  .option('--signing', 'Rotate signing key')
  .option('--encryption', 'Rotate encryption key')
  .option('--all', 'Rotate both key types')
  .option('--register', 'Auto-register new signing key with sequencer')
  .action(async (options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const agentId = options.agentId || config.identity.agentId;
    if (!agentId) {
      console.error(chalk.red('No agent ID specified or configured.'));
      process.exit(1);
    }

    if (!options.signing && !options.encryption && !options.all) {
      console.error(chalk.red('Specify --signing, --encryption, or --all'));
      process.exit(1);
    }

    const spinner = ora('Rotating keys...').start();

    try {
      const keyManager = getKeyManager(getConfigDir());
      const rotateSigning = options.signing || options.all;
      const rotateEncryption = options.encryption || options.all;

      const result = {
        signingKey: null,
        encryptionKey: null,
        revokedSigningKey: null,
        revokedEncryptionKey: null,
      };

      if (rotateSigning) {
        // Get current key to revoke
        const currentSigning = await keyManager.getCurrentSigningKey(agentId);
        if (currentSigning) {
          spinner.text = 'Revoking old signing key...';
          await keyManager.revokeSigningKey(agentId, currentSigning.keyId);
          result.revokedSigningKey = currentSigning.keyId;
        }

        // Generate new key
        spinner.text = 'Generating new signing key...';
        const newSigning = await keyManager.generateSigningKey(agentId);
        result.signingKey = {
          keyId: newSigning.keyId,
          publicKey: bufferToHex(newSigning.publicKey),
        };

        // Auto-register if requested
        if (options.register) {
          spinner.text = 'Registering new signing key...';
          const client = createSequencerClient(new SyncConfig(config));
          await client.connect();
          await client.registerAgentKey({
            agentId,
            keyId: newSigning.keyId,
            publicKey: bufferToHex(newSigning.publicKey),
          });
        }
      }

      if (rotateEncryption) {
        // Get current key to revoke
        const currentEncryption = await keyManager.getCurrentEncryptionKey(agentId);
        if (currentEncryption) {
          spinner.text = 'Revoking old encryption key...';
          await keyManager.revokeEncryptionKey(agentId, currentEncryption.keyId);
          result.revokedEncryptionKey = currentEncryption.keyId;
        }

        // Generate new key
        spinner.text = 'Generating new encryption key...';
        const newEncryption = await keyManager.generateEncryptionKey(agentId);
        result.encryptionKey = {
          keyId: newEncryption.keyId,
          publicKey: bufferToHex(newEncryption.publicKey),
        };
      }

      spinner.succeed('Key rotation complete');
      console.log();
      console.log(chalk.bold('Rotation Summary:'));
      console.log(`  Agent ID: ${agentId}`);
      console.log();

      if (result.signingKey) {
        console.log(chalk.cyan('  Signing Key:'));
        console.log(`    New Key ID:     ${result.signingKey.keyId}`);
        console.log(`    New Public Key: ${result.signingKey.publicKey.substring(0, 32)}...`);
        if (result.revokedSigningKey) {
          console.log(chalk.dim(`    Revoked Key:    ${result.revokedSigningKey}`));
        }
        if (options.register) {
          console.log(chalk.green('    Registered:     ✓'));
        }
        console.log();
      }

      if (result.encryptionKey) {
        console.log(chalk.cyan('  Encryption Key:'));
        console.log(`    New Key ID:     ${result.encryptionKey.keyId}`);
        console.log(`    New Public Key: ${result.encryptionKey.publicKey.substring(0, 32)}...`);
        if (result.revokedEncryptionKey) {
          console.log(chalk.dim(`    Revoked Key:    ${result.revokedEncryptionKey}`));
        }
      }
    } catch (error) {
      spinner.fail(`Key rotation failed: ${error.message}`);
      process.exit(1);
    }
  });

// ============================================================================
// keys:export command
// ============================================================================
program
  .command('keys:export')
  .description('Export public keys for sharing')
  .option('--agent-id <uuid>', 'Agent UUID (uses configured agent if not specified)')
  .option('--format <format>', 'Output format (json, hex)', 'json')
  .action(async (options) => {
    const config = loadSyncConfig();
    if (!config) {
      console.error(chalk.red('Sync not configured. Run "stateset-sync init" first.'));
      process.exit(1);
    }

    const agentId = options.agentId || config.identity.agentId;
    if (!agentId) {
      console.error(chalk.red('No agent ID specified or configured.'));
      process.exit(1);
    }

    try {
      const keyManager = getKeyManager(getConfigDir());

      const signingKey = await keyManager.getCurrentSigningKey(agentId);
      const encryptionKey = await keyManager.getCurrentEncryptionKey(agentId);

      const exported = {
        agentId,
        tenantId: config.identity.tenantId,
        storeId: config.identity.storeId,
        signingKey: signingKey ? {
          keyId: signingKey.keyId,
          algorithm: 'Ed25519',
          publicKey: bufferToHex(signingKey.publicKey),
          createdAt: signingKey.createdAt,
        } : null,
        encryptionKey: encryptionKey ? {
          keyId: encryptionKey.keyId,
          algorithm: 'X25519',
          publicKey: bufferToHex(encryptionKey.publicKey),
          createdAt: encryptionKey.createdAt,
        } : null,
        exportedAt: new Date().toISOString(),
      };

      if (options.format === 'json') {
        console.log(JSON.stringify(exported, null, 2));
      } else if (options.format === 'hex') {
        console.log('# Agent Public Keys');
        console.log(`agent_id=${agentId}`);
        if (signingKey) {
          console.log(`signing_key_id=${signingKey.keyId}`);
          console.log(`signing_public_key=${bufferToHex(signingKey.publicKey)}`);
        }
        if (encryptionKey) {
          console.log(`encryption_key_id=${encryptionKey.keyId}`);
          console.log(`encryption_public_key=${bufferToHex(encryptionKey.publicKey)}`);
        }
      } else {
        console.error(chalk.red(`Unknown format: ${options.format}`));
        process.exit(1);
      }
    } catch (error) {
      console.error(chalk.red(`Export failed: ${error.message}`));
      process.exit(1);
    }
  });

// Parse and run
program.parse();
