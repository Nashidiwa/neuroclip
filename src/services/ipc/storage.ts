/**
 * NeuroClip Pre-flight Storage & Cache Management IPC Client
 *
 * Provides strongly-typed interfaces, runtime type guards, and IPC invoke wrappers
 * for disk free space checks, cache pruning, and workspace directory management.
 * Strict adherence to docs/CONVENTIONS.md (zero-any, exactOptionalPropertyTypes).
 */

import { invoke } from '@tauri-apps/api/core';
import {
  ERROR_CODES,
  parseIpcError,
  type IpcErrorResponse,
} from './error.ts';

// ---------------------------------------------------------------------------
// Data Contracts: Pre-flight Disk Status
// ---------------------------------------------------------------------------

/**
 * Wire JSON format returned from Tauri command `check_preflight_storage`.
 */
export interface DiskStatusJson {
  readonly free_space_mb: number;
  readonly is_sufficient: boolean;
  readonly cache_dir: string;
  readonly free_mb?: number;
}

/**
 * Frontend Domain Model for Disk Status.
 * Supports both camelCase convenience accessors and snake_case wire properties.
 */
export interface DiskStatus {
  readonly freeSpaceMb: number;
  readonly isSufficient: boolean;
  readonly cacheDir: string;
  readonly free_space_mb: number;
  readonly is_sufficient: boolean;
  readonly cache_dir: string;
}

/**
 * Type guard for wire DiskStatusJson payload.
 */
export function isDiskStatusJson(val: unknown): val is DiskStatusJson {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['free_space_mb'] === 'number' &&
    typeof obj['is_sufficient'] === 'boolean' &&
    typeof obj['cache_dir'] === 'string'
  );
}

/**
 * Convert wire JSON or domain status into a normalized DiskStatus object.
 */
export function toDiskStatus(raw: DiskStatusJson | DiskStatus): DiskStatus {
  const freeSpaceMb = 'freeSpaceMb' in raw ? raw.freeSpaceMb : raw.free_space_mb;
  const isSufficient = 'isSufficient' in raw ? raw.isSufficient : raw.is_sufficient;
  const cacheDir = 'cacheDir' in raw ? raw.cacheDir : raw.cache_dir;

  return {
    freeSpaceMb,
    isSufficient,
    cacheDir,
    free_space_mb: freeSpaceMb,
    is_sufficient: isSufficient,
    cache_dir: cacheDir,
  };
}

// ---------------------------------------------------------------------------
// Data Contracts: Cache Cleanup Result
// ---------------------------------------------------------------------------

/**
 * Wire JSON format returned from Tauri command `clean_expired_cache`.
 */
export interface CacheCleanupResultJson {
  readonly files_deleted: number;
  readonly bytes_freed: number;
  readonly freed_bytes?: number;
  readonly freed_mb?: number;
  readonly errors_count?: number;
}

/**
 * Frontend Domain Model for Cache Cleanup Result.
 */
export interface CacheCleanupResult {
  readonly filesDeleted: number;
  readonly bytesFreed: number;
  readonly freedMb: number;
  readonly errorsCount: number;
}

/**
 * Type guard for wire CacheCleanupResultJson payload.
 */
export function isCacheCleanupResultJson(val: unknown): val is CacheCleanupResultJson {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['files_deleted'] === 'number' &&
    typeof obj['bytes_freed'] === 'number'
  );
}

/**
 * Convert wire JSON into a normalized CacheCleanupResult domain object.
 */
export function toCacheCleanupResult(raw: CacheCleanupResultJson): CacheCleanupResult {
  const bytesFreed = raw.bytes_freed;
  const freedMb = raw.freed_mb !== undefined ? raw.freed_mb : Math.floor(bytesFreed / (1024 * 1024));
  return {
    filesDeleted: raw.files_deleted,
    bytesFreed,
    freedMb,
    errorsCount: raw.errors_count !== undefined ? raw.errors_count : 0,
  };
}

// ---------------------------------------------------------------------------
// Data Contracts: Storage Paths
// ---------------------------------------------------------------------------

/**
 * Wire JSON format returned from Tauri command `initialize_storage`.
 */
export interface StoragePathsJson {
  readonly cache_dir: string;
  readonly projects_dir: string;
}

/**
 * Frontend Domain Model for Storage Paths.
 */
export interface StoragePaths {
  readonly cacheDir: string;
  readonly projectsDir: string;
}

/**
 * Type guard for StoragePathsJson payload.
 */
export function isStoragePathsJson(val: unknown): val is StoragePathsJson {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['cache_dir'] === 'string' &&
    typeof obj['projects_dir'] === 'string'
  );
}

/**
 * Convert wire JSON into normalized StoragePaths domain object.
 */
export function toStoragePaths(raw: StoragePathsJson): StoragePaths {
  return {
    cacheDir: raw.cache_dir,
    projectsDir: raw.projects_dir,
  };
}

