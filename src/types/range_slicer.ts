/**
 * Data contracts, schemas, and type guards for TASK-M4-13 Lossless Precision Range Slicer.
 *
 * Enforces zero-any, strict null safety, and dual property access (camelCase & snake_case).
 * Complies with docs/CONVENTIONS.md and docs/ARCHITECTURE.md §4.2 & §5.2.
 */

// ============================================================================
// Constants & SLA Thresholds
// ============================================================================

/** Tolerance threshold in milliseconds for audio/video duration drift (ED-13.1). */
export const MAX_KEYFRAME_DRIFT_MS = 50.0;

/** Maximum allowable start presentation timestamp in seconds (AC-13.3). */
export const MAX_INITIAL_PTS_DELAY_SEC = 0.05;

/** Minimum free disk space in MB required for pre-flight check. */
export const MIN_SLICING_DISK_SPACE_MB = 500;

/** Standard 8-hour 1080p60 VOD reference size in MB (approx 30GB). */
export const FULL_VOD_REFERENCE_SIZE_MB = 30720;

// ============================================================================
// Wire Format: Request Payload
// ============================================================================

/**
 * Raw JSON request structure sent across Tauri IPC to `slice_video_segment`.
 */
export interface SliceVideoSegmentRequestJson {
  readonly vod_id: string;
  readonly start_sec: number;
  readonly end_sec: number;
  readonly cluster_id?: string | null;
  readonly output_filename?: string | null;
  readonly custom_input_source?: string | null;
}

/**
 * Domain model for slice video segment request supporting both camelCase and snake_case.
 */
export interface SliceVideoSegmentRequest {
  readonly vodId: string;
  readonly startSec: number;
  readonly endSec: number;
  readonly clusterId?: string | null | undefined;
  readonly outputFilename?: string | null | undefined;
  readonly customInputSource?: string | null | undefined;

  // Snake_case aliases matching Schema 5.2 wire specification
  readonly vod_id?: string | undefined;
  readonly start_sec?: number | undefined;
  readonly end_sec?: number | undefined;
  readonly cluster_id?: string | null | undefined;
  readonly output_filename?: string | null | undefined;
  readonly custom_input_source?: string | null | undefined;
}

// ============================================================================
// Wire Format: Response Payload
// ============================================================================

/**
 * Raw JSON result returned from Tauri IPC command `slice_video_segment`.
 * Accepts either snake_case (wire default per CONVENTIONS.md §4.2) or camelCase properties.
 */
export interface SliceResultJson {
  readonly clip_path?: string;
  readonly duration_seconds?: number;
  readonly file_size_bytes?: number;
  readonly video_duration?: number | null;
  readonly audio_duration?: number | null;
  readonly stream_drift_ms?: number;
  readonly keyframe_aligned?: boolean;

  readonly clipPath?: string;
  readonly durationSeconds?: number;
  readonly fileSizeBytes?: number;
  readonly videoDuration?: number | null;
  readonly audioDuration?: number | null;
  readonly streamDriftMs?: number;
  readonly keyframeAligned?: boolean;
}

/**
 * Domain model for sliced video segment result.
 */
export interface SliceResult {
  readonly clipPath: string;
  readonly durationSeconds: number;
  readonly fileSizeBytes: number;
  readonly videoDuration: number | null;
  readonly audioDuration: number | null;
  readonly streamDriftMs: number;
  readonly keyframeAligned: boolean;

  // Snake_case wire aliases
  readonly clip_path: string;
  readonly duration_seconds: number;
  readonly file_size_bytes: number;
  readonly video_duration: number | null;
  readonly audio_duration: number | null;
  readonly stream_drift_ms: number;
  readonly keyframe_aligned: boolean;

  // Derived helper properties
  readonly fileSizeMb: number;
  readonly durationDeltaMs: number;
}

export type SliceVideoSegmentResponse = SliceResult;
export type SliceVideoSegmentResponseJson = SliceResultJson;

// ============================================================================
// Type Guards
// ============================================================================

