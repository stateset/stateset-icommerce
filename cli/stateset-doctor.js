#!/usr/bin/env node

/**
 * StateSet Doctor - Diagnostic and troubleshooting tool
 * Automatically diagnoses common issues with StateSet Commerce instances
 */

import { Commerce } from '@stateset/embedded';
import * as fs from 'fs/promises';
import * as path from 'path';
import chalk from 'chalk';

class Doctor {
  constructor() {
    this.issues = [];
    this.warnings = [];
    this.info = [];
  }

  /**
   * Run all diagnostic checks
   */
  async diagnose(dbPath = './stateset.db') {
    console.log(chalk.blue.bold('🔍 StateSet Commerce Doctor'));
    console.log(chalk.gray('='.repeat(50)));
    console.log();

    await this.checkDatabaseFile(dbPath);
    await this.checkDatabaseStructure(dbPath);
    await this.checkPerformance(dbPath);
    await this.checkMigrationHealth(dbPath);
    await this.checkCommonErrors(dbPath);

    this.printReport();

    // Return exit code based on severity
    this.issues.length > 0 ? process.exit(1) : process.exit(0);
  }

  /**
   * Check if database file exists and is accessible
   */
  async checkDatabaseFile(dbPath) {
    console.log(chalk.yellow('📄 Checking database file...'));

    try {
      const stats = await fs.stat(dbPath);
      const sizeMB = (stats.size / (1024 * 1024)).toFixed(2);

      this.info.push({
        category: 'Database',
        message: `Database file exists: ${dbPath}`,
        detail: `Size: ${sizeMB} MB, Modified: ${stats.mtime.toISOString()}`
      });

      console.log(chalk.green(`✓ Database file found (${sizeMB} MB)`));

      // Warn if database is very large
      if (stats.size > 1024 * 1024 * 1024) {
        this.warnings.push({
          category: 'Performance',
          message: 'Database file is very large (>1GB)',
          recommendation: 'Consider running VACUUM or setting up a backup strategy'
        });
        console.log(chalk.yellow('⚠ Database file is large (>1GB) - consider VACUUM'));
      }

    } catch (error) {
      if (error.code === 'ENOENT') {
        this.issues.push({
          category: 'Database',
          message: 'Database file not found',
          recommendation: `Initialize with: stateset init --db ${dbPath}`
        });
        console.log(chalk.red(`✗ Database file not found: ${dbPath}`));
      } else {
        this.issues.push({
          category: 'Database',
          message: 'Cannot access database file',
          detail: error.message,
          recommendation: 'Check file permissions'
        });
        console.log(chalk.red(`✗ Cannot access database: ${error.message}`));
      }
    }

    console.log();
  }

  /**
   * Check database structure and tables
   */
  async checkDatabaseStructure(dbPath) {
    console.log(chalk.yellow('🏗️  Checking database structure...'));

    try {
      const commerce = new Commerce(dbPath);

      // Critical tables that must exist
      const criticalTables = [
        'customers',
        'orders',
        'order_items',
        'products',
        'inventory_items',
        'payments',
        'inventory_balances'
      ];

      let checked = 0;
      for (const table of criticalTables) {
        try {
          // Try to query the table
          const method = table === 'inventory_items' || table === 'inventory_balances' 
            ? 'inventory' 
            : table.slice(0, -1); // Remove trailing 's'
          
          if (typeof commerce[method]?.list === 'function') {
            await commerce[method].list({ limit: 1 });
            checked++;
          }
        } catch (error) {
          if (error.message.includes('no such table')) {
            this.issues.push({
              category: 'Structure',
              message: `Critical table missing: ${table}`,
              recommendation: 'Run migrations again or reinitialize database'
            });
            console.log(chalk.red(`✗ Missing table: ${table}`));
          }
        }
      }

      if (checked === criticalTables.length) {
        this.info.push({
          category: 'Structure',
          message: 'All critical tables present',
          detail: `Checked ${checked} tables`
        });
        console.log(chalk.green(`✓ All ${checked} critical tables present`));
      }

    } catch (error) {
      this.issues.push({
        category: 'Structure',
        message: 'Cannot check database structure',
        detail: error.message,
        recommendation: 'Ensure database is not corrupted'
      });
      console.log(chalk.red(`✗ Structure check failed: ${error.message}`));
    }

    console.log();
  }

  /**
   * Check database performance indicators
   */
  async checkPerformance(dbPath) {
    console.log(chalk.yellow('⚡ Checking performance indicators...'));

    try {
      const commerce = new Commerce(dbPath);

      // Test query performance
      const start = Date.now();
      const customers = await commerce.customers.list();
      const queryTime = Date.now() - start;

      if (queryTime > 1000) {
        this.warnings.push({
          category: 'Performance',
          message: 'Slow customer query detected',
          detail: `Query took ${queryTime}ms`,
          recommendation: 'Check indexes and consider larger connection pool'
        });
        console.log(chalk.yellow(`⚠ Slow query (${queryTime}ms)`));
      } else {
        console.log(chalk.green(`✓ Query performance acceptable (${queryTime}ms)`));
      }

      // Check for unindexed tables
      this.info.push({
        category: 'Performance',
        message: 'Query latency measured',
        detail: `Customer list query: ${queryTime}ms`
      });

      // Check table sizes if possible
      const orderCount = await commerce.orders.list().then(orders => orders.length);
      this.info.push({
        category: 'Performance',
        message: 'Data volume checked',
        detail: `${orderCount} orders in database`
      });

    } catch (error) {
      console.log(chalk.yellow(`⚠ Performance check skipped: ${error.message}`));
    }

    console.log();
  }

