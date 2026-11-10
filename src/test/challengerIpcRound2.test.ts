/**
 * CHALLENGER ROUND 2 INDEPENDENT ADVERSARIAL STRESS TEST
 *
 * Target 1: High-frequency burst queue drainage in logger.ts
 * Target 2: Corrupt / null log entry filtering in useLogStore.ts
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { LoggerService } from '../services/logging/logger.ts';
import { useLogStore, selectFilteredLogs } from '../stores/useLogStore.ts';
import { type LogEntry, toLogEntry } from '../services/logging/types.ts';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

describe('CHALLENGER R2 IPC & STORE ADVERSARIAL SUITE', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useLogStore.getState().reset();
    (window as unknown as { __TAURI_INTERNALS__: object }).__TAURI_INTERNALS__ = {};
  });

  afterEach(() => {
    delete (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__;
    vi.useRealTimers();
  });

  describe('1. High-Frequency Burst Queue Drainage (logger.ts)', () => {
    it('dispatches exactly 100 logs when fired synchronously within 50ms with simulated async IPC flush', async () => {
      vi.useFakeTimers();
      const service = new LoggerService();
      const dispatchedItems: Array<{ cmd: string; args: unknown }> = [];

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        dispatchedItems.push({ cmd, args });
        // Simulate async IPC delay of 5ms per log dispatch
        await new Promise((resolve) => setTimeout(resolve, 5));
        return Promise.resolve();
      });

      // Fire 100 rapid logs synchronously within a 50ms window
      const burstStartTime = Date.now();
      for (let i = 1; i <= 100; i++) {
        service.info(`Synchronous burst log #${i}`, { index: i });
      }
      const elapsedSyncMs = Date.now() - burstStartTime;

      // Ensure that synchronous firing happened within 50ms
      expect(elapsedSyncMs).toBeLessThanOrEqual(50);

      // Advance timers to allow all async queue flushes and while loops to settle
      await vi.runAllTimersAsync();

      // Invariants:
      // 1. Zero stranded logs remaining in queue
      expect(service.getQueueLength()).toBe(0);
      // 2. Exactly 100 items dispatched to Tauri IPC
      expect(dispatchedItems.length).toBe(100);
      expect(mockedInvoke).toHaveBeenCalledTimes(100);

      // Verify sequence integrity
      for (let i = 0; i < 100; i++) {
        const payload = (dispatchedItems[i]?.args as { message: string });
        expect(payload.message).toBe(`Synchronous burst log #${i + 1}`);
      }

      service.destroy();
    });

    it('handles heavy burst of 250 items with jittered async IPC latency without dropping any log', async () => {
      vi.useFakeTimers();
      const service = new LoggerService();
      const dispatchedItems: unknown[] = [];

      mockedInvoke.mockImplementation(async (_cmd: string, args?: unknown) => {
        dispatchedItems.push(args);
        // Random latency between 1ms and 15ms
        const latency = 1 + (dispatchedItems.length % 15);
        await new Promise((resolve) => setTimeout(resolve, latency));
        return Promise.resolve();
      });

      for (let i = 1; i <= 250; i++) {
        if (i % 50 === 0) {
          service.error(`Burst error #${i}`);
        } else if (i % 25 === 0) {
          service.warn(`Burst warn #${i}`);
        } else {
          service.debug(`Burst debug #${i}`);
        }
      }

      await vi.runAllTimersAsync();

      expect(service.getQueueLength()).toBe(0);
      expect(dispatchedItems.length).toBe(250);

      service.destroy();
    });

    it('recovers and continues draining if individual IPC calls throw exceptions', async () => {
      vi.useFakeTimers();
      const service = new LoggerService();
      const receivedItems: unknown[] = [];
      let failureCount = 0;

      mockedInvoke.mockImplementation(async (_cmd: string, args?: unknown) => {
        receivedItems.push(args);
        failureCount++;
        if (failureCount % 3 === 0) {
          // Every 3rd log fails at IPC level
          throw new Error('IPC Bridge transport glitch');
        }
        await new Promise((resolve) => setTimeout(resolve, 2));
        return Promise.resolve();
      });

      for (let i = 1; i <= 30; i++) {
        service.info(`Fault tolerance item #${i}`);
      }

      await vi.runAllTimersAsync();

      // Even with IPC failures, the internal queue must be drained completely
      expect(service.getQueueLength()).toBe(0);
      expect(receivedItems.length).toBe(30);

      service.destroy();
    });

    it('handles concurrent manual flush() calls while burst logging without race conditions', async () => {
      vi.useFakeTimers();
      const service = new LoggerService();
      const calls: unknown[] = [];

      mockedInvoke.mockImplementation(async (_cmd: string, args?: unknown) => {
        calls.push(args);
        await new Promise((resolve) => setTimeout(resolve, 10));
        return Promise.resolve();
      });

      // Rapidly fire logs and interleaved manual flushes
      for (let i = 1; i <= 50; i++) {
        service.info(`Concurrent item #${i}`);
        if (i % 10 === 0) {
          void service.flush();
        }
      }

      await vi.runAllTimersAsync();

      expect(service.getQueueLength()).toBe(0);
      expect(calls.length).toBe(50);

      service.destroy();
    });
  });

  describe('2. Corrupt / Null Log Entry Filtering (useLogStore.ts)', () => {
    it('defends against null, undefined, and non-string fields in selectFilteredLogs without TypeError', () => {
      const corruptEntries: LogEntry[] = [
        {
          id: 'corrupt-1',
          timestamp: null as unknown as string,
          level: 'error',
          target: null as unknown as string,
          message: null as unknown as string,
        },
        {
          id: 'corrupt-2',
          timestamp: undefined as unknown as string,
          level: 'warn',
          target: undefined as unknown as string,
          message: undefined as unknown as string,
        },
        {
          id: 'corrupt-3',
          timestamp: '2026-11-10T12:00:00.000Z',
          level: null as unknown as 'info',
          target: 'neuroclip::valid',
          message: null as unknown as string,
        },
        {
          id: 'corrupt-4',
          timestamp: '2026-11-10T12:05:00.000Z',
          level: 'info',
          target: null as unknown as string,
          message: 'Valid message content matching query',
        },
        {
          id: 'corrupt-5',
          timestamp: '2026-11-10T12:10:00.000Z',
          level: 'debug',
          target: 'neuroclip::searchable',
          message: 'Another valid log message',
        },
      ];

      useLogStore.setState({ entries: corruptEntries });

      // Scenario A: Search text query against corrupt messages and targets
      const testQueries = [
        'matching',
        'searchable',
        'VALID',
        'null',
        'undefined',
        '',
        '   ',
        'nonexistent-query-xyz',
      ];

      for (const query of testQueries) {
        expect(() => {
          useLogStore.getState().setFilter({ search: query });
          const filtered = selectFilteredLogs(useLogStore.getState());
          expect(Array.isArray(filtered)).toBe(true);
        }).not.toThrow();
      }

      // Scenario B: Target filter with null/undefined targets
      const targetQueries = ['neuroclip', 'valid', 'searchable', 'corrupt', ''];
      for (const tQuery of targetQueries) {
        expect(() => {
          useLogStore.getState().setFilter({ target: tQuery });
          const filtered = selectFilteredLogs(useLogStore.getState());
          expect(Array.isArray(filtered)).toBe(true);
        }).not.toThrow();
      }

      // Reset filters before time boundary test
      useLogStore.getState().resetFilter();

      // Scenario C: Time boundary filter with null/undefined timestamps
      expect(() => {
        useLogStore.getState().setFilter({
          startTime: '2026-11-10T12:01:00.000Z',
          endTime: '2026-11-10T12:09:00.000Z',
        });
        const filtered = selectFilteredLogs(useLogStore.getState());
        expect(Array.isArray(filtered)).toBe(true);
        // Only corrupt-4 falls within [12:01, 12:09]
        expect(filtered.length).toBe(1);
        expect(filtered[0]?.id).toBe('corrupt-4');
      }).not.toThrow();

      // Scenario D: Combined adversarial filter with level, search, target, and time
      useLogStore.getState().resetFilter();
      expect(() => {
        useLogStore.getState().setFilter({
          level: 'info',
          search: 'matching',
          target: '',
          startTime: '2026-11-10T12:00:00.000Z',
          endTime: '2026-11-10T12:15:00.000Z',
        });
        const filtered = selectFilteredLogs(useLogStore.getState());
        expect(filtered.length).toBe(1);
        expect(filtered[0]?.id).toBe('corrupt-4');
      }).not.toThrow();
    });

    it('safely matches valid messages even when neighboring entries have null/undefined properties', () => {
      const mixedCorpus: LogEntry[] = [
        {
          id: 'null-1',
          timestamp: null as unknown as string,
          level: 'info',
          target: null as unknown as string,
          message: null as unknown as string,
        },
        {
          id: 'target-match',
          timestamp: '2026-11-10T12:00:00.000Z',
          level: 'warn',
          target: 'neuroclip::targetmatch',
          message: null as unknown as string,
        },
        {
          id: 'message-match',
          timestamp: '2026-11-10T12:01:00.000Z',
          level: 'error',
          target: null as unknown as string,
          message: 'Critical failure in subsystem',
        },
        {
          id: 'both-null-no-match',
          timestamp: '2026-11-10T12:02:00.000Z',
          level: 'info',
          target: undefined as unknown as string,
          message: undefined as unknown as string,
        },
      ];

      useLogStore.setState({ entries: mixedCorpus });

      // Search matches target even if message is null
      useLogStore.getState().setFilter({ search: 'targetmatch' });
      let filtered = selectFilteredLogs(useLogStore.getState());
      expect(filtered.length).toBe(1);
      expect(filtered[0]?.id).toBe('target-match');

      // Search matches message even if target is null
      useLogStore.getState().setFilter({ search: 'subsystem' });
      filtered = selectFilteredLogs(useLogStore.getState());
      expect(filtered.length).toBe(1);
      expect(filtered[0]?.id).toBe('message-match');

      // Search for something that doesn't match
      useLogStore.getState().setFilter({ search: 'nonexistent' });
      filtered = selectFilteredLogs(useLogStore.getState());
      expect(filtered.length).toBe(0);
    });

    it('toLogEntry handles raw JSON entries with null or missing message safely', () => {
      const rawWithNullMsg = {
        timestamp: '2026-11-10T12:00:00.000Z',
        level: 'INFO',
        target: 'test',
        message: null as unknown as string,
      };

      const normalized = toLogEntry(rawWithNullMsg as unknown as import('../services/logging/types.ts').LogEntryJson);
      expect(typeof normalized.id).toBe('string');
      expect(normalized.timestamp).toBe('2026-11-10T12:00:00.000Z');
      expect(normalized.level).toBe('info');
      expect(normalized.target).toBe('test');
      expect(normalized.message).toBe('');

      // Add to store via addLogEntry
      expect(() => {
        useLogStore.getState().addLogEntry(normalized);
      }).not.toThrow();

      expect(useLogStore.getState().entries.length).toBe(1);
      const state = useLogStore.getState();
      expect(selectFilteredLogs(state).length).toBe(1);
    });
  });
});
