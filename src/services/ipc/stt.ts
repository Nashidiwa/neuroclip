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
  type ModelInfo,
  type ModelInfoJson,
  type SttProgressPayload,
  type SttProgressPayloadJson,
  type TranscriptSegmentsContainer,
  type TranscriptSegmentsContainerJson,
  normalizeModelInfo,
  normalizeSttProgressPayload,
  normalizeTranscriptSegmentsContainer,
  toTranscriptSegmentsContainerJson,
} from '../../types/transcript.ts';

export * from '../../types/transcript.ts';

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
  modelName?: string
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
 * Verify cryptographic SHA-256 integrity of a local Whisper model file.
 *
 * @param modelName Optional model identifier to check.
 * @returns Verified ModelInfo metadata.
 */
export async function verifySttModel(
  modelName?: string
): Promise<ModelInfo> {
  try {
    const raw = await invoke<ModelInfoJson>('verify_stt_model', {
      modelName: modelName ?? null,
    });
    return normalizeModelInfo(raw);
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Export transcript segments to an Advanced SubStation Alpha (.ass) file with VTuber styles.
 *
 * @param container Transcript container to export.
 * @param outputPath Target file destination path on disk.
 * @returns Path to the exported subtitle file.
 */
export async function exportSubtitlesAss(
  container: TranscriptSegmentsContainer,
  outputPath: string
): Promise<string> {
  try {
    const wireContainer = toTranscriptSegmentsContainerJson(container);
    return await invoke<string>('export_subtitles_ass', {
      container: wireContainer,
      outputPath,
    });
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

/**
 * Export transcript segments to a standard SubRip (.srt) subtitle file.
 *
 * @param container Transcript container to export.
 * @param outputPath Target file destination path on disk.
 * @returns Path to the exported subtitle file.
 */
export async function exportSubtitlesSrt(
  container: TranscriptSegmentsContainer,
  outputPath: string
): Promise<string> {
  try {
    const wireContainer = toTranscriptSegmentsContainerJson(container);
    return await invoke<string>('export_subtitles_srt', {
      container: wireContainer,
      outputPath,
    });
  } catch (err: unknown) {
    throw parseIpcError(err);
  }
}

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
  callback: (payload: SttProgressPayload) => void
): Promise<UnlistenFn> {
  return listen<SttProgressPayloadJson>('stt:progress', (event) => {
    callback(normalizeSttProgressPayload(event.payload));
  });
}
