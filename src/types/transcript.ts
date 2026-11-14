/**
 * NeuroClip Transcript Contracts & Schema 5.5 / 5.6 Types
 *
 * Implements Schema 5.5 (transcript_segments.json) and Schema 5.6 (subtitle_project.json),
 * runtime type guards, dual camelCase/snake_case accessors, and serialization helpers.
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

// ---------------------------------------------------------------------------
// Speaker Roles
// ---------------------------------------------------------------------------

export type SpeakerRole = 'vedal' | 'neuro' | 'evil' | 'guest' | 'chat' | 'unknown';

export const VALID_SPEAKER_ROLES: readonly SpeakerRole[] = [
  'vedal',
  'neuro',
  'evil',
  'guest',
  'chat',
  'unknown',
];

// ---------------------------------------------------------------------------
// Wire JSON Types (Schema 5.5 & 5.6)
// ---------------------------------------------------------------------------

export interface WordTimingJson {
  readonly word: string;
  readonly start_sec: number;
  readonly end_sec: number;
  readonly probability?: number | undefined;
  readonly confidence?: number | undefined;
}

export interface TranscriptSegmentJson {
  readonly segment_id: string;
  readonly start_sec: number;
  readonly end_sec: number;
  readonly speaker_id: SpeakerRole | string;
  readonly text_en: string;
  readonly text_vi?: string | null | undefined;
  readonly confidence: number;
  readonly cps_score: number;
  readonly is_flagged_for_review?: boolean | undefined;
  readonly words?: readonly WordTimingJson[] | undefined;
}

export interface TranscriptSegmentsContainerJson {
  readonly clip_id: string;
  readonly segments: readonly TranscriptSegmentJson[];
}

export interface SpeakerStyleJson {
  readonly color_hex: string;
  readonly font_name: string;
  readonly font_size: number;
  readonly outline_color_hex: string;
}

export interface SubtitleProjectJson {
  readonly project_id: string;
  readonly project_name: string;
  readonly vod_id: string;
  readonly video_file_path?: string | null | undefined;
  readonly target_layout: string;
  readonly segments: readonly TranscriptSegmentJson[];
  readonly speaker_styles: Readonly<Record<string, SpeakerStyleJson>>;
  readonly updated_at: string;
}

export interface ModelInfoJson {
  readonly modelName?: string | undefined;
  readonly model_name?: string | undefined;
  readonly filename: string;
  readonly path: string;
  readonly sizeBytes?: number | undefined;
  readonly size_bytes?: number | undefined;
  readonly sha256: string;
  readonly isVerified?: boolean | undefined;
  readonly is_verified?: boolean | undefined;
}

export interface SttProgressPayloadJson {
  readonly taskId?: string | undefined;
  readonly task_id?: string | undefined;
  readonly stage: string;
  readonly percent: number;
  readonly etaSec?: number | null | undefined;
  readonly eta_sec?: number | null | undefined;
  readonly currentChunk?: number | undefined;
  readonly current_chunk?: number | undefined;
  readonly totalChunks?: number | undefined;
  readonly total_chunks?: number | undefined;
}

// ---------------------------------------------------------------------------
// Normalized Frontend Domain Models (Dual CamelCase & Snake_case Accessors)
// ---------------------------------------------------------------------------

export interface WordTiming {
  readonly word: string;
  readonly startSec: number;
  readonly endSec: number;
  readonly probability: number;
  readonly confidence: number;

  // Snake_case aliases matching Schema 5.5
  readonly start_sec: number;
  readonly end_sec: number;
}

export interface TranscriptSegment {
  readonly segmentId: string;
  readonly startSec: number;
  readonly endSec: number;
  readonly speakerId: SpeakerRole;
  readonly textEn: string;
  readonly textVi: string | null;
  readonly confidence: number;
  readonly cpsScore: number;
  readonly isFlaggedForReview: boolean;
  readonly words: readonly WordTiming[];

  // Snake_case aliases matching Schema 5.5
  readonly segment_id: string;
  readonly start_sec: number;
  readonly end_sec: number;
  readonly speaker_id: SpeakerRole;
  readonly text_en: string;
  readonly text_vi: string | null;
  readonly cps_score: number;
  readonly is_flagged_for_review: boolean;
}

export interface TranscriptSegmentsContainer {
  readonly clipId: string;
  readonly segments: readonly TranscriptSegment[];

  // Snake_case alias matching Schema 5.5
  readonly clip_id: string;
}

export interface SpeakerStyle {
  readonly colorHex: string;
  readonly fontName: string;
  readonly fontSize: number;
  readonly outlineColorHex: string;

  // Snake_case aliases
  readonly color_hex: string;
  readonly font_name: string;
  readonly font_size: number;
  readonly outline_color_hex: string;
}

export interface SubtitleProject {
  readonly projectId: string;
  readonly projectName: string;
  readonly vodId: string;
  readonly videoFilePath: string | null;
  readonly targetLayout: string;
  readonly segments: readonly TranscriptSegment[];
  readonly speakerStyles: Readonly<Record<string, SpeakerStyle>>;
  readonly updatedAt: string;

  // Snake_case aliases
  readonly project_id: string;
  readonly project_name: string;
  readonly vod_id: string;
  readonly video_file_path: string | null;
  readonly target_layout: string;
  readonly speaker_styles: Readonly<Record<string, SpeakerStyle>>;
  readonly updated_at: string;
}

export interface ModelInfo {
  readonly modelName: string;
  readonly filename: string;
  readonly path: string;
  readonly sizeBytes: number;
  readonly sha256: string;
  readonly isVerified: boolean;

  readonly model_name: string;
  readonly size_bytes: number;
  readonly is_verified: boolean;
}

export interface SttProgressPayload {
  readonly taskId: string;
  readonly stage: string;
  readonly percent: number;
  readonly etaSec: number | null;
  readonly currentChunk: number;
  readonly totalChunks: number;

  readonly task_id: string;
  readonly eta_sec: number | null;
  readonly current_chunk: number;
  readonly total_chunks: number;
}

// ---------------------------------------------------------------------------
// Runtime Type Guards
// ---------------------------------------------------------------------------

export function isSpeakerRole(val: unknown): val is SpeakerRole {
  return typeof val === 'string' && (VALID_SPEAKER_ROLES as readonly string[]).includes(val);
}

export function isWordTimingJson(val: unknown): val is WordTimingJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  const isFiniteNum = (key: string): boolean =>
    typeof obj[key] === 'number' && Number.isFinite(obj[key]);

  return (
    typeof obj['word'] === 'string' &&
    isFiniteNum('start_sec') &&
    isFiniteNum('end_sec') &&
    (obj['probability'] === undefined || isFiniteNum('probability')) &&
    (obj['confidence'] === undefined || isFiniteNum('confidence'))
  );
}

export function isTranscriptSegmentJson(val: unknown): val is TranscriptSegmentJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  const isFiniteNum = (key: string): boolean =>
    typeof obj[key] === 'number' && Number.isFinite(obj[key]);

  return (
    typeof obj['segment_id'] === 'string' &&
    isFiniteNum('start_sec') &&
    isFiniteNum('end_sec') &&
    typeof obj['speaker_id'] === 'string' &&
    typeof obj['text_en'] === 'string' &&
    (obj['text_vi'] === undefined || obj['text_vi'] === null || typeof obj['text_vi'] === 'string') &&
    isFiniteNum('confidence') &&
    isFiniteNum('cps_score') &&
    (obj['is_flagged_for_review'] === undefined || typeof obj['is_flagged_for_review'] === 'boolean') &&
    (obj['words'] === undefined || (Array.isArray(obj['words']) && obj['words'].every(isWordTimingJson)))
  );
}

export function isTranscriptSegmentsContainerJson(
  val: unknown,
): val is TranscriptSegmentsContainerJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['clip_id'] === 'string' &&
    Array.isArray(obj['segments']) &&
    obj['segments'].every(isTranscriptSegmentJson)
  );
}

export function isSpeakerStyleJson(val: unknown): val is SpeakerStyleJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    typeof obj['color_hex'] === 'string' &&
    typeof obj['font_name'] === 'string' &&
    typeof obj['font_size'] === 'number' &&
    Number.isFinite(obj['font_size']) &&
    typeof obj['outline_color_hex'] === 'string'
  );
}

export function isSubtitleProjectJson(val: unknown): val is SubtitleProjectJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  if (
    typeof obj['project_id'] !== 'string' ||
    typeof obj['project_name'] !== 'string' ||
    typeof obj['vod_id'] !== 'string' ||
    typeof obj['target_layout'] !== 'string' ||
    typeof obj['updated_at'] !== 'string' ||
    !Array.isArray(obj['segments']) ||
    !obj['segments'].every(isTranscriptSegmentJson) ||
    typeof obj['speaker_styles'] !== 'object' ||
    obj['speaker_styles'] === null
  ) {
    return false;
  }

  const styles = obj['speaker_styles'] as Record<string, unknown>;
  return Object.values(styles).every(isSpeakerStyleJson);
}

export function isModelInfoJson(val: unknown): val is ModelInfoJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  const name = typeof obj['modelName'] === 'string' ? obj['modelName'] : obj['model_name'];
  return (
    typeof name === 'string' &&
    typeof obj['filename'] === 'string' &&
    typeof obj['path'] === 'string' &&
    typeof obj['sha256'] === 'string'
  );
}

export function isSttProgressPayloadJson(val: unknown): val is SttProgressPayloadJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  return (
    (typeof obj['taskId'] === 'string' || typeof obj['task_id'] === 'string') &&
    typeof obj['stage'] === 'string' &&
    typeof obj['percent'] === 'number' &&
    Number.isFinite(obj['percent'])
  );
}

// ---------------------------------------------------------------------------
// Model Normalization Helpers
// ---------------------------------------------------------------------------

export function normalizeWordTiming(json: WordTimingJson): WordTiming {
  const prob = json.probability ?? json.confidence ?? 1.0;
  return {
    word: json.word,
    startSec: json.start_sec,
    endSec: json.end_sec,
    probability: prob,
    confidence: prob,

    start_sec: json.start_sec,
    end_sec: json.end_sec,
  };
}

export function normalizeTranscriptSegment(json: TranscriptSegmentJson): TranscriptSegment {
  const words = Array.isArray(json.words) ? json.words.map(normalizeWordTiming) : [];
  const speaker: SpeakerRole = isSpeakerRole(json.speaker_id) ? json.speaker_id : 'unknown';
  const textVi = json.text_vi ?? null;
  const isFlagged = Boolean(json.is_flagged_for_review);

  return {
    segmentId: json.segment_id,
    startSec: json.start_sec,
    endSec: json.end_sec,
    speakerId: speaker,
    textEn: json.text_en,
    textVi,
    confidence: json.confidence,
    cpsScore: json.cps_score,
    isFlaggedForReview: isFlagged,
    words,

    segment_id: json.segment_id,
    start_sec: json.start_sec,
    end_sec: json.end_sec,
    speaker_id: speaker,
    text_en: json.text_en,
    text_vi: textVi,
    cps_score: json.cps_score,
    is_flagged_for_review: isFlagged,
  };
}

export function normalizeTranscriptSegmentsContainer(
  json: TranscriptSegmentsContainerJson,
): TranscriptSegmentsContainer {
  const segments = json.segments.map(normalizeTranscriptSegment);
  return {
    clipId: json.clip_id,
    segments,

    clip_id: json.clip_id,
  };
}

export function normalizeSpeakerStyle(json: SpeakerStyleJson): SpeakerStyle {
  return {
    colorHex: json.color_hex,
    fontName: json.font_name,
    fontSize: json.font_size,
    outlineColorHex: json.outline_color_hex,

    color_hex: json.color_hex,
    font_name: json.font_name,
    font_size: json.font_size,
    outline_color_hex: json.outline_color_hex,
  };
}

export function normalizeSubtitleProject(json: SubtitleProjectJson): SubtitleProject {
  const segments = json.segments.map(normalizeTranscriptSegment);
  const speakerStyles: Record<string, SpeakerStyle> = {};

  for (const [key, styleJson] of Object.entries(json.speaker_styles)) {
    speakerStyles[key] = normalizeSpeakerStyle(styleJson);
  }

  const videoFilePath = json.video_file_path ?? null;

  return {
    projectId: json.project_id,
    projectName: json.project_name,
    vodId: json.vod_id,
    videoFilePath,
    targetLayout: json.target_layout,
    segments,
    speakerStyles,
    updatedAt: json.updated_at,

    project_id: json.project_id,
    project_name: json.project_name,
    vod_id: json.vod_id,
    video_file_path: videoFilePath,
    target_layout: json.target_layout,
    speaker_styles: speakerStyles,
    updated_at: json.updated_at,
  };
}

export function normalizeModelInfo(json: ModelInfoJson): ModelInfo {
  const modelName = json.modelName ?? json.model_name ?? 'unknown';
  const sizeBytes = json.sizeBytes ?? json.size_bytes ?? 0;
  const isVerified = json.isVerified ?? json.is_verified ?? false;

  return {
    modelName,
    filename: json.filename,
    path: json.path,
    sizeBytes,
    sha256: json.sha256,
    isVerified,

    model_name: modelName,
    size_bytes: sizeBytes,
    is_verified: isVerified,
  };
}

export function normalizeSttProgressPayload(json: SttProgressPayloadJson): SttProgressPayload {
  const taskId = json.taskId ?? json.task_id ?? '';
  const etaSec = json.etaSec ?? json.eta_sec ?? null;
  const currentChunk = json.currentChunk ?? json.current_chunk ?? 1;
  const totalChunks = json.totalChunks ?? json.total_chunks ?? 1;

  return {
    taskId,
    stage: json.stage,
    percent: json.percent,
    etaSec,
    currentChunk,
    totalChunks,

    task_id: taskId,
    eta_sec: etaSec,
    current_chunk: currentChunk,
    total_chunks: totalChunks,
  };
}

// ---------------------------------------------------------------------------
// Serialization Helper (Domain to Wire JSON)
// ---------------------------------------------------------------------------

export function toTranscriptSegmentsContainerJson(
  container: TranscriptSegmentsContainer,
): TranscriptSegmentsContainerJson {
  return {
    clip_id: container.clipId,
    segments: container.segments.map((seg) => ({
      segment_id: seg.segmentId,
      start_sec: seg.startSec,
      end_sec: seg.endSec,
      speaker_id: seg.speakerId,
      text_en: seg.textEn,
      text_vi: seg.textVi,
      confidence: seg.confidence,
      cps_score: seg.cpsScore,
      is_flagged_for_review: seg.isFlaggedForReview,
      words: seg.words.map((w) => ({
        word: w.word,
        start_sec: w.startSec,
        end_sec: w.endSec,
        probability: w.probability,
        confidence: w.confidence,
      })),
    })),
  };
}
