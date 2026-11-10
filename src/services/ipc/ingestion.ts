/**
 * NeuroClip Audio-First Ingestion & VOD Stream IPC Client
 *
 * Provides typed interfaces, runtime type guards, and Tauri IPC invoke wrappers
 * for probing VOD metadata, anti-bot cookie passing, fast live rejection, and
 * audio extraction streaming.
 * Strict adherence to docs/CONVENTIONS.md (zero-any, exactOptionalPropertyTypes).
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  ERROR_CODES,
  parseIpcError,
  type IpcErrorResponse,
} from './error.ts';

// ---------------------------------------------------------------------------
// Data Contracts: VOD Manifest (Schema 5.2)
// ---------------------------------------------------------------------------

/**
 * Wire JSON format matching Schema 5.2 defined in docs/ARCHITECTURE.md.
 */
export interface VodManifestJson {
  readonly vod_id: string;
  readonly source_type: 'twitch_vod' | 'youtube_vod' | 'local_file';
  readonly original_url?: string | null;
  readonly title: string;
  readonly streamer?: string | null;
  readonly duration_seconds: number;
  readonly audio_path: string;
  readonly chat_file_path?: string | null;
  readonly video_stream_url?: string | null;
  readonly local_video_path?: string | null;
  readonly created_at: string;
}

/**
 * Normalized Frontend Domain Model for VOD Manifest.
 * Supports both camelCase and snake_case property access.
 */
export interface VodManifest {
  readonly vodId: string;
  readonly sourceType: 'twitch_vod' | 'youtube_vod' | 'local_file';
  readonly originalUrl: string | null;
  readonly title: string;
  readonly streamer: string | null;
  readonly durationSeconds: number;
  readonly audioPath: string;
  readonly chatFilePath: string | null;
  readonly videoStreamUrl: string | null;
  readonly localVideoPath: string | null;
  readonly createdAt: string;

  // Snake_case aliases matching Schema 5.2 wire specification
  readonly vod_id: string;
  readonly source_type: 'twitch_vod' | 'youtube_vod' | 'local_file';
  readonly original_url: string | null;
  readonly duration_seconds: number;
  readonly audio_path: string;
  readonly chat_file_path: string | null;
  readonly video_stream_url: string | null;
  readonly local_video_path: string | null;
  readonly created_at: string;
}

/**
 * Type guard validating raw wire `VodManifestJson` payloads.
 */
export function isVodManifestJson(val: unknown): val is VodManifestJson {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;

  const hasVodId = typeof obj['vod_id'] === 'string' && obj['vod_id'].trim().length > 0;
  const validSourceType =
    obj['source_type'] === 'twitch_vod' ||
    obj['source_type'] === 'youtube_vod' ||
    obj['source_type'] === 'local_file';
  const hasTitle = typeof obj['title'] === 'string';
  const hasDuration = typeof obj['duration_seconds'] === 'number' && !Number.isNaN(obj['duration_seconds']);
  const hasAudioPath = typeof obj['audio_path'] === 'string';
  const hasCreatedAt = typeof obj['created_at'] === 'string';

  return hasVodId && validSourceType && hasTitle && hasDuration && hasAudioPath && hasCreatedAt;
}

/**
 * Convert wire JSON or domain manifest to normalized VodManifest.
 */
export function toVodManifest(raw: VodManifestJson | VodManifest): VodManifest {
  const vodId = 'vodId' in raw ? raw.vodId : raw.vod_id;
  const sourceType = 'sourceType' in raw ? raw.sourceType : raw.source_type;
  const originalUrl = 'originalUrl' in raw ? raw.originalUrl : (raw.original_url ?? null);
  const title = raw.title;
  const streamer = raw.streamer ?? null;
  const durationSeconds = 'durationSeconds' in raw ? raw.durationSeconds : raw.duration_seconds;
  const audioPath = 'audioPath' in raw ? raw.audioPath : raw.audio_path;
  const chatFilePath = 'chatFilePath' in raw ? raw.chatFilePath : (raw.chat_file_path ?? null);
  const videoStreamUrl = 'videoStreamUrl' in raw ? raw.videoStreamUrl : (raw.video_stream_url ?? null);
  const localVideoPath = 'localVideoPath' in raw ? raw.localVideoPath : (raw.local_video_path ?? null);
  const createdAt = 'createdAt' in raw ? raw.createdAt : raw.created_at;

  return {
    vodId,
    sourceType,
    originalUrl,
    title,
    streamer,
    durationSeconds,
    audioPath,
    chatFilePath,
    videoStreamUrl,
    localVideoPath,
    createdAt,
    vod_id: vodId,
    source_type: sourceType,
    original_url: originalUrl,
    duration_seconds: durationSeconds,
    audio_path: audioPath,
    chat_file_path: chatFilePath,
    video_stream_url: videoStreamUrl,
    local_video_path: localVideoPath,
    created_at: createdAt,
  };
}

