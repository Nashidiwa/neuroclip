/**
 * NeuroClip Project & Storage Workspace Store
 *
 * Reactive store managing pre-flight disk checking, custom drive redirection,
 * project workspace initialization, and cache pruning.
 *
 * Built on native `useSyncExternalStore` (Zustand-compatible API without external runtime deps).
 * Strict adherence to docs/CONVENTIONS.md (zero-any, exactOptionalPropertyTypes).
 */

import { createStore } from './createStore.ts';
import {
  type DiskStatus,
  type CacheCleanupResult,
  checkPreflightStorage,
  cleanExpiredCache,
  initializeProject as initializeProjectIpc,
} from '../services/ipc/storage.ts';
import { type IpcErrorResponse, ERROR_CODES, parseIpcError } from '../services/ipc/error.ts';

/// Minimum required free disk space threshold (5GB = 5120MB).
export const MIN_REQUIRED_DISK_SPACE_MB = 5120;

// ---------------------------------------------------------------------------
// Store Interface Contract
// ---------------------------------------------------------------------------

export interface ProjectState {
  // Storage & Disk Space Status
  readonly diskStatus: DiskStatus | null;
  readonly isCheckingStorage: boolean;
  readonly isInsufficientSpace: boolean;
  readonly storageError: IpcErrorResponse | null;
  readonly customCacheDir: string | null;

  // Active Project Workspace
  readonly projectId: string | null;
  readonly projectName: string | null;
  readonly projectDir: string | null;

  // Cache Maintenance
  readonly isCleaningCache: boolean;
  readonly lastCleanedAt: string | null;

  // Actions
  checkPreflight: (targetPath?: string) => Promise<DiskStatus>;
  setCustomCacheDir: (path: string) => Promise<DiskStatus>;
  cleanCache: (maxAgeDays?: number) => Promise<CacheCleanupResult | null>;
  initializeProject: (
    projectId: string,
    projectName?: string,
    customCacheDir?: string,
  ) => Promise<boolean>;
  clearStorageError: () => void;
  reset: () => void;
}

// ---------------------------------------------------------------------------
// Project Store Instance
// ---------------------------------------------------------------------------

// Generation sequence counter for pre-flight disk checks.
// Prevents out-of-order async resolutions from overwriting newer user selections.
let preflightSeq = 0;

export const useProjectStore = createStore<ProjectState>((set, get) => ({
  diskStatus: null,
  isCheckingStorage: false,
  isInsufficientSpace: false,
  storageError: null,
  customCacheDir: null,
  projectId: null,
  projectName: null,
  projectDir: null,
  isCleaningCache: false,
  lastCleanedAt: null,

  checkPreflight: async (targetPath?: string): Promise<DiskStatus> => {
    const currentSeq = ++preflightSeq;
    set({ isCheckingStorage: true });
    try {
      const pathToCheck = targetPath ?? get().customCacheDir ?? undefined;
      const status = await checkPreflightStorage(pathToCheck);

      // Discard late out-of-order state mutations
      if (currentSeq !== preflightSeq) {
        return status;
      }

      const isInsufficient =
        !status.is_sufficient || status.free_space_mb < MIN_REQUIRED_DISK_SPACE_MB;

      if (isInsufficient) {
        const errorResponse: IpcErrorResponse = {
          code: ERROR_CODES.INGEST_DISK_FULL,
          message: `Dung lượng ổ đĩa không đủ (cần tối thiểu ${MIN_REQUIRED_DISK_SPACE_MB}MB, hiện có ${status.free_space_mb}MB). Vui lòng giải phóng bộ nhớ hoặc chọn thư mục lưu trữ khác.`,
          details: `Đường dẫn: ${status.cache_dir}`,
          recoverable: true,
        };
        set({
          diskStatus: status,
          isCheckingStorage: false,
          isInsufficientSpace: true,
          storageError: errorResponse,
        });
      } else {
        set({
          diskStatus: status,
          isCheckingStorage: false,
          isInsufficientSpace: false,
          storageError: null,
        });
      }

      return status;
    } catch (err: unknown) {
      const parsed = parseIpcError(err, 'Không thể kiểm tra dung lượng ổ đĩa');
      if (currentSeq !== preflightSeq) {
        throw parsed;
      }
      set({
        isCheckingStorage: false,
        isInsufficientSpace: true,
        storageError: parsed,
      });
      throw parsed;
    }
  },

  setCustomCacheDir: async (path: string): Promise<DiskStatus> => {
    const trimmed = path.trim();
    set({ customCacheDir: trimmed });
    return await get().checkPreflight(trimmed);
  },

  cleanCache: async (maxAgeDays?: number): Promise<CacheCleanupResult | null> => {
    set({ isCleaningCache: true });
    try {
      const cacheDir = get().customCacheDir ?? undefined;
      const result = await cleanExpiredCache(cacheDir, maxAgeDays ?? 7);
      set({
        isCleaningCache: false,
        lastCleanedAt: new Date().toISOString(),
      });
      // Refresh disk status after pruning expired cache
      await get().checkPreflight();
      return result;
    } catch (err: unknown) {
      const parsed = parseIpcError(err, 'Không thể dọn dẹp bộ nhớ đệm');
      set({
        isCleaningCache: false,
        storageError: parsed,
      });
      return null;
    }
  },

  initializeProject: async (
    projectId: string,
    projectName?: string,
    customCacheDir?: string,
  ): Promise<boolean> => {
    if (customCacheDir !== undefined && customCacheDir.trim().length > 0) {
      await get().setCustomCacheDir(customCacheDir);
    } else if (get().diskStatus === null) {
      await get().checkPreflight();
    }

    // DEFENSE INVARIANT: Block project initialization if disk space is insufficient
    if (get().isInsufficientSpace) {
      return false;
    }

    try {
      const projectDir = await initializeProjectIpc(projectId);
      set({
        projectId,
        projectName:
          projectName !== undefined && projectName.trim().length > 0 ? projectName : projectId,
        projectDir,
      });
      return true;
    } catch (err: unknown) {
      const parsed = parseIpcError(err, 'Không thể khởi tạo thư mục dự án');
      set({ storageError: parsed });
      return false;
    }
  },

  clearStorageError: (): void => {
    set({ storageError: null });
  },

  reset: (): void => {
    preflightSeq++;
    set({
      diskStatus: null,
      isCheckingStorage: false,
      isInsufficientSpace: false,
      storageError: null,
      customCacheDir: null,
      projectId: null,
      projectName: null,
      projectDir: null,
      isCleaningCache: false,
      lastCleanedAt: null,
    });
  },
}));
