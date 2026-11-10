/**
 * ADVERSARIAL STRESS TEST: TASK-M4-05 Storage Preflight, Drive Redirection & ErrorBanner
 *
 * Challenger 2: Frontend & IPC Adversarial Verification Suite
 *
 * Vectors Tested:
 * 1. Concurrency & Race Conditions (rapid checkPreflight, initializeProject, and cache redirection races)
 * 2. Malformed IPC Payloads & Type Guards (null, negative space, corrupted fields, type guard fallbacks)
 * 3. State Transition & Drive Redirection Cycling (bouncing between full and sufficient drives, path edge cases)
 * 4. ErrorBanner & Retry Under Failure Loop (rapid repeated retry clicks, recovery transition, malformed payloads)
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useProjectStore } from '../stores/useProjectStore.ts';
import {
  type DiskStatusJson,
  type CacheCleanupResultJson,
  isDiskStatusJson,
  isCacheCleanupResultJson,
  isStoragePathsJson,
  toCacheCleanupResult,
  checkPreflightStorage,
  cleanExpiredCache,
} from '../services/ipc/storage.ts';
import {
  ERROR_CODES,
  type IpcErrorResponse,
} from '../services/ipc/error.ts';
import { ErrorBanner } from '../components/common/ErrorBanner.tsx';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

describe('CHALLENGER 2 ADVERSARIAL SUITE: Storage Preflight & IPC Subsystem (TASK-M4-05)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useProjectStore.getState().reset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // =========================================================================
  // VECTOR 1: Concurrency & Race Conditions
  // =========================================================================
  describe('Vector 1: Concurrency & Race Conditions', () => {
    it('handles 50 simultaneous concurrent checkPreflight calls with jittered resolution without state corruption', async () => {
      let callCount = 0;
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'check_preflight_storage') {
          callCount++;
          // Jittered latency between 1ms and 20ms
          const delay = (callCount % 10) + 1;
          await new Promise((resolve) => setTimeout(resolve, delay));
          return {
            free_space_mb: 25600,
            is_sufficient: true,
            cache_dir: 'C:\\NeuroClip\\cache',
          } satisfies DiskStatusJson;
        }
        throw new Error(`Unexpected command: ${cmd}`);
      });

      // Fire 50 concurrent preflight calls
      const promises = Array.from({ length: 50 }, () =>
        useProjectStore.getState().checkPreflight(),
      );

      const results = await Promise.all(promises);

      // Verify all 50 calls resolved successfully
      expect(results).toHaveLength(50);
      for (const res of results) {
        expect(res.freeSpaceMb).toBe(25600);
        expect(res.isSufficient).toBe(true);
      }

      // Verify store settled cleanly
      const state = useProjectStore.getState();
      expect(state.isCheckingStorage).toBe(false);
      expect(state.isInsufficientSpace).toBe(false);
      expect(state.storageError).toBeNull();
      expect(state.diskStatus?.freeSpaceMb).toBe(25600);
    });

    it('proactively blocks 20 concurrent initializeProject calls when disk space is insufficient', async () => {
      // Mock preflight reporting insufficient disk space (2000MB < 5120MB)
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'check_preflight_storage') {
          await new Promise((resolve) => setTimeout(resolve, 5));
          return {
            free_space_mb: 2000,
            is_sufficient: false,
            cache_dir: 'C:\\cache',
          } satisfies DiskStatusJson;
        }
        if (cmd === 'initialize_project') {
          throw new Error('initialize_project IPC MUST NOT be called when space is insufficient!');
        }
        throw new Error(`Unexpected command: ${cmd}`);
      });

      // Perform initial preflight
      await useProjectStore.getState().checkPreflight();
      expect(useProjectStore.getState().isInsufficientSpace).toBe(true);

      // Fire 20 concurrent initializeProject calls
      const promises = Array.from({ length: 20 }, (_, i) =>
        useProjectStore.getState().initializeProject(`proj-concurrent-${i}`),
      );

      const results = await Promise.all(promises);

      // Every single call should be blocked and return false
      for (const result of results) {
        expect(result).toBe(false);
      }

      // Verify backend initialize_project was NEVER called
      expect(mockedInvoke).not.toHaveBeenCalledWith('initialize_project', expect.anything());
      expect(useProjectStore.getState().projectId).toBeNull();
      expect(useProjectStore.getState().projectDir).toBeNull();
    });

    it('eliminates out-of-order resolution race condition when initial checkPreflight is overtaken by setCustomCacheDir', async () => {
      // Setup: C: check takes 40ms, D: check takes 5ms
      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'check_preflight_storage') {
          const path = (args as { path?: string | null })?.path;
          if (path === 'D:\\fast') {
            await new Promise((resolve) => setTimeout(resolve, 5));
            return {
              free_space_mb: 50000,
              is_sufficient: true,
              cache_dir: 'D:\\fast',
            } satisfies DiskStatusJson;
          }
          // Default / C: check is slow (40ms)
          await new Promise((resolve) => setTimeout(resolve, 40));
          return {
            free_space_mb: 1000,
            is_sufficient: false,
            cache_dir: 'C:\\cache',
          } satisfies DiskStatusJson;
        }
        throw new Error(`Unexpected command: ${cmd}`);
      });

      // User triggers initial check (C:)
      const slowInitialCheck = useProjectStore.getState().checkPreflight();

      // After 10ms, user redirects to D:\fast
      await new Promise((resolve) => setTimeout(resolve, 10));
      const fastRedirect = useProjectStore.getState().setCustomCacheDir('D:\\fast');

      // Wait for both to complete
      await Promise.all([slowInitialCheck, fastRedirect]);

      const finalState = useProjectStore.getState();

      // HARDENED REGRESSION ASSERTIONS (TASK-M4-05 Remediation):
      // Stale resolution from slow initial check (C:\, 1000MB) MUST NOT overwrite fast redirect (D:\fast, 50000MB)
      expect(finalState.customCacheDir).toBe('D:\\fast');
      expect(finalState.diskStatus).not.toBeNull();
      expect(finalState.diskStatus?.cacheDir).toBe('D:\\fast');
      expect(finalState.diskStatus?.freeSpaceMb).toBe(50000);
      expect(finalState.diskStatus?.isSufficient).toBe(true);
      expect(finalState.isInsufficientSpace).toBe(false);
      expect(finalState.storageError).toBeNull();
      expect(finalState.isCheckingStorage).toBe(false);
    });

    it('interleaves preflight, cache clean, and drive redirection without stranded flags', async () => {
      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        await new Promise((resolve) => setTimeout(resolve, 5));
        if (cmd === 'check_preflight_storage') {
          const path = (args as { path?: string | null })?.path;
          const isD = path?.startsWith('D:');
          return {
            free_space_mb: isD ? 60000 : 3000,
            is_sufficient: Boolean(isD),
            cache_dir: path ?? 'C:\\cache',
          } satisfies DiskStatusJson;
        }
        if (cmd === 'clean_expired_cache') {
          return {
            files_deleted: 5,
            bytes_freed: 10485760,
            freed_mb: 10,
          } satisfies CacheCleanupResultJson;
        }
        throw new Error(`Unexpected command: ${cmd}`);
      });

      // Interleave multiple actions
      const task1 = useProjectStore.getState().checkPreflight();
      const task2 = useProjectStore.getState().setCustomCacheDir('D:\\fast-drive');
      const task3 = useProjectStore.getState().cleanCache(3);

      await Promise.allSettled([task1, task2, task3]);

      const state = useProjectStore.getState();
      expect(state.isCheckingStorage).toBe(false);
      expect(state.isCleaningCache).toBe(false);
      expect(state.customCacheDir).toBe('D:\\fast-drive');
    });

    it('serializes project initialization when valid disk space exists', async () => {
      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'check_preflight_storage') {
          return {
            free_space_mb: 40000,
            is_sufficient: true,
            cache_dir: 'C:\\cache',
          } satisfies DiskStatusJson;
        }
        if (cmd === 'initialize_project') {
          const id = (args as { projectId: string }).projectId;
          return `C:\\NeuroClip\\projects\\${id}`;
        }
        throw new Error(`Unexpected command: ${cmd}`);
      });

      const success = await useProjectStore
        .getState()
        .initializeProject('alpha-stream', 'Alpha Stream Project');

      expect(success).toBe(true);
      const state = useProjectStore.getState();
      expect(state.projectId).toBe('alpha-stream');
      expect(state.projectName).toBe('Alpha Stream Project');
      expect(state.projectDir).toBe('C:\\NeuroClip\\projects\\alpha-stream');
    });
  });

  // =========================================================================
  // VECTOR 2: Malformed IPC Payloads & Runtime Type Guards
  // =========================================================================
  describe('Vector 2: Malformed IPC Payloads & Runtime Type Guards', () => {
    describe('isDiskStatusJson type guard stress testing', () => {
      it('rejects corrupt, null, primitive, or incomplete objects', () => {
        expect(isDiskStatusJson(null)).toBe(false);
        expect(isDiskStatusJson(undefined)).toBe(false);
        expect(isDiskStatusJson(12345)).toBe(false);
        expect(isDiskStatusJson('free_space_mb: 10000')).toBe(false);
        expect(isDiskStatusJson([])).toBe(false);
        expect(isDiskStatusJson({})).toBe(false);
        expect(isDiskStatusJson({ bad: 123 })).toBe(false);

        // Missing fields
        expect(isDiskStatusJson({ free_space_mb: 10000 })).toBe(false);
        expect(isDiskStatusJson({ free_space_mb: 10000, is_sufficient: true })).toBe(false);
        expect(isDiskStatusJson({ free_space_mb: 10000, cache_dir: 'C:\\' })).toBe(false);

        // Wrong types for fields
        expect(
          isDiskStatusJson({ free_space_mb: '10000', is_sufficient: true, cache_dir: 'C:\\' }),
        ).toBe(false);
        expect(
          isDiskStatusJson({ free_space_mb: 10000, is_sufficient: 'yes', cache_dir: 'C:\\' }),
        ).toBe(false);
        expect(
          isDiskStatusJson({ free_space_mb: 10000, is_sufficient: true, cache_dir: 123 }),
        ).toBe(false);

        // Valid payload
        expect(
          isDiskStatusJson({ free_space_mb: 10000, is_sufficient: true, cache_dir: 'C:\\' }),
        ).toBe(true);
      });
    });

    describe('isCacheCleanupResultJson type guard stress testing', () => {
      it('rejects invalid cache cleanup payload shapes', () => {
        expect(isCacheCleanupResultJson(null)).toBe(false);
        expect(isCacheCleanupResultJson(undefined)).toBe(false);
        expect(isCacheCleanupResultJson({})).toBe(false);
        expect(isCacheCleanupResultJson({ files_deleted: 5 })).toBe(false);
        expect(isCacheCleanupResultJson({ bytes_freed: 1000 })).toBe(false);
        expect(isCacheCleanupResultJson({ files_deleted: '5', bytes_freed: 1000 })).toBe(false);
        expect(isCacheCleanupResultJson({ files_deleted: 5, bytes_freed: '1000' })).toBe(false);

        // Valid
        expect(isCacheCleanupResultJson({ files_deleted: 5, bytes_freed: 1000 })).toBe(true);
        expect(
          isCacheCleanupResultJson({ files_deleted: 0, bytes_freed: 0, freed_mb: 0 }),
        ).toBe(true);
      });
    });

    describe('isStoragePathsJson type guard stress testing', () => {
      it('rejects invalid storage paths payload shapes', () => {
        expect(isStoragePathsJson(null)).toBe(false);
        expect(isStoragePathsJson(undefined)).toBe(false);
        expect(isStoragePathsJson({})).toBe(false);
        expect(isStoragePathsJson({ cache_dir: 'C:\\cache' })).toBe(false);
        expect(isStoragePathsJson({ projects_dir: 'C:\\projects' })).toBe(false);
        expect(isStoragePathsJson({ cache_dir: 123, projects_dir: 'C:\\projects' })).toBe(false);

        // Valid
        expect(
          isStoragePathsJson({ cache_dir: 'C:\\cache', projects_dir: 'C:\\projects' }),
        ).toBe(true);
      });
    });

    describe('toCacheCleanupResult normalization', () => {
      it('derives freedMb from bytesFreed when freed_mb is omitted', () => {
        const normalized = toCacheCleanupResult({
          files_deleted: 10,
          bytes_freed: 52428800, // 50MB
        });
        expect(normalized.filesDeleted).toBe(10);
        expect(normalized.bytesFreed).toBe(52428800);
        expect(normalized.freedMb).toBe(50);
        expect(normalized.errorsCount).toBe(0);
      });
    });

    describe('IPC Error Fallbacks and Corrupt Payloads', () => {
      it('rejects malformed IPC payload { bad: 123 } with validation error and never sets free_space_mb to undefined', async () => {
        const malformedPayload = { bad: 123 };

        // 1. Direct Service Level Rejection
        mockedInvoke.mockResolvedValueOnce(malformedPayload);
        await expect(checkPreflightStorage()).rejects.toSatisfy((err: unknown) => {
          if (typeof err !== 'object' || err === null || !('code' in err)) return false;
          const code = (err as { code: string }).code;
          return code === ERROR_CODES.STORAGE_DISK_CHECK_FAILED;
        });

        // 2. Store Level Rejection & State Protection
        mockedInvoke.mockResolvedValueOnce(malformedPayload);
        await expect(useProjectStore.getState().checkPreflight()).rejects.toSatisfy((err: unknown) => {
          if (typeof err !== 'object' || err === null || !('code' in err)) return false;
          const code = (err as { code: string }).code;
          return code === ERROR_CODES.STORAGE_DISK_CHECK_FAILED;
        });

        const state = useProjectStore.getState();

        // 3. Invariant Enforcements
        // A. diskStatus must remain null and not contain undefined fields
        expect(state.diskStatus).toBeNull();

        // B. Error code must be STORAGE_DISK_CHECK_FAILED, NEVER INGEST_DISK_FULL
        expect(state.storageError).not.toBeNull();
        expect(state.storageError?.code).toBe(ERROR_CODES.STORAGE_DISK_CHECK_FAILED);
        expect(state.storageError?.recoverable).toBe(true);

        // C. Error message must NEVER contain "undefinedMB", "undefined", or "NaN"
        expect(state.storageError?.message).not.toContain('undefinedMB');
        expect(state.storageError?.message).not.toContain('undefined');
        expect(state.storageError?.message).not.toContain('NaN');
      });

      it('rejects when IPC returns null with validation error', async () => {
        // When invoke returns null
        mockedInvoke.mockResolvedValueOnce(null);

        await expect(checkPreflightStorage()).rejects.toSatisfy((err: unknown) => {
          if (typeof err !== 'object' || err === null || !('code' in err)) return false;
          const code = (err as { code: string }).code;
          return code === ERROR_CODES.STORAGE_DISK_CHECK_FAILED;
        });
      });

      it('rejects when cleanExpiredCache receives malformed object without runtime guard validation', async () => {
        mockedInvoke.mockResolvedValueOnce({ bad: 123 });

        await expect(cleanExpiredCache()).rejects.toSatisfy((err: unknown) => {
          if (typeof err !== 'object' || err === null || !('code' in err)) return false;
          const code = (err as { code: string }).code;
          return code === ERROR_CODES.STORAGE_CACHE_CLEANUP_FAILED;
        });
      });

      it('handles negative free space safely by flagging insufficient space', async () => {
        // Negative free space e.g. from an OS reporting anomaly
        mockedInvoke.mockResolvedValueOnce({
          free_space_mb: -100,
          is_sufficient: false,
          cache_dir: 'C:\\NeuroClip\\cache',
        });

        const status = await useProjectStore.getState().checkPreflight();

        expect(status.freeSpaceMb).toBe(-100);
        expect(useProjectStore.getState().isInsufficientSpace).toBe(true);
        expect(useProjectStore.getState().storageError?.code).toBe(ERROR_CODES.INGEST_DISK_FULL);
      });

      it('handles 0MB free space by flagging insufficient space', async () => {
        mockedInvoke.mockResolvedValueOnce({
          free_space_mb: 0,
          is_sufficient: false,
          cache_dir: 'C:\\cache',
        });

        const status = await useProjectStore.getState().checkPreflight();

        expect(status.freeSpaceMb).toBe(0);
        expect(useProjectStore.getState().isInsufficientSpace).toBe(true);
        expect(useProjectStore.getState().storageError?.code).toBe(ERROR_CODES.INGEST_DISK_FULL);
      });

      it('handles IPC rejection with an unhandled exception or non-Error string', async () => {
        mockedInvoke.mockRejectedValueOnce('Disk I/O failure (0x80070017)');

        await expect(useProjectStore.getState().checkPreflight()).rejects.toMatchObject({
          code: ERROR_CODES.INTERNAL_ERROR,
          message: expect.stringContaining('Disk I/O failure'),
          recoverable: false,
        });

        const state = useProjectStore.getState();
        expect(state.isCheckingStorage).toBe(false);
        expect(state.isInsufficientSpace).toBe(true);
        expect(state.storageError).not.toBeNull();
      });

      it('handles IPC rejection with structured AppError JSON envelope', async () => {
        const errorEnvelope: IpcErrorResponse = {
          code: ERROR_CODES.INGEST_DISK_FULL,
          message: 'Dung lượng ổ đĩa không đủ',
          details: 'Yêu cầu tối thiểu 5120MB',
          recoverable: true,
        };
        mockedInvoke.mockRejectedValueOnce(errorEnvelope);

        await expect(useProjectStore.getState().checkPreflight()).rejects.toMatchObject({
          code: ERROR_CODES.INGEST_DISK_FULL,
          recoverable: true,
        });

        const state = useProjectStore.getState();
        expect(state.isInsufficientSpace).toBe(true);
        expect(state.storageError?.code).toBe(ERROR_CODES.INGEST_DISK_FULL);
      });

      it('handles cleanCache failure gracefully without crashing the store', async () => {
        mockedInvoke.mockRejectedValueOnce(new Error('Permission denied on temporary files'));

        const result = await useProjectStore.getState().cleanCache();

        expect(result).toBeNull();
        const state = useProjectStore.getState();
        expect(state.isCleaningCache).toBe(false);
        expect(state.storageError).not.toBeNull();
        expect(state.storageError?.message).toContain('Permission denied');
      });

      it('handles initializeProject failure when Rust backend rejects path traversal', async () => {
        mockedInvoke.mockResolvedValueOnce({
          free_space_mb: 20000,
          is_sufficient: true,
          cache_dir: 'C:\\cache',
        });
        mockedInvoke.mockRejectedValueOnce({
          code: 'ERR_INGEST_INVALID_PROJECT_ID',
          message: 'Invalid project ID: path traversal forbidden',
          recoverable: false,
        });

        const success = await useProjectStore
          .getState()
          .initializeProject('../../../Windows/System32');

        expect(success).toBe(false);
        const state = useProjectStore.getState();
        expect(state.projectId).toBeNull();
        expect(state.storageError).not.toBeNull();
      });
    });
  });

  // =========================================================================
  // VECTOR 3: State Transition & Drive Redirection Cycling
  // =========================================================================
  describe('Vector 3: State Transition & Drive Redirection Cycling', () => {
    it('cycles cleanly between full C: drive and sufficient D: drive 10 times without stale error flags', async () => {
      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'check_preflight_storage') {
          const path = (args as { path?: string | null })?.path;
          if (path?.startsWith('D:')) {
            return {
              free_space_mb: 50000,
              is_sufficient: true,
              cache_dir: 'D:\\NeuroClipCache',
            } satisfies DiskStatusJson;
          }
          return {
            free_space_mb: 1200,
            is_sufficient: false,
            cache_dir: 'C:\\Users\\User\\AppData\\Local\\NeuroClip\\cache',
          } satisfies DiskStatusJson;
        }
        throw new Error(`Unexpected command: ${cmd}`);
      });

      for (let cycle = 1; cycle <= 10; cycle++) {
        // Step A: Full C: drive
        await useProjectStore.getState().setCustomCacheDir('C:\\Users\\User\\AppData\\Local\\NeuroClip\\cache');
        let state = useProjectStore.getState();
        expect(state.isInsufficientSpace).toBe(true);
        expect(state.storageError).not.toBeNull();
        expect(state.storageError?.code).toBe(ERROR_CODES.INGEST_DISK_FULL);
        expect(state.diskStatus?.freeSpaceMb).toBe(1200);

        // Step B: Sufficient D: drive
        await useProjectStore.getState().setCustomCacheDir('D:\\NeuroClipCache');
        state = useProjectStore.getState();
        expect(state.isInsufficientSpace).toBe(false);
        expect(state.storageError).toBeNull();
        expect(state.diskStatus?.freeSpaceMb).toBe(50000);
      }
    });

    it('correctly handles multi-drive hopping (C: full -> D: full -> E: sufficient -> F: full -> E: sufficient)', async () => {
      mockedInvoke.mockImplementation(async (_cmd: string, args?: unknown) => {
        const path = (args as { path?: string | null })?.path ?? '';
        if (path.startsWith('E:')) {
          return {
            free_space_mb: 75000,
            is_sufficient: true,
            cache_dir: 'E:\\NeuroClipStore',
          } satisfies DiskStatusJson;
        }
        return {
          free_space_mb: 2048,
          is_sufficient: false,
          cache_dir: path,
        } satisfies DiskStatusJson;
      });

      // Hop 1: C: (full)
      await useProjectStore.getState().setCustomCacheDir('C:\\cache');
      expect(useProjectStore.getState().isInsufficientSpace).toBe(true);

      // Hop 2: D: (full)
      await useProjectStore.getState().setCustomCacheDir('D:\\cache');
      expect(useProjectStore.getState().isInsufficientSpace).toBe(true);

      // Hop 3: E: (sufficient)
      await useProjectStore.getState().setCustomCacheDir('E:\\NeuroClipStore');
      expect(useProjectStore.getState().isInsufficientSpace).toBe(false);
      expect(useProjectStore.getState().storageError).toBeNull();

      // Hop 4: F: (full)
      await useProjectStore.getState().setCustomCacheDir('F:\\cache');
      expect(useProjectStore.getState().isInsufficientSpace).toBe(true);

      // Hop 5: Return to E: (sufficient)
      await useProjectStore.getState().setCustomCacheDir('E:\\NeuroClipStore');
      expect(useProjectStore.getState().isInsufficientSpace).toBe(false);
      expect(useProjectStore.getState().storageError).toBeNull();
    });

    it('trims leading and trailing whitespace on custom drive path', async () => {
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 30000,
        is_sufficient: true,
        cache_dir: 'D:\\NeuroClipCache',
      });

      await useProjectStore.getState().setCustomCacheDir('   D:\\NeuroClipCache   ');

      expect(useProjectStore.getState().customCacheDir).toBe('D:\\NeuroClipCache');
      expect(mockedInvoke).toHaveBeenCalledWith('check_preflight_storage', {
        path: 'D:\\NeuroClipCache',
      });
    });

    it('handles UNC network paths for cache redirection', async () => {
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 100000,
        is_sufficient: true,
        cache_dir: '\\\\nas\\share\\neuroclip\\cache',
      });

      await useProjectStore.getState().setCustomCacheDir('\\\\nas\\share\\neuroclip\\cache');

      expect(useProjectStore.getState().customCacheDir).toBe('\\\\nas\\share\\neuroclip\\cache');
      expect(useProjectStore.getState().isInsufficientSpace).toBe(false);
    });

    it('preserves clean isolation when reset() is called after complex error state', async () => {
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 1000,
        is_sufficient: false,
        cache_dir: 'C:\\cache',
      });

      await useProjectStore.getState().checkPreflight();
      expect(useProjectStore.getState().isInsufficientSpace).toBe(true);

      useProjectStore.getState().reset();

      const state = useProjectStore.getState();
      expect(state.diskStatus).toBeNull();
      expect(state.customCacheDir).toBeNull();
      expect(state.isInsufficientSpace).toBe(false);
      expect(state.storageError).toBeNull();
      expect(state.projectId).toBeNull();
    });
  });

  // =========================================================================
  // VECTOR 4: ErrorBanner & Retry Under Failure Loop
  // =========================================================================
  describe('Vector 4: ErrorBanner & Retry Under Failure Loop', () => {
    it('survives rapid repeated retry button clicks while drive remains full, then recovers cleanly', async () => {
      let attempts = 0;
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'check_preflight_storage') {
          attempts++;
          // First 5 attempts fail, 6th attempt succeeds (user freed up disk space)
          const isNowSufficient = attempts >= 6;
          return {
            free_space_mb: isNowSufficient ? 15000 : 2500,
            is_sufficient: isNowSufficient,
            cache_dir: 'C:\\cache',
          } satisfies DiskStatusJson;
        }
        throw new Error(`Unexpected command: ${cmd}`);
      });

      // 1. Initial preflight triggers error
      await useProjectStore.getState().checkPreflight();
      expect(useProjectStore.getState().isInsufficientSpace).toBe(true);

      const handleRetry = async () => {
        try {
          await useProjectStore.getState().checkPreflight();
        } catch {
          // Handled in store
        }
      };

      const { rerender } = render(
        React.createElement(ErrorBanner, {
          error: useProjectStore.getState().storageError,
          onRetry: handleRetry,
        }),
      );

      const retryBtn = screen.getByTestId('error-retry-btn');
      expect(retryBtn).toBeInTheDocument();

      // 2. Click Retry 4 more times (attempts 2, 3, 4, 5) while drive remains full
      for (let i = 0; i < 4; i++) {
        fireEvent.click(retryBtn);
        await vi.waitFor(() => {
          expect(useProjectStore.getState().isInsufficientSpace).toBe(true);
        });
        expect(useProjectStore.getState().storageError?.code).toBe(ERROR_CODES.INGEST_DISK_FULL);
      }

      expect(attempts).toBe(5);

      // 3. 6th retry: drive now has 15000MB free space
      fireEvent.click(retryBtn);

      await vi.waitFor(() => {
        expect(useProjectStore.getState().isInsufficientSpace).toBe(false);
      });

      expect(attempts).toBe(6);
      expect(useProjectStore.getState().storageError).toBeNull();

      // Rerender ErrorBanner with cleared error
      rerender(
        React.createElement(ErrorBanner, {
          error: useProjectStore.getState().storageError,
          onRetry: handleRetry,
        }),
      );

      // ErrorBanner should now unmount cleanly
      expect(screen.queryByTestId('error-banner')).toBeNull();
    });

    it('renders ErrorBanner with raw string error without throwing', () => {
      render(React.createElement(ErrorBanner, { error: 'Direct storage failure occurred' }));

      expect(screen.getByTestId('error-message')).toHaveTextContent(
        'Direct storage failure occurred',
      );
      expect(screen.getByTestId('error-code-badge')).toHaveTextContent(ERROR_CODES.INTERNAL_ERROR);
    });

    it('renders ErrorBanner with native Error object without throwing', () => {
      const err = new Error('Windows OS low-level I/O error');
      render(React.createElement(ErrorBanner, { error: err }));

      expect(screen.getByTestId('error-message')).toHaveTextContent(
        'Windows OS low-level I/O error',
      );
    });

    it('renders ErrorBanner with arbitrary malformed object safely', () => {
      const weirdObj = { somethingUnexpected: 999, nested: { status: 'crash' } };
      render(React.createElement(ErrorBanner, { error: weirdObj }));

      expect(screen.getByTestId('error-banner')).toBeInTheDocument();
      expect(screen.getByTestId('error-code-badge')).toHaveTextContent(ERROR_CODES.INTERNAL_ERROR);
    });

    it('renders null when error is null or undefined', () => {
      const { container: c1 } = render(React.createElement(ErrorBanner, { error: null }));
      expect(c1.firstChild).toBeNull();

      const { container: c2 } = render(React.createElement(ErrorBanner, { error: undefined }));
      expect(c2.firstChild).toBeNull();
    });

    it('toggles technical details accordion and simulates copy button without crashing', async () => {
      const errorWithDetails: IpcErrorResponse = {
        code: ERROR_CODES.INGEST_DISK_FULL,
        message: 'Dung lượng ổ đĩa không đủ',
        details: 'Volume C:\\ has only 1200MB free. Required threshold is 5120MB.',
        recoverable: true,
      };

      render(React.createElement(ErrorBanner, { error: errorWithDetails }));

      // Toggle button should be present
      const toggleBtn = screen.getByTestId('toggle-details-btn');
      expect(toggleBtn).toBeInTheDocument();
      expect(screen.queryByTestId('error-details-panel')).toBeNull();

      // Open details panel
      fireEvent.click(toggleBtn);
      expect(screen.getByTestId('error-details-panel')).toBeInTheDocument();
      expect(screen.getByText(/Volume C:\\ has only 1200MB free/)).toBeInTheDocument();

      // Click copy button
      const copyBtn = screen.getByTestId('copy-details-btn');
      expect(copyBtn).toBeInTheDocument();
      fireEvent.click(copyBtn);

      // Close details panel
      fireEvent.click(toggleBtn);
      expect(screen.queryByTestId('error-details-panel')).toBeNull();
    });

    it('hides retry button when error is unrecoverable (recoverable: false)', () => {
      const fatalError: IpcErrorResponse = {
        code: ERROR_CODES.INTERNAL_ERROR,
        message: 'Lỗi bộ nhớ phần cứng không thể phục hồi',
        details: 'Physical drive unmounted',
        recoverable: false,
      };

      render(
        React.createElement(ErrorBanner, {
          error: fatalError,
          onRetry: vi.fn(),
        }),
      );

      expect(screen.getByTestId('error-banner')).toBeInTheDocument();
      expect(screen.queryByTestId('error-retry-btn')).toBeNull();
      expect(screen.getByTestId('icon-unrecoverable')).toBeInTheDocument();
    });
  });
});
