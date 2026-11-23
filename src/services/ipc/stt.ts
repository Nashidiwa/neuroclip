/**
 * NeuroClip Whisper STT & Subtitle Export IPC Client
 *
 * Implements Schema 5.5 / 5.6 contracts, runtime type guards, and Tauri IPC invoke wrappers.
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { parseIpcError } from './error.ts';
import {
  type GuardrailPolicy,
  type ModelInfo,
  type ModelInfoJson,
  type SttProgressPayload,
  type TranscriptSegment,
  type TranscriptSegmentJson,
  type TranscriptSegmentsContainer,
  type TranscriptSegmentsContainerJson,
  isSttProgressPayloadJson,
  isTranscriptSegmentJson,
  normalizeModelInfo,
  normalizeSttProgressPayload,
  normalizeTranscriptSegment,
  normalizeTranscriptSegmentsContainer,
} from '../../types/transcript.ts';

export * from '../../types/transcript.ts';

export const STT_PROGRESS_EVENT = 'stt:progress' as const;
export const STT_CHUNK_EVENT = 'stt:chunk' as const;

export interface TranscribeClipOptions {
  readonly apiKey?: string | undefined;
  readonly isMusicMode?: boolean | undefined;
}

export interface FasterWhisperOptions {
  readonly model?: string | undefined;
  readonly quantization?: string | undefined;
  readonly device?: string | undefined;
  readonly guardrailPolicy?: GuardrailPolicy | undefined;
}

export interface SttChunkPayload {
  readonly clipId: string;
  readonly chunkIndex: number;
  readonly segment: TranscriptSegment;
}

export interface SttChunkPayloadJson {
  readonly clip_id?: string | undefined;
  readonly clipId?: string | undefined;
  readonly chunk_index?: number | undefined;
  readonly chunkIndex?: number | undefined;
  readonly segment: TranscriptSegmentJson;
}

export function isSttChunkPayloadJson(val: unknown): val is SttChunkPayloadJson {
  if (typeof val !== 'object' || val === null) {
    return false;
  }
  const obj = val as Record<string, unknown>;
  const hasValidClipId =
    (obj['clip_id'] === undefined || typeof obj['clip_id'] === 'string') &&
    (obj['clipId'] === undefined || typeof obj['clipId'] === 'string');
  const hasValidChunkIndex =
    (obj['chunk_index'] === undefined ||
      (typeof obj['chunk_index'] === 'number' && Number.isFinite(obj['chunk_index']))) &&
    (obj['chunkIndex'] === undefined ||
      (typeof obj['chunkIndex'] === 'number' && Number.isFinite(obj['chunkIndex'])));

  return hasValidClipId && hasValidChunkIndex && isTranscriptSegmentJson(obj['segment']);
}

// ---------------------------------------------------------------------------
// IPC Invoke Wrappers
// ---------------------------------------------------------------------------

/**
 * Trigger local Whisper.cpp speech-to-text transcription on an audio file.
 *
 * @param clipId Unique identifier for the highlight clip.
 * @param audioPath Absolute path to the 16kHz mono WAV audio file.
 * @param modelName Optional Whisper model identifier (default: "base.en-q5_k").
 * @returns TranscriptSegmentsContainer with domain and wire accessors.
 */
export async function startWhisperTranscription(
  clipId: string,
  audioPath: string,
  modelName?: string,
): Promise<TranscriptSegmentsContainer> {
  try {
    const raw = await invoke<TranscriptSegmentsContainerJson>('start_whisper_transcription', {
      clipId,
      audioPath,
      modelName: modelName ?? null,
    });
    return normalizeTranscriptSegmentsContainer(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Transcribe a highlight clip using Groq Whisper Cloud API (whisper-large-v3).
 *
 * Automatically manages 20MB chunking via Silero VAD pause split with 1.5s lookback overlap,
 * rate limit backoff retry (HTTP 429), and karaoke repetition guardrail (isMusicMode).
 *
 * @param clipId Unique identifier for the highlight clip.
 * @param audioPath Absolute path to the audio file on disk.
 * @param options Optional configuration (Groq API key, isMusicMode).
 * @returns TranscriptSegmentsContainer adhering to Schema 5.5.
 */
export async function transcribeHighlightClip(
  clipId: string,
  audioPath: string,
  options?: TranscribeClipOptions,
): Promise<TranscriptSegmentsContainer> {
  try {
    const raw = await invoke<TranscriptSegmentsContainerJson>('transcribe_highlight_clip', {
      clipId,
      audioPath,
      apiKey: options?.apiKey ?? null,
      isMusicMode: options?.isMusicMode ?? false,
    });
    return normalizeTranscriptSegmentsContainer(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Trigger local Faster-Whisper INT8 transcription with hardware VRAM guardrail protection.
 *
 * @param clipId Unique identifier for the highlight clip.
 * @param audioPath Absolute path to the audio file on disk.
 * @param options Optional Faster-Whisper model, quantization, device, and guardrail policy.
 * @returns TranscriptSegmentsContainer adhering to Schema 5.5.
 */
export async function startFasterWhisperTranscription(
  clipId: string,
  audioPath: string,
  options?: FasterWhisperOptions,
): Promise<TranscriptSegmentsContainer> {
  try {
    const raw = await invoke<TranscriptSegmentsContainerJson>(
      'start_faster_whisper_transcription',
      {
        clipId,
        audioPath,
        model: options?.model ?? null,
        quantization: options?.quantization ?? null,
        device: options?.device ?? null,
        guardrailPolicy: options?.guardrailPolicy ?? null,
      },
    );
    return normalizeTranscriptSegmentsContainer(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Verify cryptographic SHA-256 integrity of a local Whisper model file.
 *
 * @param modelName Optional model identifier to check.
 * @returns Verified ModelInfo metadata.
 */
export async function verifySttModel(modelName?: string): Promise<ModelInfo> {
  try {
    const raw = await invoke<ModelInfoJson>('verify_stt_model', {
      modelName: modelName ?? null,
    });
    return normalizeModelInfo(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

// Re-export subtitle export functions from the dedicated export subsystem
export { exportSubtitlesAss, exportSubtitlesSrt } from './export.ts';

// ---------------------------------------------------------------------------
// Event Stream Listeners
// ---------------------------------------------------------------------------

/**
 * Subscribe to Whisper transcription progress events.
 *
 * @param callback Progress handler invoked on each percent update.
 * @returns Unlisten function to detach subscription.
 */
export async function listenSttProgress(
  callback: (payload: SttProgressPayload) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<unknown>(STT_PROGRESS_EVENT, (event) => {
      if (isSttProgressPayloadJson(event.payload)) {
        callback(normalizeSttProgressPayload(event.payload));
      }
    });
  } catch {
    return () => {};
  }
}

/**
 * Subscribe to streamed transcription chunk events as they are emitted from the backend.
 *
 * @param callback Handler receiving each emitted transcript segment.
 * @returns Unlisten function to detach subscription.
 */
export async function listenSttChunk(
  callback: (payload: SttChunkPayload) => void,
): Promise<UnlistenFn> {
  try {
    return await listen<unknown>(STT_CHUNK_EVENT, (event) => {
      if (isSttChunkPayloadJson(event.payload)) {
        const payload = event.payload;
        callback({
          clipId: payload.clip_id ?? payload.clipId ?? '',
          chunkIndex: payload.chunk_index ?? payload.chunkIndex ?? 0,
          segment: normalizeTranscriptSegment(payload.segment),
        });
      }
    });
  } catch {
    return () => {};
  }
}
