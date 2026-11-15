/**
 * NeuroClip Highlight Analytics & Chat Velocity Contracts
 *
 * Implements Schema 5.3 data models for chat velocity series, 5-second fixed binning,
 * adjusted Z-score algorithm with smoothing constant epsilon=0.5 and message floor=15.
 * Adheres strictly to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

// ---------------------------------------------------------------------------
// Constants & Algorithm Parameters
// ---------------------------------------------------------------------------

export const DEFAULT_BUCKET_WIDTH_SEC = 5.0;
export const DEFAULT_BASELINE_WINDOW_SEC = 60.0;
export const SMOOTHING_EPSILON = 0.5;
export const MESSAGE_FLOOR_THRESHOLD = 15;
export const SPAM_BOT_MAX_MESSAGES_PER_AUTHOR = 3;

// ---------------------------------------------------------------------------
// Wire JSON Types (Schema 5.3 Rust Serialization)
// ---------------------------------------------------------------------------

export interface ChatBucketJson {
  readonly index: number;
  readonly timestamp_sec: number;
  readonly raw_count: number;
  readonly effective_count: number;
}

export interface ChatVelocityPointJson {
  readonly timestamp_sec: number;
  readonly count: number;
  readonly z_score: number;
}

export interface ChatVelocitySeriesResponseJson {
  readonly vod_id?: string | undefined;
  readonly bucket_width_sec: number;
  readonly baseline_window_sec: number;
  readonly total_buckets: number;
  readonly points: readonly ChatVelocityPointJson[];
}

// ---------------------------------------------------------------------------
// Normalized Frontend Domain Models (Dual CamelCase & Snake_case Accessors)
// ---------------------------------------------------------------------------

export interface ChatBucket {
  readonly index: number;
  readonly timestampSec: number;
  readonly rawCount: number;
  readonly effectiveCount: number;

  // Snake_case aliases matching Rust JSON contract
  readonly timestamp_sec: number;
  readonly raw_count: number;
  readonly effective_count: number;
  readonly filtered_count: number;
}

export interface ChatVelocityPoint {
  readonly timestampSec: number;
  readonly count: number;
  readonly zScore: number;

  // Snake_case aliases matching Rust JSON contract
  readonly timestamp_sec: number;
  readonly z_score: number;
}

export interface ChatVelocitySeriesResponse {
  readonly vodId?: string | undefined;
  readonly bucketWidthSec: number;
  readonly baselineWindowSec: number;
  readonly totalBuckets: number;
  readonly points: readonly ChatVelocityPoint[];

  // Snake_case aliases
  readonly vod_id?: string | undefined;
  readonly bucket_width_sec: number;
  readonly baseline_window_sec: number;
  readonly total_buckets: number;
}

// ---------------------------------------------------------------------------
// Runtime Type Guards
// ---------------------------------------------------------------------------

export function isChatBucketJson(val: unknown): val is ChatBucketJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['index'] === 'number' &&
    Number.isInteger(obj['index']) &&
    obj['index'] >= 0 &&
    typeof obj['timestamp_sec'] === 'number' &&
    Number.isFinite(obj['timestamp_sec']) &&
    typeof obj['raw_count'] === 'number' &&
    Number.isInteger(obj['raw_count']) &&
    obj['raw_count'] >= 0 &&
    typeof obj['effective_count'] === 'number' &&
    Number.isInteger(obj['effective_count']) &&
    obj['effective_count'] >= 0
  );
}

export function isChatBucket(val: unknown): val is ChatBucket {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['index'] === 'number' &&
    typeof obj['timestampSec'] === 'number' &&
    typeof obj['rawCount'] === 'number' &&
    typeof obj['effectiveCount'] === 'number'
  );
}

export function isChatVelocityPointJson(val: unknown): val is ChatVelocityPointJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['timestamp_sec'] === 'number' &&
    Number.isFinite(obj['timestamp_sec']) &&
    typeof obj['count'] === 'number' &&
    Number.isInteger(obj['count']) &&
    obj['count'] >= 0 &&
    typeof obj['z_score'] === 'number' &&
    Number.isFinite(obj['z_score'])
  );
}

export function isChatVelocityPoint(val: unknown): val is ChatVelocityPoint {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['timestampSec'] === 'number' &&
    typeof obj['count'] === 'number' &&
    typeof obj['zScore'] === 'number'
  );
}

export function isChatVelocitySeriesResponseJson(
  val: unknown,
): val is ChatVelocitySeriesResponseJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    (obj['vod_id'] === undefined || typeof obj['vod_id'] === 'string') &&
    typeof obj['bucket_width_sec'] === 'number' &&
    Number.isFinite(obj['bucket_width_sec']) &&
    typeof obj['baseline_window_sec'] === 'number' &&
    Number.isFinite(obj['baseline_window_sec']) &&
    typeof obj['total_buckets'] === 'number' &&
    Number.isInteger(obj['total_buckets']) &&
    Array.isArray(obj['points']) &&
    obj['points'].every(isChatVelocityPointJson)
  );
}

// ---------------------------------------------------------------------------
// Model Normalizers
// ---------------------------------------------------------------------------

export function normalizeChatBucket(raw: ChatBucketJson): ChatBucket {
  return {
    index: raw.index,
    timestampSec: raw.timestamp_sec,
    rawCount: raw.raw_count,
    effectiveCount: raw.effective_count,
    timestamp_sec: raw.timestamp_sec,
    raw_count: raw.raw_count,
    effective_count: raw.effective_count,
    filtered_count: raw.effective_count,
  };
}

export function normalizeChatVelocityPoint(raw: ChatVelocityPointJson): ChatVelocityPoint {
  return {
    timestampSec: raw.timestamp_sec,
    count: raw.count,
    zScore: raw.z_score,
    timestamp_sec: raw.timestamp_sec,
    z_score: raw.z_score,
  };
}

// ---------------------------------------------------------------------------
// Pure Client-Side Math Helpers
// ---------------------------------------------------------------------------

/**
 * Pure client-side validation of the adjusted Z-Score formula:
 * If count < floorThreshold => 0.0
 * Else => (count - mean) / (stdDev + epsilon)
 */
export function calculateAdjustedZScore(
  count: number,
  mean: number,
  stdDev: number,
  epsilon: number = SMOOTHING_EPSILON,
  floorThreshold: number = MESSAGE_FLOOR_THRESHOLD,
): number {
  if (count < floorThreshold) {
    return 0.0;
  }
  const denom = stdDev + epsilon;
  if (denom <= 0 || !Number.isFinite(denom)) {
    return 0.0;
  }
  const z = (count - mean) / denom;
  return Number.isFinite(z) ? z : 0.0;
}
