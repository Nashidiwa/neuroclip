/**
 * NeuroClip Transcript State Store
 *
 * Reactive store managing local Whisper.cpp STT execution, streaming progress,
 * Schema 5.5 segments, editing (speaker / Vietnamese text), and ASS / SRT exports.
 *
 * Built on native `useSyncExternalStore` (Zustand-compatible architecture).
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

import { useSyncExternalStore } from 'react';
import {
  type ModelInfo,
  type SpeakerRole,
  type SttProgressPayload,
  type TranscriptSegment,
  type TranscriptSegmentsContainer,
  exportSubtitlesAss,
  exportSubtitlesSrt,
  listenSttProgress,
  startWhisperTranscription,
  verifySttModel,
} from '../services/ipc/stt.ts';
import { type IpcErrorResponse, parseIpcError } from '../services/ipc/error.ts';

// ---------------------------------------------------------------------------
// Store Interface Contract
// ---------------------------------------------------------------------------

export interface TranscriptState {
  // Reactive State
  readonly clipId: string | null;
  readonly isTranscribing: boolean;
  readonly container: TranscriptSegmentsContainer | null;
  readonly activeSegmentId: string | null;
  readonly progress: SttProgressPayload | null;
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
  verifyModel: (modelName?: string) => Promise<ModelInfo>;
  updateSegmentText: (segmentId: string, textVi: string) => void;
  updateSegmentSpeaker: (segmentId: string, speakerId: SpeakerRole) => void;
  exportAss: (outputPath: string) => Promise<string>;
  exportSrt: (outputPath: string) => Promise<string>;
  clearError: () => void;
  reset: () => void;
}

// ---------------------------------------------------------------------------
// Type-Safe Reactive Store Core (Zustand-Compatible Architecture)
// ---------------------------------------------------------------------------

type SetStateFn<T> = (
  partial: Partial<T> | ((state: T) => Partial<T>),
) => void;
type GetStateFn<T> = () => T;
type Listener<T> = (state: T, prevState: T) => void;

interface StoreApi<T> {
  getState: GetStateFn<T>;
  setState: SetStateFn<T>;
  subscribe: (listener: Listener<T>) => () => void;
}

export type StoreHook<T> = {
  (): T;
  <U>(selector: (state: T) => U): U;
} & StoreApi<T>;

function createStore<T extends object>(
  initializer: (set: SetStateFn<T>, get: GetStateFn<T>) => T,
): StoreHook<T> {
  let state: T;
  const listeners = new Set<Listener<T>>();

  const getState: GetStateFn<T> = () => state;

  const setState: SetStateFn<T> = (partial) => {
    const prevState = state;
    const nextState =
      typeof partial === 'function'
        ? { ...prevState, ...partial(prevState) }
        : { ...prevState, ...partial };

    if (!Object.is(prevState, nextState)) {
      state = nextState;
      listeners.forEach((listener) => {
        listener(state, prevState);
      });
    }
  };

  const subscribe = (listener: Listener<T>): (() => void) => {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  };

  state = initializer(setState, getState);

  const useStore = (<U>(selector?: (state: T) => U): T | U => {
    return useSyncExternalStore(
      (notify) => subscribe(notify),
      () => {
        const cur = getState();
        return selector ? selector(cur) : cur;
      },
      () => {
        const cur = getState();
        return selector ? selector(cur) : cur;
      },
    );
  }) as StoreHook<T>;

  useStore.getState = getState;
  useStore.setState = setState;
  useStore.subscribe = subscribe;

  return useStore;
}

// ---------------------------------------------------------------------------
// Initial State & Event Subscription Management
// ---------------------------------------------------------------------------

const initialTranscriptState = {
  clipId: null,
  isTranscribing: false,
  container: null,
  activeSegmentId: null,
  progress: null,
  error: null,
  isModelVerified: false,
  lastExportedPath: null,
};

let progressUnlisten: (() => void) | null = null;

async function ensureTranscriptListeners(set: SetStateFn<TranscriptState>): Promise<void> {
  if (!progressUnlisten) {
    try {
      progressUnlisten = await listenSttProgress((payload) => {
        set({ progress: payload });
      });
    } catch {
      // Non-Tauri testing environment fallback
    }
  }
}

function cleanupTranscriptListeners(): void {
  if (progressUnlisten) {
    progressUnlisten();
    progressUnlisten = null;
  }
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
    set({
      isTranscribing: true,
      error: null,
      clipId,
      progress: {
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
      },
    });

    try {
      await ensureTranscriptListeners(set);
      const container = await startWhisperTranscription(clipId, audioPath, modelName);
      set({
        container,
        isTranscribing: false,
        error: null,
        activeSegmentId: container.segments.length > 0 ? container.segments[0]?.segmentId ?? null : null,
      });
      return container;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      set({
        isTranscribing: false,
        error: parsed,
      });
      throw parsed;
    }
  },

  verifyModel: async (modelName?: string): Promise<ModelInfo> => {
    try {
      const info = await verifySttModel(modelName);
      set({ isModelVerified: info.isVerified });
      return info;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      set({
        isModelVerified: false,
        error: parsed,
      });
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

    try {
      const path = await exportSubtitlesAss(cur, outputPath);
      set({ lastExportedPath: path, error: null });
      return path;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      set({ error: parsed });
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

    try {
      const path = await exportSubtitlesSrt(cur, outputPath);
      set({ lastExportedPath: path, error: null });
      return path;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      set({ error: parsed });
      throw parsed;
    }
  },

  clearError: (): void => {
    set({ error: null });
  },

  reset: (): void => {
    cleanupTranscriptListeners();
    set(initialTranscriptState);
  },
}));
