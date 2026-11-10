/**
 * NeuroClip Reactive Log Viewer Store & Ring Buffer.
 * Compatible with Zustand API: useLogStore(selector), getState(), setState(), subscribe()
 * Strict adherence to docs/CONVENTIONS.md §3.3 & §5.3 (zero-any, exactOptionalPropertyTypes).
 *
 * Ring buffer capped at 1,000 entries by default to guarantee <90MB Idle RAM SLA.
 */

import { useSyncExternalStore } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  type LogEntry,
  type LogEntryJson,
  type LogFilter,
  toLogEntry,
} from '../services/logging/types.ts';
import { isTauriEnvironment } from '../services/logging/logger.ts';
import { parseIpcError } from '../services/ipc/error.ts';

export const DEFAULT_MAX_LOG_ENTRIES = 1_000;

export interface LogState {
  // Reactive State
  readonly entries: readonly LogEntry[];
  readonly maxEntries: number;
  readonly isLoading: boolean;
  readonly isClearing: boolean;
  readonly error: string | null;
  readonly filter: LogFilter;
  readonly autoScroll: boolean;
  readonly autoRefresh: boolean;

  // Actions
  fetchLogs: (limit?: number) => Promise<void>;
  clearLogs: () => Promise<boolean>;
  addLogEntry: (entry: LogEntry | LogEntryJson) => void;
  setFilter: (partial: Partial<LogFilter>) => void;
  resetFilter: () => void;
  setAutoScroll: (autoScroll: boolean) => void;
  setAutoRefresh: (autoRefresh: boolean) => void;
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
// Initial Filter & Store State
// ---------------------------------------------------------------------------

const INITIAL_FILTER: LogFilter = {
  level: 'all',
  search: '',
  target: '',
};

// ---------------------------------------------------------------------------
// Store Instance
// ---------------------------------------------------------------------------

export const useLogStore = createStore<LogState>((set, get) => ({
  entries: [],
  maxEntries: DEFAULT_MAX_LOG_ENTRIES,
  isLoading: false,
  isClearing: false,
  error: null,
  filter: INITIAL_FILTER,
  autoScroll: true,
  autoRefresh: true,

  fetchLogs: async (limit?: number) => {
    set({ isLoading: true, error: null });

    if (!isTauriEnvironment()) {
      set({ isLoading: false });
      return;
    }

    try {
      const activeFilter = get().filter;
      const levelFilter =
        activeFilter.level && activeFilter.level !== 'all'
          ? activeFilter.level.toUpperCase()
          : null;

      const rawLogs = await invoke<readonly LogEntryJson[]>('get_recent_logs', {
        limit: limit ?? get().maxEntries,
        levelFilter,
      });

      const max = get().maxEntries;
      const domainLogs = rawLogs.map((json, idx) => toLogEntry(json, idx));
      const boundedLogs =
        domainLogs.length > max ? domainLogs.slice(-max) : domainLogs;

      set({ entries: boundedLogs, isLoading: false, error: null });
    } catch (err: unknown) {
      const parsed = parseIpcError(err, 'Không thể tải danh sách nhật ký hệ thống.');
      set({ error: parsed.message, isLoading: false });
    }
  },

  clearLogs: async (): Promise<boolean> => {
    set({ isClearing: true, error: null });

    if (!isTauriEnvironment()) {
      set({ entries: [], isClearing: false, error: null });
      return true;
    }

    try {
      await invoke('clear_logs');
      set({ entries: [], isClearing: false, error: null });
      return true;
    } catch (err: unknown) {
      const parsed = parseIpcError(err, 'Không thể dọn dẹp tệp nhật ký.');
      set({ error: parsed.message, isClearing: false });
      return false;
    }
  },

  addLogEntry: (entry: LogEntry | LogEntryJson) => {
    const domainEntry: LogEntry = 'id' in entry ? entry : toLogEntry(entry);
    const max = get().maxEntries;
    const current = get().entries;

    let updated: readonly LogEntry[];
    if (current.length >= max) {
      // Ring buffer eviction of oldest entry
      updated = [...current.slice(1), domainEntry];
    } else {
      updated = [...current, domainEntry];
    }

    set({ entries: updated });
  },

  setFilter: (partial: Partial<LogFilter>) => {
    set((state) => ({
      filter: {
        ...state.filter,
        ...partial,
      },
    }));
  },

  resetFilter: () => {
    set({ filter: INITIAL_FILTER });
  },

  setAutoScroll: (autoScroll: boolean) => {
    set({ autoScroll });
  },

  setAutoRefresh: (autoRefresh: boolean) => {
    set({ autoRefresh });
  },

  clearError: () => {
    set({ error: null });
  },

  reset: () => {
    set({
      entries: [],
      maxEntries: DEFAULT_MAX_LOG_ENTRIES,
      isLoading: false,
      isClearing: false,
      error: null,
      filter: INITIAL_FILTER,
      autoScroll: true,
      autoRefresh: true,
    });
  },
}));

// ---------------------------------------------------------------------------
// Selector Helpers
// ---------------------------------------------------------------------------

/**
 * Filter logs based on the active LogFilter settings.
 */
export function selectFilteredLogs(state: LogState): readonly LogEntry[] {
  const { entries, filter } = state;
  const { level, search, target, startTime, endTime } = filter;

  return entries.filter((entry) => {
    // 1. Level Filter
    if (level && level !== 'all' && entry.level !== level) {
      return false;
    }

    // 2. Target Filter
    if (target && target.trim() !== '') {
      const normalizedTarget = target.trim().toLowerCase();
      const entryTarget = (entry.target ?? '').toLowerCase();
      if (!entryTarget.includes(normalizedTarget)) {
        return false;
      }
    }

    // 3. Search Filter (Message and Target)
    if (search && search.trim() !== '') {
      const query = search.trim().toLowerCase();
      const inMsg = (entry.message ?? '').toLowerCase().includes(query);
      const inTgt = (entry.target ?? '').toLowerCase().includes(query);
      if (!inMsg && !inTgt) {
        return false;
      }
    }

    // 4. Start Time Filter
    if (startTime && startTime.trim() !== '') {
      if ((entry.timestamp ?? '') < startTime) {
        return false;
      }
    }

    // 5. End Time Filter
    if (endTime && endTime.trim() !== '') {
      if ((entry.timestamp ?? '') > endTime) {
        return false;
      }
    }

    return true;
  });
}