/**
 * Check if an unknown value satisfies `SliceResultJson`.
 * Accepts either snake_case or camelCase properties.
 */
export function isSliceResultJson(val: unknown): val is SliceResultJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  const hasClipPath = typeof obj['clip_path'] === 'string' || typeof obj['clipPath'] === 'string';
  const hasDuration =
    typeof obj['duration_seconds'] === 'number' || typeof obj['durationSeconds'] === 'number';
  const hasFileSize =
    typeof obj['file_size_bytes'] === 'number' || typeof obj['fileSizeBytes'] === 'number';
  const hasDrift =
    typeof obj['stream_drift_ms'] === 'number' || typeof obj['streamDriftMs'] === 'number';
  const hasKeyframe =
    typeof obj['keyframe_aligned'] === 'boolean' || typeof obj['keyframeAligned'] === 'boolean';

  return hasClipPath && hasDuration && hasFileSize && hasDrift && hasKeyframe;
}

/**
 * Check if an unknown value satisfies `SliceResult`.
 * Accepts either camelCase or snake_case properties.
 */
export function isSliceResult(val: unknown): val is SliceResult {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  const hasClipPath = typeof obj['clipPath'] === 'string' || typeof obj['clip_path'] === 'string';
  const hasDuration =
    typeof obj['durationSeconds'] === 'number' || typeof obj['duration_seconds'] === 'number';
  const hasFileSize =
    typeof obj['fileSizeBytes'] === 'number' || typeof obj['file_size_bytes'] === 'number';
  const hasDrift =
    typeof obj['streamDriftMs'] === 'number' || typeof obj['stream_drift_ms'] === 'number';
  const hasKeyframe =
    typeof obj['keyframeAligned'] === 'boolean' || typeof obj['keyframe_aligned'] === 'boolean';

  return hasClipPath && hasDuration && hasFileSize && hasDrift && hasKeyframe;
}

/**
 * Check if an unknown value satisfies `SliceVideoSegmentRequestJson`.
 */
export function isSliceVideoSegmentRequestJson(val: unknown): val is SliceVideoSegmentRequestJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['vod_id'] === 'string' &&
    typeof obj['start_sec'] === 'number' &&
    typeof obj['end_sec'] === 'number'
  );
}

// ============================================================================
// Normalizers & Serialization
// ============================================================================

/**
 * Normalize raw JSON `SliceResultJson` into domain model `SliceResult`.
 * Safely extracts fields whether in snake_case or camelCase without undefined exceptions.
 */
export function normalizeSliceResult(json: SliceResultJson): SliceResult {
  const obj = json as Record<string, unknown>;

  const clipPath = (
    typeof json.clip_path === 'string'
      ? json.clip_path
      : typeof obj['clipPath'] === 'string'
        ? (obj['clipPath'] as string)
        : ''
  ) as string;

  const durationSeconds = (
    typeof json.duration_seconds === 'number'
      ? json.duration_seconds
      : typeof obj['durationSeconds'] === 'number'
        ? (obj['durationSeconds'] as number)
        : 0
  ) as number;

  const fileSizeBytes = (
    typeof json.file_size_bytes === 'number'
      ? json.file_size_bytes
      : typeof obj['fileSizeBytes'] === 'number'
        ? (obj['fileSizeBytes'] as number)
        : 0
  ) as number;

  const rawVideo = json.video_duration !== undefined ? json.video_duration : obj['videoDuration'];
  const videoDuration = typeof rawVideo === 'number' ? rawVideo : null;

  const rawAudio = json.audio_duration !== undefined ? json.audio_duration : obj['audioDuration'];
  const audioDuration = typeof rawAudio === 'number' ? rawAudio : null;

  const rawDrift =
    typeof json.stream_drift_ms === 'number'
      ? json.stream_drift_ms
      : typeof obj['streamDriftMs'] === 'number'
        ? (obj['streamDriftMs'] as number)
        : 0;
  const driftMs = Number(
    (typeof rawDrift === 'number' && Number.isFinite(rawDrift) ? rawDrift : 0).toFixed(3),
  );

  const rawKeyframe =
    typeof json.keyframe_aligned === 'boolean'
      ? json.keyframe_aligned
      : typeof obj['keyframeAligned'] === 'boolean'
        ? (obj['keyframeAligned'] as boolean)
        : false;
  const keyframeAligned = Boolean(rawKeyframe);

  const fileSizeMb = Number((fileSizeBytes / (1024 * 1024)).toFixed(3));

  return {
    clipPath,
    durationSeconds,
    fileSizeBytes,
    videoDuration,
    audioDuration,
    streamDriftMs: driftMs,
    keyframeAligned,

    clip_path: clipPath,
    duration_seconds: durationSeconds,
    file_size_bytes: fileSizeBytes,
    video_duration: videoDuration,
    audio_duration: audioDuration,
    stream_drift_ms: driftMs,
    keyframe_aligned: keyframeAligned,

    fileSizeMb,
    durationDeltaMs: driftMs,
  };
}

