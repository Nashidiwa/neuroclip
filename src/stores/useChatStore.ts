/**
 * NeuroClip Chat State Store
 *
 * Reactive store managing online chat extraction, streaming progress,
 * Schema 5.3 normalized events, 10s velocity heatmap, and ERR-05 fallback events.
 *
 * Built on native `useSyncExternalStore` (Zustand-compatible architecture).
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

import { createStore, type GetStateFn, type SetStateFn, type StoreHook } from './createStore.ts';
import {
  type ChatEventsContainer,
  type ChatHeatmapResponse,
  type ChatProgressPayload,
  type ChatFallbackPayload,
  type ChatExtractionOptions,
  startChatExtraction,
  cancelChatExtraction,
  getChatEvents,
  getChatVelocityHeatmap,
  listenChatProgress,
  listenChatFallback,
} from '../services/ipc/chat.ts';
import { ERROR_CODES, type IpcErrorResponse, parseIpcError } from '../services/ipc/error.ts';

// ---------------------------------------------------------------------------
// Store Interface Contract
// ---------------------------------------------------------------------------

export interface ChatState {
  // Reactive State
  readonly vodId: string | null;
  readonly isExtracting: boolean;
  readonly hasChatReplay: boolean;
  readonly chatFilePath: string | null;
  readonly chatEvents: ChatEventsContainer | null;
  readonly chatHeatmap: ChatHeatmapResponse | null;
  readonly progress: ChatProgressPayload | null;
  readonly fallbackNotice: ChatFallbackPayload | null;
  readonly error: IpcErrorResponse | null;

  // Actions
  setVodId: (vodId: string | null) => void;
  extractChat: (vodId: string, url: string, options?: ChatExtractionOptions) => Promise<string>;
  cancelChat: () => Promise<boolean>;
  fetchChatEvents: (vodId: string, limit?: number, offset?: number) => Promise<ChatEventsContainer>;
  fetchHeatmap: (vodId: string, windowSec?: number) => Promise<ChatHeatmapResponse>;
  clearFallbackNotice: () => void;
  clearError: () => void;
  reset: () => void;
}

export type { StoreHook };

// ---------------------------------------------------------------------------
// Initial State & Event Subscription Management
// ---------------------------------------------------------------------------

const initialChatState = {
  vodId: null,
  isExtracting: false,
  hasChatReplay: false,
  chatFilePath: null,
  chatEvents: null,
  chatHeatmap: null,
  progress: null,
  fallbackNotice: null,
  error: null,
};

let resetEpoch = 0;
let extractSeq = 0;
let eventsSeq = 0;
let heatmapSeq = 0;

let listenersPromise: Promise<void> | null = null;
let activeListenersToken = 0;
let progressUnlisten: (() => void) | null = null;
let fallbackUnlisten: (() => void) | null = null;

async function safeListenChat(register: () => Promise<() => void>): Promise<(() => void) | null> {
  try {
    return await register();
  } catch {
    return null;
  }
}

function ensureChatListeners(
  set: SetStateFn<ChatState>,
  get: GetStateFn<ChatState>,
): Promise<void> {
  if (progressUnlisten && fallbackUnlisten) {
    return Promise.resolve();
  }
  if (listenersPromise) {
    return listenersPromise;
  }

  const epochAtStart = resetEpoch;
  const token = ++activeListenersToken;
  const pending = (async () => {
    const [nextProgressUnlisten, nextFallbackUnlisten] = await Promise.all([
      progressUnlisten
        ? Promise.resolve<(() => void) | null>(null)
        : safeListenChat(() =>
            listenChatProgress((payload) => {
              const state = get();
              if (!state.isExtracting) return;
              if (
                state.vodId !== null &&
                payload.taskId.length > 0 &&
                payload.taskId !== state.vodId
              ) {
                return;
              }
              set({ progress: payload });
            }),
          ),
      fallbackUnlisten
        ? Promise.resolve<(() => void) | null>(null)
        : safeListenChat(() =>
            listenChatFallback((payload) => {
              const state = get();
              if (!state.isExtracting) return;
              if (
                state.vodId !== null &&
                payload.taskId.length > 0 &&
                payload.taskId !== state.vodId
              ) {
                return;
              }
              set({
                hasChatReplay: false,
                fallbackNotice: payload,
                isExtracting: false,
                error: null,
              });
            }),
          ),
    ]);

    if (epochAtStart !== resetEpoch) {
      nextProgressUnlisten?.();
      nextFallbackUnlisten?.();
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
    if (nextFallbackUnlisten) {
      if (fallbackUnlisten) {
        nextFallbackUnlisten();
      } else {
        fallbackUnlisten = nextFallbackUnlisten;
      }
    }

    if (activeListenersToken === token) {
      listenersPromise = null;
    }
  })();

  listenersPromise = pending;
  return pending;
}

function cleanupChatListeners(): void {
  ++activeListenersToken;
  listenersPromise = null;
  if (progressUnlisten) {
    progressUnlisten();
    progressUnlisten = null;
  }
  if (fallbackUnlisten) {
    fallbackUnlisten();
    fallbackUnlisten = null;
  }
}

// ---------------------------------------------------------------------------
// Chat Store Instance
// ---------------------------------------------------------------------------

export const useChatStore = createStore<ChatState>((set, get) => ({
  ...initialChatState,

  setVodId: (vodId: string | null) => {
    if (get().vodId !== vodId) {
      ++extractSeq;
      ++eventsSeq;
      ++heatmapSeq;
      set({ vodId, isExtracting: false, progress: null });
      return;
    }
    set({ vodId });
  },

  extractChat: async (
    vodId: string,
    url: string,
    options?: ChatExtractionOptions,
  ): Promise<string> => {
    const seq = ++extractSeq;
    const epoch = resetEpoch;
    set({
      vodId,
      isExtracting: true,
      hasChatReplay: false,
      chatFilePath: null,
      progress: null,
      error: null,
      fallbackNotice: null,
    });

    try {
      await ensureChatListeners(set, get);
      if (seq !== extractSeq || epoch !== resetEpoch) {
        return '';
      }

      const filePath = await startChatExtraction(vodId, url, options);
      if (seq !== extractSeq || epoch !== resetEpoch) {
        return filePath;
      }

      set({
        isExtracting: false,
        hasChatReplay: true,
        chatFilePath: filePath,
      });

      // Automatically load heatmap for dashboard visualization
      try {
        const heatmap = await getChatVelocityHeatmap(vodId);
        if (seq === extractSeq && epoch === resetEpoch) {
          set({ chatHeatmap: heatmap });
        }
      } catch {
        // Optional telemetry failure
      }

      return filePath;
    } catch (err: unknown) {
      const ipcError = parseIpcError(err);
      if (seq === extractSeq && epoch === resetEpoch) {
        if (ipcError.code === ERROR_CODES.INGEST_CHAT_UNAVAILABLE) {
          const existingFallback = get().fallbackNotice;
          set({
            isExtracting: false,
            hasChatReplay: false,
            error: null,
            fallbackNotice: existingFallback ?? {
              taskId: vodId,
              code: ipcError.code,
              message: ipcError.message,
              audioFallbackActive: true,
            },
          });
        } else {
          set({
            isExtracting: false,
            error: ipcError,
          });
        }
      }
      throw ipcError;
    }
  },

  cancelChat: async (): Promise<boolean> => {
    const activeVodId = get().vodId;
    ++extractSeq;
    set({ isExtracting: false, progress: null });
    if (!activeVodId) {
      return false;
    }
    try {
      return await cancelChatExtraction(activeVodId);
    } catch {
      return false;
    }
  },

  fetchChatEvents: async (
    vodId: string,
    limit?: number,
    offset?: number,
  ): Promise<ChatEventsContainer> => {
    const seq = ++eventsSeq;
    const epoch = resetEpoch;
    try {
      const container = await getChatEvents(vodId, limit, offset);
      if (seq === eventsSeq && epoch === resetEpoch) {
        set({ chatEvents: container });
      }
      return container;
    } catch (err: unknown) {
      const ipcError = parseIpcError(err);
      if (seq === eventsSeq && epoch === resetEpoch) {
        set({ error: ipcError });
      }
      throw ipcError;
    }
  },

  fetchHeatmap: async (vodId: string, windowSec?: number): Promise<ChatHeatmapResponse> => {
    const seq = ++heatmapSeq;
    const epoch = resetEpoch;
    try {
      const heatmap = await getChatVelocityHeatmap(vodId, windowSec);
      if (seq === heatmapSeq && epoch === resetEpoch) {
        set({ chatHeatmap: heatmap });
      }
      return heatmap;
    } catch (err: unknown) {
      const ipcError = parseIpcError(err);
      if (seq === heatmapSeq && epoch === resetEpoch) {
        set({ error: ipcError });
      }
      throw ipcError;
    }
  },

  clearFallbackNotice: () => {
    set({ fallbackNotice: null });
  },

  clearError: () => {
    set({ error: null });
  },

  reset: () => {
    ++resetEpoch;
    ++extractSeq;
    ++eventsSeq;
    ++heatmapSeq;
    cleanupChatListeners();
    set({ ...initialChatState });
  },
}));

// Backward-compatibility alias for non-React callers & existing test suites
export const chatStore = useChatStore;

export default useChatStore;
