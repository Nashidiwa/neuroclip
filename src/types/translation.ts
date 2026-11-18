/**
//! NeuroClip Gemini Translation Contracts & Event Types (`TASK-M4-17`, `M4.5`).
//!
//! Provides TypeScript domain and wire interfaces for Google Gemini 1.5 Flash
//! subtitle translation, Theatrical Framing, and progress event payloads.
//! Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
*/

import type { TranscriptSegment, TranscriptSegmentsContainer } from './transcript.ts';

export type { TranscriptSegment, TranscriptSegmentsContainer };

/**
 * Translation style preset (`PRD.md` US4.2).
 */
export type StylePreset =
  | 'TikTok Gen Z Trend'
  | 'Formal Subber'
  | 'tiktok_gen_z'
  | 'formal_subber';

/**
 * Options for triggering Vietsub subtitle translation via Gemini 1.5 Flash.
 */
export interface TranslateVietsubOptions {
  readonly apiKey?: string | undefined;
  readonly model?: string | undefined;
  readonly chunkSize?: number | undefined;
  readonly contextWindowSize?: number | undefined;
  readonly stylePreset?: StylePreset | string | undefined;
  readonly castMembers?: readonly string[] | undefined;
}

/**
 * Wire JSON payload emitted by Tauri event `translation:progress`.
 */
export interface TranslationProgressPayloadJson {
  readonly clipId?: string | undefined;
  readonly clip_id?: string | undefined;
  readonly translatedCount?: number | undefined;
  readonly translated_count?: number | undefined;
  readonly totalCount?: number | undefined;
  readonly total_count?: number | undefined;
}

/**
 * Normalized frontend domain model for translation progress updates.
 */
export interface TranslationProgressPayload {
  readonly clipId: string;
  readonly translatedCount: number;
  readonly totalCount: number;
  readonly percent: number;
}

/**
 * Single translated segment item returned by the LLM.
 */
export interface TranslatedSegmentItem {
  readonly segmentId: string;
  readonly speakerId?: string | undefined;
  readonly textVi: string;
}

/**
 * Normalize wire JSON `translation:progress` event into frontend domain model.
 */
export function normalizeTranslationProgressPayload(
  raw: TranslationProgressPayloadJson,
): TranslationProgressPayload {
  const clipId = raw.clipId ?? raw.clip_id ?? '';
  const translatedCount = raw.translatedCount ?? raw.translated_count ?? 0;
  const totalCount = raw.totalCount ?? raw.total_count ?? 0;
  const percent =
    totalCount > 0 ? Math.min(100, Math.round((translatedCount / totalCount) * 100)) : 0;

  return {
    clipId,
    translatedCount,
    totalCount,
    percent,
  };
}
