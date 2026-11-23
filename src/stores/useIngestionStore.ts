/**
 * NeuroClip Audio-First Ingestion Store
 *
 * Reactive store managing online VOD probing, anti-bot cookie settings,
 * real-time download progress, and VOD manifest state.
 *
 * Built on native `useSyncExternalStore` (Zustand-compatible API without external runtime deps).
 * Strict adherence to docs/CONVENTIONS.md (zero-any, monotonic sequence counter).
 */

import { createStore, type GetStateFn, type SetStateFn } from './createStore.ts';
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
import { ERROR_CODES, type IpcErrorResponse, parseIpcError } from '../services/ipc/error.ts';

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
// Ingestion Store Instance & Monotonic Sequence Counter
// ---------------------------------------------------------------------------

let resetEpoch = 0;
let probeSeq = 0;
let ingestSeq = 0;
let listenersPromise: Promise<void> | null = null;
let activeListenersToken = 0;
let progressUnlisten: (() => void) | null = null;

async function safeListenIngest(register: () => Promise<() => void>): Promise<(() => void) | null> {
  try {
    return await register();
  } catch {
    return null;
  }
}

function ensureIngestListener(
  set: SetStateFn<IngestionState>,
  get: GetStateFn<IngestionState>,
): Promise<void> {
  if (progressUnlisten) {
    return Promise.resolve();
  }
  if (listenersPromise) {
    return listenersPromise;
  }

  const epochAtStart = resetEpoch;
  const token = ++activeListenersToken;
  const pending = (async () => {
    const nextUnlisten = await safeListenIngest(() =>
      listenIngestProgress((event) => {
        const cur = get();
        if (!cur.isIngesting) return;
        if (cur.activeTaskId !== null && event.taskId !== cur.activeTaskId) {
          return;
        }
        set({
          progress: event,
          activeTaskId: event.taskId,
        });
      }),
    );

    if (epochAtStart !== resetEpoch) {
      nextUnlisten?.();
      if (activeListenersToken === token) {
        listenersPromise = null;
      }
      return;
    }

    if (nextUnlisten) {
      if (progressUnlisten) {
        nextUnlisten();
      } else {
        progressUnlisten = nextUnlisten;
      }
    }

    if (activeListenersToken === token) {
      listenersPromise = null;
    }
  })();

  listenersPromise = pending;
  return pending;
}

function cleanupIngestListener(): void {
  ++activeListenersToken;
  listenersPromise = null;
  if (progressUnlisten) {
    progressUnlisten();
    progressUnlisten = null;
  }
}

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
    const seq = ++probeSeq;
    const epoch = resetEpoch;
    const targetUrl = urlOverride ?? get().url;

    set({ isProbing: true, error: null });

    try {
      const meta = await probeVod(targetUrl, get().cookiesPath, get().browserCookies);

      if (seq === probeSeq && epoch === resetEpoch) {
        set({
          metadata: meta,
          isProbing: false,
        });
      }

      return meta;
    } catch (err: unknown) {
      const normalizedError = parseIpcError(err, 'Không thể kiểm tra thông tin VOD');
      if (seq === probeSeq && epoch === resetEpoch) {
        set({
          isProbing: false,
          error: normalizedError,
        });
      }
      throw normalizedError;
    }
  },

  startIngestion: async (urlOverride?: string): Promise<VodManifest> => {
    const seq = ++ingestSeq;
    const epoch = resetEpoch;
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

    const listenerReady = ensureIngestListener(set, get);

    try {
      const [manifest] = await Promise.all([
        startVodIngestion(targetUrl, {
          audioOnly: get().audioOnly,
          cookiesPath: get().cookiesPath,
          browserCookies: get().browserCookies,
          customCacheDir: get().customCacheDir,
          taskId,
        }),
        listenerReady,
      ]);

      if (seq === ingestSeq && epoch === resetEpoch) {
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
      if (seq === ingestSeq && epoch === resetEpoch) {
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
    const seqAtStart = ingestSeq;
    const epoch = resetEpoch;
    const taskId = get().activeTaskId ?? '';
    try {
      const ok = await cancelVodIngestion(taskId);
      const cur = get();
      if (
        ok &&
        seqAtStart === ingestSeq &&
        epoch === resetEpoch &&
        cur.isIngesting &&
        cur.activeTaskId === taskId
      ) {
        ++ingestSeq;
        set({
          isIngesting: false,
          activeTaskId: null,
          error: {
            code: ERROR_CODES.TASK_CANCELLED,
            message: 'Tác vụ thu nạp đã bị hủy bỏ',
            details: `Cancelled task: ${taskId}`,
            recoverable: false,
          },
        });
        return true;
      }
      return false;
    } catch (err: unknown) {
      const normalizedError = parseIpcError(err, 'Không thể hủy tác vụ');
      const cur = get();
      if (
        seqAtStart === ingestSeq &&
        epoch === resetEpoch &&
        cur.isIngesting &&
        cur.activeTaskId === taskId
      ) {
        set({ error: normalizedError });
      }
      return false;
    }
  },

  clearError: () => {
    set({ error: null });
  },

  reset: () => {
    ++resetEpoch;
    ++probeSeq;
    ++ingestSeq;
    cleanupIngestListener();
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
