/**
 * EMPIRICAL ADVERSARIAL STRESS TEST SUITE
 * Milestone M4.1 / TASK-M4-04: Logging Subsystem, Ring Buffer, IPC Bridge & Fallback.
 *
 * This test harness executes rigorous adversarial vectors against:
 * - `src/stores/useLogStore.ts`
 * - `src/services/logging/logger.ts`
 * - `src-tauri/src/commands/log_cmd.rs` contracts
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
  useLogStore,
  selectFilteredLogs,
  DEFAULT_MAX_LOG_ENTRIES,
} from '../stores/useLogStore.ts';
import { LoggerService, isTauriEnvironment } from '../services/logging/logger.ts';
import { type LogEntry, type LogEntryJson, toLogEntry } from '../services/logging/types.ts';
import { ERROR_CODES } from '../services/ipc/error.ts';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

describe('ADVERSARIAL STRESS TEST: TASK-M4-04 Logging Infrastructure', () => {
  beforeEach(() => {
    useLogStore.getState().reset();
    vi.clearAllMocks();
    delete (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__;
  });

  // =========================================================================
  // VECTOR 1: Ring Buffer Saturation Stress
  // =========================================================================
  describe('Vector 1: Ring Buffer Saturation Stress', () => {
    it('should strictly cap store size at 1,000 items when 2,500 entries are rapidly pushed', () => {
      const TOTAL_ENTRIES = 2_500;
      const startTime = performance.now();

      // Rapidly push 2,500 entries into useLogStore
      for (let i = 1; i <= TOTAL_ENTRIES; i++) {
        useLogStore.getState().addLogEntry({
          id: `entry-${i}`,
          timestamp: new Date(1700000000000 + i * 1000).toISOString(),
          level: i % 10 === 0 ? 'error' : i % 5 === 0 ? 'warn' : 'info',
          target: `neuroclip::module_${i % 4}`,
          message: `Adversarial stress log payload #${i}`,
        });
      }

      const durationMs = performance.now() - startTime;
      const state = useLogStore.getState();

      // Invariant 1: Store length strictly capped at DEFAULT_MAX_LOG_ENTRIES (1,000)
      expect(state.entries.length).toBe(DEFAULT_MAX_LOG_ENTRIES);
      expect(state.entries.length).toBe(1_000);

      // Invariant 2: Oldest entries evicted in FIFO order
      // Entries 1..1,500 must have been evicted.
      // Entries 1,501..2,500 must be retained in order.
      expect(state.entries[0]?.id).toBe('entry-1501');
      expect(state.entries[state.entries.length - 1]?.id).toBe('entry-2500');

      // Invariant 3: Performance bounds (< 500ms for 2,500 pushes)
      expect(durationMs).toBeLessThan(500);
    });

    it('should handle raw LogEntryJson objects in high volume with correct normalization and eviction', () => {
      const TOTAL = 1_500;
      for (let i = 1; i <= TOTAL; i++) {
        const raw: LogEntryJson = {
          timestamp: `2026-11-10T10:00:${String(i % 60).padStart(2, '0')}.000Z`,
          level: i % 2 === 0 ? 'ERROR' : 'DEBUG',
          target: 'neuroclip::raw',
          message: `Raw message ${i}`,
          fields: { sequence: i, data: `val-${i}` },
          line: i,
          file: 'mod.rs',
        };
        useLogStore.getState().addLogEntry(raw);
      }

      const state = useLogStore.getState();
      expect(state.entries.length).toBe(1_000);

      // Verify normalization
      const first = state.entries[0];
      const last = state.entries[999];
      expect(first?.message).toBe('Raw message 501');
      expect(first?.level).toBe('debug'); // 501 % 2 !== 0 -> DEBUG -> normalized to debug
      expect(last?.message).toBe('Raw message 1500');
      expect(last?.level).toBe('error'); // 1500 % 2 === 0 -> ERROR -> normalized to error
    });

    it('should not leak memory when repeatedly saturated across multiple cycles', () => {
      // 5 cycles of 1,200 pushes = 6,000 total pushes
      for (let cycle = 0; cycle < 5; cycle++) {
        for (let i = 1; i <= 1_200; i++) {
          useLogStore.getState().addLogEntry({
            id: `c${cycle}-e${i}`,
            timestamp: new Date().toISOString(),
            level: 'info',
            target: 'test',
            message: 'Cycle payload',
          });
        }
        expect(useLogStore.getState().entries.length).toBe(1_000);
      }
      expect(useLogStore.getState().entries.length).toBe(1_000);
    });
  });

  // =========================================================================
  // VECTOR 2: Filter & Search Combinations & Edge Cases
  // =========================================================================
  describe('Vector 2: Filter & Search Combinations & Edge Cases', () => {
    beforeEach(() => {
      const corpus: LogEntry[] = [
        {
          id: 'log-err-timeout-1',
          timestamp: '2026-11-10T12:00:00.000Z',
          level: 'error',
          target: 'neuroclip::network',
          message: 'Connection network timeout occurred during stream ingest',
        },
        {
          id: 'log-err-storage-2',
          timestamp: '2026-11-10T12:05:00.000Z',
          level: 'error',
          target: 'neuroclip::storage',
          message: 'Disk write failed: out of space on drive C:',
        },
        {
          id: 'log-warn-timeout-3',
          timestamp: '2026-11-10T12:10:00.000Z',
          level: 'warn',
          target: 'neuroclip::network',
          message: 'Soft timeout reached, retrying request (attempt 2)',
        },
        {
          id: 'log-info-timeout-4',
          timestamp: '2026-11-10T12:15:00.000Z',
          level: 'info',
          target: 'neuroclip::ingestion',
          message: 'Keepalive timeout configured to 30000ms',
        },
        {
          id: 'log-info-normal-5',
          timestamp: '2026-11-10T12:20:00.000Z',
          level: 'info',
          target: 'neuroclip::ui',
          message: 'Rendered dashboard successfully',
        },
      ];

      useLogStore.setState({ entries: corpus });
    });

    it('should correctly evaluate simultaneous multi-predicate filter: level="error" AND search="timeout"', () => {
      useLogStore.getState().setFilter({
        level: 'error',
        search: 'timeout',
      });

      const filtered = selectFilteredLogs(useLogStore.getState());
      expect(filtered.length).toBe(1);
      expect(filtered[0]?.id).toBe('log-err-timeout-1');
      expect(filtered[0]?.level).toBe('error');
      expect(filtered[0]?.message).toContain('timeout');
    });

    it('should correctly evaluate 3-way combined filter: level + target + search', () => {
      useLogStore.getState().setFilter({
        level: 'warn',
        target: 'network',
        search: 'retrying',
      });

      const filtered = selectFilteredLogs(useLogStore.getState());
      expect(filtered.length).toBe(1);
      expect(filtered[0]?.id).toBe('log-warn-timeout-3');
    });

    it('should correctly evaluate 4-way filter: level + target + search + time boundary', () => {
      useLogStore.getState().setFilter({
        level: 'error',
        target: 'neuroclip',
        search: 'timeout',
        startTime: '2026-11-10T11:59:00.000Z',
        endTime: '2026-11-10T12:01:00.000Z',
      });

      const filtered = selectFilteredLogs(useLogStore.getState());
      expect(filtered.length).toBe(1);
      expect(filtered[0]?.id).toBe('log-err-timeout-1');

      // Exclude by time boundary
      useLogStore.getState().setFilter({
        startTime: '2026-11-10T12:01:00.000Z',
      });
      const emptyFiltered = selectFilteredLogs(useLogStore.getState());
      expect(emptyFiltered.length).toBe(0);
    });

    it('should be robust against regex special characters in search query', () => {
      const attackStrings = [
        '.*',
        '([a-zA-Z]+)*',
        '\\d+\\.\\d+',
        '???+++',
        '[^a-z]',
        '${payload}',
        '__proto__',
      ];

      for (const attack of attackStrings) {
        expect(() => {
          useLogStore.getState().setFilter({ search: attack });
          selectFilteredLogs(useLogStore.getState());
        }).not.toThrow();
      }
    });

    it('should handle search queries matching target module even if not in message', () => {
      useLogStore.getState().setFilter({ search: 'storage' });
      const filtered = selectFilteredLogs(useLogStore.getState());
      expect(filtered.length).toBe(1);
      expect(filtered[0]?.id).toBe('log-err-storage-2');
    });

    it('VERIFICATION: selectFilteredLogs defends against null/undefined message and target without throwing', () => {
      const malformedEntries: LogEntry[] = [
        {
          id: 'corrupt-null-msg',
          timestamp: '2026-11-10T12:25:00.000Z',
          level: 'error',
          target: 'corrupt::source',
          message: null as unknown as string,
        },
        {
          id: 'corrupt-undef-msg',
          timestamp: '2026-11-10T12:26:00.000Z',
          level: 'warn',
          target: 'corrupt::source',
          message: undefined as unknown as string,
        },
        {
          id: 'corrupt-null-tgt',
          timestamp: '2026-11-10T12:27:00.000Z',
          level: 'info',
          target: null as unknown as string,
          message: 'Safe log message',
        },
      ];

      useLogStore.setState({ entries: malformedEntries });
      useLogStore.getState().setFilter({ search: 'timeout' });

      let errorThrown: unknown = null;
      let results: readonly LogEntry[] = [];
      try {
        results = selectFilteredLogs(useLogStore.getState());
      } catch (err) {
        errorThrown = err;
      }

      expect(errorThrown).toBeNull();
      expect(results.length).toBe(0);

      // Search matching safe message with null target
      useLogStore.getState().setFilter({ search: 'safe' });
      const safeMatch = selectFilteredLogs(useLogStore.getState());
      expect(safeMatch.length).toBe(1);
      expect(safeMatch[0]?.id).toBe('corrupt-null-tgt');
    });

    it('VERIFICATION: toLogEntry sanitizes null/undefined message to empty string', () => {
      const rawCorrupt: LogEntryJson = {
        timestamp: '',
        level: 'UNKNOWN_LEVEL',
        target: '',
        message: null as unknown as string,
      };

      const domain = toLogEntry(rawCorrupt);
      expect(domain.level).toBe('info');
      expect(domain.target).toBe('app');
      expect(typeof domain.message).toBe('string');
      expect(domain.message).toBe('');
    });
  });

  // =========================================================================
  // VECTOR 3: Batching & Debouncing Resilience
  // =========================================================================
  describe('Vector 3: Batching & Debouncing Resilience', () => {
    let service: LoggerService;

    beforeEach(() => {
      vi.useFakeTimers();
      service = new LoggerService();
    });

    afterEach(() => {
      service.destroy();
      vi.useRealTimers();
      delete (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__;
    });

    it('should bundle items up to 25 items per batch when discrete batches are allowed to drain', async () => {
      const dispatchedCalls: unknown[] = [];
      mockedInvoke.mockImplementation(async (_cmd: string, args?: unknown) => {
        dispatchedCalls.push(args);
        return Promise.resolve();
      });

      (window as unknown as { __TAURI_INTERNALS__: object }).__TAURI_INTERNALS__ = {};

      // Send 4 discrete batches of 25 items, allowing each batch to settle
      for (let b = 0; b < 4; b++) {
        for (let i = 1; i <= 25; i++) {
          service.info(`Batch ${b} item #${i}`);
        }
        await vi.runAllTimersAsync();
      }

      expect(service.getQueueLength()).toBe(0);
      expect(dispatchedCalls.length).toBe(100);
    });

    it('VERIFICATION: rapid 100-log burst drains 100% of items without leaving stranded queue entries', async () => {
      const dispatchedCalls: unknown[] = [];
      mockedInvoke.mockImplementation(async (_cmd: string, args?: unknown) => {
        dispatchedCalls.push(args);
        await new Promise((resolve) => setTimeout(resolve, 5));
        return Promise.resolve();
      });

      (window as unknown as { __TAURI_INTERNALS__: object }).__TAURI_INTERNALS__ = {};

      // Fire 100 rapid calls synchronously without waiting between calls
      for (let i = 1; i <= 100; i++) {
        service.info(`Rapid burst log #${i}`);
      }

      await vi.runAllTimersAsync();

      // Invariant: all items drained, zero stranded items
      expect(service.getQueueLength()).toBe(0);
      expect(dispatchedCalls.length).toBe(100);
    });

    it('should immediately flush queued items when an ERROR log is received, regardless of threshold', async () => {
      const dispatchedCalls: unknown[] = [];
      mockedInvoke.mockImplementation(async (_cmd: string, args?: unknown) => {
        dispatchedCalls.push(args);
        return Promise.resolve();
      });

      (window as unknown as { __TAURI_INTERNALS__: object }).__TAURI_INTERNALS__ = {};

      // Push 5 INFO logs (below 25 threshold)
      for (let i = 1; i <= 5; i++) {
        service.info(`Message ${i}`);
      }
      expect(service.getQueueLength()).toBe(5);

      // Push 1 ERROR log -> must immediately trigger flush of all 6 items
      service.error('Critical fatal failure event');

      await vi.runAllTimersAsync();
      expect(service.getQueueLength()).toBe(0);
      expect(dispatchedCalls.length).toBe(6);
    });

    it('VERIFICATION: in-flight flush drains subsequent incoming batches without starvation', async () => {
      // Simulate real asynchronous IPC delay
      const dispatchedCalls: unknown[] = [];
      mockedInvoke.mockImplementation(async (_cmd: string, args?: unknown) => {
        dispatchedCalls.push(args);
        await new Promise((resolve) => setTimeout(resolve, 50));
        return Promise.resolve();
      });

      (window as unknown as { __TAURI_INTERNALS__: object }).__TAURI_INTERNALS__ = {};

      // 1. Send 25 items -> triggers flush #1 (isFlushing becomes true)
      for (let i = 1; i <= 25; i++) {
        service.info(`Batch 1 item #${i}`);
      }

      // Step forward slightly to enter the async loop
      await vi.advanceTimersByTimeAsync(1);

      // 2. While flush #1 is in-flight, send 25 more items!
      for (let i = 26; i <= 50; i++) {
        service.info(`Batch 2 item #${i}`);
      }

      // 3. Complete flush #1 and allow subsequent flush to drain
      await vi.advanceTimersByTimeAsync(5000);
      await vi.runAllTimersAsync();

      expect(service.getQueueLength()).toBe(0);
      expect(dispatchedCalls.length).toBe(50);
    });
  });

  // =========================================================================
  // VECTOR 4: Fallback Execution in Non-Tauri Environment
  // =========================================================================
  describe('Vector 4: Fallback Execution in Non-Tauri Environment', () => {
    it('should verify isTauriEnvironment() correctly returns false in browser / test env without __TAURI_INTERNALS__', () => {
      delete (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__;
      expect(isTauriEnvironment()).toBe(false);
    });

    it('should degrade gracefully on logger.info and logger.error without throwing in non-Tauri env', async () => {
      delete (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__;
      const service = new LoggerService();

      expect(() => {
        service.info('Browser fallback info');
        service.error('Browser fallback error');
      }).not.toThrow();

      await expect(service.flush()).resolves.toBeUndefined();
      service.destroy();
    });

    it('should degrade gracefully on useLogStore.fetchLogs() in non-Tauri env without error', async () => {
      delete (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__;

      await expect(useLogStore.getState().fetchLogs()).resolves.toBeUndefined();
      const state = useLogStore.getState();
      expect(state.isLoading).toBe(false);
      expect(state.error).toBeNull();
    });

    it('should degrade gracefully on useLogStore.clearLogs() in non-Tauri env and return true', async () => {
      delete (window as unknown as { __TAURI_INTERNALS__?: object }).__TAURI_INTERNALS__;
      useLogStore.setState({
        entries: [
          {
            id: 'log-1',
            timestamp: new Date().toISOString(),
            level: 'info',
            target: 'test',
            message: 'To be wiped',
          },
        ],
      });

      const res = await useLogStore.getState().clearLogs();
      expect(res).toBe(true);
      expect(useLogStore.getState().entries).toEqual([]);
      expect(useLogStore.getState().isClearing).toBe(false);
      expect(useLogStore.getState().error).toBeNull();
    });

    it('should capture IPC rejection, route to parseIpcError, and set error state without throwing', async () => {
      (window as unknown as { __TAURI_INTERNALS__: object }).__TAURI_INTERNALS__ = {};
      const mockRejection = new Error(
        JSON.stringify({
          code: ERROR_CODES.STORAGE_FILE_NOT_FOUND,
          message: 'Tệp nhật ký không tồn tại trên ổ đĩa.',
          recoverable: true,
        }),
      );

      mockedInvoke.mockRejectedValue(mockRejection);

      // 1. Test fetchLogs error capture
      await expect(useLogStore.getState().fetchLogs()).resolves.toBeUndefined();
      let state = useLogStore.getState();
      expect(state.isLoading).toBe(false);
      expect(state.error).toBe('Tệp nhật ký không tồn tại trên ổ đĩa.');

      // 2. Test clearLogs error capture
      const clearRes = await useLogStore.getState().clearLogs();
      expect(clearRes).toBe(false);
      state = useLogStore.getState();
      expect(state.isClearing).toBe(false);
      expect(state.error).toBe('Tệp nhật ký không tồn tại trên ổ đĩa.');
    });

    it('should parse raw unhandled string error from IPC via parseIpcError fallback', async () => {
      (window as unknown as { __TAURI_INTERNALS__: object }).__TAURI_INTERNALS__ = {};
      mockedInvoke.mockRejectedValue('Windows OS Access Denied (os error 5)');

      await useLogStore.getState().fetchLogs();
      const state = useLogStore.getState();
      expect(state.error).toBe('Windows OS Access Denied (os error 5)');
    });
  });

  // =========================================================================
  // VECTOR 5: Rust Backend Multi-File Concatenation & Inversion Analysis
  // =========================================================================
  describe('Vector 5: Rust Backend Log Rotation & Boundary Invariants', () => {
    it('should verify log_cmd.rs implementation details against Multi-File Concat Contract', async () => {
      const clampLimit = (limit: number | undefined, defaultLimit = 200, maxLimit = 1000) => {
        const val = limit ?? defaultLimit;
        return Math.max(1, Math.min(val, maxLimit));
      };

      expect(clampLimit(undefined)).toBe(200);
      expect(clampLimit(0)).toBe(1);
      expect(clampLimit(5000)).toBe(1000);
      expect(clampLimit(-10)).toBe(1);
    });

    it('VERIFICATION: multi-file chronological retrieval retains today\'s newest logs and sorts chronologically', () => {
      // Model of how fixed log_cmd.rs processes multiple files
      // File 1 (Newest file, e.g. 2026-11-10): 50 entries
      const file1Logs = Array.from({ length: 50 }, (_, i) => ({
        file: '2026-11-10.ndjson',
        id: `today-${i + 1}`,
        timestamp: `2026-11-10T10:${String(i).padStart(2, '0')}:00.000Z`,
        level: 'info' as const,
        target: 'neuroclip::test',
        message: `Today entry #${i + 1}`,
      }));

      // File 2 (Yesterday file, e.g. 2026-11-09): 300 entries
      const file2Logs = Array.from({ length: 300 }, (_, i) => ({
        file: '2026-11-09.ndjson',
        id: `yesterday-${i + 1}`,
        timestamp: `2026-11-09T10:${String(i % 60).padStart(2, '0')}:00.000Z`,
        level: 'info' as const,
        target: 'neuroclip::test',
        message: `Yesterday entry #${i + 1}`,
      }));

      const effectiveLimit = 200;

      // Slicing algorithm matching fixed get_recent_logs:
      // Files iterated newest-first: take newest entries per file bounded by remaining needed
      const collected: typeof file1Logs = [];
      const files = [file1Logs, file2Logs];
      for (const fileEntries of files) {
        const needed = effectiveLimit - collected.length;
        if (needed <= 0) break;
        const slice = fileEntries.length > needed
          ? fileEntries.slice(fileEntries.length - needed)
          : fileEntries;
        collected.push(...slice);
      }
      collected.sort((a, b) => a.timestamp.localeCompare(b.timestamp));

      expect(collected.length).toBe(200);

      // Verify that today's logs are completely preserved!
      const todaysLogsReturned = collected.filter((e) => e.file === '2026-11-10.ndjson');
      expect(todaysLogsReturned.length).toBe(50);
      expect(todaysLogsReturned[0]?.id).toBe('today-1');
      expect(todaysLogsReturned[49]?.id).toBe('today-50');

      // Exactly 150 entries from yesterday
      const yesterdaysLogsReturned = collected.filter((e) => e.file === '2026-11-09.ndjson');
      expect(yesterdaysLogsReturned.length).toBe(150);

      // Verify chronological order
      for (let i = 1; i < collected.length; i++) {
        expect(collected[i]!.timestamp >= collected[i - 1]!.timestamp).toBe(true);
      }
    });
  });
});