/**
 * Convert domain `SliceResult` back to wire `SliceResultJson`.
 */
export function toSliceResultJson(domain: SliceResult): SliceResultJson {
  return {
    clip_path: domain.clipPath,
    duration_seconds: domain.durationSeconds,
    file_size_bytes: domain.fileSizeBytes,
    video_duration: domain.videoDuration,
    audio_duration: domain.audioDuration,
    stream_drift_ms: domain.streamDriftMs,
    keyframe_aligned: domain.keyframeAligned,

    clipPath: domain.clipPath,
    durationSeconds: domain.durationSeconds,
    fileSizeBytes: domain.fileSizeBytes,
    videoDuration: domain.videoDuration,
    audioDuration: domain.audioDuration,
    streamDriftMs: domain.streamDriftMs,
    keyframeAligned: domain.keyframeAligned,
  };
}

/**
 * Convert domain `SliceVideoSegmentRequest` to wire `SliceVideoSegmentRequestJson`.
 */
export function toSliceVideoSegmentRequestJson(
  req: SliceVideoSegmentRequest,
): SliceVideoSegmentRequestJson {
  return {
    vod_id: req.vodId ?? req.vod_id ?? '',
    start_sec: req.startSec ?? req.start_sec ?? 0,
    end_sec: req.endSec ?? req.end_sec ?? 0,
    cluster_id: req.clusterId ?? req.cluster_id ?? null,
    output_filename: req.outputFilename ?? req.output_filename ?? null,
    custom_input_source: req.customInputSource ?? req.custom_input_source ?? null,
  };
}

// ============================================================================
// Mathematical Invariant Helpers
// ============================================================================

/**
 * Calculate percentage of bandwidth saved compared to downloading full VOD (AC-13.2).
 *
 * @param clipSizeBytes Sliced segment file size in bytes.
 * @param fullVodSizeBytes Full VOD size in bytes (defaults to 30GB reference).
 * @returns Percentage savings (e.g. 99.77 for 70MB clip vs 30GB VOD).
 */
export function calculateBandwidthSavings(
  clipSizeBytes: number,
  fullVodSizeBytes: number = FULL_VOD_REFERENCE_SIZE_MB * 1024 * 1024,
): number {
  if (fullVodSizeBytes <= 0 || clipSizeBytes < 0) {
    return 0;
  }
  const ratio = 1.0 - clipSizeBytes / fullVodSizeBytes;
  return Number((Math.max(0, ratio) * 100).toFixed(2));
}

/**
 * Evaluate keyframe alignment based on drift in ms and optional video start PTS (ED-13.1).
 *
 * @param driftMs Absolute difference between video and audio duration in milliseconds.
 * @param videoStartTimeSec Optional start presentation timestamp in seconds.
 * @returns True if drift <= 50ms and start PTS <= 50ms.
 */
export function isKeyframeAligned(driftMs: number, videoStartTimeSec: number = 0.0): boolean {
  return driftMs <= MAX_KEYFRAME_DRIFT_MS && videoStartTimeSec <= MAX_INITIAL_PTS_DELAY_SEC;
}
