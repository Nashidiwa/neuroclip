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
export const MAX_TOP_EMOTES_PER_BUCKET = 5;

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

export interface EmoteDensityPointJson {
  readonly timestamp_sec: number;
  readonly top_emotes: readonly string[];
  readonly e_ratio: number;
  readonly total_tokens?: number | undefined;
  readonly emote_count?: number | undefined;
  readonly swarm_count?: number | undefined;
  readonly weighted_score?: number | undefined;
}

export interface EmoteDensitySeriesResponseJson {
  readonly vod_id?: string | undefined;
  readonly bucket_width_sec: number;
  readonly total_buckets: number;
  readonly points: readonly EmoteDensityPointJson[];
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

export interface EmoteDensityPoint {
  readonly timestampSec: number;
  readonly topEmotes: readonly string[];
  readonly eRatio: number;
  readonly totalTokens: number;
  readonly emoteCount: number;
  readonly swarmCount: number;
  readonly weightedScore: number;

  // Snake_case aliases matching Rust JSON contract
  readonly timestamp_sec: number;
  readonly top_emotes: readonly string[];
  readonly e_ratio: number;
  readonly total_tokens: number;
  readonly emote_count: number;
  readonly swarm_count: number;
  readonly weighted_score: number;
}

export interface EmoteDensitySeriesResponse {
  readonly vodId?: string | undefined;
  readonly bucketWidthSec: number;
  readonly totalBuckets: number;
  readonly points: readonly EmoteDensityPoint[];

  // Snake_case aliases
  readonly vod_id?: string | undefined;
  readonly bucket_width_sec: number;
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

export function isEmoteDensityPointJson(val: unknown): val is EmoteDensityPointJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['timestamp_sec'] === 'number' &&
    Number.isFinite(obj['timestamp_sec']) &&
    Array.isArray(obj['top_emotes']) &&
    obj['top_emotes'].every((item) => typeof item === 'string') &&
    typeof obj['e_ratio'] === 'number' &&
    Number.isFinite(obj['e_ratio']) &&
    obj['e_ratio'] >= 0.0 &&
    obj['e_ratio'] <= 1.0 &&
    (obj['total_tokens'] === undefined ||
      (typeof obj['total_tokens'] === 'number' && Number.isInteger(obj['total_tokens']) && obj['total_tokens'] >= 0)) &&
    (obj['emote_count'] === undefined ||
      (typeof obj['emote_count'] === 'number' && Number.isInteger(obj['emote_count']) && obj['emote_count'] >= 0)) &&
    (obj['weighted_score'] === undefined ||
      (typeof obj['weighted_score'] === 'number' && Number.isFinite(obj['weighted_score']) && obj['weighted_score'] >= 0))
  );
}

export function isEmoteDensityPoint(val: unknown): val is EmoteDensityPoint {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['timestampSec'] === 'number' &&
    Array.isArray(obj['topEmotes']) &&
    typeof obj['eRatio'] === 'number' &&
    typeof obj['totalTokens'] === 'number' &&
    typeof obj['emoteCount'] === 'number' &&
    typeof obj['weightedScore'] === 'number' &&
    typeof obj['timestamp_sec'] === 'number' &&
    Array.isArray(obj['top_emotes']) &&
    typeof obj['e_ratio'] === 'number' &&
    typeof obj['total_tokens'] === 'number' &&
    typeof obj['emote_count'] === 'number' &&
    typeof obj['weighted_score'] === 'number'
  );
}

export function normalizeEmoteDensityPoint(raw: EmoteDensityPointJson): EmoteDensityPoint {
  const count = raw.emote_count ?? raw.swarm_count ?? 0;
  const tokens = raw.total_tokens ?? 0;
  const weighted = raw.weighted_score ?? 0.0;
  return {
    timestampSec: raw.timestamp_sec,
    topEmotes: Object.freeze([...raw.top_emotes]),
    eRatio: raw.e_ratio,
    totalTokens: tokens,
    emoteCount: count,
    swarmCount: count,
    weightedScore: weighted,

    timestamp_sec: raw.timestamp_sec,
    top_emotes: Object.freeze([...raw.top_emotes]),
    e_ratio: raw.e_ratio,
    total_tokens: tokens,
    emote_count: count,
    swarm_count: count,
    weighted_score: weighted,
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

/**
 * Pure client-side validation of unweighted Emote Swarm Ratio:
 * If totalTokens <= 0 => 0.0
 * Else => clamp(emoteCount / totalTokens, 0.0, 1.0)
 */
export function calculateEmoteRatio(emoteCount: number, totalTokens: number): number {
  if (!Number.isFinite(totalTokens) || totalTokens <= 0) {
    return 0.0;
  }
  if (!Number.isFinite(emoteCount) || emoteCount <= 0) {
    return 0.0;
  }
  const ratio = emoteCount / totalTokens;
  return Math.min(1.0, Math.max(0.0, Number.isFinite(ratio) ? ratio : 0.0));
}
