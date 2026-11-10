/**
 * NeuroClip Audio-First Ingestion Store
 *
 * Reactive store managing online VOD probing, anti-bot cookie settings,
 * real-time download progress, and VOD manifest state.
 *
 * Built on native `useSyncExternalStore` (Zustand-compatible API without external runtime deps).
 * Strict adherence to docs/CONVENTIONS.md (zero-any, monotonic sequence counter).
 */

import { useSyncExternalStore } from 'react';
import {
  type VodManifest,
  type VodMetadata,
  type IngestProgressEvent,
  type BrowserCookieType,
  probeVod,
  startVodIngestion,
  cancelVodIngestion,
  listenIngestProgress,
} from '../services/ipc/ingestion.ts';
import {
  type IpcErrorResponse,
  parseIpcError,
} from '../services/ipc/error.ts';

// ---------------------------------------------------------------------------
// Store Interface Contract
// ---------------------------------------------------------------------------

export interface IngestionState {
  // Input parameters
  readonly url: string;
  readonly audioOnly: boolean;
  readonly cookiesPath: string | null;
  readonly browserCookies: BrowserCookieType | null;
  readonly customCacheDir: string | null;

  // Active status
  readonly isProbing: boolean;
  readonly isIngesting: boolean;
  readonly activeTaskId: string | null;

  // Results & Telemetry
  readonly metadata: VodMetadata | null;
  readonly manifest: VodManifest | null;
  readonly progress: IngestProgressEvent | null;
  readonly error: IpcErrorResponse | null;

  // Actions
  setUrl: (url: string) => void;
  setAudioOnly: (enabled: boolean) => void;
  setCookiesPath: (path: string | null) => void;
  setBrowserCookies: (browser: BrowserCookieType | null) => void;
  setCustomCacheDir: (dir: string | null) => void;
  probe: (urlOverride?: string) => Promise<VodMetadata>;
  startIngestion: (urlOverride?: string) => Promise<VodManifest>;
  cancelIngestion: () => Promise<boolean>;
  clearError: () => void;
  reset: () => void;
}

// ---------------------------------------------------------------------------
// Native Store Implementation (Zustand-Compatible Architecture)
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

type StoreHook<T> = {
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
// Ingestion Store Instance & Monotonic Sequence Counter
// ---------------------------------------------------------------------------

let ingestionSeq = 0;
let progressUnlisten: (() => void) | null = null;

export const useIngestionStore = createStore<IngestionState>((set, get) => ({
  url: '',
  audioOnly: true,
  cookiesPath: null,
  browserCookies: null,
  customCacheDir: null,
  isProbing: false,
  isIngesting: false,
  activeTaskId: null,
  metadata: null,
  manifest: null,
  progress: null,
  error: null,

  setUrl: (url: string) => {
    set({ url: url.trim(), error: null });
  },

  setAudioOnly: (enabled: boolean) => {
    set({ audioOnly: enabled });
  },

  setCookiesPath: (path: string | null) => {
    set({ cookiesPath: path !== null && path.trim().length > 0 ? path.trim() : null });
  },

  setBrowserCookies: (browser: BrowserCookieType | null) => {
    set({ browserCookies: browser });
  },

  setCustomCacheDir: (dir: string | null) => {
    set({ customCacheDir: dir !== null && dir.trim().length > 0 ? dir.trim() : null });
  },

  probe: async (urlOverride?: string): Promise<VodMetadata> => {
    const seq = ++ingestionSeq;
    const targetUrl = urlOverride ?? get().url;

    set({ isProbing: true, error: null });

    try {
      const meta = await probeVod(
        targetUrl,
        get().cookiesPath,
        get().browserCookies,
      );

      if (seq === ingestionSeq) {
        set({
          metadata: meta,
          isProbing: false,
        });
      }

      return meta;
    } catch (err: unknown) {
      const normalizedError = parseIpcError(err, 'Không thể kiểm tra thông tin VOD');
      if (seq === ingestionSeq) {
        set({
          isProbing: false,
          error: normalizedError,
        });
      }
      throw normalizedError;
    }
  },

  startIngestion: async (urlOverride?: string): Promise<VodManifest> => {
    const seq = ++ingestionSeq;
    const targetUrl = urlOverride ?? get().url;
    const taskId = `task_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;

    // Reset previous run status and register activeTaskId immediately for early cancellation
    set({
      isIngesting: true,
      activeTaskId: taskId,
      error: null,
      manifest: null,
      progress: {
        taskId,
        stage: 'starting',
        percent: 0,
        speedKbps: 0,
        etaSec: 0,
      },
    });

    // Setup real-time progress listener if not already bound
    if (!progressUnlisten) {
      void listenIngestProgress((event) => {
        const cur = get();
        if (cur.isIngesting) {
          set({
            progress: event,
            activeTaskId: event.taskId,
          });
        }
      }).then((unlisten) => {
        progressUnlisten = unlisten;
      });
    }

    try {
      const manifest = await startVodIngestion(targetUrl, {
        audioOnly: get().audioOnly,
        cookiesPath: get().cookiesPath,
        browserCookies: get().browserCookies,
        customCacheDir: get().customCacheDir,
        taskId,
      });

      if (seq === ingestionSeq) {
        set({
          manifest,
          isIngesting: false,
          activeTaskId: null,
          progress: {
            taskId: manifest.vodId,
            stage: 'complete',
            percent: 100,
            speedKbps: 0,
            etaSec: 0,
          },
        });
      }

      return manifest;
    } catch (err: unknown) {
      const normalizedError = parseIpcError(err, 'Lỗi khi thu nạp luồng âm thanh VOD');
      if (seq === ingestionSeq) {
        set({
          isIngesting: false,
          activeTaskId: null,
          error: normalizedError,
        });
      }
      throw normalizedError;
    }
  },

  cancelIngestion: async (): Promise<boolean> => {
    const taskId = get().activeTaskId ?? '';
    try {
      const ok = await cancelVodIngestion(taskId);
      set({
        isIngesting: false,
        activeTaskId: null,
        error: {
          code: 'ERR_TASK_CANCELLED',
          message: 'Tác vụ thu nạp đã bị hủy bỏ',
          details: `Cancelled task: ${taskId}`,
          recoverable: true,
        },
      });
      return ok;
    } catch (err: unknown) {
      const normalizedError = parseIpcError(err, 'Không thể hủy tác vụ');
      set({ error: normalizedError });
      return false;
    }
  },

  clearError: () => {
    set({ error: null });
  },

  reset: () => {
    ++ingestionSeq;
    if (progressUnlisten) {
      progressUnlisten();
      progressUnlisten = null;
    }
    set({
      url: '',
      audioOnly: true,
      cookiesPath: null,
      browserCookies: null,
      customCacheDir: null,
      isProbing: false,
      isIngesting: false,
      activeTaskId: null,
      metadata: null,
      manifest: null,
      progress: null,
      error: null,
    });
  },
}));
