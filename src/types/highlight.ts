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

// Audio RMS Peak Detector Constants (TASK-M4-11)
export const DEFAULT_RMS_BLOCK_DURATION_SEC = 0.5;
export const DEFAULT_AUDIO_SAMPLE_RATE = 16000;
export const DEFAULT_AUDIO_BASELINE_WINDOW_SEC = 30.0;
export const SILENCE_FLOOR_DBFS = -96.0;
export const SILENCE_FLOOR_AMPLITUDE = 1.5848931924611136e-5;
export const DEFAULT_PEAK_DB_SPIKE_THRESHOLD = 18.0;
export const DEFAULT_PEAK_SIGMA_THRESHOLD = 2.5;
export const EPSILON_RMS = 1e-6;

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

// ---------------------------------------------------------------------------
// Wire JSON Types (Schema 5.4 Audio RMS Series)
// ---------------------------------------------------------------------------

export interface AudioRmsPointJson {
  readonly timestamp_sec: number;
  readonly rms_energy: number;
  readonly is_peak: boolean;
  readonly dbfs?: number | undefined;
  readonly delta_db?: number | undefined;
  readonly z_score?: number | undefined;
}

export interface AudioRmsSeriesResponseJson {
  readonly vod_id?: string | undefined;
  readonly block_duration_sec: number;
  readonly baseline_window_sec: number;
  readonly total_blocks: number;
  readonly peak_count: number;
  readonly points: readonly AudioRmsPointJson[];
}

// ---------------------------------------------------------------------------
// Domain Models (Dual CamelCase & Snake_case Accessors)
// ---------------------------------------------------------------------------

export interface AudioRmsPoint {
  readonly timestampSec: number;
  readonly rmsEnergy: number;
  readonly isPeak: boolean;
  readonly dbfs?: number | undefined;
  readonly deltaDb?: number | undefined;
  readonly zScore?: number | undefined;

  // Snake_case aliases matching Rust JSON contract
  readonly timestamp_sec: number;
  readonly rms_energy: number;
  readonly is_peak: boolean;
  readonly delta_db?: number | undefined;
  readonly z_score?: number | undefined;
}

export interface AudioRmsSeriesResponse {
  readonly vodId?: string | undefined;
  readonly blockDurationSec: number;
  readonly baselineWindowSec: number;
  readonly totalBlocks: number;
  readonly peakCount: number;
  readonly points: readonly AudioRmsPoint[];

  // Snake_case aliases
  readonly vod_id?: string | undefined;
  readonly block_duration_sec: number;
  readonly baseline_window_sec: number;
  readonly total_blocks: number;
  readonly peak_count: number;
}

// ---------------------------------------------------------------------------
// Runtime Type Guards
// ---------------------------------------------------------------------------

export function isAudioRmsPointJson(val: unknown): val is AudioRmsPointJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['timestamp_sec'] === 'number' &&
    Number.isFinite(obj['timestamp_sec']) &&
    obj['timestamp_sec'] >= 0 &&
    typeof obj['rms_energy'] === 'number' &&
    Number.isFinite(obj['rms_energy']) &&
    obj['rms_energy'] >= 0.0 &&
    obj['rms_energy'] <= 1.0 &&
    typeof obj['is_peak'] === 'boolean' &&
    (obj['dbfs'] === undefined ||
      (typeof obj['dbfs'] === 'number' && Number.isFinite(obj['dbfs']))) &&
    (obj['delta_db'] === undefined ||
      (typeof obj['delta_db'] === 'number' && Number.isFinite(obj['delta_db']))) &&
    (obj['z_score'] === undefined ||
      (typeof obj['z_score'] === 'number' && Number.isFinite(obj['z_score'])))
  );
}

export function isAudioRmsPoint(val: unknown): val is AudioRmsPoint {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['timestampSec'] === 'number' &&
    typeof obj['rmsEnergy'] === 'number' &&
    typeof obj['isPeak'] === 'boolean' &&
    typeof obj['timestamp_sec'] === 'number' &&
    typeof obj['rms_energy'] === 'number' &&
    typeof obj['is_peak'] === 'boolean'
  );
}

export function isAudioRmsSeriesResponseJson(
  val: unknown,
): val is AudioRmsSeriesResponseJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    (obj['vod_id'] === undefined || typeof obj['vod_id'] === 'string') &&
    typeof obj['block_duration_sec'] === 'number' &&
    Number.isFinite(obj['block_duration_sec']) &&
    obj['block_duration_sec'] > 0 &&
    typeof obj['baseline_window_sec'] === 'number' &&
    Number.isFinite(obj['baseline_window_sec']) &&
    obj['baseline_window_sec'] > 0 &&
    typeof obj['total_blocks'] === 'number' &&
    Number.isInteger(obj['total_blocks']) &&
    obj['total_blocks'] >= 0 &&
    typeof obj['peak_count'] === 'number' &&
    Number.isInteger(obj['peak_count']) &&
    obj['peak_count'] >= 0 &&
    Array.isArray(obj['points']) &&
    obj['points'].every(isAudioRmsPointJson)
  );
}

// ---------------------------------------------------------------------------
// Model Normalizers
// ---------------------------------------------------------------------------

