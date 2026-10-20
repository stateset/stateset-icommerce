import assert from 'node:assert/strict';
import { test } from 'node:test';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import Database from 'better-sqlite3';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { TerminalBackend } from '../../src/tui/backend.js';
import { shipmentKernel } from '../fixtures/tui-shipments.js';
import { hashFiles } from '../fixtures/sqlite-file-evidence.js';
const execute = promisify(execFile);
test(
  'native shipping consumes reserved stock for partial and final dispatch, refuses stale versions, replays once and recovers read-only',
  { timeout: 240000 },
  async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'tui-shipment-engine-')),
      database = path.join(directory, 'owned.db'),
      journal = path.join(directory, 'history'),
      kernel = shipmentKernel();
    let backend,
      writer,
      passed = false;
    try {
      const fixture = JSON.parse(
        (
          await execute(
            process.execPath,
            [
              '--input-type=module',
              '-e',
              `import {shipmentFixture} from ${JSON.stringify(new URL('../fixtures/tui-shipments.js', import.meta.url).href)};console.log(JSON.stringify(await shipmentFixture(process.argv[1])));`,
              database,
            ],
            { timeout: 45000 },
          )
        ).stdout,
      );
      writer = new Database(database);
      writer.pragma('journal_mode = WAL');
      writer.pragma('wal_autocheckpoint = 0');
      writer.prepare('UPDATE customers SET last_name=?').run('Committed WAL fixture');
      const files = [database, database + '-wal'],
        before = await hashFiles(files);
      const stock = () =>
        writer
          .prepare(
            'SELECT quantity_on_hand,quantity_allocated,quantity_available FROM inventory_balances',
          )
          .get();
      const originalStock = stock();
      backend = new TerminalBackend(database, {
        allowApply: true,
        loadKernel: () => kernel,
        operationsDirectory: journal,
      });
      const params = {
        orderId: fixture.order.id,
        trackingNumber: 'PARTIAL-ONE',
        lines: [{ orderItemId: fixture.order.items[0].id, quantity: 1 }],
      };
      const first = await backend.request('preview-shipment', params),
        competitor = await backend.request('preview-shipment', params);
      assert.equal(first.receipt.status, 'previewed', JSON.stringify(first));
      assert.equal(first.context.status, 'partially_shipped');
      assert.deepEqual(await hashFiles(files), before);
      assert.equal(writer.prepare('SELECT count(*) n FROM kernel_receipts').get().n, 0);
      const unpaid = await backend.request('preview-shipment', {
        ...params,
        orderId: fixture.orders[0].id,
        lines: [{ orderItemId: fixture.orders[0].items[0].id, quantity: 1 }],
      });
      assert.notEqual(unpaid.receipt.status, 'previewed');
      assert.equal(unpaid.applyAllowed, false);
      const expired = await backend.request('preview-shipment', params);
      const reservation = writer
        .prepare('SELECT id,quantity,expires_at FROM inventory_reservations WHERE order_item_id=?')
        .get(params.lines[0].orderItemId);
      writer
        .prepare("UPDATE inventory_reservations SET expires_at='2000-01-01T00:00:00Z' WHERE id=?")
        .run(reservation.id);
      const expirationRefusal = await backend.request('apply-shipment', {
        ticketId: expired.ticketId,
      });
      assert.equal(expirationRefusal.receipt.error_code, 'commerce.reservation_expired');
      assert.deepEqual(stock(), originalStock);
      writer
        .prepare('UPDATE inventory_reservations SET expires_at=? WHERE id=?')
        .run(reservation.expires_at, reservation.id);
      const invalid = await backend.request('preview-shipment', params);
      writer
        .prepare("UPDATE inventory_reservations SET quantity='0' WHERE id=?")
        .run(reservation.id);
      const inventoryRefusal = await backend.request('apply-shipment', {
        ticketId: invalid.ticketId,
      });
      assert.equal(inventoryRefusal.receipt.error_code, 'commerce.shipment_inventory_invalid');
      assert.deepEqual(stock(), originalStock);
      writer
        .prepare('UPDATE inventory_reservations SET quantity=? WHERE id=?')
        .run(reservation.quantity, reservation.id);
      const applied = await backend.request('apply-shipment', { ticketId: first.ticketId });
      assert.equal(applied.receipt.status, 'succeeded', JSON.stringify(applied));
      assert.deepEqual(applied.reviewIssues, []);
      assert.equal(applied.receipt.result.items[0].shipped_quantity, 1);
      assert.equal(Number(stock().quantity_on_hand), Number(originalStock.quantity_on_hand) - 1);
      assert.equal(
        Number(stock().quantity_allocated),
        Number(originalStock.quantity_allocated) - 1,
      );
      assert.equal(stock().quantity_available, originalStock.quantity_available);
      const stale = await backend.request('apply-shipment', { ticketId: competitor.ticketId });
      assert.equal(stale.receipt.error_code, 'kernel.version_conflict', JSON.stringify(stale));
      assert.equal(
        (await backend.request('apply-shipment', { ticketId: first.ticketId })).replayed,
        true,
      );
      const replay = JSON.parse(
        (
          await execute(
            process.execPath,
            [
              '--input-type=module',
              '-e',
              `import {applyShipment} from ${JSON.stringify(new URL('../../src/tui/shipment-engine.js', import.meta.url).href)};console.log(JSON.stringify(await applyShipment(process.argv[1],JSON.parse(process.argv[2]),JSON.parse(process.argv[3]))));`,
              database,
              JSON.stringify(kernel),
              JSON.stringify({
                params,
                expectedVersion: first.order.version,
                idempotencyKey: first.idempotencyKey,
              }),
            ],
            { timeout: 45000 },
          )
        ).stdout,
      );
      assert.equal(replay.receipt.command_id, applied.receipt.command_id);
      const final = await backend.request('preview-shipment', {
        ...params,
        trackingNumber: '',
      });
      assert.equal(final.receipt.status, 'previewed');
      assert.equal(final.context.status, 'shipped');
      assert.equal(final.context.trackingNumber, 'PARTIAL-ONE');
      // Exercise the real MCP dispatch too: the reviewed version is a tool
      // argument, while identity and policy stay in trusted host configuration.
      const agent = JSON.parse(
        (
          await execute(
            process.execPath,
            [
              '--input-type=module',
              '-e',
              `
      import {getCommerceCtor,adaptCommerceApis} from ${JSON.stringify(new URL('../../src/commerce.js', import.meta.url).href)};
      import {createStatesetMcpServer} from ${JSON.stringify(new URL('../../src/mcp-server.js', import.meta.url).href)};
      const commerce=adaptCommerceApis(await getCommerceCtor().open(process.argv[1],{maxConnections:1}));
      const server=createStatesetMcpServer({commerce,kernel:JSON.parse(process.argv[2]),allowApply:true,toolProfile:'core'});
      try { console.log(JSON.stringify(await server.executeTool('ship_reserved_order',JSON.parse(process.argv[3]),{commitment:{quantity:'1'}}))); }
      finally { await server.close(); await commerce.close(); }
    `,
              database,
              JSON.stringify(kernel),
              JSON.stringify({
                ...params,
                trackingNumber: '',
                expectedVersion: final.order.version,
                idempotencyKey: final.idempotencyKey,
              }),
            ],
            { timeout: 45000 },
          )
        ).stdout,
      );
      assert.equal(agent.result.receipt.status, 'succeeded', JSON.stringify(agent));
      const finished = await backend.request('apply-shipment', { ticketId: final.ticketId });
      assert.equal(finished.receipt.command_id, agent.result.receipt.command_id);
      assert.equal(finished.receipt.status, 'succeeded', JSON.stringify(finished));
      assert.deepEqual(finished.reviewIssues, []);
      assert.equal(finished.receipt.result.items[0].shipped_quantity, 2);
      assert.equal(Number(stock().quantity_on_hand), Number(originalStock.quantity_on_hand) - 2);
      assert.equal(
        Number(stock().quantity_allocated),
        Number(originalStock.quantity_allocated) - 2,
      );
      assert.equal(stock().quantity_available, originalStock.quantity_available);
      for (const receipt of [applied.receipt, finished.receipt]) {
        assert.equal(
          writer
            .prepare('SELECT count(*) n FROM kernel_receipt_audit_log WHERE command_id=?')
            .get(receipt.command_id).n,
          1,
        );
        assert.ok(receipt.event_ids.length > 0);
        for (const event of receipt.event_ids)
          assert.equal(
            writer.prepare('SELECT command_id FROM kernel_outbox WHERE id=?').get(event).command_id,
            receipt.command_id,
          );
      }
      const overship = await backend.request('preview-shipment', params);
      assert.notEqual(overship.receipt.status, 'previewed');
      assert.equal(overship.applyAllowed, false);
      await backend.close();
      backend = new TerminalBackend(database, {
        loadKernel: () => kernel,
        operationsDirectory: journal,
      });
      const recoveryBefore = await hashFiles(files);
      const recovered = await backend.request('reconcile-shipment', { historyId: first.ticketId });
      assert.equal(recovered.receipt.command_id, applied.receipt.command_id);
      assert.deepEqual(recovered.reviewIssues, []);
      assert.deepEqual(await hashFiles(files), recoveryBefore);
      assert.equal((await backend.request('shipment-history')).entries.length, 5);
      assert.deepEqual((await backend.request('order-history')).entries, []);
      passed = true;
    } finally {
      await backend?.close();
      writer?.close();
      if (passed) await fs.rm(directory, { recursive: true, force: true });
      else console.error('Shipment evidence retained:', directory);
    }
  },
);