// ---------------------------------------------------------------------------
// Helpers & IPC Invocation Wrappers
// ---------------------------------------------------------------------------

/**
 * Safely serializes an unknown value for error logging/details.
 */
function safeSerialize(raw: unknown): string {
  if (raw === undefined) return 'undefined';
  if (raw === null) return 'null';
  if (typeof raw === 'object') {
    try {
      return JSON.stringify(raw);
    } catch {
      return '[Unserializable object]';
    }
  }
  return String(raw);
}

/**
 * Query available free disk space on default cache or custom directory path.
 *
 * SLA execution time: <= 50ms (Win32 GetDiskFreeSpaceExW).
 *
 * @param path Optional custom directory path (e.g. 'D:\\NeuroClipCache').
 * @returns Normalized DiskStatus containing available space and sufficiency status.
 */
export async function checkPreflightStorage(path?: string): Promise<DiskStatus> {
  try {
    const raw: unknown = await invoke('check_preflight_storage', {
      path: path !== undefined && path.trim().length > 0 ? path.trim() : null,
    });
    if (!isDiskStatusJson(raw)) {
      throw {
        code: ERROR_CODES.STORAGE_DISK_CHECK_FAILED,
        message: 'Dữ liệu phản hồi kiểm tra dung lượng ổ đĩa không đúng định dạng',
        details: `Malformed payload from check_preflight_storage: ${safeSerialize(raw)}`,
        recoverable: true,
      } satisfies IpcErrorResponse;
    }
    return toDiskStatus(raw);
  } catch (err: unknown) {
    throw parseIpcError(err, 'Không thể kiểm tra dung lượng ổ đĩa');
  }
}

/**
 * Clean expired `.tmp` and `.wav` cache files older than `maxAgeDays`.
 *
 * Safety Invariant: NEVER deletes project manifests, transcript JSONs, or subtitles.
 *
 * @param cacheDir Optional cache directory path to prune.
 * @param maxAgeDays Expiration age threshold in days (default: 7 days).
 */
export async function cleanExpiredCache(
  cacheDir?: string,
  maxAgeDays: number = 7,
): Promise<CacheCleanupResult> {
  try {
    const raw: unknown = await invoke('clean_expired_cache', {
      cacheDir: cacheDir !== undefined && cacheDir.trim().length > 0 ? cacheDir.trim() : null,
      maxAgeDays,
    });
    if (!isCacheCleanupResultJson(raw)) {
      throw {
        code: ERROR_CODES.STORAGE_CACHE_CLEANUP_FAILED,
        message: 'Dữ liệu phản hồi dọn dẹp bộ nhớ đệm không đúng định dạng',
        details: `Malformed payload from clean_expired_cache: ${safeSerialize(raw)}`,
        recoverable: true,
      } satisfies IpcErrorResponse;
    }
    return toCacheCleanupResult(raw);
  } catch (err: unknown) {
    throw parseIpcError(err, 'Không thể dọn dẹp bộ nhớ đệm');
  }
}

/**
 * Initialize root application storage directories (%LOCALAPPDATA%/NeuroClip/cache & projects).
 *
 * @param customCacheDir Optional custom cache directory.
 */
export async function initializeStorage(customCacheDir?: string): Promise<StoragePaths> {
  try {
    const raw: unknown = await invoke('initialize_storage', {
      customCacheDir: customCacheDir !== undefined && customCacheDir.trim().length > 0 ? customCacheDir.trim() : null,
    });
    if (!isStoragePathsJson(raw)) {
      throw {
        code: ERROR_CODES.STORAGE_DIRECTORY_CREATION_FAILED,
        message: 'Dữ liệu phản hồi khởi tạo thư mục lưu trữ không đúng định dạng',
        details: `Malformed payload from initialize_storage: ${safeSerialize(raw)}`,
        recoverable: true,
      } satisfies IpcErrorResponse;
    }
    return toStoragePaths(raw);
  } catch (err: unknown) {
    throw parseIpcError(err, 'Không thể khởi tạo thư mục lưu trữ');
  }
}

/**
 * Initialize a dedicated workspace directory for a specific project.
 *
 * @param projectId Project unique identifier (must be alphanumeric, hyphens, underscores).
 */
export async function initializeProject(projectId: string): Promise<string> {
  try {
    const raw: unknown = await invoke('initialize_project', {
      projectId,
    });
    if (typeof raw !== 'string' || raw.trim().length === 0) {
      throw {
        code: ERROR_CODES.STORAGE_DIRECTORY_CREATION_FAILED,
        message: 'Dữ liệu phản hồi khởi tạo thư mục dự án không đúng định dạng',
        details: `Malformed payload from initialize_project: ${safeSerialize(raw)}`,
        recoverable: true,
      } satisfies IpcErrorResponse;
    }
    return raw;
  } catch (err: unknown) {
    throw parseIpcError(err, 'Không thể khởi tạo thư mục dự án');
  }
}