export function normalizeAudioRmsPoint(raw: AudioRmsPointJson): AudioRmsPoint {
  return {
    timestampSec: raw.timestamp_sec,
    rmsEnergy: raw.rms_energy,
    isPeak: raw.is_peak,
    dbfs: raw.dbfs,
    deltaDb: raw.delta_db,
    zScore: raw.z_score,

    timestamp_sec: raw.timestamp_sec,
    rms_energy: raw.rms_energy,
    is_peak: raw.is_peak,
    delta_db: raw.delta_db,
    z_score: raw.z_score,
  };
}

export function normalizeAudioRmsSeriesResponse(
  raw: AudioRmsSeriesResponseJson,
): AudioRmsSeriesResponse {
  const points = Object.freeze(raw.points.map(normalizeAudioRmsPoint));
  return {
    vodId: raw.vod_id,
    blockDurationSec: raw.block_duration_sec,
    baselineWindowSec: raw.baseline_window_sec,
    totalBlocks: raw.total_blocks,
    peakCount: raw.peak_count,
    points,

    vod_id: raw.vod_id,
    block_duration_sec: raw.block_duration_sec,
    baseline_window_sec: raw.baseline_window_sec,
    total_blocks: raw.total_blocks,
    peak_count: raw.peak_count,
  };
}

// ---------------------------------------------------------------------------
// Pure Client-Side Audio Math Helpers
// ---------------------------------------------------------------------------

/**
 * Pure client-side calculation of raw RMS from 16-bit signed PCM samples.
 * Normalizes samples by 32768.0 into [-1.0, 1.0].
 */
export function calculateAudioRms(samples: readonly number[]): number {
  if (samples.length === 0) {
    return 0.0;
  }
  let sumSq = 0;
  for (const sample of samples) {
    const normalized = sample / 32768.0;
    sumSq += normalized * normalized;
  }
  const meanSq = sumSq / samples.length;
  return meanSq > 0 ? Math.min(1.0, Math.sqrt(meanSq)) : 0.0;
}

/**
 * Pure client-side conversion of linear RMS amplitude to Decibels relative to Full Scale (dBFS).
 * Clamps to SILENCE_FLOOR_DBFS (-96.0 dBFS) for silence or non-finite inputs.
 */
export function rmsToDbfs(rawRms: number): number {
  if (!Number.isFinite(rawRms) || rawRms <= SILENCE_FLOOR_AMPLITUDE) {
    return SILENCE_FLOOR_DBFS;
  }
  return Math.max(SILENCE_FLOOR_DBFS, Math.min(0.0, 20.0 * Math.log10(rawRms)));
}

/**
 * Pure client-side conversion of dBFS to linear RMS amplitude.
 * Values <= SILENCE_FLOOR_DBFS (-96.0 dBFS) return 0.0.
 */
export function dbfsToRms(dbfs: number): number {
  if (!Number.isFinite(dbfs) || dbfs <= SILENCE_FLOOR_DBFS) {
    return 0.0;
  }
  return Math.max(0.0, Math.min(1.0, Math.pow(10.0, dbfs / 20.0)));
}

/**
 * Pure client-side linear normalization of dBFS in [-96.0, 0.0] into [0.0, 1.0].
 */
export function dbfsToNormalizedScale(dbfs: number): number {
  if (!Number.isFinite(dbfs) || dbfs <= SILENCE_FLOOR_DBFS) {
    return 0.0;
  }
  return Math.max(0.0, Math.min(1.0, (dbfs - SILENCE_FLOOR_DBFS) / (0.0 - SILENCE_FLOOR_DBFS)));
}

/**
 * Pure client-side dual-condition peak classification:
 * Relative jump >= 18dB (e.g. +21dB spike) OR statistical outlier >= 2.5 sigma -> is_peak = true.
 * Silence blocks cannot be peaks.
 */
export function isAudioRmsPeak(
  rms: number,
  baselineMeanRms: number,
  baselineStdDevRms: number,
  dbfs: number,
  baselineMeanDbfs: number,
  options?: { peakDbThreshold?: number; peakSigmaThreshold?: number },
): boolean {
  if (!Number.isFinite(rms) || rms <= SILENCE_FLOOR_AMPLITUDE || dbfs <= SILENCE_FLOOR_DBFS) {
    return false;
  }
  const dbThreshold = options?.peakDbThreshold ?? DEFAULT_PEAK_DB_SPIKE_THRESHOLD;
  const sigmaThreshold = options?.peakSigmaThreshold ?? DEFAULT_PEAK_SIGMA_THRESHOLD;

  const deltaDbFromMean = dbfs - baselineMeanDbfs;
  const deltaDbFromRatio =
    baselineMeanRms > SILENCE_FLOOR_AMPLITUDE && rms > 0
      ? 20.0 * Math.log10(rms / baselineMeanRms)
      : dbfs - SILENCE_FLOOR_DBFS;
  const deltaDb = Math.max(deltaDbFromMean, deltaDbFromRatio);

  const isDbPeak = deltaDb >= dbThreshold;
  const isSigmaPeak =
    baselineStdDevRms > EPSILON_RMS &&
    (rms - baselineMeanRms) >= sigmaThreshold * baselineStdDevRms;

  return isDbPeak || isSigmaPeak;
}