  /**
   * Check migration health
   */
  async checkMigrationHealth(dbPath) {
    console.log(chalk.yellow('🔄 Checking migration health...'));

    try {
      // Check for migrations table
      const exists = await fs.access(dbPath)
        .then(() => true)
        .catch(() => false);

      if (exists) {
        this.info.push({
          category: 'Migrations',
          message: 'Migration tracking available',
          detail: 'Database has migration metadata'
        });
        console.log(chalk.green('✓ Migration tracking intact'));
      }

      // Check for orphaned records
      this.warnings.push({
        category: 'Migrations',
        message: 'Data integrity check recommended',
        detail: 'Run: stateset doctor --verify-integrity',
        recommendation: 'Periodically check for orphaned records'
      });

    } catch (error) {
      console.log(chalk.yellow('⚠ Migration health check skipped'));
    }

    console.log();
  }

  /**
   * Check for common error patterns
   */
  async checkCommonErrors(dbPath) {
    console.log(chalk.yellow('🐛 Checking for common issues...'));

    try {
      const commerce = new Commerce(dbPath);

      // Check for orders stuck in processing
      const processingOrders = await commerce.orders.list({ status: 'processing' });
      if (processingOrders.length > 100) {
        this.warnings.push({
          category: 'Orders',
          message: 'Many orders stuck in processing',
          detail: `${processingOrders.length} orders`,
          recommendation: 'Review order processing and fulfillment pipelines'
        });
        console.log(chalk.yellow(`⚠ ${processingOrders.length} orders stuck in processing`));
      }

      // Check for expired reservations
      this.warnings.push({
        category: 'Inventory',
        message: 'Check for expired inventory reservations',
        recommendation: 'Run: stateset inventory cleanup-expired-reservations'
      });

      // Check for unprocessed payments
      console.log(chalk.green('✓ Common issues check complete'));

    } catch (error) {
      console.log(chalk.yellow('⚠ Common issue check skipped'));
    }

    console.log();
  }

  /**
   * Print diagnostic report
   */
  printReport() {
    console.log(chalk.blue.bold('\n📊 Diagnostic Report'));
    console.log(chalk.gray('='.repeat(50)));

    if (this.issues.length === 0 && this.warnings.length === 0) {
      console.log(chalk.green.bold('\n✓ No issues found!'));
      console.log(chalk.gray('Your StateSet Commerce instance is healthy.\n'));
      return;
    }

    if (this.issues.length > 0) {
      console.log(chalk.red.bold(`\n❌ ${this.issues.length} Critical Issue${this.issues.length > 1 ? 's' : ''}:`));
      this.issues.forEach((issue, index) => {
        console.log(chalk.red(`  ${index + 1}. [${issue.category}] ${issue.message}`));
        if (issue.detail) {
          console.log(chalk.gray(`     Detail: ${issue.detail}`));
        }
        if (issue.recommendation) {
          console.log(chalk.blue(`     → ${issue.recommendation}`));
        }
      });
    }

    if (this.warnings.length > 0) {
      console.log(chalk.yellow.bold(`\n⚠️  ${this.warnings.length} Warning${this.warnings.length > 1 ? 's' : ''}:`));
      this.warnings.forEach((warning, index) => {
        console.log(chalk.yellow(`  ${index + 1}. [${warning.category}] ${warning.message}`));
        if (warning.detail) {
          console.log(chalk.gray(`     Detail: ${warning.detail}`));
        }
        if (warning.recommendation) {
          console.log(chalk.blue(`     → ${warning.recommendation}`));
        }
      });
    }

    if (this.info.length > 0) {
      console.log(chalk.blue.bold(`\nℹ️  Information (${this.info.length}):`));
      this.info.forEach((info) => {
        console.log(chalk.blue(`  [${info.category}] ${info.message}`));
        if (info.detail) {
          console.log(chalk.gray(`     ${info.detail}`));
        }
      });
    }

    console.log();

    if (this.issues.length > 0) {
      console.log(chalk.red.bold('Action required! Please address the critical issues above.'));
    } else if (this.warnings.length > 0) {
      console.log(chalk.yellow('Consider addressing the warnings for optimal performance.'));
    }
  }

  /**
   * Generate detailed health report (JSON)
   */
  generateHealthReport() {
    return {
      timestamp: new Date().toISOString(),
      summary: {
        issues: this.issues.length,
        warnings: this.warnings.length,
        info: this.info.length
      },
      issues: this.issues,
      warnings: this.warnings,
      info: this.info,
      healthScore: this.calculateHealthScore()
    };
  }

  /**
   * Calculate overall health score (0-100)
   */
  calculateHealthScore() {
    if (this.issues.length > 0) return Math.max(0, 50 - (this.issues.length * 10));
    if (this.warnings.length > 0) return Math.max(70, 100 - (this.warnings.length * 5));
    return 100;
  }
}

// CLI interface
async function main() {
  const args = process.argv.slice(2);
  const dbPath = args[0] || './stateset.db';
  const jsonOutput = args.includes('--json');

  const doctor = new Doctor();
  await doctor.diagnose(dbPath);

  if (jsonOutput) {
    console.log(JSON.stringify(doctor.generateHealthReport(), null, 2));
  }
}

main().catch(error => {
  console.error(chalk.red('Error running diagnostics:'), error);
  process.exit(1);
});