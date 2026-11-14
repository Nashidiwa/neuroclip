/**
 * NeuroClip Chat Contracts & Schema 5.3 Types
 *
 * Implements Schema 5.3 data models for chat events, 10s velocity heatmap,
 * runtime type guards, and dual camelCase/snake_case accessors.
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

// ---------------------------------------------------------------------------
// Wire JSON Types (Schema 5.3)
// ---------------------------------------------------------------------------

export interface ChatEventJson {
  readonly timestamp_sec: number;
  readonly author: string;
  readonly message: string;
  readonly badges?: readonly string[] | undefined;
  readonly emotes_detected: readonly string[];
  readonly is_swarm_emote?: boolean | undefined;
}

export interface ChatEventsContainerJson {
  readonly vod_id: string;
  readonly total_messages: number;
  readonly events: readonly ChatEventJson[];
}

export interface ChatDensityPointJson {
  readonly timestamp_sec: number;
  readonly message_count: number;
  readonly velocity_mps: number;
  readonly swarm_count: number;
  readonly swarm_ratio: number;
  readonly z_score: number;
  readonly normalized_density: number;
  readonly is_peak?: boolean | undefined;
}

export interface ChatHeatmapResponseJson {
  readonly vod_id: string;
  readonly window_seconds: number;
  readonly step_seconds: number;
  readonly total_buckets: number;
  readonly max_velocity_mps: number;
  readonly peak_timestamp_sec: number;
  readonly points: readonly ChatDensityPointJson[];
}

// ---------------------------------------------------------------------------
// Normalized Frontend Domain Models (Dual CamelCase & Snake_case Accessors)
// ---------------------------------------------------------------------------

export interface ChatEvent {
  readonly timestampSec: number;
  readonly author: string;
  readonly message: string;
  readonly badges: readonly string[];
  readonly emotesDetected: readonly string[];
  readonly isSwarmEmote: boolean;

  // Snake_case aliases matching Schema 5.3
  readonly timestamp_sec: number;
  readonly emotes_detected: readonly string[];
  readonly is_swarm_emote: boolean;
}

export interface ChatEventsContainer {
  readonly vodId: string;
  readonly totalMessages: number;
  readonly events: readonly ChatEvent[];

  // Snake_case aliases matching Schema 5.3
  readonly vod_id: string;
  readonly total_messages: number;
}

export interface ChatDensityPoint {
  readonly timestampSec: number;
  readonly messageCount: number;
  readonly velocityMps: number;
  readonly swarmCount: number;
  readonly swarmRatio: number;
  readonly zScore: number;
  readonly normalizedDensity: number;
  readonly isPeak: boolean;

  // Snake_case aliases
  readonly timestamp_sec: number;
  readonly message_count: number;
  readonly velocity_mps: number;
  readonly swarm_count: number;
  readonly swarm_ratio: number;
  readonly z_score: number;
  readonly normalized_density: number;
  readonly is_peak: boolean;
}

export interface ChatHeatmapResponse {
  readonly vodId: string;
  readonly windowSeconds: number;
  readonly stepSeconds: number;
  readonly totalBuckets: number;
  readonly maxVelocityMps: number;
  readonly peakTimestampSec: number;
  readonly points: readonly ChatDensityPoint[];

  // Snake_case aliases
  readonly vod_id: string;
  readonly window_seconds: number;
  readonly step_seconds: number;
  readonly total_buckets: number;
  readonly max_velocity_mps: number;
  readonly peak_timestamp_sec: number;
}

// ---------------------------------------------------------------------------
// Runtime Type Guards
// ---------------------------------------------------------------------------

export function isChatEventJson(val: unknown): val is ChatEventJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;

  const hasValidTimestamp =
    typeof obj['timestamp_sec'] === 'number' && Number.isFinite(obj['timestamp_sec']);
  const hasValidAuthor = typeof obj['author'] === 'string';
  const hasValidMessage = typeof obj['message'] === 'string';
  const hasValidEmotes =
    Array.isArray(obj['emotes_detected']) &&
    obj['emotes_detected'].every((e) => typeof e === 'string');

  const hasValidBadges =
    obj['badges'] === undefined ||
    (Array.isArray(obj['badges']) && obj['badges'].every((b) => typeof b === 'string'));

  const hasValidSwarm =
    obj['is_swarm_emote'] === undefined || typeof obj['is_swarm_emote'] === 'boolean';

  return (
    hasValidTimestamp &&
    hasValidAuthor &&
    hasValidMessage &&
    hasValidEmotes &&
    hasValidBadges &&
    hasValidSwarm
  );
}

export function isChatEventsContainerJson(val: unknown): val is ChatEventsContainerJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['vod_id'] === 'string' &&
    typeof obj['total_messages'] === 'number' &&
    Number.isFinite(obj['total_messages']) &&
    Array.isArray(obj['events']) &&
    obj['events'].every(isChatEventJson)
  );
}

export function isChatDensityPointJson(val: unknown): val is ChatDensityPointJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  const isFiniteNum = (key: string): boolean =>
    typeof obj[key] === 'number' && Number.isFinite(obj[key]);

  return (
    isFiniteNum('timestamp_sec') &&
    isFiniteNum('message_count') &&
    isFiniteNum('velocity_mps') &&
    isFiniteNum('swarm_count') &&
    isFiniteNum('swarm_ratio') &&
    isFiniteNum('z_score') &&
    isFiniteNum('normalized_density') &&
    (obj['is_peak'] === undefined || typeof obj['is_peak'] === 'boolean')
  );
}

export function isChatHeatmapResponseJson(val: unknown): val is ChatHeatmapResponseJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  const isFiniteNum = (key: string): boolean =>
    typeof obj[key] === 'number' && Number.isFinite(obj[key]);

  return (
    typeof obj['vod_id'] === 'string' &&
    isFiniteNum('window_seconds') &&
    isFiniteNum('step_seconds') &&
    typeof obj['total_buckets'] === 'number' &&
    Number.isFinite(obj['total_buckets']) &&
    isFiniteNum('max_velocity_mps') &&
    isFiniteNum('peak_timestamp_sec') &&
    Array.isArray(obj['points']) &&
    obj['points'].every(isChatDensityPointJson)
  );
}

// ---------------------------------------------------------------------------
// Model Normalization Helpers
// ---------------------------------------------------------------------------

export function normalizeChatEvent(json: ChatEventJson): ChatEvent {
  const isSwarm = Boolean(json.is_swarm_emote);
  const badges = Array.isArray(json.badges) ? [...json.badges] : [];
  const emotes = Array.isArray(json.emotes_detected) ? [...json.emotes_detected] : [];

  return {
    timestampSec: json.timestamp_sec,
    author: json.author,
    message: json.message,
    badges,
    emotesDetected: emotes,
    isSwarmEmote: isSwarm,

    timestamp_sec: json.timestamp_sec,
    emotes_detected: emotes,
    is_swarm_emote: isSwarm,
  };
}

export function normalizeChatEventsContainer(json: ChatEventsContainerJson): ChatEventsContainer {
  const events = json.events.map(normalizeChatEvent);
  return {
    vodId: json.vod_id,
    totalMessages: json.total_messages,
    events,

    vod_id: json.vod_id,
    total_messages: json.total_messages,
  };
}

export function normalizeChatDensityPoint(json: ChatDensityPointJson): ChatDensityPoint {
  const isPeak = Boolean(json.is_peak);
  return {
    timestampSec: json.timestamp_sec,
    messageCount: json.message_count,
    velocityMps: json.velocity_mps,
    swarmCount: json.swarm_count,
    swarmRatio: json.swarm_ratio,
    zScore: json.z_score,
    normalizedDensity: json.normalized_density,
    isPeak,

    timestamp_sec: json.timestamp_sec,
    message_count: json.message_count,
    velocity_mps: json.velocity_mps,
    swarm_count: json.swarm_count,
    swarm_ratio: json.swarm_ratio,
    z_score: json.z_score,
    normalized_density: json.normalized_density,
    is_peak: isPeak,
  };
}

export function normalizeChatHeatmapResponse(json: ChatHeatmapResponseJson): ChatHeatmapResponse {
  const points = json.points.map(normalizeChatDensityPoint);
  return {
    vodId: json.vod_id,
    windowSeconds: json.window_seconds,
    stepSeconds: json.step_seconds,
    totalBuckets: json.total_buckets,
    maxVelocityMps: json.max_velocity_mps,
    peakTimestampSec: json.peak_timestamp_sec,
    points,

    vod_id: json.vod_id,
    window_seconds: json.window_seconds,
    step_seconds: json.step_seconds,
    total_buckets: json.total_buckets,
    max_velocity_mps: json.max_velocity_mps,
    peak_timestamp_sec: json.peak_timestamp_sec,
  };
}