// ---------------------------------------------------------------------------
// Data Contracts: VOD Metadata Probe
// ---------------------------------------------------------------------------

export interface VodMetadataJson {
  readonly id: string;
  readonly title: string;
  readonly uploader?: string | null;
  readonly channel?: string | null;
  readonly duration?: number | null;
  readonly is_live?: boolean | null;
  readonly live_status?: string | null;
}

export interface VodMetadata {
  readonly id: string;
  readonly title: string;
  readonly uploader: string | null;
  readonly duration: number | null;
  readonly isLive: boolean;
}

export function isVodMetadataJson(val: unknown): val is VodMetadataJson {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  return typeof obj['id'] === 'string' && typeof obj['title'] === 'string';
}

export function toVodMetadata(raw: VodMetadataJson): VodMetadata {
  const isLive = Boolean(raw.is_live ?? (raw.live_status === 'is_live'));
  return {
    id: raw.id,
    title: raw.title,
    uploader: raw.uploader ?? raw.channel ?? null,
    duration: raw.duration ?? null,
    isLive,
  };
}

// ---------------------------------------------------------------------------
// Data Contracts: Ingestion Options & Progress Events
// ---------------------------------------------------------------------------

export type BrowserCookieType = 'chrome' | 'firefox' | 'edge';

export interface IngestOptions {
  readonly audioOnly?: boolean;
  readonly cookiesPath?: string | null;
  readonly browserCookies?: BrowserCookieType | null;
  readonly customUserAgent?: string | null;
  readonly customCacheDir?: string | null;
  readonly taskId?: string | null;
}

export interface IngestProgressEvent {
  readonly taskId: string;
  readonly stage: string;
  readonly percent: number;
  readonly speedKbps: number;
  readonly etaSec: number;
}

export function isIngestProgressEvent(val: unknown): val is IngestProgressEvent {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['taskId'] === 'string' &&
    typeof obj['stage'] === 'string' &&
    typeof obj['percent'] === 'number' &&
    typeof obj['speedKbps'] === 'number' &&
    typeof obj['etaSec'] === 'number'
  );
}

// ---------------------------------------------------------------------------
// Helper serialization
// ---------------------------------------------------------------------------

function safeSerialize(val: unknown): string {
  try {
    return JSON.stringify(val);
  } catch {
    return String(val);
  }
}

// ---------------------------------------------------------------------------
// IPC Invocation API
// ---------------------------------------------------------------------------

/**
 * Probe metadata for an online VOD (duration, streamer, live status) without downloading.
 *
 * Fast rejection (AC-06.5): Livestreams throw ERR_INGEST_LIVE_STREAM_UNSUPPORTED in <= 500ms.
 */
export async function probeVod(
  url: string,
  cookiesPath?: string | null,
  browserCookies?: BrowserCookieType | null,
): Promise<VodMetadata> {
  try {
    const raw: unknown = await invoke('probe_vod', {
      url: url.trim(),
      cookiesPath: cookiesPath ?? null,
      browserCookies: browserCookies ?? null,
    });
    if (!isVodMetadataJson(raw)) {
      throw {
        code: ERROR_CODES.INGEST_EXTRACTION_FAILED,
        message: 'Dữ liệu phản hồi kiểm tra VOD không đúng định dạng',
        details: `Malformed payload from probe_vod: ${safeSerialize(raw)}`,
        recoverable: true,
      } satisfies IpcErrorResponse;
    }
    return toVodMetadata(raw);
  } catch (err: unknown) {
    throw parseIpcError(err, 'Không thể kiểm tra thông tin VOD');
  }
}

/**
 * Start full Audio-First VOD ingestion pipeline.
 *
 * Enforces pre-flight storage check (>= 5GB) and downloads ONLY the audio stream
 * saving >= 98.5% bandwidth/disk space (AC-06.1, AC-06.2).
 */
export async function startVodIngestion(
  url: string,
  options?: IngestOptions,
): Promise<VodManifest> {
  try {
    const raw: unknown = await invoke('start_vod_ingestion', {
      url: url.trim(),
      options: options ? {
        audioOnly: options.audioOnly ?? true,
        cookiesPath: options.cookiesPath ?? null,
        browserCookies: options.browserCookies ?? null,
        customUserAgent: options.customUserAgent ?? null,
        customCacheDir: options.customCacheDir ?? null,
        taskId: options.taskId ?? null,
      } : null,
    });
    if (!isVodManifestJson(raw)) {
      throw {
        code: ERROR_CODES.INGEST_EXTRACTION_FAILED,
        message: 'Dữ liệu phản hồi manifest VOD không đúng định dạng Schema 5.2',
        details: `Malformed payload from start_vod_ingestion: ${safeSerialize(raw)}`,
        recoverable: true,
      } satisfies IpcErrorResponse;
    }
    return toVodManifest(raw);
  } catch (err: unknown) {
    throw parseIpcError(err, 'Không thể thu nạp luồng âm thanh VOD');
  }
}

