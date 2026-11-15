/**
 * NeuroClip Multimodal Highlight State Store
 *
 * Reactive store managing highlight detection, clustering, Schema 5.4 containers,
 * cluster selection, and bookmark persistence.
 *
 * Built on native `useSyncExternalStore` (Zustand-compatible architecture).
 * Strict adherence to docs/CONVENTIONS.md (zero-any, strict null safety).
 */

import { useSyncExternalStore } from 'react';
import {
  type HighlightCluster,
  type HighlightClustersContainer,
  detectHighlights,
  getHighlightClusters,
  toggleHighlightBookmark,
} from '../services/ipc/highlight.ts';
import { type IpcErrorResponse, parseIpcError } from '../services/ipc/error.ts';

// ---------------------------------------------------------------------------
// Store Interface Contract
// ---------------------------------------------------------------------------

export interface HighlightState {
  // Reactive State
  readonly vodId: string | null;
  readonly isLoading: boolean;
  readonly clustersContainer: HighlightClustersContainer | null;
  readonly clusters: readonly HighlightCluster[];
  readonly selectedClusterId: string | null;
  readonly error: IpcErrorResponse | null;

  // Actions
  setVodId: (vodId: string | null) => void;
  selectCluster: (clusterId: string | null) => void;
  detectHighlightsForVod: (
    vodId: string,
    durationSeconds: number,
    options?: { chatEventsPath?: string | undefined; audioPath?: string | undefined },
  ) => Promise<HighlightClustersContainer>;
  fetchClustersForVod: (vodId: string) => Promise<HighlightClustersContainer>;
  toggleBookmark: (clusterId: string, bookmarked?: boolean | undefined) => Promise<HighlightCluster>;
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
// Highlight Store Implementation
// ---------------------------------------------------------------------------

const initialState = {
  vodId: null as string | null,
  isLoading: false,
  clustersContainer: null as HighlightClustersContainer | null,
  clusters: Object.freeze([]) as readonly HighlightCluster[],
  selectedClusterId: null as string | null,
  error: null as IpcErrorResponse | null,
};

export const useHighlightStore = createStore<HighlightState>((set, get) => ({
  ...initialState,

  setVodId: (vodId) => set({ vodId }),

  selectCluster: (selectedClusterId) => set({ selectedClusterId }),

  detectHighlightsForVod: async (vodId, durationSeconds, options) => {
    set({ isLoading: true, error: null, vodId });
    try {
      const container = await detectHighlights(vodId, durationSeconds, options);
      set({
        isLoading: false,
        clustersContainer: container,
        clusters: container.clusters,
        error: null,
      });
      return container;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      set({ isLoading: false, error: parsed });
      throw parsed;
    }
  },

  fetchClustersForVod: async (vodId) => {
    set({ isLoading: true, error: null, vodId });
    try {
      const container = await getHighlightClusters(vodId);
      set({
        isLoading: false,
        clustersContainer: container,
        clusters: container.clusters,
        error: null,
      });
      return container;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      set({ isLoading: false, error: parsed });
      throw parsed;
    }
  },

  toggleBookmark: async (clusterId, bookmarked) => {
    const { vodId, clusters, clustersContainer } = get();
    if (!vodId) {
      const err: IpcErrorResponse = {
        code: 'ERR_INVALID_ARGUMENT',
        message: 'Không thể đánh dấu cụm highlight khi chưa chọn VOD',
        recoverable: false,
      };
      set({ error: err });
      throw err;
    }

    try {
      const updated = await toggleHighlightBookmark(vodId, clusterId, bookmarked);
      const nextClusters = clusters.map((c) =>
        c.clusterId === clusterId ? updated : c,
      );
      const nextContainer = clustersContainer
        ? { ...clustersContainer, clusters: nextClusters }
        : null;

      set({
        clusters: Object.freeze(nextClusters),
        clustersContainer: nextContainer,
        error: null,
      });

      return updated;
    } catch (err: unknown) {
      const parsed = parseIpcError(err);
      set({ error: parsed });
      throw parsed;
    }
  },

  clearError: () => set({ error: null }),

  reset: () => set(initialState),
}));

export const highlightStore = {
  getState: useHighlightStore.getState,
  setState: useHighlightStore.setState,
  subscribe: useHighlightStore.subscribe,
};
