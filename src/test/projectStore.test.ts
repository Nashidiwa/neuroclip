import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  useProjectStore,
  MIN_REQUIRED_DISK_SPACE_MB,
} from '../stores/useProjectStore.ts';
import {
  type DiskStatusJson,
  type CacheCleanupResultJson,
  toDiskStatus,
  isDiskStatusJson,
  isCacheCleanupResultJson,
  isStoragePathsJson,
  toStoragePaths,
} from '../services/ipc/storage.ts';
import { ERROR_CODES } from '../services/ipc/error.ts';
import { ErrorBanner } from '../components/common/ErrorBanner.tsx';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

describe('TASK-M4-05: Project & Storage Store Subsystem (useProjectStore)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useProjectStore.getState().reset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // -------------------------------------------------------------------------
  // TC-STORE-01: Initial State & Clean Defaults
  // -------------------------------------------------------------------------
  describe('TC-STORE-01: Initial State & Clean Defaults', () => {
    it('should initialize with pristine defaults and no active error', () => {
      const state = useProjectStore.getState();
      expect(state.diskStatus).toBeNull();
      expect(state.isCheckingStorage).toBe(false);
      expect(state.isInsufficientSpace).toBe(false);
      expect(state.storageError).toBeNull();
      expect(state.customCacheDir).toBeNull();
      expect(state.projectId).toBeNull();
      expect(state.projectName).toBeNull();
      expect(state.projectDir).toBeNull();
      expect(state.isCleaningCache).toBe(false);
      expect(state.lastCleanedAt).toBeNull();
    });

    it('should enforce MIN_REQUIRED_DISK_SPACE_MB threshold constant at 5120MB (5GB)', () => {
      expect(MIN_REQUIRED_DISK_SPACE_MB).toBe(5120);
    });
  });

  // -------------------------------------------------------------------------
  // TC-STORE-02: UT-05.1 Happy Path Disk Check (20GB >= 5GB)
  // -------------------------------------------------------------------------
  describe('TC-STORE-02: Happy Path Disk Pre-flight (UT-05.1)', () => {
    it('should successfully pass pre-flight when disk free space is 20GB', async () => {
      const mockPayload: DiskStatusJson = {
        free_space_mb: 20480,
        is_sufficient: true,
        cache_dir: 'C:\\Users\\User\\AppData\\Local\\NeuroClip\\cache',
      };
      mockedInvoke.mockResolvedValueOnce(mockPayload);

      const status = await useProjectStore.getState().checkPreflight();

      expect(status.freeSpaceMb).toBe(20480);
      expect(status.free_space_mb).toBe(20480);
      expect(status.isSufficient).toBe(true);
      expect(status.cacheDir).toBe('C:\\Users\\User\\AppData\\Local\\NeuroClip\\cache');

      const state = useProjectStore.getState();
      expect(state.isInsufficientSpace).toBe(false);
      expect(state.storageError).toBeNull();
      expect(state.diskStatus).toEqual(status);
      expect(mockedInvoke).toHaveBeenCalledWith('check_preflight_storage', { path: null });
    });
  });

  // -------------------------------------------------------------------------
  // TC-STORE-03: UT-05.2 Insufficient Space (< 5GB)
  // -------------------------------------------------------------------------
  describe('TC-STORE-03: Insufficient Disk Space (UT-05.2)', () => {
    it('should block and set ERR_INGEST_DISK_FULL when free space is 3.2GB (< 5GB)', async () => {
      const mockPayload: DiskStatusJson = {
        free_space_mb: 3276,
        is_sufficient: false,
        cache_dir: 'C:\\Users\\User\\AppData\\Local\\NeuroClip\\cache',
      };
      mockedInvoke.mockResolvedValueOnce(mockPayload);

      const status = await useProjectStore.getState().checkPreflight();

      expect(status.freeSpaceMb).toBe(3276);
      expect(status.isSufficient).toBe(false);

      const state = useProjectStore.getState();
      expect(state.isInsufficientSpace).toBe(true);
      expect(state.storageError).not.toBeNull();
      expect(state.storageError?.code).toBe(ERROR_CODES.INGEST_DISK_FULL);
      expect(state.storageError?.recoverable).toBe(true);
      expect(state.storageError?.message).toContain('3276MB');
      expect(state.storageError?.message).toContain('5120MB');
      expect(state.storageError?.details).toContain('C:\\Users\\User\\AppData\\Local\\NeuroClip\\cache');
    });
  });

  // -------------------------------------------------------------------------
  // TC-STORE-04: Boundary Testing (5120MB vs 5119MB)
  // -------------------------------------------------------------------------
  describe('TC-STORE-04: Boundary Condition Verification', () => {
    it('should accept exactly 5120MB as sufficient', async () => {
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 5120,
        is_sufficient: true,
        cache_dir: 'C:\\cache',
      });

      await useProjectStore.getState().checkPreflight();
      expect(useProjectStore.getState().isInsufficientSpace).toBe(false);
      expect(useProjectStore.getState().storageError).toBeNull();
    });

    it('should reject 5119MB as insufficient disk space', async () => {
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 5119,
        is_sufficient: false,
        cache_dir: 'C:\\cache',
      });

      await useProjectStore.getState().checkPreflight();
      expect(useProjectStore.getState().isInsufficientSpace).toBe(true);
      expect(useProjectStore.getState().storageError?.code).toBe(ERROR_CODES.INGEST_DISK_FULL);
    });
  });

  // -------------------------------------------------------------------------
  // TC-STORE-05: ED-05.1 Drive Redirection Defense
  // -------------------------------------------------------------------------
  describe('TC-STORE-05: Alternative Drive Redirection (ED-05.1 Defense)', () => {
    it('should clear disk error when redirecting from full C: drive to D: drive with sufficient space', async () => {
      // 1. Initial check on C: drive fails
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 1500,
        is_sufficient: false,
        cache_dir: 'C:\\Users\\User\\AppData\\Local\\NeuroClip\\cache',
      });

      await useProjectStore.getState().checkPreflight();
      expect(useProjectStore.getState().isInsufficientSpace).toBe(true);
      expect(useProjectStore.getState().storageError).not.toBeNull();

      // 2. User redirects cache directory to D:\NeuroClipCache (50GB free)
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 51200,
        is_sufficient: true,
        cache_dir: 'D:\\NeuroClipCache',
      });

      const redirectedStatus = await useProjectStore.getState().setCustomCacheDir('D:\\NeuroClipCache');

      expect(redirectedStatus.freeSpaceMb).toBe(51200);
      expect(redirectedStatus.isSufficient).toBe(true);
      expect(redirectedStatus.cacheDir).toBe('D:\\NeuroClipCache');

      const state = useProjectStore.getState();
      expect(state.customCacheDir).toBe('D:\\NeuroClipCache');
      expect(state.isInsufficientSpace).toBe(false);
      expect(state.storageError).toBeNull();
      expect(mockedInvoke).toHaveBeenLastCalledWith('check_preflight_storage', {
        path: 'D:\\NeuroClipCache',
      });
    });
  });

  // -------------------------------------------------------------------------
  // TC-STORE-06 & TC-STORE-07: Project Initialization Guard
  // -------------------------------------------------------------------------
  describe('TC-STORE-06 & 07: Project Workspace Initialization Guard', () => {
    it('should proactively block project initialization when disk space is insufficient', async () => {
      // Mock insufficient disk space
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 2048,
        is_sufficient: false,
        cache_dir: 'C:\\cache',
      });

      const initSuccess = await useProjectStore.getState().initializeProject('proj-stream-01');

      expect(initSuccess).toBe(false);
      const state = useProjectStore.getState();
      expect(state.isInsufficientSpace).toBe(true);
      expect(state.projectId).toBeNull();
      expect(state.projectDir).toBeNull();
      // Backend initialize_project IPC should NEVER be invoked
      expect(mockedInvoke).not.toHaveBeenCalledWith('initialize_project', expect.anything());
    });

    it('should successfully initialize project workspace when disk space is sufficient', async () => {
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 30720,
        is_sufficient: true,
        cache_dir: 'C:\\cache',
      });
      mockedInvoke.mockResolvedValueOnce('C:\\projects\\proj-neuro-stream');

      const initSuccess = await useProjectStore
        .getState()
        .initializeProject('proj-neuro-stream', 'Neuro Stream Highlights');

      expect(initSuccess).toBe(true);
      const state = useProjectStore.getState();
      expect(state.projectId).toBe('proj-neuro-stream');
      expect(state.projectName).toBe('Neuro Stream Highlights');
      expect(state.projectDir).toBe('C:\\projects\\proj-neuro-stream');
      expect(mockedInvoke).toHaveBeenCalledWith('initialize_project', {
        projectId: 'proj-neuro-stream',
      });
    });
  });

  // -------------------------------------------------------------------------
  // TC-STORE-08: ED-05.2 Cache Cleanup & Pruning
  // -------------------------------------------------------------------------
  describe('TC-STORE-08: Expired Cache Pruning (ED-05.2)', () => {
    it('should prune temporary files and trigger pre-flight status refresh', async () => {
      const mockCleanup: CacheCleanupResultJson = {
        files_deleted: 14,
        bytes_freed: 157286400, // 150MB
        freed_mb: 150,
        errors_count: 0,
      };
      mockedInvoke.mockResolvedValueOnce(mockCleanup);

      // Subsequent refresh check returns reclaimed space
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 15360,
        is_sufficient: true,
        cache_dir: 'C:\\cache',
      });

      const cleanupResult = await useProjectStore.getState().cleanCache(7);

      expect(cleanupResult).not.toBeNull();
      expect(cleanupResult?.filesDeleted).toBe(14);
      expect(cleanupResult?.bytesFreed).toBe(157286400);
      expect(cleanupResult?.freedMb).toBe(150);

      const state = useProjectStore.getState();
      expect(state.lastCleanedAt).not.toBeNull();
      expect(state.diskStatus?.freeSpaceMb).toBe(15360);
      expect(mockedInvoke).toHaveBeenCalledWith('clean_expired_cache', {
        cacheDir: null,
        maxAgeDays: 7,
      });
    });
  });

  // -------------------------------------------------------------------------
  // TC-STORE-09: Error Clearing & Store Reset
  // -------------------------------------------------------------------------
  describe('TC-STORE-09: Error Dismissal & Store Reset', () => {
    it('should clear storage error when clearStorageError is called', async () => {
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 1000,
        is_sufficient: false,
        cache_dir: 'C:\\cache',
      });

      await useProjectStore.getState().checkPreflight();
      expect(useProjectStore.getState().storageError).not.toBeNull();

      useProjectStore.getState().clearStorageError();
      expect(useProjectStore.getState().storageError).toBeNull();
      // Insufficient flag remains until a valid check passes
      expect(useProjectStore.getState().isInsufficientSpace).toBe(true);
    });

    it('should reset all store state to pristine defaults', async () => {
      useProjectStore.setState({
        projectId: 'old-proj',
        projectName: 'Old Name',
        isInsufficientSpace: true,
        customCacheDir: 'D:\\custom',
      });

      useProjectStore.getState().reset();
      const state = useProjectStore.getState();
      expect(state.projectId).toBeNull();
      expect(state.projectName).toBeNull();
      expect(state.isInsufficientSpace).toBe(false);
      expect(state.customCacheDir).toBeNull();
    });

    it('should invalidate in-flight preflight checks when reset is invoked', async () => {
      mockedInvoke.mockImplementationOnce(async () => {
        await new Promise((resolve) => setTimeout(resolve, 30));
        return {
          free_space_mb: 2000,
          is_sufficient: false,
          cache_dir: 'C:\\cache',
        } satisfies DiskStatusJson;
      });

      const inFlightCheck = useProjectStore.getState().checkPreflight();
      expect(useProjectStore.getState().isCheckingStorage).toBe(true);

      // Invoke reset while check is still running
      useProjectStore.getState().reset();
      expect(useProjectStore.getState().isCheckingStorage).toBe(false);

      // Await resolution of the in-flight check
      const status = await inFlightCheck;
      expect(status.freeSpaceMb).toBe(2000);

      // Store must remain pristine and NOT mutated by the invalidated check
      const state = useProjectStore.getState();
      expect(state.diskStatus).toBeNull();
      expect(state.isInsufficientSpace).toBe(false);
      expect(state.storageError).toBeNull();
      expect(state.isCheckingStorage).toBe(false);
    });
  });

  // -------------------------------------------------------------------------
  // TC-STORE-10: ErrorBanner UI Integration
  // -------------------------------------------------------------------------
  describe('TC-STORE-10: ErrorBanner Component Integration', () => {
    it('should render storage error inside ErrorBanner without crashing', async () => {
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 2500,
        is_sufficient: false,
        cache_dir: 'C:\\NeuroClip\\cache',
      });

      await useProjectStore.getState().checkPreflight();
      const error = useProjectStore.getState().storageError;
      expect(error).not.toBeNull();

      render(
        React.createElement(ErrorBanner, {
          error,
          onRetry: () => {
            useProjectStore.getState().checkPreflight();
          },
        }),
      );

      expect(screen.getByTestId('error-code-badge')).toHaveTextContent(ERROR_CODES.INGEST_DISK_FULL);
      expect(screen.getByTestId('error-message')).toHaveTextContent('Dung lượng ổ đĩa không đủ');
      expect(screen.getByTestId('error-retry-btn')).toBeInTheDocument();

      // Trigger retry button
      mockedInvoke.mockResolvedValueOnce({
        free_space_mb: 10240,
        is_sufficient: true,
        cache_dir: 'C:\\NeuroClip\\cache',
      });
      fireEvent.click(screen.getByTestId('error-retry-btn'));

      expect(mockedInvoke).toHaveBeenCalledTimes(2);
    });
  });

  // -------------------------------------------------------------------------
  // TC-STORE-11: Data Contracts & Type Guards
  // -------------------------------------------------------------------------
  describe('TC-STORE-11: Runtime Type Guards & Data Model Contracts', () => {
    it('should validate DiskStatusJson correctly', () => {
      const valid: DiskStatusJson = {
        free_space_mb: 12000,
        is_sufficient: true,
        cache_dir: 'C:\\cache',
      };
      expect(isDiskStatusJson(valid)).toBe(true);
      expect(isDiskStatusJson({ bad: 123 })).toBe(false);
      expect(isDiskStatusJson(null)).toBe(false);

      const domain = toDiskStatus(valid);
      expect(domain.freeSpaceMb).toBe(12000);
      expect(domain.free_space_mb).toBe(12000);
      expect(domain.isSufficient).toBe(true);
      expect(domain.cacheDir).toBe('C:\\cache');
    });

    it('should validate CacheCleanupResultJson and StoragePathsJson', () => {
      expect(isCacheCleanupResultJson({ files_deleted: 3, bytes_freed: 1024 })).toBe(true);
      expect(isCacheCleanupResultJson({ bad: 'entry' })).toBe(false);

      expect(isStoragePathsJson({ cache_dir: 'C:\\cache', projects_dir: 'C:\\projects' })).toBe(true);
      expect(isStoragePathsJson({ only_cache: 'C:\\cache' })).toBe(false);

      const paths = toStoragePaths({ cache_dir: 'C:\\cache', projects_dir: 'C:\\projects' });
      expect(paths.cacheDir).toBe('C:\\cache');
      expect(paths.projectsDir).toBe('C:\\projects');
    });
  });
});