/**
 * Cancel an active VOD ingestion task by its ID.
 */
export async function cancelVodIngestion(taskId: string): Promise<boolean> {
  try {
    const res: unknown = await invoke('cancel_vod_ingestion', { taskId });
    return typeof res === 'boolean' ? res : true;
  } catch (err: unknown) {
    throw parseIpcError(err, 'Không thể hủy tác vụ thu nạp');
  }
}

/**
 * Retrieve the Schema 5.2 `vod_manifest.json` for an existing project.
 */
export async function getVodManifest(projectId: string): Promise<VodManifest> {
  try {
    const raw: unknown = await invoke('get_vod_manifest', { projectId });
    if (!isVodManifestJson(raw)) {
      throw {
        code: ERROR_CODES.STORAGE_FILE_NOT_FOUND,
        message: 'Tệp vod_manifest.json không đúng định dạng Schema 5.2',
        details: `Malformed payload from get_vod_manifest: ${safeSerialize(raw)}`,
        recoverable: true,
      } satisfies IpcErrorResponse;
    }
    return toVodManifest(raw);
  } catch (err: unknown) {
    throw parseIpcError(err, 'Không thể đọc vod_manifest.json của dự án');
  }
}

/**
 * Listen for real-time `ingest:progress` events emitted from the Rust backend.
 *
 * @param callback Handler receiving progress telemetry (percent, speed, ETA).
 * @returns Unlisten function to unsubscribe.
 */
export async function listenIngestProgress(
  callback: (event: IngestProgressEvent) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<IngestProgressEvent>('ingest:progress', (event) => {
      if (isIngestProgressEvent(event.payload)) {
        callback(event.payload);
      }
    });
  } catch {
    // Fallback unlisten stub for non-Tauri / mock environments
    return () => {};
  }
}

// ---------------------------------------------------------------------------
// Data Contracts: Audio Energy Sliding Window
// ---------------------------------------------------------------------------

export interface AudioEnergyWindowJson {
  readonly window_index: number;
  readonly timestamp_sec: number;
  readonly rms_energy: number;
  readonly peak_amplitude: number;
}

export interface AudioEnergyWindow {
  readonly windowIndex: number;
  readonly timestampSec: number;
  readonly rmsEnergy: number;
  readonly peakAmplitude: number;
}

export function isAudioEnergyWindowJson(val: unknown): val is AudioEnergyWindowJson {
  if (typeof val !== 'object' || val === null) return false;
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['window_index'] === 'number' &&
    typeof obj['timestamp_sec'] === 'number' &&
    typeof obj['rms_energy'] === 'number' &&
    typeof obj['peak_amplitude'] === 'number'
  );
}

/**
 * Stream audio from input source through FFmpeg 16kHz mono pipe and process
 * samples via AudioRingBuffer sliding window to compute continuous RMS energy telemetry.
 *
 * Complies with docs/ARCHITECTURE.md §4.3 and TASK-M4-06.
 */
export async function extractPcmStream(
  sourcePath: string,
  windowSamplesOrTaskId?: number | string | null,
  maxWindows?: number | null,
  taskId?: string | null,
): Promise<AudioEnergyWindow[]> {
  try {
    let actualWindowSamples: number | null = null;
    let actualTaskId: string | null = taskId ?? null;

    if (typeof windowSamplesOrTaskId === 'number') {
      actualWindowSamples = windowSamplesOrTaskId;
    } else if (typeof windowSamplesOrTaskId === 'string') {
      actualTaskId = windowSamplesOrTaskId;
    }

    const raw: unknown = await invoke('extract_pcm_stream', {
      sourcePath: sourcePath.trim(),
      windowSamples: actualWindowSamples,
      maxWindows: maxWindows ?? null,
      taskId: actualTaskId,
    });
    if (!Array.isArray(raw)) {
      throw {
        code: ERROR_CODES.INGEST_EXTRACTION_FAILED,
        message: 'Dữ liệu phản hồi trích xuất PCM không đúng định dạng',
        details: `Expected array from extract_pcm_stream, got: ${safeSerialize(raw)}`,
        recoverable: true,
      } satisfies IpcErrorResponse;
    }
    return raw.map((item) => {
      const record = item as Record<string, number>;
      return {
        windowIndex: record['windowIndex'] ?? record['window_index'] ?? 0,
        timestampSec: record['timestampSec'] ?? record['timestamp_sec'] ?? 0,
        rmsEnergy: record['rmsEnergy'] ?? record['rms_energy'] ?? 0,
        peakAmplitude: record['peakAmplitude'] ?? record['peak_amplitude'] ?? 0,
      };
    });
  } catch (err: unknown) {
    throw parseIpcError(err, 'Không thể trích xuất luồng PCM');
  }
}

