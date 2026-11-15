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


// ===========================================================================
// TASK-M4-12: Multimodal Fusion & Highlight Clustering Types (Schema 5.4)
// ===========================================================================

export const STREAM_LATENCY_COMPENSATION_SEC = 4.5;
export const DEFAULT_PRE_ROLL_BUFFER_SEC = 10.0;
export const DEFAULT_POST_ROLL_BUFFER_SEC = 15.0;
export const MIN_PRE_ROLL_OFFSET_SEC = 14.5; // 4.5s latency + 10.0s pre-roll
export const MAX_PRE_ROLL_BUFFER_SEC = 20.0; // Dynamic padding limit
export const MIN_CLUSTER_DURATION_SEC = 15.0;
export const CLUSTER_MERGE_DISTANCE_SEC = 30.0;
export const MAX_CLUSTER_OVERLAP_RATIO = 0.50; // > 50% overlap triggers NMS
export const FUSION_WEIGHT_Z_SCORE = 0.60;
export const FUSION_WEIGHT_EMOTE = 0.25;
export const FUSION_WEIGHT_RMS = 0.15;
export const FUSION_AUDIO_ONLY_WEIGHT_RMS = 1.0;
export const DEFAULT_TOP_CLUSTERS_LIMIT = 20;

/**
 * Wire JSON representation of a highlight cluster conforming strictly to
 * docs/ARCHITECTURE.md §5.4 Schema 5.4 Draft-07.
 */
export interface HighlightClusterJson {
  readonly cluster_id: string;
  readonly start_sec: number;
  readonly end_sec: number;
  readonly peak_sec: number;
  readonly score: number;
  readonly primary_reason: string;
  readonly chat_z_score?: number | null | undefined;
  readonly audio_rms_peak?: number | null | undefined;
  readonly top_emotes?: readonly string[] | undefined;
  readonly is_bookmarked?: boolean | undefined;
}

/**
 * Container object conforming to Schema 5.4 document format on disk and IPC wire.
 */
export interface HighlightClustersContainerJson {
  readonly vod_id: string;
  readonly clusters: readonly HighlightClusterJson[];
  readonly total_clusters?: number | undefined;
  readonly generated_at?: string | undefined;
}

/**
 * Domain Model for HighlightCluster with dual camelCase & snake_case accessors.
 */
export interface HighlightCluster {
  readonly clusterId: string;
  readonly startSec: number;
  readonly endSec: number;
  readonly peakSec: number;
  readonly score: number; // 0.0 - 100.0
  readonly primaryReason: string;
  readonly chatZScore?: number | null | undefined;
  readonly audioRmsPeak?: number | null | undefined;
  readonly topEmotes: readonly string[];
  readonly isBookmarked: boolean;

  // Snake_case aliases matching Rust JSON contract
  readonly cluster_id: string;
  readonly start_sec: number;
  readonly end_sec: number;
  readonly peak_sec: number;
  readonly primary_reason: string;
  readonly chat_z_score?: number | null | undefined;
  readonly audio_rms_peak?: number | null | undefined;
  readonly top_emotes: readonly string[];
  readonly is_bookmarked: boolean;
}

/**
 * Domain Model for HighlightClustersContainer.
 */
export interface HighlightClustersContainer {
  readonly vodId: string;
  readonly clusters: readonly HighlightCluster[];
  readonly totalClusters?: number | undefined;
  readonly generatedAt?: string | undefined;

  // Snake_case aliases
  readonly vod_id: string;
  readonly total_clusters?: number | undefined;
  readonly generated_at?: string | undefined;
}

// ---------------------------------------------------------------------------
// Runtime Type Guards
// ---------------------------------------------------------------------------

export function isHighlightClusterJson(val: unknown): val is HighlightClusterJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['cluster_id'] === 'string' &&
    obj['cluster_id'].length > 0 &&
    typeof obj['start_sec'] === 'number' &&
    Number.isFinite(obj['start_sec']) &&
    obj['start_sec'] >= 0 &&
    typeof obj['end_sec'] === 'number' &&
    Number.isFinite(obj['end_sec']) &&
    obj['end_sec'] >= obj['start_sec'] &&
    typeof obj['peak_sec'] === 'number' &&
    Number.isFinite(obj['peak_sec']) &&
    obj['peak_sec'] >= 0 &&
    typeof obj['score'] === 'number' &&
    Number.isFinite(obj['score']) &&
    obj['score'] >= 0 &&
    obj['score'] <= 100 &&
    typeof obj['primary_reason'] === 'string' &&
    (obj['chat_z_score'] === undefined ||
      obj['chat_z_score'] === null ||
      (typeof obj['chat_z_score'] === 'number' && Number.isFinite(obj['chat_z_score']))) &&
    (obj['audio_rms_peak'] === undefined ||
      obj['audio_rms_peak'] === null ||
      (typeof obj['audio_rms_peak'] === 'number' && Number.isFinite(obj['audio_rms_peak']))) &&
    (obj['top_emotes'] === undefined ||
      (Array.isArray(obj['top_emotes']) &&
        obj['top_emotes'].every((item) => typeof item === 'string'))) &&
    (obj['is_bookmarked'] === undefined || typeof obj['is_bookmarked'] === 'boolean')
  );
}

