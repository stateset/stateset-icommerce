/**
 * Print Station Tools Module
 *
 * MCP tool definitions for paired print stations and their print job queue.
 */

import { z } from 'zod';
import { applyRequired } from '../utils/apply-guard.js';

const withPolicyDomain = (policyDomain, tools) => tools.map((tool) => ({ policyDomain, ...tool }));

export const printStationTools = withPolicyDomain('print-stations', [
  {
    name: 'list_print_stations',
    description: 'List paired print stations.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const stations = await commerce.printStations.listStations();
      return { success: true, count: stations.length, stations };
    },
  },
  {
    name: 'get_print_station',
    description: 'Get a print station by ID.',
    inputSchema: {
      id: z.string().min(1).describe('Print station ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const station = await commerce.printStations.getStation(params.id);
      if (!station) {
        return { success: false, error: 'Print station not found' };
      }
      return { success: true, station };
    },
  },
  {
    name: 'pair_print_station',
    description: 'Pair a new print station. Returns a one-time pairing token.',
    inputSchema: {
      name: z.string().min(1).describe('Station name'),
      printers: z.array(z.string().min(1)).optional().describe('Printer names on the station'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Pair print station', params);
      }
      const result = await commerce.printStations.pair({
        name: params.name,
        printers: params.printers,
      });
      return {
        success: true,
        message: 'Print station paired',
        station: result.station,
        token: result.token,
      };
    },
  },
  {
    name: 'revoke_print_station',
    description: 'Revoke a paired print station.',
    inputSchema: {
      id: z.string().min(1).describe('Print station ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Revoke print station', params);
      }
      const station = await commerce.printStations.revokeStation(params.id);
      return { success: true, message: 'Print station revoked', station };
    },
  },
  {
    name: 'list_print_jobs',
    description: 'List print jobs for a station.',
    inputSchema: {
      stationId: z.string().min(1).describe('Print station ID'),
      status: z
        .enum(['queued', 'picked_up', 'printed', 'failed'])
        .optional()
        .describe('Filter by job status'),
      limit: z.number().int().positive().optional().describe('Maximum results'),
      offset: z.number().int().min(0).optional().describe('Results to skip'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const jobs = await commerce.printStations.listJobs(params.stationId, {
        status: params.status,
        limit: params.limit,
        offset: params.offset,
      });
      return { success: true, count: jobs.length, jobs };
    },
  },
  {
    name: 'enqueue_print_job',
    description: 'Enqueue a print job to a station.',
    inputSchema: {
      stationId: z.string().min(1).describe('Print station ID'),
      payload: z.string().min(1).describe('Print payload'),
      payloadKind: z.enum(['zpl', 'pdf']).optional().describe('Payload kind (defaults to zpl)'),
      printerName: z.string().min(1).optional().describe('Target printer name'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Enqueue print job', params);
      }
      const job = await commerce.printStations.enqueueJob(params.stationId, {
        payload: params.payload,
        payloadKind: params.payloadKind,
        printerName: params.printerName,
      });
      return { success: true, message: 'Print job enqueued', job };
    },
  },
  {
    name: 'pick_up_next_print_job',
    description: 'Pick up the next queued print job for a station.',
    inputSchema: {
      stationId: z.string().min(1).describe('Print station ID'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Pick up next print job', params);
      }
      const job = await commerce.printStations.nextJob(params.stationId);
      if (!job) {
        return { success: false, error: 'No queued print job for this station' };
      }
      return { success: true, message: 'Print job picked up', job };
    },
  },
  {
    name: 'complete_print_job',
    description: 'Mark a print job printed or failed.',
    inputSchema: {
      jobId: z.string().min(1).describe('Print job ID'),
      success: z.boolean().describe('True to mark printed, false to mark failed'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return applyRequired('Complete print job', params);
      }
      const job = await commerce.printStations.completeJob(params.jobId, params.success);
      return { success: true, message: 'Print job completed', job };
    },
  },
]);

export default printStationTools;
