/**
 * NeuroClip Managed Sidecars Auto-Updater & Anti-Bot IPC Client (`TASK-M4-29`).
 *
 * Implements typed interfaces, runtime type guards, and IPC wrappers for:
 * - `check_sidecar_updates`
 * - `download_and_apply_sidecar_update`
 * - `get_sidecar_status`
 * - `get_anti_bot_guidance`
 * - `trigger_bot_defense_update`
 * - `sidecar:status` event streaming
 *
 * Complies with docs/CONVENTIONS.md (zero-any, exactOptionalPropertyTypes).
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { parseIpcError } from './error.ts';

export const SIDECAR_STATUS_EVENT = 'sidecar:status' as const;

export type SidecarToolType = 'yt-dlp' | 'ffmpeg';
export type SidecarUpdateStatus = 'ready' | 'updating' | 'failed';

/**
 * Event payload emitted on `sidecar:status` channel (`CONVENTIONS.md` §4.2).
 */
export interface SidecarStatusEventPayload {
  readonly binary: SidecarToolType | string;
  readonly status: SidecarUpdateStatus;
  readonly message?: string | undefined;
}

/**
 * Release check metadata returned by `check_sidecar_updates`.
 */
export interface ReleaseCheckInfo {
  readonly tool: SidecarToolType | string;
  readonly current_version: string | null;
  readonly currentVersion?: string | null | undefined;
  readonly latest_version: string;
  readonly latestVersion?: string | undefined;
  readonly update_available: boolean;
  readonly updateAvailable?: boolean | undefined;
  readonly binary_download_url: string;
  readonly binaryDownloadUrl?: string | undefined;
  readonly checksum_download_url: string;
  readonly checksumDownloadUrl?: string | undefined;
  readonly expected_sha256: string;
  readonly expectedSha256?: string | undefined;
  readonly binary_size: number;
  readonly binarySize?: number | undefined;
  readonly published_at?: string | null | undefined;
  readonly publishedAt?: string | null | undefined;
  readonly release_notes?: string | null | undefined;
  readonly releaseNotes?: string | null | undefined;
}

/**
 * Update application report returned by `download_and_apply_sidecar_update`.
 */
export interface SidecarUpdateReport {
  readonly tool: SidecarToolType | string;
  readonly previous_version: string | null;
  readonly previousVersion?: string | null | undefined;
  readonly updated_version: string;
  readonly updatedVersion?: string | undefined;
  readonly installed_path: string;
  readonly installedPath?: string | undefined;
  readonly sha256_hash: string;
  readonly sha256Hash?: string | undefined;
  readonly success: boolean;
  readonly message: string;
}

/**
 * Sidecar inventory item returned by `get_sidecar_status`.
 */
export interface SidecarStatusReport {
  readonly tool: SidecarToolType | string;
  readonly tool_name: string;
  readonly toolName?: string | undefined;
  readonly binary_path: string;
  readonly binaryPath?: string | undefined;
  readonly exists: boolean;
  readonly version: string | null;
  readonly sha256_hash?: string | null | undefined;
  readonly sha256Hash?: string | null | undefined;
  readonly last_checked_at?: string | null | undefined;
  readonly lastCheckedAt?: string | null | undefined;
}

/**
 * Multi-layer anti-bot guidance returned by `get_anti_bot_guidance` and `trigger_bot_defense_update`.
 */
export interface AntiBotGuidance {
  readonly tool: SidecarToolType | string;
  readonly current_version: string | null;
  readonly currentVersion?: string | null | undefined;
  readonly is_latest: boolean;
  readonly isLatest?: boolean | undefined;
  readonly rotated_user_agent: string;
  readonly rotatedUserAgent?: string | undefined;
  readonly supported_browsers: readonly string[];
  readonly supportedBrowsers?: readonly string[] | undefined;
  readonly recommended_cookie_sources: readonly string[];
  readonly recommendedCookieSources?: readonly string[] | undefined;
  readonly cli_flags_example: readonly string[];
  readonly cliFlagsExample?: readonly string[] | undefined;
  readonly instructions_vi: string;
  readonly instructionsVi?: string | undefined;
  readonly bot_blocked_details?: string | null | undefined;
  readonly botBlockedDetails?: string | null | undefined;
}

/**
 * Runtime type guard for `SidecarStatusEventPayload`.
 */
export function isSidecarStatusEventPayload(val: unknown): val is SidecarStatusEventPayload {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  const hasBinary = typeof obj['binary'] === 'string' && obj['binary'].trim().length > 0;
  const hasStatus =
    obj['status'] === 'ready' || obj['status'] === 'updating' || obj['status'] === 'failed';
  const validMessage =
    obj['message'] === undefined || obj['message'] === null || typeof obj['message'] === 'string';

  return hasBinary && hasStatus && validMessage;
}

/**
 * Query for sidecar updates via GitHub Releases API.
 */
export async function checkSidecarUpdates(
  toolName?: string,
  force?: boolean,
): Promise<ReleaseCheckInfo[]> {
  try {
    return await invoke<ReleaseCheckInfo[]>('check_sidecar_updates', {
      toolName: toolName ?? null,
      force: force ?? null,
    });
  } catch (err: unknown) {
    throw parseIpcError(err, 'Lỗi kiểm tra bản cập nhật công cụ phụ trợ');
  }
}

/**
 * Download, verify SHA-256 hash, and atomically apply sidecar update in place.
 */
export async function downloadAndApplySidecarUpdate(
  toolName?: string,
): Promise<SidecarUpdateReport> {
  try {
    return await invoke<SidecarUpdateReport>('download_and_apply_sidecar_update', {
      toolName: toolName ?? null,
    });
  } catch (err: unknown) {
    throw parseIpcError(err, 'Lỗi tải và cài đặt bản cập nhật công cụ phụ trợ');
  }
}

/**
 * Query sidecar inventory status and filesystem presence.
 */
export async function getSidecarStatus(toolName?: string): Promise<SidecarStatusReport[]> {
  try {
    return await invoke<SidecarStatusReport[]>('get_sidecar_status', {
      toolName: toolName ?? null,
    });
  } catch (err: unknown) {
    throw parseIpcError(err, 'Lỗi kiểm tra trạng thái công cụ phụ trợ');
  }
}

/**
 * Query multi-layer anti-bot guidance for resolving YouTube bot challenge.
 */
export async function getAntiBotGuidance(errorDetails?: string): Promise<AntiBotGuidance> {
  try {
    return await invoke<AntiBotGuidance>('get_anti_bot_guidance', {
      errorDetails: errorDetails ?? null,
    });
  } catch (err: unknown) {
    throw parseIpcError(err, 'Lỗi tạo hướng dẫn xử lý xác minh bot');
  }
}

/**
 * Trigger defensive sidecar update upon receiving `ERR_INGEST_BOT_BLOCKED`.
 */
export async function triggerBotDefenseUpdate(errorDetails?: string): Promise<AntiBotGuidance> {
  try {
    return await invoke<AntiBotGuidance>('trigger_bot_defense_update', {
      errorDetails: errorDetails ?? null,
    });
  } catch (err: unknown) {
    throw parseIpcError(err, 'Lỗi kích hoạt nâng cấp phòng thủ bot');
  }
}

/**
 * Subscribe to real-time `sidecar:status` updates emitted by Rust.
 */
export async function listenSidecarStatus(
  callback: (event: SidecarStatusEventPayload) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<unknown>(SIDECAR_STATUS_EVENT, (event) => {
      if (isSidecarStatusEventPayload(event.payload)) {
        callback(event.payload);
      }
    });
  } catch {
    return () => {};
  }
}
