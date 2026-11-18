/**
 * NeuroClip Chat State Store
 *
 * Reactive store managing online chat extraction, streaming progress,
 * Schema 5.3 normalized events, 10s velocity heatmap, and ERR-05 fallback events.
 *
 * Built on native `useSyncExternalStore` (Zustand-compatible architecture).
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

import { createStore, type SetStateFn, type StoreHook } from './createStore.ts';
import {
  type ChatEventsContainer,
  type ChatHeatmapResponse,
  type ChatProgressPayload,
  type ChatFallbackPayload,
  startChatExtraction,
  getChatEvents,
  getChatVelocityHeatmap,
  listenChatProgress,
  listenChatFallback,
} from '../services/ipc/chat.ts';
import { type IpcErrorResponse, parseIpcError } from '../services/ipc/error.ts';

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
  extractChat: (vodId: string, url: string) => Promise<string>;
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

let progressUnlisten: (() => void) | null = null;
let fallbackUnlisten: (() => void) | null = null;

async function ensureChatListeners(set: SetStateFn<ChatState>): Promise<void> {
  if (!progressUnlisten) {
    try {
      progressUnlisten = await listenChatProgress((payload) => {
        set({ progress: payload });
      });
    } catch {
      // Non-Tauri testing environment fallback
    }
  }

  if (!fallbackUnlisten) {
    try {
      fallbackUnlisten = await listenChatFallback((payload) => {
        set({
          hasChatReplay: false,
          fallbackNotice: payload,
          isExtracting: false,
        });
      });
    } catch {
      // Non-Tauri testing environment fallback
    }
  }
}

function cleanupChatListeners(): void {
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

export const useChatStore = createStore<ChatState>((set) => ({
  ...initialChatState,

  setVodId: (vodId: string | null) => {
    set({ vodId });
  },

  extractChat: async (vodId: string, url: string): Promise<string> => {
    void ensureChatListeners(set);
    set({
      vodId,
      isExtracting: true,
      error: null,
      fallbackNotice: null,
    });

    try {
      const filePath = await startChatExtraction(vodId, url);
      set({
        isExtracting: false,
        hasChatReplay: true,
        chatFilePath: filePath,
      });

      // Automatically load heatmap for dashboard visualization
      try {
        const heatmap = await getChatVelocityHeatmap(vodId);
        set({ chatHeatmap: heatmap });
      } catch {
        // Optional telemetry failure
      }

      return filePath;
    } catch (err: unknown) {
      const ipcError = parseIpcError(err);
      set({
        isExtracting: false,
        error: ipcError,
      });
      throw ipcError;
    }
  },

  fetchChatEvents: async (
    vodId: string,
    limit?: number,
    offset?: number,
  ): Promise<ChatEventsContainer> => {
    try {
      const container = await getChatEvents(vodId, limit, offset);
      set({ chatEvents: container });
      return container;
    } catch (err: unknown) {
      const ipcError = parseIpcError(err);
      set({ error: ipcError });
      throw ipcError;
    }
  },

  fetchHeatmap: async (vodId: string, windowSec?: number): Promise<ChatHeatmapResponse> => {
    try {
      const heatmap = await getChatVelocityHeatmap(vodId, windowSec);
      set({ chatHeatmap: heatmap });
      return heatmap;
    } catch (err: unknown) {
      const ipcError = parseIpcError(err);
      set({ error: ipcError });
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
    cleanupChatListeners();
    set({ ...initialChatState });
  },
}));

// Backward-compatibility alias for non-React callers & existing test suites
export const chatStore = useChatStore;

export default useChatStore;