export function isHighlightCluster(val: unknown): val is HighlightCluster {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['clusterId'] === 'string' &&
    typeof obj['startSec'] === 'number' &&
    typeof obj['endSec'] === 'number' &&
    typeof obj['peakSec'] === 'number' &&
    typeof obj['score'] === 'number' &&
    typeof obj['primaryReason'] === 'string' &&
    Array.isArray(obj['topEmotes']) &&
    typeof obj['isBookmarked'] === 'boolean'
  );
}

export function isHighlightClustersContainerJson(
  val: unknown,
): val is HighlightClustersContainerJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['vod_id'] === 'string' &&
    obj['vod_id'].length > 0 &&
    Array.isArray(obj['clusters']) &&
    obj['clusters'].every(isHighlightClusterJson)
  );
}

export function isHighlightClustersContainer(
  val: unknown,
): val is HighlightClustersContainer {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['vodId'] === 'string' &&
    Array.isArray(obj['clusters']) &&
    obj['clusters'].every(isHighlightCluster)
  );
}

// ---------------------------------------------------------------------------
// Model Normalizers
// ---------------------------------------------------------------------------

export function normalizeHighlightCluster(raw: HighlightClusterJson): HighlightCluster {
  const emotes = Object.freeze(raw.top_emotes ? [...raw.top_emotes] : []);
  const bookmarked = raw.is_bookmarked ?? false;
  const chatZ = raw.chat_z_score ?? null;
  const audioPeak = raw.audio_rms_peak ?? null;

  return {
    clusterId: raw.cluster_id,
    startSec: raw.start_sec,
    endSec: raw.end_sec,
    peakSec: raw.peak_sec,
    score: raw.score,
    primaryReason: raw.primary_reason,
    chatZScore: chatZ,
    audioRmsPeak: audioPeak,
    topEmotes: emotes,
    isBookmarked: bookmarked,

    cluster_id: raw.cluster_id,
    start_sec: raw.start_sec,
    end_sec: raw.end_sec,
    peak_sec: raw.peak_sec,
    primary_reason: raw.primary_reason,
    chat_z_score: chatZ,
    audio_rms_peak: audioPeak,
    top_emotes: emotes,
    is_bookmarked: bookmarked,
  };
}

export function normalizeHighlightClustersContainer(
  raw: HighlightClustersContainerJson,
): HighlightClustersContainer {
  const clusters = Object.freeze(raw.clusters.map(normalizeHighlightCluster));
  return {
    vodId: raw.vod_id,
    clusters,
    totalClusters: raw.total_clusters ?? clusters.length,
    generatedAt: raw.generated_at,

    vod_id: raw.vod_id,
    total_clusters: raw.total_clusters ?? clusters.length,
    generated_at: raw.generated_at,
  };
}

export function toHighlightClustersContainerJson(
  container: HighlightClustersContainer,
): HighlightClustersContainerJson {
  return {
    vod_id: container.vodId,
    clusters: container.clusters.map((c) => ({
      cluster_id: c.clusterId,
      start_sec: c.startSec,
      end_sec: c.endSec,
      peak_sec: c.peakSec,
      score: c.score,
      primary_reason: c.primaryReason,
      chat_z_score: c.chatZScore ?? null,
      audio_rms_peak: c.audioRmsPeak ?? null,
      top_emotes: [...c.topEmotes],
      is_bookmarked: c.isBookmarked,
    })),
    total_clusters: container.totalClusters ?? container.clusters.length,
    generated_at: container.generatedAt,
  };
}

// ---------------------------------------------------------------------------
// Pure Client-Side Formatting & Math Helpers
// ---------------------------------------------------------------------------

export function formatTimecode(
  seconds: number,
  options?: { forceHours?: boolean; includeMilliseconds?: boolean },
): string {
  if (!Number.isFinite(seconds) || seconds <= 0) {
    return options?.forceHours ? '00:00:00' : '00:00';
  }
  const totalSec = Math.floor(seconds);
  const ms = Math.floor((seconds - totalSec) * 10);
  const h = Math.floor(totalSec / 3600);
  const m = Math.floor((totalSec % 3600) / 60);
  const s = totalSec % 60;

  const msSuffix = options?.includeMilliseconds ? `.${ms}` : '';
  if (h > 0 || options?.forceHours) {
    return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}${msSuffix}`;
  }
  return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}${msSuffix}`;
}

