/**
 * NeuroClip Transcript & Translation State Store
 *
 * Reactive store managing local Whisper.cpp / Faster-Whisper / Groq Cloud STT,
 * streaming progress & chunk events, Gemini 1.5 Flash Vietsub translation,
 * Schema 5.5 segment editing, and ASS / SRT exports.
 *
 * Built on shared `./createStore.ts` (`useSyncExternalStore` with Zustand-compatible API).
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

import { createStore, type GetStateFn, type SetStateFn, type StoreHook } from './createStore.ts';
import {
  type FasterWhisperOptions,
  type ModelInfo,
  type SpeakerRole,
  type SttProgressPayload,
  type TranscribeClipOptions,
  type TranscriptSegment,
  type TranscriptSegmentsContainer,
  exportSubtitlesAss,
  exportSubtitlesSrt,
  listenSttChunk,
  listenSttProgress,
  normalizeTranscriptSegmentsContainer,
  startFasterWhisperTranscription,
  startWhisperTranscription,
  transcribeHighlightClip,
  verifySttModel,
} from '../services/ipc/stt.ts';
import {
  type TranslateVietsubOptions,
  type TranslationProgressPayload,
  listenTranslationProgress,
  testGeminiConnection as testGeminiConnectionIpc,
  translateVietsub,
  updateSpeakerRole,
} from '../services/ipc/translation.ts';
import { ERROR_CODES, type IpcErrorResponse, parseIpcError } from '../services/ipc/error.ts';

export type { StoreHook };

// ---------------------------------------------------------------------------
// Store Interface Contract
// ---------------------------------------------------------------------------

export interface TranscriptState {
  // Reactive State
  readonly clipId: string | null;
  readonly isTranscribing: boolean;
  readonly container: TranscriptSegmentsContainer | null;
  readonly streamedChunks: readonly TranscriptSegment[];
  readonly activeSegmentId: string | null;
  readonly progress: SttProgressPayload | null;
  readonly isTranslating: boolean;
  readonly translationProgress: TranslationProgressPayload | null;
  readonly isTestingGeminiConnection: boolean;
  readonly isGeminiConnected: boolean | null;
  readonly error: IpcErrorResponse | null;
  readonly isModelVerified: boolean;
  readonly lastExportedPath: string | null;

  // Actions
  setClipId: (clipId: string | null) => void;
  setActiveSegmentId: (segmentId: string | null) => void;
  transcribe: (
    clipId: string,
    audioPath: string,
    modelName?: string,
  ) => Promise<TranscriptSegmentsContainer>;
  transcribeClip: (
    clipId: string,
    audioPath: string,
    options?: TranscribeClipOptions,
  ) => Promise<TranscriptSegmentsContainer>;
  transcribeFasterWhisper: (
    clipId: string,
    audioPath: string,
    options?: FasterWhisperOptions,
  ) => Promise<TranscriptSegmentsContainer>;
  verifyModel: (modelName?: string) => Promise<ModelInfo>;
  translate: (options?: TranslateVietsubOptions) => Promise<TranscriptSegmentsContainer>;
  testGeminiConnection: (apiKey: string) => Promise<boolean>;
  updateSegmentText: (segmentId: string, textVi: string) => void;
  updateSegmentSpeaker: (segmentId: string, speakerId: SpeakerRole) => void;
  syncSpeakerRole: (
    segmentId: string,
    newSpeaker: SpeakerRole,
  ) => Promise<TranscriptSegmentsContainer>;
  exportAss: (outputPath: string) => Promise<string>;
  exportSrt: (outputPath: string) => Promise<string>;
  clearError: () => void;
  reset: () => void;
}

// ---------------------------------------------------------------------------
// Initial State & Event Subscription Management
// ---------------------------------------------------------------------------

const initialTranscriptState = {
  clipId: null as string | null,
  isTranscribing: false,
  container: null as TranscriptSegmentsContainer | null,
  streamedChunks: [] as readonly TranscriptSegment[],
  activeSegmentId: null as string | null,
  progress: null as SttProgressPayload | null,
  isTranslating: false,
  translationProgress: null as TranslationProgressPayload | null,
  isTestingGeminiConnection: false,
  isGeminiConnected: null as boolean | null,
  error: null as IpcErrorResponse | null,
  isModelVerified: false,
  lastExportedPath: null as string | null,
};

let resetEpoch = 0;
let transcriptionSeq = 0;
let translationSeq = 0;
let verifySeq = 0;
let geminiTestSeq = 0;
let exportSeq = 0;
let speakerSyncSeq = 0;

let listenersPromise: Promise<void> | null = null;
let activeListenersToken = 0;
let progressUnlisten: (() => void) | null = null;
let chunkUnlisten: (() => void) | null = null;
let translationProgressUnlisten: (() => void) | null = null;

async function safeListen(register: () => Promise<() => void>): Promise<(() => void) | null> {
  try {
    return await register();
  } catch {
    // Non-Tauri testing environment fallback
    return null;
  }
}

function ensureTranscriptListeners(
  set: SetStateFn<TranscriptState>,
  get: GetStateFn<TranscriptState>,
): Promise<void> {
  if (progressUnlisten && chunkUnlisten && translationProgressUnlisten) {
    return Promise.resolve();
  }
  if (listenersPromise) {
    return listenersPromise;
  }

  const epochAtStart = resetEpoch;
  const token = ++activeListenersToken;
  const pending = (async () => {
    const [nextProgressUnlisten, nextChunkUnlisten, nextTranslationUnlisten] = await Promise.all([
      progressUnlisten
        ? Promise.resolve<(() => void) | null>(null)
        : safeListen(() =>
            listenSttProgress((payload) => {
              const state = get();
              if (state.isTranscribing && state.clipId !== null) {
                set({ progress: payload });
              }
            }),
          ),
      chunkUnlisten
        ? Promise.resolve<(() => void) | null>(null)
        : safeListen(() =>
            listenSttChunk((payload) => {
              const activeClipId = get().clipId;
              if (activeClipId !== null && payload.clipId === activeClipId) {
                set((state) => {
                  const idx = state.streamedChunks.findIndex(
                    (seg) => seg.segmentId === payload.segment.segmentId,
                  );
                  const nextChunks =
                    idx >= 0
                      ? state.streamedChunks.map((seg, i) => (i === idx ? payload.segment : seg))
                      : [...state.streamedChunks, payload.segment];
                  return { streamedChunks: nextChunks };
                });
              }
            }),
          ),
      translationProgressUnlisten
        ? Promise.resolve<(() => void) | null>(null)
        : safeListen(() =>
            listenTranslationProgress((payload) => {
              const activeClipId = get().clipId;
              if (activeClipId !== null && payload.clipId === activeClipId) {
                set({ translationProgress: payload });
              }
            }),
          ),
    ]);

    if (epochAtStart !== resetEpoch) {
      nextProgressUnlisten?.();
      nextChunkUnlisten?.();
      nextTranslationUnlisten?.();
      if (activeListenersToken === token) {
        listenersPromise = null;
      }
      return;
    }

    if (nextProgressUnlisten) {
      if (progressUnlisten) {
        nextProgressUnlisten();
      } else {
        progressUnlisten = nextProgressUnlisten;
      }
    }
    if (nextChunkUnlisten) {
      if (chunkUnlisten) {
        nextChunkUnlisten();
      } else {
        chunkUnlisten = nextChunkUnlisten;
      }
    }
    if (nextTranslationUnlisten) {
      if (translationProgressUnlisten) {
        nextTranslationUnlisten();
      } else {
        translationProgressUnlisten = nextTranslationUnlisten;
      }
    }

    if (activeListenersToken === token) {
      listenersPromise = null;
    }
  })();

  listenersPromise = pending;
  return pending;
}

function cleanupTranscriptListeners(): void {
  ++activeListenersToken;
  listenersPromise = null;
  if (progressUnlisten) {
    progressUnlisten();
    progressUnlisten = null;
  }
  if (chunkUnlisten) {
    chunkUnlisten();
    chunkUnlisten = null;
  }
  if (translationProgressUnlisten) {
    translationProgressUnlisten();
    translationProgressUnlisten = null;
  }
}

function createInitialProgress(clipId: string): SttProgressPayload {
  return {
    taskId: clipId,
    stage: 'starting',
    percent: 0.0,
    etaSec: null,
    currentChunk: 1,
    totalChunks: 1,
    task_id: clipId,
    eta_sec: null,
    current_chunk: 1,
    total_chunks: 1,
  };
}

// ---------------------------------------------------------------------------
// Store Instance Definition
// ---------------------------------------------------------------------------

export const useTranscriptStore = createStore<TranscriptState>((set, get) => ({
  ...initialTranscriptState,

  setClipId: (clipId: string | null): void => {
    set({ clipId });
  },

  setActiveSegmentId: (activeSegmentId: string | null): void => {
    set({ activeSegmentId });
  },

  transcribe: async (
    clipId: string,
    audioPath: string,
    modelName?: string,
  ): Promise<TranscriptSegmentsContainer> => {
    const seq = ++transcriptionSeq;
    const epoch = resetEpoch;
    set({
      isTranscribing: true,
      error: null,
      clipId,
      streamedChunks: [],
      progress: createInitialProgress(clipId),
    });

    try {
      await ensureTranscriptListeners(set, get);
      if (seq !== transcriptionSeq || epoch !== resetEpoch) {
        return normalizeTranscriptSegmentsContainer({ clip_id: clipId, segments: [] });
      }
      const container = await startWhisperTranscription(clipId, audioPath, modelName);
      if (seq === transcriptionSeq && epoch === resetEpoch) {
        set({
          container,
          isTranscribing: false,
          error: null,
          activeSegmentId:
            container.segments.length > 0 ? (container.segments[0]?.segmentId ?? null) : null,
        });
      }
      return container;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      if (seq === transcriptionSeq && epoch === resetEpoch) {
        set({
          isTranscribing: false,
          error: parsed,
        });
      }
      throw parsed;
    }
  },

  transcribeClip: async (
    clipId: string,
    audioPath: string,
    options?: TranscribeClipOptions,
  ): Promise<TranscriptSegmentsContainer> => {
    const seq = ++transcriptionSeq;
    const epoch = resetEpoch;
    set({
      isTranscribing: true,
      error: null,
      clipId,
      streamedChunks: [],
      progress: createInitialProgress(clipId),
    });

    try {
      await ensureTranscriptListeners(set, get);
      if (seq !== transcriptionSeq || epoch !== resetEpoch) {
        return normalizeTranscriptSegmentsContainer({ clip_id: clipId, segments: [] });
      }
      const container = await transcribeHighlightClip(clipId, audioPath, options);
      if (seq === transcriptionSeq && epoch === resetEpoch) {
        set({
          container,
          isTranscribing: false,
          error: null,
          activeSegmentId:
            container.segments.length > 0 ? (container.segments[0]?.segmentId ?? null) : null,
        });
      }
      return container;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      if (seq === transcriptionSeq && epoch === resetEpoch) {
        set({
          isTranscribing: false,
          error: parsed,
        });
      }
      throw parsed;
    }
  },

  transcribeFasterWhisper: async (
    clipId: string,
    audioPath: string,
    options?: FasterWhisperOptions,
  ): Promise<TranscriptSegmentsContainer> => {
    const seq = ++transcriptionSeq;
    const epoch = resetEpoch;
    set({
      isTranscribing: true,
      error: null,
      clipId,
      streamedChunks: [],
      progress: createInitialProgress(clipId),
    });

    try {
      await ensureTranscriptListeners(set, get);
      if (seq !== transcriptionSeq || epoch !== resetEpoch) {
        return normalizeTranscriptSegmentsContainer({ clip_id: clipId, segments: [] });
      }
      const container = await startFasterWhisperTranscription(clipId, audioPath, options);
      if (seq === transcriptionSeq && epoch === resetEpoch) {
        set({
          container,
          isTranscribing: false,
          error: null,
          activeSegmentId:
            container.segments.length > 0 ? (container.segments[0]?.segmentId ?? null) : null,
        });
      }
      return container;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      if (seq === transcriptionSeq && epoch === resetEpoch) {
        set({
          isTranscribing: false,
          error: parsed,
        });
      }
      throw parsed;
    }
  },

  verifyModel: async (modelName?: string): Promise<ModelInfo> => {
    const seq = ++verifySeq;
    const epoch = resetEpoch;
    try {
      const info = await verifySttModel(modelName);
      if (seq === verifySeq && epoch === resetEpoch) {
        set({ isModelVerified: info.isVerified });
      }
      return info;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      if (seq === verifySeq && epoch === resetEpoch) {
        set({
          isModelVerified: false,
          error: parsed,
        });
      }
      throw parsed;
    }
  },

  translate: async (options?: TranslateVietsubOptions): Promise<TranscriptSegmentsContainer> => {
    const cur = get().container;
    if (!cur) {
      const err: IpcErrorResponse = {
        code: ERROR_CODES.TRANSLATE_API_FAILED,
        message: 'Không có dữ liệu bóc băng để dịch thuật',
        details: 'Transcript container is null',
        recoverable: true,
      };
      set({ error: err });
      throw err;
    }

    const seq = ++translationSeq;
    const epoch = resetEpoch;
    const targetClipId = get().clipId ?? cur.clipId;
    set({
      isTranslating: true,
      error: null,
      translationProgress: null,
    });

    try {
      await ensureTranscriptListeners(set, get);
      if (seq !== translationSeq || epoch !== resetEpoch) {
        return cur;
      }
      const updated = await translateVietsub(targetClipId, cur, options);
      if (seq === translationSeq && epoch === resetEpoch) {
        set({
          container: updated,
          isTranslating: false,
          error: null,
        });
      }
      return updated;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      if (seq === translationSeq && epoch === resetEpoch) {
        set({
          isTranslating: false,
          error: parsed,
        });
      }
      throw parsed;
    }
  },

  testGeminiConnection: async (apiKey: string): Promise<boolean> => {
    const seq = ++geminiTestSeq;
    const epoch = resetEpoch;
    set({ isTestingGeminiConnection: true, error: null });
    try {
      const ok = await testGeminiConnectionIpc(apiKey);
      if (seq === geminiTestSeq && epoch === resetEpoch) {
        set({
          isTestingGeminiConnection: false,
          isGeminiConnected: ok,
          error: null,
        });
      }
      return ok;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      if (seq === geminiTestSeq && epoch === resetEpoch) {
        set({
          isTestingGeminiConnection: false,
          isGeminiConnected: false,
          error: parsed,
        });
      }
      throw parsed;
    }
  },

  updateSegmentText: (segmentId: string, textVi: string): void => {
    const cur = get().container;
    if (!cur) return;

    const updatedSegments: TranscriptSegment[] = cur.segments.map((seg) => {
      if (seg.segmentId === segmentId || seg.segment_id === segmentId) {
        return {
          ...seg,
          textVi,
          text_vi: textVi,
        };
      }
      return seg;
    });

    set({
      container: {
        ...cur,
        segments: updatedSegments,
      },
    });
  },

  updateSegmentSpeaker: (segmentId: string, speakerId: SpeakerRole): void => {
    const cur = get().container;
    if (!cur) return;

    const updatedSegments: TranscriptSegment[] = cur.segments.map((seg) => {
      if (seg.segmentId === segmentId || seg.segment_id === segmentId) {
        return {
          ...seg,
          speakerId,
          speaker_id: speakerId,
        };
      }
      return seg;
    });

    set({
      container: {
        ...cur,
        segments: updatedSegments,
      },
    });
  },

  syncSpeakerRole: async (
    segmentId: string,
    newSpeaker: SpeakerRole,
  ): Promise<TranscriptSegmentsContainer> => {
    const cur = get().container;
    if (!cur) {
      const err: IpcErrorResponse = {
        code: ERROR_CODES.INTERNAL_ERROR,
        message: 'Không có dữ liệu bóc băng để cập nhật vai nhân vật',
        details: 'Transcript container is null',
        recoverable: false,
      };
      set({ error: err });
      throw err;
    }

    const seq = ++speakerSyncSeq;
    const epoch = resetEpoch;
    try {
      const updated = await updateSpeakerRole(cur, segmentId, newSpeaker);
      if (seq === speakerSyncSeq && epoch === resetEpoch) {
        set({
          container: updated,
          error: null,
        });
      }
      return updated;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      if (seq === speakerSyncSeq && epoch === resetEpoch) {
        set({ error: parsed });
      }
      throw parsed;
    }
  },

  exportAss: async (outputPath: string): Promise<string> => {
    const cur = get().container;
    if (!cur) {
      const err: IpcErrorResponse = {
        code: 'ERR_EXPORT_FILE_WRITE_FAILED',
        message: 'Không có dữ liệu bóc băng để xuất phụ đề ASS',
        details: 'Transcript container is null',
        recoverable: true,
      };
      set({ error: err });
      throw err;
    }

    const seq = ++exportSeq;
    const epoch = resetEpoch;
    try {
      const path = await exportSubtitlesAss(cur, outputPath);
      if (seq === exportSeq && epoch === resetEpoch) {
        set({ lastExportedPath: path, error: null });
      }
      return path;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      if (seq === exportSeq && epoch === resetEpoch) {
        set({ error: parsed });
      }
      throw parsed;
    }
  },

  exportSrt: async (outputPath: string): Promise<string> => {
    const cur = get().container;
    if (!cur) {
      const err: IpcErrorResponse = {
        code: 'ERR_EXPORT_FILE_WRITE_FAILED',
        message: 'Không có dữ liệu bóc băng để xuất phụ đề SRT',
        details: 'Transcript container is null',
        recoverable: true,
      };
      set({ error: err });
      throw err;
    }

    const seq = ++exportSeq;
    const epoch = resetEpoch;
    try {
      const path = await exportSubtitlesSrt(cur, outputPath);
      if (seq === exportSeq && epoch === resetEpoch) {
        set({ lastExportedPath: path, error: null });
      }
      return path;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      if (seq === exportSeq && epoch === resetEpoch) {
        set({ error: parsed });
      }
      throw parsed;
    }
  },

  clearError: (): void => {
    set({ error: null });
  },

  reset: (): void => {
    ++resetEpoch;
    cleanupTranscriptListeners();
    set(initialTranscriptState);
  },
}));
