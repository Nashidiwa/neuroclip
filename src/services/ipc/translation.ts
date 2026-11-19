/**
 * NeuroClip Gemini 1.5 Flash Translation IPC Client (`TASK-M4-17`, `M4.5`).
 *
 * Implements Schema 5.5 subtitle translation contracts, Theatrical Framing bridge,
 * error normalization, and Tauri IPC invoke wrappers.
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { parseIpcError } from './error.ts';
import {
  type TranscriptSegmentsContainer,
  type TranscriptSegmentsContainerJson,
  normalizeTranscriptSegmentsContainer,
  toTranscriptSegmentsContainerJson,
} from '../../types/transcript.ts';
import {
  type TranslateVietsubOptions,
  type TranslationProgressPayload,
  type TranslationProgressPayloadJson,
  normalizeTranslationProgressPayload,
} from '../../types/translation.ts';

export * from '../../types/translation.ts';

export interface WrapVietnameseSubtitlesOptions {
  maxCharsPerLine?: number;
  maxLines?: number;
}

export interface OptimizeTranscriptCpsOptions {
  clipDuration?: number;
  maxBorrowMs?: number;
  minGapMs?: number;
  targetCps?: number;
}

/**
 * Translate transcript segments into natural Vietnamese subtitles using Gemini 1.5 Flash.
 *
 * Encapsulates:
 * - Theatrical Framing System Prompt packaging (`PRD.md` §6.3).
 * - `BLOCK_NONE` safety settings for all 4 categories.
 * - Defensive Safety Fallback (`ERR-07` / `ED-17.1`).
 * - Sliding context window (5 lines) across 25–35 line dialogue chunks.
 *
 * @param clipId Unique identifier for the highlight clip.
 * @param container Transcript container adhering to Schema 5.5.
 * @param options Optional configuration (API key, model override).
 * @returns Updated TranscriptSegmentsContainer with `text_vi` and flags populated.
 */
export async function translateVietsub(
  clipId: string,
  container: TranscriptSegmentsContainer,
  options?: TranslateVietsubOptions,
): Promise<TranscriptSegmentsContainer> {
  try {
    const wireContainer = toTranscriptSegmentsContainerJson(container);
    const raw = await invoke<TranscriptSegmentsContainerJson>('translate_vietsub', {
      clipId,
      container: wireContainer,
      apiKey: options?.apiKey ?? null,
      model: options?.model ?? null,
      stylePreset: options?.stylePreset ?? null,
      castMembers: options?.castMembers ? [...options.castMembers] : null,
    });
    return normalizeTranscriptSegmentsContainer(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Verify Google Gemini API credentials with a lightweight ping prompt.
 *
 * @param apiKey Google Gemini API key.
 * @returns Promise resolving to true if valid.
 */
export async function testGeminiConnection(apiKey: string): Promise<boolean> {
  try {
    return await invoke<boolean>('test_gemini_connection', { apiKey });
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Update the speaker role for a specific transcript segment.
 *
 * @param container Transcript container to update.
 * @param segmentId Segment identifier to modify.
 * @param newSpeaker New speaker identifier (e.g. "vedal", "neuro", "evil").
 * @returns Updated TranscriptSegmentsContainer.
 */
export async function updateSpeakerRole(
  container: TranscriptSegmentsContainer,
  segmentId: string,
  newSpeaker: string,
): Promise<TranscriptSegmentsContainer> {
  try {
    const wireContainer = toTranscriptSegmentsContainerJson(container);
    const raw = await invoke<TranscriptSegmentsContainerJson>('update_speaker_role', {
      container: wireContainer,
      segmentId,
      newSpeaker,
    });
    return normalizeTranscriptSegmentsContainer(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Wrap Vietnamese subtitles across all segments, preserving compound words.
 *
 * @param container Transcript container adhering to Schema 5.5.
 * @param options Line wrapping constraints (maxCharsPerLine, maxLines).
 * @returns Updated TranscriptSegmentsContainer with wrapped text_vi.
 */
export async function wrapVietnameseSubtitles(
  container: TranscriptSegmentsContainer,
  options?: WrapVietnameseSubtitlesOptions,
): Promise<TranscriptSegmentsContainer> {
  try {
    const wireContainer = toTranscriptSegmentsContainerJson(container);
    const raw = await invoke<TranscriptSegmentsContainerJson>('wrap_vietnamese_subtitles', {
      container: wireContainer,
      maxCharsPerLine: options?.maxCharsPerLine ?? null,
      maxLines: options?.maxLines ?? null,
    });
    return normalizeTranscriptSegmentsContainer(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Optimize transcript CPS reading speed via Time Borrowing.
 *
 * @param container Transcript container adhering to Schema 5.5.
 * @param options Time Borrowing parameters (clipDuration, maxBorrowMs, minGapMs, targetCps).
 * @returns Updated TranscriptSegmentsContainer with optimized end_sec and cps_score.
 */
export async function optimizeTranscriptCps(
  container: TranscriptSegmentsContainer,
  options?: OptimizeTranscriptCpsOptions,
): Promise<TranscriptSegmentsContainer> {
  try {
    const wireContainer = toTranscriptSegmentsContainerJson(container);
    const raw = await invoke<TranscriptSegmentsContainerJson>('optimize_transcript_cps', {
      container: wireContainer,
      clipDuration: options?.clipDuration ?? null,
      maxBorrowMs: options?.maxBorrowMs ?? null,
      minGapMs: options?.minGapMs ?? null,
      targetCps: options?.targetCps ?? null,
    });
    return normalizeTranscriptSegmentsContainer(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Subscribe to batch translation progress events.
 *
 * @param callback Progress handler invoked on each chunk translated.
 * @returns Unlisten function to detach event subscription.
 */
export async function listenTranslationProgress(
  callback: (payload: TranslationProgressPayload) => void,
): Promise<UnlistenFn> {
  return listen<TranslationProgressPayloadJson>('translation:progress', (event) => {
    callback(normalizeTranslationProgressPayload(event.payload));
  });
}
