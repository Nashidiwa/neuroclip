/**
 * ADVERSARIAL CONCURRENCY & EXTREME LOAD STRESS TEST SUITE
 *
 * Task: TASK-M4-05 (Pre-flight Disk Check & Storage / Cache Subsystem)
 * Milestone: M4.2 (Round 2 Audit)
 * Challenger: Challenger 1 (Backend & Concurrency Adversarial Verifier)
 *
 * Vectors Tested:
 * 1. Extreme Concurrency (50+ parallel checkPreflight calls with chaotic jitter)
 * 2. Reverse Resolution Inversion (Call 50 finishes first at 1ms, Call 1 finishes last at 100ms)
 * 3. Rapid Drive Redirection Hopping (50 interleaved setCustomCacheDir calls)
 * 4. In-Flight Race Invalidation via reset() (30 concurrent calls overtaken by reset)
 * 5. High-Load Mixed Error/Success Stress (100 parallel preflight requests with random failures)
 * 6. Concurrent initializeProject Safety Barrier (50 parallel project allocations blocked by full disk)
 * 7. Interleaved Preflight, Clean, and Reset Multi-Threaded Simulation
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { useProjectStore, MIN_REQUIRED_DISK_SPACE_MB } from '../stores/useProjectStore.ts';
import {
  type DiskStatusJson,
  type CacheCleanupResultJson,
} from '../services/ipc/storage.ts';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

describe('CHALLENGER 1 (R2): Extreme Concurrency & Store Race Condition Harness', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useProjectStore.getState().reset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // =========================================================================
  // TEST 1: 50 SIMULTANEOUS PARALLEL PREFLIGHT CHECKS WITH CHAOTIC JITTER
  // =========================================================================
  it('survives 50 simultaneous parallel checkPreflight calls with randomized completion times without stranded loading flags', async () => {
    let completedCount = 0;

    mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === 'check_preflight_storage') {
        const path = (args as { path?: string | null })?.path ?? 'C:\\cache';
        // Random latency between 2ms and 35ms to simulate chaotic I/O scheduling
        const delay = Math.floor(Math.random() * 33) + 2;
        await new Promise((resolve) => setTimeout(resolve, delay));
        completedCount++;
        return {
          free_space_mb: 32000,
          is_sufficient: true,
          cache_dir: path,
        } satisfies DiskStatusJson;
      }
      throw new Error(`Unexpected command: ${cmd}`);
    });

    // Launch 50 parallel preflight requests simultaneously
    const promises = Array.from({ length: 50 }, (_, i) =>
      useProjectStore.getState().checkPreflight(`C:\\concurrent_path_${i}`),
    );

    // Initial state should indicate storage check in progress
    expect(useProjectStore.getState().isCheckingStorage).toBe(true);

    const results = await Promise.all(promises);

    expect(results).toHaveLength(50);
    expect(completedCount).toBe(50);

    const finalState = useProjectStore.getState();
    // Invariants:
    // 1. Loading flag MUST be reset to false once all complete
    expect(finalState.isCheckingStorage).toBe(false);
    // 2. No error should exist
    expect(finalState.storageError).toBeNull();
    // 3. Space sufficient flag must be false for isInsufficientSpace
    expect(finalState.isInsufficientSpace).toBe(false);
    // 4. DiskStatus must be populated and valid
    expect(finalState.diskStatus).not.toBeNull();
    expect(finalState.diskStatus?.isSufficient).toBe(true);
    expect(finalState.diskStatus?.freeSpaceMb).toBe(32000);
  });

  // =========================================================================
  // TEST 2: REVERSE RESOLUTION INVERSION (CALL 50 FINISHES FIRST, CALL 1 LAST)
  // =========================================================================
  it('strictly preserves the latest dispatched call when 50 calls resolve in exact reverse order (Call 50 at 1ms, Call 1 at 100ms)', async () => {
    // Call index i (0..49):
    // Call 0 (first dispatched) will delay 100ms
    // Call 49 (last dispatched) will delay 2ms
    let callIdx = 0;
    mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === 'check_preflight_storage') {
        const currentIdx = callIdx++;
        const path = (args as { path?: string | null })?.path ?? '';
        // Inverse delay: index 0 gets 100ms, index 49 gets ~2ms
        const delay = Math.max(2, 100 - currentIdx * 2);
        await new Promise((resolve) => setTimeout(resolve, delay));

        // Different free space for each to test for stale overwrites
        const freeMb = (currentIdx + 1) * 1000;
        return {
          free_space_mb: freeMb,
          is_sufficient: freeMb >= MIN_REQUIRED_DISK_SPACE_MB,
          cache_dir: path,
        } satisfies DiskStatusJson;
      }
      throw new Error(`Unexpected command: ${cmd}`);
    });

    const calls: Promise<unknown>[] = [];
    for (let i = 0; i < 50; i++) {
      calls.push(useProjectStore.getState().checkPreflight(`Drive_${i}:\\cache`));
    }

    await Promise.all(calls);

    const finalState = useProjectStore.getState();

    // Call 49 was the LAST dispatched (Drive_49:\cache, 50,000MB free).
    // Because Call 0 resolved last (at 100ms), in an unhardened store Call 0 would
    // clobber Call 49.
    // In our sequence-hardened store, Call 0's mutation is discarded!
    expect(finalState.diskStatus?.cacheDir).toBe('Drive_49:\\cache');
    expect(finalState.diskStatus?.freeSpaceMb).toBe(50000);
    expect(finalState.diskStatus?.isSufficient).toBe(true);
    expect(finalState.isInsufficientSpace).toBe(false);
    expect(finalState.isCheckingStorage).toBe(false);
  });

  // =========================================================================
  // TEST 3: RAPID DRIVE REDIRECTION HOPPING (50 INTERLEAVED setCustomCacheDir)
  // =========================================================================
  it('correctly tracks the final drive across 50 rapid alternating setCustomCacheDir calls with out-of-order latency', async () => {
    mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === 'check_preflight_storage') {
        const path = (args as { path?: string | null })?.path ?? '';
        // Alternate latencies: even drives take 30ms, odd drives take 5ms
        const isOdd = path.includes('odd');
        const delay = isOdd ? 5 : 30;
        await new Promise((resolve) => setTimeout(resolve, delay));

        const isD = path.startsWith('D:');
        return {
          free_space_mb: isD ? 80000 : 2000,
          is_sufficient: isD,
          cache_dir: path,
        } satisfies DiskStatusJson;
      }
      throw new Error(`Unexpected command: ${cmd}`);
    });

    const hopPromises: Promise<unknown>[] = [];
    for (let i = 0; i < 50; i++) {
      const driveLetter = i % 2 === 0 ? 'C' : 'D';
      const parity = i % 2 === 0 ? 'even' : 'odd';
      const target = `${driveLetter}:\\hop_${parity}_${i}`;
      hopPromises.push(useProjectStore.getState().setCustomCacheDir(target));
    }

    await Promise.all(hopPromises);

    const finalState = useProjectStore.getState();
    // Last iteration was i = 49 (odd, D: drive, D:\hop_odd_49)
    expect(finalState.customCacheDir).toBe('D:\\hop_odd_49');
    expect(finalState.diskStatus?.cacheDir).toBe('D:\\hop_odd_49');
    expect(finalState.diskStatus?.freeSpaceMb).toBe(80000);
    expect(finalState.isInsufficientSpace).toBe(false);
    expect(finalState.storageError).toBeNull();
  });

  // =========================================================================
  // TEST 4: IN-FLIGHT INVALIDATION VIA reset() UNDER CONCURRENT FLIGHT
  // =========================================================================
  it('prevents 30 in-flight preflight calls from mutating state after reset() is called mid-flight', async () => {
    mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === 'check_preflight_storage') {
        const path = (args as { path?: string | null })?.path ?? '';
        // In-flight calls take 40ms to complete
        await new Promise((resolve) => setTimeout(resolve, 40));
        return {
          free_space_mb: 15000,
          is_sufficient: true,
          cache_dir: path,
        } satisfies DiskStatusJson;
      }
      throw new Error(`Unexpected command: ${cmd}`);
    });

    // Fire 30 in-flight calls
    const inFlight = Array.from({ length: 30 }, (_, i) =>
      useProjectStore.getState().checkPreflight(`Z:\\stale_${i}`),
    );

    // After 10ms, user resets store (e.g. navigates away / cancels)
    await new Promise((resolve) => setTimeout(resolve, 10));
    useProjectStore.getState().reset();

    // Verify immediate reset state
    expect(useProjectStore.getState().diskStatus).toBeNull();
    expect(useProjectStore.getState().isCheckingStorage).toBe(false);

    // Wait for all 30 slow calls to finish resolving
    await Promise.all(inFlight);

    // CRITICAL INVARIANT: The 30 resolved calls MUST NOT pollute the reset state!
    const postResolutionState = useProjectStore.getState();
    expect(postResolutionState.diskStatus).toBeNull();
    expect(postResolutionState.customCacheDir).toBeNull();
    expect(postResolutionState.isCheckingStorage).toBe(false);
    expect(postResolutionState.isInsufficientSpace).toBe(false);
    expect(postResolutionState.storageError).toBeNull();
    expect(postResolutionState.projectId).toBeNull();
  });

  // =========================================================================
  // TEST 5: HIGH-LOAD MIXED ERROR & SUCCESS STRESS (100 PARALLEL REQUESTS)
  // =========================================================================
  it('safely handles 100 rapid concurrent calls with mixed errors and successes without unhandled state crashes', async () => {
    let reqCount = 0;
    mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === 'check_preflight_storage') {
        const id = reqCount++;
        const delay = (id % 15) + 1;
        await new Promise((resolve) => setTimeout(resolve, delay));

        // Every 3rd call rejects with an OS I/O error
        if (id % 3 === 0) {
          throw new Error(`Simulated Win32 I/O Error code ${id}`);
        }

        const path = (args as { path?: string | null })?.path ?? 'C:\\cache';
        return {
          free_space_mb: 20000,
          is_sufficient: true,
          cache_dir: path,
        } satisfies DiskStatusJson;
      }
      throw new Error(`Unexpected command: ${cmd}`);
    });

    const tasks: Promise<unknown>[] = [];
    for (let i = 0; i < 100; i++) {
      tasks.push(
        useProjectStore
          .getState()
          .checkPreflight(`Drive_${i}:\\path`)
          .catch((e: unknown) => ({ caught: true, error: e })),
      );
    }

    const settled = await Promise.allSettled(tasks);
    expect(settled).toHaveLength(100);

    const state = useProjectStore.getState();
    // System must not remain stuck in checking state
    expect(state.isCheckingStorage).toBe(false);
  });

  // =========================================================================
  // TEST 6: CONCURRENT initializeProject BARRIER UNDER INSUFFICIENT DISK
  // =========================================================================
  it('blocks 50 concurrent initializeProject calls with 100% rejection rate when disk space is insufficient', async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_preflight_storage') {
        return {
          free_space_mb: 1200, // < 5120MB threshold
          is_sufficient: false,
          cache_dir: 'C:\\full_disk\\cache',
        } satisfies DiskStatusJson;
      }
      if (cmd === 'initialize_project') {
        throw new Error('FATAL: initialize_project backend IPC MUST NOT be called when space is insufficient!');
      }
      throw new Error(`Unexpected command: ${cmd}`);
    });

    // Run initial preflight
    await useProjectStore.getState().checkPreflight();
    expect(useProjectStore.getState().isInsufficientSpace).toBe(true);

    // Fire 50 concurrent initializeProject attempts
    const attempts = Array.from({ length: 50 }, (_, i) =>
      useProjectStore.getState().initializeProject(`blocked-project-${i}`),
    );

    const results = await Promise.all(attempts);

    // 100% must return false
    expect(results).toHaveLength(50);
    expect(results.every((res) => res === false)).toBe(true);

    // Store state must remain clean
    expect(useProjectStore.getState().projectId).toBeNull();
    expect(useProjectStore.getState().projectDir).toBeNull();
    expect(mockedInvoke).not.toHaveBeenCalledWith('initialize_project', expect.anything());
  });

  // =========================================================================
  // TEST 7: INTERLEAVED PREFLIGHT, CLEAN CACHE, AND REDIRECTION CONCURRENCY
  // =========================================================================
  it('interleaves 20 cleanCache, 20 setCustomCacheDir, and 20 checkPreflight calls without deadlocks or stranded flags', async () => {
    mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
      const delay = Math.floor(Math.random() * 10) + 1;
      await new Promise((resolve) => setTimeout(resolve, delay));

      if (cmd === 'check_preflight_storage') {
        const path = (args as { path?: string | null })?.path ?? 'D:\\cache';
        return {
          free_space_mb: 45000,
          is_sufficient: true,
          cache_dir: path,
        } satisfies DiskStatusJson;
      }

      if (cmd === 'clean_expired_cache') {
        return {
          files_deleted: 3,
          bytes_freed: 3145728,
          freed_mb: 3,
        } satisfies CacheCleanupResultJson;
      }

      throw new Error(`Unexpected command: ${cmd}`);
    });

    const mixedOps: Promise<unknown>[] = [];
    for (let i = 0; i < 20; i++) {
      mixedOps.push(useProjectStore.getState().checkPreflight(`D:\\interleaved_${i}`));
      mixedOps.push(useProjectStore.getState().setCustomCacheDir(`D:\\custom_${i}`));
      mixedOps.push(useProjectStore.getState().cleanCache(7));
    }

    await Promise.allSettled(mixedOps);

    const finalState = useProjectStore.getState();
    expect(finalState.isCheckingStorage).toBe(false);
    expect(finalState.isCleaningCache).toBe(false);
    expect(finalState.lastCleanedAt).not.toBeNull();
  });
});