export function formatScore(score: number): string {
  if (!Number.isFinite(score) || score <= 0) return '0.0';
  const clamped = Math.min(100, Math.max(0, score));
  return clamped.toFixed(1);
}

export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0s';
  if (seconds < 60) return `${seconds.toFixed(1)}s`;
  const m = Math.floor(seconds / 60);
  const s = Math.round(seconds % 60);
  return `${m}m ${s}s`;
}

/**
 * Multimodal score calculation (UT-12.1):
 * Score = (0.6 * Z_norm + 0.25 * E_ratio + 0.15 * RMS_normalized) * 100.0
 * where Z_norm = clamp(Z_adj / 10.0, 0.0, 1.0)
 */
export function calculateMultimodalScore(
  zNorm: number,
  eRatio: number,
  rmsNorm: number,
): number {
  const z = Math.min(1.0, Math.max(0.0, Number.isFinite(zNorm) ? zNorm : 0.0));
  const e = Math.min(1.0, Math.max(0.0, Number.isFinite(eRatio) ? eRatio : 0.0));
  const rms = Math.min(1.0, Math.max(0.0, Number.isFinite(rmsNorm) ? rmsNorm : 0.0));

  const rawScore =
    FUSION_WEIGHT_Z_SCORE * z +
    FUSION_WEIGHT_EMOTE * e +
    FUSION_WEIGHT_RMS * rms;

  return Math.min(100.0, Math.max(0.0, Math.round(rawScore * 1000.0) / 10.0));
}

/**
 * Audio-only fallback score calculation (UT-12.2):
 * Score = 1.0 * RMS_normalized * 100.0
 */
export function calculateAudioOnlyScore(rmsNorm: number): number {
  const rms = Math.min(1.0, Math.max(0.0, Number.isFinite(rmsNorm) ? rmsNorm : 0.0));
  return Math.min(100.0, Math.max(0.0, Math.round(FUSION_AUDIO_ONLY_WEIGHT_RMS * rms * 1000.0) / 10.0));
}

/**
 * Boundary calculation with latency compensation and dynamic padding at EOF (UT-12.3, ED-12.1, ED-12.3).
 *
 * Latency comp: 4.5s
 * Default pre-roll: 10.0s (total pre-peak offset 14.5s)
 * Default post-roll: 15.0s
 * EOF padding: If peak + post_roll > duration, clamp end to duration and shift missing post-roll
 * into pre-roll buffer (up to max 20.0s), guaranteeing clip length >= 15.0s without exceeding EOF.
 */
export function computeHighlightBounds(
  peakSec: number,
  vodDurationSec: number,
  options?: {
    preRollSec?: number | undefined;
    postRollSec?: number | undefined;
    latencyCompensationSec?: number | undefined;
    isAudioOnly?: boolean | undefined;
  },
): { startSec: number; endSec: number; durationSec: number } {
  const duration = Math.max(0.0, vodDurationSec);
  const latency = options?.isAudioOnly
    ? 0.0
    : (options?.latencyCompensationSec ?? STREAM_LATENCY_COMPENSATION_SEC);
  const initialPreRoll = options?.preRollSec ?? DEFAULT_PRE_ROLL_BUFFER_SEC;
  const targetPostRoll = options?.postRollSec ?? DEFAULT_POST_ROLL_BUFFER_SEC;

  let endSec = Math.min(duration, peakSec + targetPostRoll);
  const baseStart = Math.max(0.0, peakSec - latency - initialPreRoll);
  let startSec = baseStart;

  // Dynamic padding at end of stream (ED-12.3)
  if (endSec - startSec < MIN_CLUSTER_DURATION_SEC && duration >= MIN_CLUSTER_DURATION_SEC) {
    const actualPostRoll = Math.max(0.0, endSec - peakSec);
    const missingPostRoll = Math.max(0.0, targetPostRoll - actualPostRoll);
    const needed = MIN_CLUSTER_DURATION_SEC - (endSec - startSec);
    const extension = Math.min(
      MAX_PRE_ROLL_BUFFER_SEC - initialPreRoll,
      Math.max(missingPostRoll, needed),
    );
    startSec = Math.max(0.0, peakSec - latency - (initialPreRoll + extension));

    if (endSec - startSec < MIN_CLUSTER_DURATION_SEC) {
      startSec = Math.max(0.0, endSec - MIN_CLUSTER_DURATION_SEC);
    }
  }

  // Bound invariant: 0.0 <= startSec < endSec <= duration (unless duration == 0)
  if (startSec >= endSec && duration > 0.0) {
    startSec = Math.max(0.0, endSec - 1.0);
  }

  return {
    startSec: Math.round(startSec * 10.0) / 10.0,
    endSec: Math.round(endSec * 10.0) / 10.0,
    durationSec: Math.round((endSec - startSec) * 10.0) / 10.0,
  };
}
